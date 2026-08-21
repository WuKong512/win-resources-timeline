use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};

pub const PROCESS_ROLLUP_PROCESSING_VERSION: &str = "process-rollup-v1";
pub const PROCESS_ROLLUP_MINUTE_MS: i64 = 60_000;
pub const PROCESS_ROLLUP_HOUR_MS: i64 = 3_600_000;
pub const PROCESS_ROLLUP_DAY_MS: i64 = 86_400_000;
const MAX_ROLLUP_OBSERVATION_MS: i64 = 15_000;
const DEFAULT_BATCH_SIZE: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub struct RollupWindow {
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub struct RollupMaintenanceStatus {
    pub last_run_at_ms: Option<i64>,
    pub last_duration_ms: Option<u64>,
    pub last_system_rows: u64,
    pub last_process_rows: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessRollupResult {
    pub minute_buckets: usize,
    pub hour_buckets: usize,
    pub daily_rows: usize,
}

#[allow(dead_code)]
pub const ROLLUP_TABLES: &[&str] = &[
    "system_rollup_1m",
    "process_rollup_1m",
    "process_rollup_1h",
    "app_usage_daily",
    "app_resource_daily",
    "energy_rollup_daily",
];

#[allow(dead_code)]
pub fn pending_frame_window(
    conn: &Connection,
    after_ms: Option<i64>,
) -> rusqlite::Result<Option<RollupWindow>> {
    let (start, end): (Option<i64>, Option<i64>) = conn.query_row(
        "SELECT MIN(ts), MAX(ts) FROM sample_frame WHERE (?1 IS NULL OR ts > ?1)",
        [after_ms],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(start
        .zip(end)
        .map(|(start_ms, end_ms)| RollupWindow { start_ms, end_ms }))
}

#[allow(dead_code)]
pub fn maintenance_status() -> RollupMaintenanceStatus {
    RollupMaintenanceStatus::default()
}

/// Runs a bounded maintenance slice. Only complete minute/hour buckets are processed, so the
/// current bucket cannot be mistaken for a final bucket. The settings table is used as the
/// checkpoint store; no schema change is needed for catch-up state.
pub fn run_process_rollups(
    conn: &Connection,
    now_ms: i64,
    batch_size: usize,
) -> rusqlite::Result<ProcessRollupResult> {
    let batch_size = batch_size.clamp(1, DEFAULT_BATCH_SIZE);
    let complete_minute_before = floor_bucket(now_ms, PROCESS_ROLLUP_MINUTE_MS);
    let minute_checkpoint = setting_i64(conn, "process_rollup_1m_checkpoint_ms")?;
    let minute_buckets = candidate_buckets(
        conn,
        PROCESS_ROLLUP_MINUTE_MS,
        complete_minute_before,
        minute_checkpoint,
        batch_size,
    )?;
    let mut result = ProcessRollupResult::default();
    let mut affected_hours = BTreeSet::new();
    let mut affected_dates = BTreeSet::new();
    for bucket_start_ms in minute_buckets.iter().copied() {
        rebuild_process_rollup_1m_bucket(conn, bucket_start_ms)?;
        affected_hours.insert(floor_bucket(bucket_start_ms, PROCESS_ROLLUP_HOUR_MS));
        if let Some(local_date) = local_date_for_timestamp(conn, bucket_start_ms)? {
            affected_dates.insert(local_date);
        }
        set_setting_i64(conn, "process_rollup_1m_checkpoint_ms", bucket_start_ms)?;
        result.minute_buckets += 1;
    }

    let hour_checkpoint = setting_i64(conn, "process_rollup_1h_checkpoint_ms")?;
    for bucket_start_ms in candidate_rollup_hour_buckets(
        conn,
        floor_bucket(now_ms, PROCESS_ROLLUP_HOUR_MS),
        hour_checkpoint,
        batch_size,
    )? {
        affected_hours.insert(bucket_start_ms);
        if let Some(local_date) = local_date_for_timestamp(conn, bucket_start_ms)? {
            affected_dates.insert(local_date);
        }
    }
    for bucket_start_ms in affected_hours {
        if bucket_start_ms >= floor_bucket(now_ms, PROCESS_ROLLUP_HOUR_MS)
            || hour_checkpoint.is_some_and(|checkpoint| bucket_start_ms <= checkpoint)
        {
            continue;
        }
        rebuild_process_rollup_1h_bucket(conn, bucket_start_ms)?;
        set_setting_i64(conn, "process_rollup_1h_checkpoint_ms", bucket_start_ms)?;
        result.hour_buckets += 1;
    }

    for local_date in affected_dates {
        rebuild_app_resource_daily(conn, &local_date)?;
        result.daily_rows += 1;
    }
    Ok(result)
}

#[allow(dead_code)]
pub fn rebuild_process_rollups_for_range(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<ProcessRollupResult> {
    if end_ms <= start_ms {
        return Ok(ProcessRollupResult::default());
    }
    let first_minute = floor_bucket(start_ms, PROCESS_ROLLUP_MINUTE_MS);
    let last_minute = floor_bucket(end_ms.saturating_sub(1), PROCESS_ROLLUP_MINUTE_MS);
    let mut result = ProcessRollupResult::default();
    let mut affected_hours = BTreeSet::new();
    let mut affected_dates = BTreeSet::new();
    let mut bucket = first_minute;
    while bucket <= last_minute {
        rebuild_process_rollup_1m_bucket(conn, bucket)?;
        affected_hours.insert(floor_bucket(bucket, PROCESS_ROLLUP_HOUR_MS));
        if let Some(local_date) = local_date_for_timestamp(conn, bucket)? {
            affected_dates.insert(local_date);
        }
        result.minute_buckets += 1;
        bucket = bucket.saturating_add(PROCESS_ROLLUP_MINUTE_MS);
    }
    let complete_hour_before = floor_bucket(end_ms, PROCESS_ROLLUP_HOUR_MS);
    for hour in affected_hours {
        if hour < complete_hour_before {
            rebuild_process_rollup_1h_bucket(conn, hour)?;
            result.hour_buckets += 1;
        }
    }
    for date in affected_dates {
        rebuild_app_resource_daily(conn, &date)?;
        result.daily_rows += 1;
    }
    Ok(result)
}

pub fn rebuild_process_rollup_1m_bucket(
    conn: &Connection,
    bucket_start_ms: i64,
) -> rusqlite::Result<()> {
    let bucket_end_ms = bucket_start_ms.saturating_add(PROCESS_ROLLUP_MINUTE_MS);
    let tx = conn.unchecked_transaction()?;
    let mut statement = tx.prepare(
        "WITH ordered_frames AS (
             SELECT id, ts, duration_ms,
                    LEAD(ts) OVER (ORDER BY ts, id) AS next_ts
             FROM sample_frame WHERE ts >= ?1 AND ts < ?2
         )
         SELECT f.ts, f.duration_ms, f.next_ts, e.app_id, p.cpu_pct, p.cpu_time_delta_us,
                p.working_set_bytes, p.private_bytes, p.gpu_pct, p.read_bps, p.write_bps,
                p.selection_reason
         FROM process_sample p
         JOIN ordered_frames f ON f.id = p.frame_id
         JOIN process_instance i ON i.id = p.process_instance_id
         JOIN app_executable e ON e.id = i.app_executable_id
         WHERE f.ts >= ?1 AND f.ts < ?2
         ORDER BY e.app_id, f.ts, p.process_instance_id",
    )?;
    let rows = statement
        .query_map(params![bucket_start_ms, bucket_end_ms], |row| {
            Ok(RawProcessRow {
                timestamp_ms: row.get(0)?,
                duration_ms: row.get(1)?,
                next_timestamp_ms: row.get(2)?,
                app_id: row.get(3)?,
                cpu_pct: row.get(4)?,
                cpu_time_delta_us: row.get(5)?,
                working_set_bytes: row.get(6)?,
                private_bytes: row.get(7)?,
                gpu_pct: row.get(8)?,
                read_bps: row.get(9)?,
                write_bps: row.get(10)?,
                selection_reason: row.get(11)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let mut aggregates = BTreeMap::<i64, MinuteAccumulator>::new();
    for row in rows {
        let start_ms = row.timestamp_ms.max(bucket_start_ms);
        let end_ms =
            effective_observation_end(row.timestamp_ms, row.duration_ms, row.next_timestamp_ms)
                .min(bucket_end_ms);
        if end_ms <= start_ms {
            continue;
        }
        let covered_ms = end_ms.saturating_sub(start_ms);
        aggregates
            .entry(row.app_id)
            .or_default()
            .add(row, start_ms, end_ms, covered_ms);
    }

    tx.execute(
        "DELETE FROM process_rollup_1m WHERE bucket_start_ms = ?1",
        [bucket_start_ms],
    )?;
    for (app_id, aggregate) in aggregates {
        let covered_ms = aggregate.covered_ms.min(PROCESS_ROLLUP_MINUTE_MS);
        let coverage = (covered_ms as f64 / PROCESS_ROLLUP_MINUTE_MS as f64).clamp(0.0, 1.0);
        tx.execute(
            "INSERT INTO process_rollup_1m(
                 bucket_start_ms, app_id, weighted_cpu_pct, max_working_set_bytes,
                 cpu_time_us, read_bytes, write_bytes, gpu_active_ms, sample_count, coverage,
                 selection_reason_mask, source_start_ms, source_end_ms, processing_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                bucket_start_ms,
                app_id,
                aggregate.weighted_cpu(),
                aggregate.max_working_set_bytes,
                aggregate.cpu_time_us,
                aggregate.read_bytes,
                aggregate.write_bytes,
                aggregate.gpu_active_ms,
                aggregate.sample_count,
                coverage,
                aggregate.selection_reason_mask,
                aggregate.source_start_ms,
                aggregate.source_end_ms,
                PROCESS_ROLLUP_PROCESSING_VERSION,
            ],
        )?;
    }
    tx.commit()
}

pub fn rebuild_process_rollup_1h_bucket(
    conn: &Connection,
    bucket_start_ms: i64,
) -> rusqlite::Result<()> {
    let bucket_end_ms = bucket_start_ms.saturating_add(PROCESS_ROLLUP_HOUR_MS);
    let tx = conn.unchecked_transaction()?;
    let mut statement = tx.prepare(
        "SELECT bucket_start_ms, app_id, weighted_cpu_pct, max_working_set_bytes,
                cpu_time_us, read_bytes, write_bytes, gpu_active_ms, sample_count, coverage,
                selection_reason_mask, source_start_ms, source_end_ms
         FROM process_rollup_1m
         WHERE bucket_start_ms >= ?1 AND bucket_start_ms < ?2
           AND processing_version = ?3
         ORDER BY app_id, bucket_start_ms",
    )?;
    let rows = statement
        .query_map(
            params![
                bucket_start_ms,
                bucket_end_ms,
                PROCESS_ROLLUP_PROCESSING_VERSION
            ],
            |row| {
                Ok(RollupChildRow {
                    app_id: row.get(1)?,
                    weighted_cpu_pct: row.get(2)?,
                    max_working_set_bytes: row.get(3)?,
                    cpu_time_us: row.get(4)?,
                    read_bytes: row.get(5)?,
                    write_bytes: row.get(6)?,
                    gpu_active_ms: row.get(7)?,
                    sample_count: row.get(8)?,
                    coverage: row.get(9)?,
                    selection_reason_mask: row.get(10)?,
                    source_start_ms: row.get(11)?,
                    source_end_ms: row.get(12)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let mut aggregates = BTreeMap::<i64, HourAccumulator>::new();
    for row in rows {
        aggregates.entry(row.app_id).or_default().add(row);
    }
    tx.execute(
        "DELETE FROM process_rollup_1h WHERE bucket_start_ms = ?1",
        [bucket_start_ms],
    )?;
    for (app_id, aggregate) in aggregates {
        let coverage =
            (aggregate.covered_ms as f64 / PROCESS_ROLLUP_HOUR_MS as f64).clamp(0.0, 1.0);
        tx.execute(
            "INSERT INTO process_rollup_1h(
                 bucket_start_ms, app_id, weighted_cpu_pct, max_working_set_bytes,
                 cpu_time_us, read_bytes, write_bytes, gpu_active_ms, sample_count, coverage,
                 selection_reason_mask, source_start_ms, source_end_ms, processing_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                bucket_start_ms,
                app_id,
                aggregate.weighted_cpu(),
                aggregate.max_working_set_bytes,
                aggregate.cpu_time_us,
                aggregate.read_bytes,
                aggregate.write_bytes,
                aggregate.gpu_active_ms,
                aggregate.sample_count,
                coverage,
                aggregate.selection_reason_mask,
                aggregate.source_start_ms,
                aggregate.source_end_ms,
                PROCESS_ROLLUP_PROCESSING_VERSION,
            ],
        )?;
    }
    tx.commit()
}

/// Rebuilds one local calendar day from the minute layer. Crash/hang counters are copied from
/// any existing verified row and are never inferred from resource evidence.
pub fn rebuild_app_resource_daily(conn: &Connection, local_date: &str) -> rusqlite::Result<()> {
    if local_date.trim().is_empty() {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    let mut existing = BTreeMap::<i64, (i64, i64)>::new();
    {
        let mut statement = tx.prepare(
            "SELECT app_id, crash_count, hang_count FROM app_resource_daily WHERE local_date = ?1",
        )?;
        for row in statement.query_map([local_date], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })? {
            let (app_id, crash_count, hang_count) = row?;
            existing.insert(app_id, (crash_count, hang_count));
        }
    }
    let mut statement = tx.prepare(
        "SELECT app_id, SUM(cpu_time_us), MAX(max_working_set_bytes), SUM(gpu_active_ms),
                SUM(read_bytes), SUM(write_bytes),
                SUM(COALESCE(coverage, 0.0) * ?2), SUM(sample_count)
         FROM process_rollup_1m
         WHERE date(bucket_start_ms / 1000.0, 'unixepoch', 'localtime') = ?1
           AND processing_version = ?3
         GROUP BY app_id",
    )?;
    let rows = statement
        .query_map(
            params![
                local_date,
                PROCESS_ROLLUP_MINUTE_MS,
                PROCESS_ROLLUP_PROCESSING_VERSION
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<f64>>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                ))
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    tx.execute(
        "DELETE FROM app_resource_daily WHERE local_date = ?1",
        [local_date],
    )?;
    for (
        app_id,
        cpu_time_us,
        memory_peak_bytes,
        gpu_active_ms,
        read_bytes,
        write_bytes,
        covered_ms,
        _sample_count,
    ) in rows
    {
        let coverage = (covered_ms.unwrap_or(0.0) / PROCESS_ROLLUP_DAY_MS as f64).clamp(0.0, 1.0);
        let (crash_count, hang_count) = existing.get(&app_id).copied().unwrap_or((0, 0));
        tx.execute(
            "INSERT INTO app_resource_daily(
                 local_date, app_id, cpu_time_us, memory_peak_bytes, gpu_active_ms,
                 read_bytes, write_bytes, crash_count, hang_count, coverage, processing_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                local_date,
                app_id,
                cpu_time_us,
                memory_peak_bytes,
                gpu_active_ms,
                read_bytes,
                write_bytes,
                crash_count,
                hang_count,
                coverage,
                PROCESS_ROLLUP_PROCESSING_VERSION,
            ],
        )?;
    }
    tx.commit()
}

fn candidate_buckets(
    conn: &Connection,
    bucket_ms: i64,
    complete_before_ms: i64,
    checkpoint: Option<i64>,
    limit: usize,
) -> rusqlite::Result<Vec<i64>> {
    let after = checkpoint.unwrap_or(i64::MIN);
    let sql = format!(
        "SELECT DISTINCT (f.ts / {bucket_ms}) * {bucket_ms}
         FROM process_sample p JOIN sample_frame f ON f.id = p.frame_id
         WHERE f.ts < ?1 AND ((f.ts / {bucket_ms}) * {bucket_ms}) > ?2
         ORDER BY 1 LIMIT ?3"
    );
    let mut statement = conn.prepare(&sql)?;
    let result = statement
        .query_map(params![complete_before_ms, after, limit as i64], |row| {
            row.get(0)
        })?
        .collect();
    result
}

fn candidate_rollup_hour_buckets(
    conn: &Connection,
    complete_before_ms: i64,
    checkpoint: Option<i64>,
    limit: usize,
) -> rusqlite::Result<Vec<i64>> {
    let after = checkpoint.unwrap_or(i64::MIN);
    let mut statement = conn.prepare(
        "SELECT DISTINCT (bucket_start_ms / ?1) * ?1
         FROM process_rollup_1m
         WHERE bucket_start_ms < ?2 AND bucket_start_ms > ?3
           AND processing_version = ?4
         ORDER BY 1 LIMIT ?5",
    )?;
    let result = statement
        .query_map(
            params![
                PROCESS_ROLLUP_HOUR_MS,
                complete_before_ms,
                after,
                PROCESS_ROLLUP_PROCESSING_VERSION,
                limit as i64,
            ],
            |row| row.get(0),
        )?
        .collect();
    result
}

fn floor_bucket(timestamp_ms: i64, bucket_ms: i64) -> i64 {
    timestamp_ms.div_euclid(bucket_ms) * bucket_ms
}

fn local_date_for_timestamp(
    conn: &Connection,
    timestamp_ms: i64,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT date(?1 / 1000.0, 'unixepoch', 'localtime')",
        [timestamp_ms],
        |row| row.get(0),
    )
    .optional()
}

fn setting_i64(conn: &Connection, key: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

fn set_setting_i64(conn: &Connection, key: &str, value: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value, updated_at_ms) VALUES (?1, ?2, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
        params![key, value],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
struct RawProcessRow {
    timestamp_ms: i64,
    duration_ms: i64,
    next_timestamp_ms: Option<i64>,
    app_id: i64,
    cpu_pct: Option<f64>,
    cpu_time_delta_us: Option<i64>,
    working_set_bytes: Option<i64>,
    private_bytes: Option<i64>,
    gpu_pct: Option<f64>,
    read_bps: Option<i64>,
    write_bps: Option<i64>,
    selection_reason: i64,
}

fn effective_observation_end(
    timestamp_ms: i64,
    duration_ms: i64,
    next_timestamp_ms: Option<i64>,
) -> i64 {
    let mut end_ms = timestamp_ms.saturating_add(duration_ms.max(1));
    if let Some(next_timestamp_ms) = next_timestamp_ms.filter(|value| *value > timestamp_ms) {
        let gap_ms = next_timestamp_ms.saturating_sub(timestamp_ms);
        end_ms = if gap_ms <= MAX_ROLLUP_OBSERVATION_MS || gap_ms == duration_ms.max(1) {
            end_ms.min(next_timestamp_ms)
        } else {
            end_ms.min(timestamp_ms.saturating_add(MAX_ROLLUP_OBSERVATION_MS))
        };
    }
    end_ms
}

#[derive(Debug, Default)]
struct FrameAccumulator {
    start_ms: i64,
    end_ms: i64,
    cpu_pct_sum: Option<f64>,
    working_set_bytes: Option<i64>,
    cpu_time_us: i64,
    read_bytes: i64,
    write_bytes: i64,
    gpu_active_ms: i64,
    sample_count: i64,
    selection_reason_mask: i64,
}

#[derive(Debug, Default)]
struct MinuteAccumulator {
    frames: BTreeMap<i64, FrameAccumulator>,
    covered_ms: i64,
    source_start_ms: Option<i64>,
    source_end_ms: Option<i64>,
    selection_reason_mask: i64,
    sample_count: i64,
    cpu_weighted_sum: f64,
    cpu_weight_ms: i64,
    max_working_set_bytes: Option<i64>,
    cpu_time_us: i64,
    read_bytes: i64,
    write_bytes: i64,
    gpu_active_ms: i64,
}

impl MinuteAccumulator {
    fn add(&mut self, row: RawProcessRow, start_ms: i64, end_ms: i64, covered_ms: i64) {
        let frame = self
            .frames
            .entry(row.timestamp_ms)
            .or_insert_with(|| FrameAccumulator {
                start_ms,
                end_ms,
                ..FrameAccumulator::default()
            });
        frame.start_ms = frame.start_ms.min(start_ms);
        frame.end_ms = frame.end_ms.max(end_ms);
        if let Some(value) = row.cpu_pct.filter(|value| value.is_finite()) {
            frame.cpu_pct_sum = Some(frame.cpu_pct_sum.unwrap_or(0.0) + value);
        }
        if let Some(value) = row.private_bytes.or(row.working_set_bytes) {
            frame.working_set_bytes = Some(
                frame
                    .working_set_bytes
                    .unwrap_or(0)
                    .saturating_add(value.max(0)),
            );
        }
        let cpu_time_us = row.cpu_time_delta_us.or_else(|| {
            row.cpu_pct
                .filter(|value| value.is_finite())
                .map(|value| (value * covered_ms as f64 * 10.0).round() as i64)
        });
        frame.cpu_time_us = frame
            .cpu_time_us
            .saturating_add(cpu_time_us.unwrap_or(0).max(0));
        frame.read_bytes = frame
            .read_bytes
            .saturating_add(integrate_rate(row.read_bps, covered_ms));
        frame.write_bytes = frame
            .write_bytes
            .saturating_add(integrate_rate(row.write_bps, covered_ms));
        frame.gpu_active_ms = frame.gpu_active_ms.saturating_add(
            row.gpu_pct
                .filter(|value| value.is_finite())
                .map(|value| (covered_ms as f64 * value.clamp(0.0, 100.0) / 100.0).round() as i64)
                .unwrap_or(0),
        );
        frame.sample_count = frame.sample_count.saturating_add(1);
        frame.selection_reason_mask |= row.selection_reason;
        self.selection_reason_mask |= row.selection_reason;
        self.sample_count = self.sample_count.saturating_add(1);
        self.source_start_ms = Some(
            self.source_start_ms
                .map_or(start_ms, |value| value.min(start_ms)),
        );
        self.source_end_ms = Some(self.source_end_ms.map_or(end_ms, |value| value.max(end_ms)));
        self.rebuild_from_frames();
    }

    fn rebuild_from_frames(&mut self) {
        self.covered_ms = 0;
        self.cpu_weighted_sum = 0.0;
        self.cpu_weight_ms = 0;
        self.max_working_set_bytes = None;
        self.cpu_time_us = 0;
        self.read_bytes = 0;
        self.write_bytes = 0;
        self.gpu_active_ms = 0;
        for frame in self.frames.values() {
            let duration_ms = frame.end_ms.saturating_sub(frame.start_ms).max(0);
            self.covered_ms = self.covered_ms.saturating_add(duration_ms);
            if let Some(cpu) = frame.cpu_pct_sum {
                self.cpu_weighted_sum += cpu * duration_ms as f64;
                self.cpu_weight_ms = self.cpu_weight_ms.saturating_add(duration_ms);
            }
            self.max_working_set_bytes = match (self.max_working_set_bytes, frame.working_set_bytes)
            {
                (Some(current), Some(next)) => Some(current.max(next)),
                (None, Some(next)) => Some(next),
                (current, None) => current,
            };
            self.cpu_time_us = self.cpu_time_us.saturating_add(frame.cpu_time_us);
            self.read_bytes = self.read_bytes.saturating_add(frame.read_bytes);
            self.write_bytes = self.write_bytes.saturating_add(frame.write_bytes);
            self.gpu_active_ms = self.gpu_active_ms.saturating_add(frame.gpu_active_ms);
        }
    }

    fn weighted_cpu(&self) -> Option<f64> {
        (self.cpu_weight_ms > 0).then_some(self.cpu_weighted_sum / self.cpu_weight_ms as f64)
    }
}

#[derive(Debug)]
struct RollupChildRow {
    app_id: i64,
    weighted_cpu_pct: Option<f64>,
    max_working_set_bytes: Option<i64>,
    cpu_time_us: i64,
    read_bytes: i64,
    write_bytes: i64,
    gpu_active_ms: i64,
    sample_count: i64,
    coverage: Option<f64>,
    selection_reason_mask: i64,
    source_start_ms: Option<i64>,
    source_end_ms: Option<i64>,
}

#[derive(Debug, Default)]
struct HourAccumulator {
    weighted_cpu_sum: f64,
    cpu_weight_ms: i64,
    covered_ms: i64,
    max_working_set_bytes: Option<i64>,
    cpu_time_us: i64,
    read_bytes: i64,
    write_bytes: i64,
    gpu_active_ms: i64,
    sample_count: i64,
    selection_reason_mask: i64,
    source_start_ms: Option<i64>,
    source_end_ms: Option<i64>,
}

impl HourAccumulator {
    fn add(&mut self, row: RollupChildRow) {
        let covered_ms = (row.coverage.unwrap_or(0.0).clamp(0.0, 1.0)
            * PROCESS_ROLLUP_MINUTE_MS as f64)
            .round() as i64;
        self.covered_ms = self.covered_ms.saturating_add(covered_ms);
        if let Some(cpu) = row.weighted_cpu_pct.filter(|value| value.is_finite()) {
            self.weighted_cpu_sum += cpu * covered_ms as f64;
            self.cpu_weight_ms = self.cpu_weight_ms.saturating_add(covered_ms);
        }
        self.max_working_set_bytes = match (self.max_working_set_bytes, row.max_working_set_bytes) {
            (Some(current), Some(next)) => Some(current.max(next)),
            (None, Some(next)) => Some(next),
            (current, None) => current,
        };
        self.cpu_time_us = self.cpu_time_us.saturating_add(row.cpu_time_us);
        self.read_bytes = self.read_bytes.saturating_add(row.read_bytes);
        self.write_bytes = self.write_bytes.saturating_add(row.write_bytes);
        self.gpu_active_ms = self.gpu_active_ms.saturating_add(row.gpu_active_ms);
        self.sample_count = self.sample_count.saturating_add(row.sample_count);
        self.selection_reason_mask |= row.selection_reason_mask;
        self.source_start_ms = match (self.source_start_ms, row.source_start_ms) {
            (Some(current), Some(next)) => Some(current.min(next)),
            (None, next) => next,
            (current, None) => current,
        };
        self.source_end_ms = match (self.source_end_ms, row.source_end_ms) {
            (Some(current), Some(next)) => Some(current.max(next)),
            (None, next) => next,
            (current, None) => current,
        };
    }

    fn weighted_cpu(&self) -> Option<f64> {
        (self.cpu_weight_ms > 0).then_some(self.weighted_cpu_sum / self.cpu_weight_ms as f64)
    }
}

fn integrate_rate(rate: Option<i64>, duration_ms: i64) -> i64 {
    let Some(rate) = rate else {
        return 0;
    };
    let value = i128::from(rate.max(0)) * i128::from(duration_ms.max(0)) / 1_000;
    value.clamp(0, i128::from(i64::MAX)) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::schema, models::AppResourceSample};
    use rusqlite::Connection;

    fn connection() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        schema::migrate(&mut conn).unwrap();
        conn.execute_batch(
            "INSERT INTO boot_session(boot_id, created_at_ms) VALUES ('test-boot', 0);
             INSERT INTO collection_session(boot_session_id, started_at_ms, schema_version)
             SELECT id, 0, 8 FROM boot_session WHERE boot_id='test-boot';",
        )
        .unwrap();
        conn
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_process_sample(
        conn: &Connection,
        timestamp_ms: i64,
        duration_ms: i64,
        app_key: &str,
        cpu: Option<f64>,
        memory: Option<i64>,
        read_bps: Option<i64>,
        reason: i64,
    ) {
        let app = AppResourceSample {
            app_key: app_key.into(),
            process_name: format!("{app_key}.exe"),
            exe_path: Some(format!(r"C:\{app_key}.exe")),
            process_count: 1,
            cpu_percent: cpu.unwrap_or(0.0),
            memory_used_bytes: memory.unwrap_or(0),
            io_read_bytes_per_sec: read_bps.unwrap_or(0),
            io_write_bytes_per_sec: 0,
            process_identity_key: Some(format!("process:{app_key}:{timestamp_ms}")),
            pid: Some(timestamp_ms as u32),
            process_creation_time_ms: Some(timestamp_ms),
            private_bytes: None,
            cpu_time_delta_us: None,
            gpu_percent: None,
            vram_bytes: None,
            network_bytes_per_sec: None,
            selection_reason: reason,
            quality_mask: 0,
            measured_cpu_percent: cpu,
            measured_working_set_bytes: memory,
            measured_read_bytes_per_sec: read_bps,
            measured_write_bytes_per_sec: Some(0),
        };
        conn.execute(
            "INSERT INTO sample_frame(collection_session_id, ts, sequence, duration_ms, process_snapshot_present)
             SELECT id, ?1, ?2, ?3, 1 FROM collection_session LIMIT 1",
            params![timestamp_ms, timestamp_ms / 1_000 + 1, duration_ms],
        )
        .unwrap();
        let frame_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO app(stable_key, process_name, display_name, first_seen_at_ms, last_seen_at_ms)
             VALUES (?1, ?2, ?3, ?3, ?3) ON CONFLICT(stable_key) DO NOTHING",
            params![app.app_key, app.process_name, timestamp_ms],
        )
        .unwrap();
        let app_id: i64 = conn
            .query_row(
                "SELECT id FROM app WHERE stable_key=?1",
                [app.app_key.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO app_executable(app_id, normalized_path, first_seen_at_ms, last_seen_at_ms)
             VALUES (?1, ?2, ?3, ?3) ON CONFLICT(app_id, normalized_path) DO NOTHING",
            params![app_id, format!("legacy:{}", app.app_key), timestamp_ms],
        )
        .unwrap();
        let executable_id: i64 = conn
            .query_row(
                "SELECT id FROM app_executable WHERE app_id=?1",
                [app_id],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO process_instance(app_executable_id, stable_key, pid, create_time_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                executable_id,
                app.process_identity_key,
                app.pid.map(i64::from),
                app.process_creation_time_ms,
            ],
        )
        .unwrap();
        let process_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO process_sample(frame_id, process_instance_id, cpu_pct, working_set_bytes,
                 process_count, read_bps, write_bps, selection_reason)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, 0, ?6)",
            params![
                frame_id,
                process_id,
                app.measured_cpu_percent,
                app.measured_working_set_bytes,
                app.measured_read_bytes_per_sec,
                app.selection_reason,
            ],
        )
        .unwrap();
    }

    #[test]
    fn one_minute_rollup_uses_weighted_average_and_integrates_rates() {
        let conn = connection();
        insert_process_sample(
            &conn,
            0,
            10_000,
            "app",
            Some(10.0),
            Some(100),
            Some(1_000),
            1,
        );
        insert_process_sample(
            &conn,
            10_000,
            20_000,
            "app",
            Some(30.0),
            Some(200),
            Some(2_000),
            2,
        );
        rebuild_process_rollup_1m_bucket(&conn, 0).unwrap();
        let row: (f64, i64, i64, i64, f64, i64) = conn
            .query_row(
                "SELECT weighted_cpu_pct, max_working_set_bytes, read_bytes, sample_count, coverage, selection_reason_mask FROM process_rollup_1m",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        assert!((row.0 - 23.333333).abs() < 0.001);
        assert_eq!(row.1, 200);
        assert_eq!(row.2, 50_000);
        assert_eq!(row.3, 2);
        assert!((row.4 - 0.5).abs() < f64::EPSILON);
        assert_eq!(row.5, 3);
    }

    #[test]
    fn gaps_are_not_bridged_and_rebuild_is_idempotent() {
        let conn = connection();
        insert_process_sample(&conn, 0, 5_000, "app", Some(10.0), Some(100), None, 0);
        rebuild_process_rollup_1m_bucket(&conn, 0).unwrap();
        let first: (f64, f64) = conn
            .query_row(
                "SELECT coverage, weighted_cpu_pct FROM process_rollup_1m",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        rebuild_process_rollup_1m_bucket(&conn, 0).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM process_rollup_1m", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!((first.0 - (5_000.0 / 60_000.0)).abs() < f64::EPSILON);
        assert_eq!(first.1, 10.0);
        assert_eq!(count, 1);
    }

    #[test]
    fn hour_and_daily_rollups_use_minute_layer_and_preserve_verified_counts() {
        let conn = connection();
        insert_process_sample(
            &conn,
            0,
            60_000,
            "app",
            Some(10.0),
            Some(100),
            Some(1_000),
            1,
        );
        let app_id: i64 = conn
            .query_row("SELECT id FROM app WHERE stable_key='app'", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO app_resource_daily(local_date, app_id, crash_count, hang_count, processing_version)
             VALUES ('1970-01-01', ?1, 7, 2, 'verified-fixture')",
            [app_id],
        )
        .unwrap();

        let result = run_process_rollups(&conn, PROCESS_ROLLUP_HOUR_MS, 8).unwrap();
        assert_eq!(result.minute_buckets, 1);
        assert_eq!(result.hour_buckets, 1);
        assert_eq!(result.daily_rows, 1);

        let hour: (f64, i64, i64, f64, i64) = conn
            .query_row(
                "SELECT weighted_cpu_pct, cpu_time_us, read_bytes, coverage, selection_reason_mask
                 FROM process_rollup_1h",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(hour.0, 10.0);
        assert_eq!(hour.1, 6_000_000);
        assert_eq!(hour.2, 60_000);
        assert!((hour.3 - (60_000.0 / 3_600_000.0)).abs() < f64::EPSILON);
        assert_eq!(hour.4, 1);

        let daily: (i64, i64, f64) = conn
            .query_row(
                "SELECT crash_count, hang_count, coverage FROM app_resource_daily",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(daily.0, 7);
        assert_eq!(daily.1, 2);
        assert!((daily.2 - (60_000.0 / 86_400_000.0)).abs() < f64::EPSILON);

        let second = run_process_rollups(&conn, PROCESS_ROLLUP_HOUR_MS, 8).unwrap();
        assert_eq!(second.minute_buckets, 0);
        assert_eq!(second.hour_buckets, 0);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM process_rollup_1h", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
