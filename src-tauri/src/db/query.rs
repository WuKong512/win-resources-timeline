use crate::{
    db::usage::{
        computer_active_duration, foreground_state_segments, intersect_foreground, StateRange,
        TimeRange,
    },
    models::{
        AppIdentity, AppResourceHistoryPoint, AppResourceSample, AppUsageSummary,
        DailyUsageSummary, ForegroundInterval, GpuSample, GpuSamplePoint, ResourceApp,
        SystemSample, TodayOverview,
    },
};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeSet, HashMap};

fn valid_range(start_ms: i64, end_ms: i64) -> rusqlite::Result<()> {
    if end_ms <= start_ms {
        return Err(rusqlite::Error::InvalidParameterName(
            "end_ms must be greater than start_ms".into(),
        ));
    }
    Ok(())
}

pub fn list_apps(conn: &Connection) -> rusqlite::Result<Vec<AppIdentity>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, a.process_name, CASE WHEN e.normalized_path LIKE 'path:%' THEN substr(e.normalized_path, 6) END, a.display_name, a.publisher, a.is_hidden, a.first_seen_at_ms, a.last_seen_at_ms FROM app_executable e JOIN app a ON a.id = e.app_id ORDER BY a.display_name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AppIdentity {
            id: r.get(0)?,
            process_name: r.get(1)?,
            exe_path: r.get(2)?,
            display_name: r.get(3)?,
            publisher: r.get(4)?,
            is_hidden: r.get::<_, i64>(5)? != 0,
            first_seen_at_ms: r.get(6)?,
            last_seen_at_ms: r.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn foreground_intervals(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    include_hidden: bool,
    include_idle: bool,
) -> rusqlite::Result<Vec<ForegroundInterval>> {
    valid_range(start_ms, end_ms)?;
    let foregrounds = load_foreground_rows(conn, start_ms, end_ms, include_hidden)?;
    let states = load_state_ranges(conn, start_ms, end_ms)?;
    let tracked_boots = load_state_boots(conn)?;
    let mut intervals = Vec::new();

    for foreground in foregrounds {
        let foreground_range = TimeRange {
            start_ms: foreground.start_ms,
            end_ms: foreground.end_ms,
        };
        let segments =
            foreground_state_segments(foreground.boot_session_id, foreground_range, &states);
        if segments.is_empty() && !tracked_boots.contains(&foreground.boot_session_id) {
            if include_idle || foreground.activity_state == "active" {
                intervals
                    .push(foreground.to_model(foreground_range, foreground.activity_state.clone()));
            }
            continue;
        }
        for (range, state) in segments {
            if !include_idle && state == "idle" {
                continue;
            }
            intervals.push(foreground.to_model(range, state));
        }
    }
    intervals.sort_by_key(|interval| (interval.start_time_ms, interval.id, interval.end_time_ms));
    Ok(intervals)
}

#[derive(Debug, Clone)]
struct ForegroundRow {
    id: i64,
    boot_session_id: i64,
    app_id: i64,
    app_name: String,
    display_name: String,
    start_ms: i64,
    end_ms: i64,
    activity_state: String,
    is_hidden: bool,
}

impl ForegroundRow {
    fn to_model(&self, range: TimeRange, activity_state: String) -> ForegroundInterval {
        ForegroundInterval {
            id: self.id,
            app_id: self.app_id,
            app_name: self.app_name.clone(),
            display_name: self.display_name.clone(),
            start_time_ms: range.start_ms,
            end_time_ms: range.end_ms,
            duration_ms: range.end_ms.saturating_sub(range.start_ms),
            activity_state,
            is_hidden: self.is_hidden,
        }
    }
}

fn load_foreground_rows(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    include_hidden: bool,
) -> rusqlite::Result<Vec<ForegroundRow>> {
    let mut statement = conn.prepare(
        r#"SELECT fi.id, fi.boot_session_id, e.app_id, a.process_name, a.display_name,
                  MAX(fi.start_time_ms, ?1), MIN(COALESCE(fi.end_time_ms, fi.last_seen_time_ms), ?2),
                  fi.activity_state, a.is_hidden
           FROM foreground_interval fi
           JOIN app_executable e ON e.id = fi.app_executable_id
           JOIN app a ON a.id = e.app_id
           WHERE fi.start_time_ms < ?2
             AND COALESCE(fi.end_time_ms, fi.last_seen_time_ms) > ?1
             AND (?3 = 1 OR a.is_hidden = 0)
           ORDER BY fi.boot_session_id, fi.start_time_ms, fi.id"#,
    )?;
    let rows = statement
        .query_map(params![start_ms, end_ms, include_hidden as i64], |row| {
            let start_ms: i64 = row.get(5)?;
            let end_ms: i64 = row.get(6)?;
            Ok(ForegroundRow {
                id: row.get(0)?,
                boot_session_id: row.get(1)?,
                app_id: row.get(2)?,
                app_name: row.get(3)?,
                display_name: row.get(4)?,
                start_ms,
                end_ms,
                activity_state: row.get(7)?,
                is_hidden: row.get::<_, i64>(8)? != 0,
            })
        })?
        .filter_map(|row| match row {
            Ok(row) if row.end_ms > row.start_ms => Some(Ok(row)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect();
    rows
}

fn load_state_ranges(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<Vec<StateRange>> {
    let checkpoint = usage_checkpoint(conn)?.unwrap_or(end_ms).min(end_ms);
    let mut statement = conn.prepare(
        r#"SELECT boot_session_id, state,
                  MAX(start_ts, ?1), MIN(COALESCE(end_ts, ?3), ?2)
           FROM computer_state_interval
           WHERE start_ts < ?2 AND COALESCE(end_ts, ?3) > ?1
           ORDER BY boot_session_id, start_ts, id"#,
    )?;
    let rows = statement
        .query_map(params![start_ms, end_ms, checkpoint], |row| {
            let start_ms: i64 = row.get(2)?;
            let end_ms: i64 = row.get(3)?;
            Ok(StateRange {
                boot_session_id: row.get(0)?,
                state: row.get(1)?,
                range: TimeRange { start_ms, end_ms },
            })
        })?
        .filter_map(|row| match row {
            Ok(row) if row.range.end_ms > row.range.start_ms => Some(Ok(row)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect();
    rows
}

fn load_state_boots(conn: &Connection) -> rusqlite::Result<BTreeSet<i64>> {
    let mut statement =
        conn.prepare("SELECT DISTINCT boot_session_id FROM computer_state_interval")?;
    let rows = statement.query_map([], |row| row.get(0))?.collect();
    rows
}

fn usage_checkpoint(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_usage_heartbeat_ms'",
        [],
        |row| row.get(0),
    )
    .optional()
}

pub fn timeline_available_dates(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = conn.prepare(
        r#"WITH RECURSIVE
           bounds AS (
             SELECT date(min(start_time_ms) / 1000.0, 'unixepoch', 'localtime') AS min_day,
                    date(max(COALESCE(end_time_ms, last_seen_time_ms)) / 1000.0, 'unixepoch', 'localtime') AS max_day
             FROM foreground_interval
           ),
           dates(day) AS (
             SELECT min_day FROM bounds WHERE min_day IS NOT NULL
             UNION ALL
             SELECT date(day, '+1 day') FROM dates, bounds WHERE day < max_day
           )
           SELECT day FROM dates
           WHERE EXISTS (
             SELECT 1 FROM foreground_interval fi
             WHERE fi.start_time_ms < CAST(strftime('%s', datetime(day, '+1 day'), 'utc') AS INTEGER) * 1000
               AND COALESCE(fi.end_time_ms, fi.last_seen_time_ms) > CAST(strftime('%s', day || ' 00:00:00', 'utc') AS INTEGER) * 1000
           )
           ORDER BY day"#,
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn resource_available_dates(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT DISTINCT date(ts / 1000.0, 'unixepoch', 'localtime') FROM sample_frame ORDER BY 1",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    rows.collect()
}

pub fn overview_available_dates(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut dates = BTreeSet::new();
    dates.extend(timeline_available_dates(conn)?);
    dates.extend(resource_available_dates(conn)?);
    Ok(dates.into_iter().collect())
}

pub fn usage_summary(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    include_hidden: bool,
) -> rusqlite::Result<Vec<AppUsageSummary>> {
    valid_range(start_ms, end_ms)?;
    let foregrounds = load_foreground_rows(conn, start_ms, end_ms, include_hidden)?;
    let states = load_state_ranges(conn, start_ms, end_ms)?;
    let tracked_boots = load_state_boots(conn)?;
    let mut grouped = std::collections::BTreeMap::<i64, AppUsageSummary>::new();
    for foreground in foregrounds {
        let range = TimeRange {
            start_ms: foreground.start_ms,
            end_ms: foreground.end_ms,
        };
        let durations = if tracked_boots.contains(&foreground.boot_session_id) {
            intersect_foreground(foreground.boot_session_id, range, &states)
        } else if foreground.activity_state == "idle" {
            crate::db::usage::UsageDurations {
                active_ms: 0,
                idle_ms: foreground.end_ms - foreground.start_ms,
            }
        } else {
            crate::db::usage::UsageDurations {
                active_ms: foreground.end_ms - foreground.start_ms,
                idle_ms: 0,
            }
        };
        let item = grouped.entry(foreground.app_id).or_insert(AppUsageSummary {
            app_id: foreground.app_id,
            app_name: foreground.app_name,
            display_name: foreground.display_name,
            foreground_total_ms: 0,
            active_usage_ms: 0,
            idle_foreground_ms: 0,
            active_seconds: 0,
            idle_seconds: 0,
            percentage: 0.0,
            is_hidden: foreground.is_hidden,
        });
        item.foreground_total_ms = item
            .foreground_total_ms
            .saturating_add(foreground.end_ms - foreground.start_ms);
        item.active_usage_ms = item.active_usage_ms.saturating_add(durations.active_ms);
        item.idle_foreground_ms = item.idle_foreground_ms.saturating_add(durations.idle_ms);
    }
    let total_active: i64 = grouped.values().map(|item| item.active_usage_ms).sum();
    let mut values: Vec<_> = grouped.into_values().collect();
    for item in &mut values {
        item.active_seconds = item.active_usage_ms / 1000;
        item.idle_seconds = item.idle_foreground_ms / 1000;
        item.percentage = if total_active > 0 {
            item.active_usage_ms as f64 * 100.0 / total_active as f64
        } else {
            0.0
        };
    }
    values.sort_by_key(|value| std::cmp::Reverse(value.active_usage_ms));
    Ok(values)
}

pub fn computer_active_time(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<i64> {
    valid_range(start_ms, end_ms)?;
    let states = load_state_ranges(conn, start_ms, end_ms)?;
    Ok(computer_active_duration(&states, start_ms, end_ms))
}

pub fn daily_usage_summary(
    conn: &Connection,
    local_date: &str,
    include_hidden: bool,
) -> rusqlite::Result<Vec<DailyUsageSummary>> {
    let mut statement = conn.prepare(
        r#"SELECT d.local_date, d.app_id, a.process_name, a.display_name,
                  d.foreground_total_ms, COALESCE(d.active_usage_ms, 0),
                  d.idle_foreground_ms, d.launch_count, d.processing_version, a.is_hidden
           FROM app_usage_daily d
           JOIN app a ON a.id = d.app_id
           WHERE d.local_date = ?1 AND (?2 = 1 OR a.is_hidden = 0)
           ORDER BY COALESCE(d.active_usage_ms, 0) DESC, d.foreground_total_ms DESC,
                    a.display_name COLLATE NOCASE"#,
    )?;
    let rows = statement
        .query_map(params![local_date, include_hidden as i64], |row| {
            Ok(DailyUsageSummary {
                local_date: row.get(0)?,
                app_id: row.get(1)?,
                app_name: row.get(2)?,
                display_name: row.get(3)?,
                foreground_total_ms: row.get(4)?,
                active_usage_ms: row.get(5)?,
                idle_foreground_ms: row.get(6)?,
                launch_count: row.get(7)?,
                processing_version: row.get(8)?,
                is_hidden: row.get::<_, i64>(9)? != 0,
            })
        })?
        .collect();
    rows
}

pub fn system_samples(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> rusqlite::Result<Vec<SystemSample>> {
    valid_range(start_ms, end_ms)?;
    let mut stmt = conn.prepare(
        r#"SELECT f.id, f.ts, f.duration_ms, cpu.usage_pct, memory.usage_pct, memory.used_bytes,
                  CASE WHEN memory.used_bytes IS NOT NULL AND memory.available_bytes IS NOT NULL
                       THEN memory.used_bytes + memory.available_bytes END,
                  disk.read_bps, disk.write_bps, f.process_snapshot_present
           FROM sample_frame f
           LEFT JOIN cpu_sample cpu ON cpu.frame_id = f.id
           LEFT JOIN memory_sample memory ON memory.frame_id = f.id
           LEFT JOIN (
             SELECT frame_id, SUM(read_bps) AS read_bps, SUM(write_bps) AS write_bps
             FROM disk_sample GROUP BY frame_id
           ) disk ON disk.frame_id = f.id
           WHERE f.ts >= ?1 AND f.ts < ?2
           ORDER BY f.ts"#,
    )?;
    let frame_rows: Vec<(i64, SystemSample)> = stmt
        .query_map(params![start_ms, end_ms], |r| {
            Ok((
                r.get(0)?,
                SystemSample {
                    timestamp_ms: r.get(1)?,
                    sample_duration_ms: r.get(2)?,
                    cpu_percent: r.get(3)?,
                    memory_percent: r.get(4)?,
                    memory_used_bytes: r.get(5)?,
                    memory_total_bytes: r.get(6)?,
                    disk_read_bytes_per_sec: r.get(7)?,
                    disk_write_bytes_per_sec: r.get(8)?,
                    gpus: Vec::new(),
                    has_app_snapshot: r.get::<_, i64>(9)? != 0,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let gpus_by_frame = gpu_samples_by_frame(conn, start_ms, end_ms)?;
    let all: Vec<SystemSample> = frame_rows
        .into_iter()
        .map(|(frame_id, mut sample)| {
            sample.gpus = gpus_by_frame.get(&frame_id).cloned().unwrap_or_default();
            sample
        })
        .collect();
    if all.len() <= max_points {
        return Ok(all);
    }
    let stride = all.len().div_ceil(max_points.max(1));
    Ok(all.into_iter().step_by(stride).collect())
}

fn gpu_samples_by_frame(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<HashMap<i64, Vec<GpuSample>>> {
    let mut stmt = conn.prepare(
        r#"SELECT f.id, d.stable_key, d.vendor, d.model, d.capacity_bytes,
                  g.usage_pct, g.memory_controller_usage_pct, g.temp_c, g.board_power_w,
                  g.core_clock_mhz, g.memory_clock_mhz, g.vram_used_bytes,
                  g.vram_total_bytes, g.power_scope
           FROM gpu_sample g
           JOIN sample_frame f ON f.id = g.frame_id
           JOIN hardware_device d ON d.id = g.device_id
           WHERE f.ts >= ?1 AND f.ts < ?2
           ORDER BY f.ts, d.id"#,
    )?;
    let rows = stmt
        .query_map(params![start_ms, end_ms], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                GpuSample {
                    device_key: r.get(1)?,
                    vendor: r.get(2)?,
                    model: r.get(3)?,
                    capacity_bytes: r.get(4)?,
                    utilization_percent: r.get(5)?,
                    memory_controller_utilization_percent: r.get(6)?,
                    temperature_celsius: r.get(7)?,
                    power_watts: r.get(8)?,
                    graphics_clock_mhz: r.get(9)?,
                    memory_clock_mhz: r.get(10)?,
                    vram_used_bytes: r.get(11)?,
                    vram_total_bytes: r.get(12)?,
                    power_scope: r.get(13)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut grouped = HashMap::new();
    for (frame_id, gpu) in rows {
        grouped.entry(frame_id).or_insert_with(Vec::new).push(gpu);
    }
    Ok(grouped)
}

pub fn gpu_samples(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    device_key: Option<&str>,
) -> rusqlite::Result<Vec<GpuSamplePoint>> {
    valid_range(start_ms, end_ms)?;
    let mut stmt = conn.prepare(
        r#"SELECT f.ts, f.duration_ms, d.stable_key, d.vendor, d.model, d.capacity_bytes,
                  g.usage_pct, g.memory_controller_usage_pct, g.temp_c, g.board_power_w,
                  g.core_clock_mhz, g.memory_clock_mhz, g.vram_used_bytes,
                  g.vram_total_bytes, g.power_scope
           FROM gpu_sample g
           JOIN sample_frame f ON f.id = g.frame_id
           JOIN hardware_device d ON d.id = g.device_id
           WHERE f.ts >= ?1 AND f.ts < ?2
             AND (?3 IS NULL OR d.stable_key = ?3)
           ORDER BY f.ts, d.id"#,
    )?;
    let rows = stmt.query_map(params![start_ms, end_ms, device_key], |r| {
        Ok(GpuSamplePoint {
            timestamp_ms: r.get(0)?,
            sample_duration_ms: r.get(1)?,
            gpu: GpuSample {
                device_key: r.get(2)?,
                vendor: r.get(3)?,
                model: r.get(4)?,
                capacity_bytes: r.get(5)?,
                utilization_percent: r.get(6)?,
                memory_controller_utilization_percent: r.get(7)?,
                temperature_celsius: r.get(8)?,
                power_watts: r.get(9)?,
                graphics_clock_mhz: r.get(10)?,
                memory_clock_mhz: r.get(11)?,
                vram_used_bytes: r.get(12)?,
                vram_total_bytes: r.get(13)?,
                power_scope: r.get(14)?,
            },
        })
    })?;
    rows.collect()
}

pub fn app_resource_samples(
    conn: &Connection,
    timestamp_ms: i64,
) -> rusqlite::Result<Vec<AppResourceSample>> {
    let mut stmt = conn.prepare(
        r#"SELECT a.stable_key, a.process_name,
                  CASE WHEN e.normalized_path LIKE 'path:%' THEN substr(e.normalized_path, 6) END,
                  p.process_count, p.cpu_pct, p.working_set_bytes, p.read_bps, p.write_bps
           FROM process_sample p
           JOIN sample_frame f ON f.id = p.frame_id
           JOIN process_instance i ON i.id = p.process_instance_id
           JOIN app_executable e ON e.id = i.app_executable_id
           JOIN app a ON a.id = e.app_id
           WHERE f.ts = ?1
           ORDER BY p.cpu_pct DESC, p.working_set_bytes DESC"#,
    )?;
    let rows = stmt.query_map([timestamp_ms], |r| {
        Ok(AppResourceSample {
            app_key: r.get(0)?,
            process_name: r.get(1)?,
            exe_path: r.get(2)?,
            process_count: r.get(3)?,
            cpu_percent: r.get(4)?,
            memory_used_bytes: r.get(5)?,
            io_read_bytes_per_sec: r.get(6)?,
            io_write_bytes_per_sec: r.get(7)?,
        })
    })?;
    rows.collect()
}

pub fn resource_apps(conn: &Connection) -> rusqlite::Result<Vec<ResourceApp>> {
    let mut stmt = conn.prepare(
        r#"WITH samples AS (
             SELECT LOWER(a.process_name) AS process_key, a.process_name, a.display_name,
                     CASE WHEN e.normalized_path LIKE 'path:%' THEN substr(e.normalized_path, 6) END AS exe_path,
                     f.ts
             FROM process_sample p
             JOIN sample_frame f ON f.id = p.frame_id
             JOIN process_instance i ON i.id = p.process_instance_id
             JOIN app_executable e ON e.id = i.app_executable_id
             JOIN app a ON a.id = e.app_id
             WHERE a.process_name IS NOT NULL AND trim(a.process_name) <> ''
           ),
           ranked AS (
             SELECT samples.*,
                    ROW_NUMBER() OVER (PARTITION BY process_key ORDER BY ts DESC) AS row_number
             FROM samples
           ),
           friendly AS (
             SELECT process_key, display_name
             FROM (
               SELECT process_key, display_name,
                      ROW_NUMBER() OVER (PARTITION BY process_key ORDER BY ts DESC) AS row_number
               FROM samples
               WHERE LOWER(TRIM(display_name)) <> LOWER(TRIM(process_name))
             )
             WHERE row_number = 1
           )
           SELECT 'process:' || ranked.process_key, ranked.process_name,
                  COALESCE(friendly.display_name, ranked.display_name), ranked.exe_path, ranked.ts
           FROM ranked
           LEFT JOIN friendly ON friendly.process_key = ranked.process_key
           WHERE ranked.row_number = 1
           ORDER BY COALESCE(friendly.display_name, ranked.display_name) COLLATE NOCASE"#,
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ResourceApp {
            app_key: r.get(0)?,
            process_name: r.get(1)?,
            display_name: r.get(2)?,
            exe_path: r.get(3)?,
            last_sample_at_ms: r.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn app_resource_available_dates(
    conn: &Connection,
    app_key: &str,
) -> rusqlite::Result<Vec<String>> {
    let (predicate, key) = app_resource_predicate(app_key);
    let predicate = predicate.replace("?3", "?1");
    let sql = format!(
        "SELECT DISTINCT date(f.ts / 1000.0, 'unixepoch', 'localtime') FROM process_sample p JOIN sample_frame f ON f.id=p.frame_id JOIN process_instance i ON i.id=p.process_instance_id JOIN app_executable e ON e.id=i.app_executable_id JOIN app a ON a.id=e.app_id WHERE {predicate} ORDER BY 1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([key], |row| row.get(0))?;
    rows.collect()
}

pub fn app_resource_history(
    conn: &Connection,
    app_key: &str,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> rusqlite::Result<Vec<AppResourceHistoryPoint>> {
    valid_range(start_ms, end_ms)?;
    let (predicate, key) = app_resource_predicate(app_key);
    let sql = format!(
        r#"WITH app_samples AS (
             SELECT p.frame_id, SUM(p.cpu_pct) AS cpu_percent, SUM(p.working_set_bytes) AS memory_used_bytes,
                    SUM(p.read_bps) AS io_read_bytes_per_sec, SUM(p.write_bps) AS io_write_bytes_per_sec
             FROM process_sample p
             JOIN process_instance i ON i.id=p.process_instance_id
             JOIN app_executable e ON e.id=i.app_executable_id
             JOIN app a ON a.id=e.app_id
             WHERE {predicate}
             GROUP BY p.frame_id
           )
           SELECT f.ts, f.duration_ms, app.cpu_percent, app.memory_used_bytes,
                  app.io_read_bytes_per_sec, app.io_write_bytes_per_sec
            FROM sample_frame f
            LEFT JOIN app_samples app ON app.frame_id=f.id
            WHERE f.process_snapshot_present = 1
              AND f.ts >= ?1 AND f.ts < ?2
           ORDER BY f.ts"#
    );
    let mut stmt = conn.prepare(&sql)?;
    let all: Vec<AppResourceHistoryPoint> = stmt
        .query_map(params![start_ms, end_ms, key], |r| {
            Ok(AppResourceHistoryPoint {
                timestamp_ms: r.get(0)?,
                sample_duration_ms: r.get(1)?,
                cpu_percent: r.get(2)?,
                memory_used_bytes: r.get(3)?,
                io_read_bytes_per_sec: r.get(4)?,
                io_write_bytes_per_sec: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    if all.len() <= max_points {
        return Ok(all);
    }
    let stride = all.len().div_ceil(max_points.max(1));
    Ok(all.into_iter().step_by(stride).collect())
}

fn app_resource_predicate(app_key: &str) -> (&'static str, &str) {
    if let Some(process_name) = app_key.strip_prefix("process:") {
        ("LOWER(a.process_name) = LOWER(?3)", process_name)
    } else {
        ("a.stable_key = ?3", app_key)
    }
}

pub fn today_overview(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<TodayOverview> {
    let all_usage = usage_summary(conn, start_ms, end_ms, true)?;
    let total_active_foreground_seconds = all_usage.iter().map(|a| a.active_seconds).sum();
    let total_idle_foreground_seconds = all_usage.iter().map(|a| a.idle_seconds).sum();
    let computer_active_seconds = computer_active_time(conn, start_ms, end_ms)? / 1000;
    let hidden_active_foreground_seconds = all_usage
        .iter()
        .filter(|a| a.is_hidden)
        .map(|a| a.active_seconds)
        .sum();
    let visible_active_seconds: i64 = all_usage
        .iter()
        .filter(|app| !app.is_hidden)
        .map(|app| app.active_seconds)
        .sum();
    let mut top_apps: Vec<_> = all_usage
        .into_iter()
        .filter(|a| !a.is_hidden)
        .take(10)
        .collect();
    for app in &mut top_apps {
        app.percentage = if visible_active_seconds > 0 {
            app.active_seconds as f64 * 100.0 / visible_active_seconds as f64
        } else {
            0.0
        };
    }
    let samples = system_samples(conn, start_ms, end_ms, 10_000)?;
    Ok(TodayOverview {
        start_ms,
        end_ms,
        total_active_foreground_seconds,
        total_idle_foreground_seconds,
        computer_active_seconds,
        hidden_active_foreground_seconds,
        top_apps,
        cpu_sampled_peak: samples
            .iter()
            .filter_map(|s| s.cpu_percent)
            .reduce(f64::max),
        memory_sampled_peak: samples
            .iter()
            .filter_map(|s| s.memory_percent)
            .reduce(f64::max),
        disk_read_sampled_peak: samples
            .iter()
            .filter_map(|s| s.disk_read_bytes_per_sec)
            .max(),
        disk_write_sampled_peak: samples
            .iter()
            .filter_map(|s| s.disk_write_bytes_per_sec)
            .max(),
    })
}
