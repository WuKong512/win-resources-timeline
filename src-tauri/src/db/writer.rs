use crate::models::{CollectionSettings, ForegroundApp, ResourceSnapshot, SystemSample};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{HashSet, VecDeque},
    io,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriterHealth {
    pub queue_depth: usize,
    pub writer_delay_ms: i64,
    pub drop_count: u64,
    pub retry_count: u64,
    pub terminal_failure_count: u64,
    pub front_retry_attempts: Option<u32>,
    pub front_retry_backoff_ms: Option<u64>,
    pub last_commit_duration_ms: u64,
    pub last_committed_timestamp_ms: Option<i64>,
    pub last_error: Option<String>,
}

struct QueuedFrame {
    snapshot: ResourceSnapshot,
    attempts: u32,
    next_attempt_at: Instant,
}

pub struct FrameWriter {
    queue: VecDeque<QueuedFrame>,
    max_queue_depth: usize,
    retry_limit: u32,
    health: WriterHealth,
}

impl FrameWriter {
    pub fn new(max_queue_depth: usize, retry_limit: u32) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_queue_depth),
            max_queue_depth,
            // retry_limit counts retries after the initial write attempt. Zero means one attempt.
            retry_limit: retry_limit.min(MAX_RETRY_LIMIT),
            health: WriterHealth::default(),
        }
    }

    pub fn enqueue(&mut self, snapshot: ResourceSnapshot) -> bool {
        if self.queue.len() >= self.max_queue_depth {
            self.health.drop_count = self.health.drop_count.saturating_add(1);
            self.health.queue_depth = self.queue.len();
            return false;
        }
        self.queue.push_back(QueuedFrame {
            snapshot,
            attempts: 0,
            next_attempt_at: Instant::now(),
        });
        self.health.queue_depth = self.queue.len();
        true
    }

    /// Attempts the queue front once.
    ///
    /// `Ok(true)` means a frame committed and was removed. `Ok(false)` means no frame was
    /// committed because the queue is empty or the front frame is still in backoff. A write
    /// failure is returned as `Err`; terminal failures are also removed and recorded in health,
    /// so callers can distinguish them from a deferred retry using `terminal_failure_count`.
    pub fn write_next(&mut self, conn: &Connection) -> rusqlite::Result<bool> {
        self.write_next_inner(conn, false, None, insert_resource_snapshot)
    }

    fn write_next_inner<F>(
        &mut self,
        conn: &Connection,
        ignore_backoff: bool,
        deadline: Option<Instant>,
        write: F,
    ) -> rusqlite::Result<bool>
    where
        F: FnOnce(&Connection, &ResourceSnapshot) -> rusqlite::Result<()>,
    {
        let Some(item) = self.queue.front() else {
            self.health.queue_depth = 0;
            self.health.front_retry_attempts = None;
            self.health.front_retry_backoff_ms = None;
            return Ok(false);
        };
        if !ignore_backoff && Instant::now() < item.next_attempt_at {
            self.health.queue_depth = self.queue.len();
            self.health.front_retry_attempts = Some(item.attempts);
            self.health.front_retry_backoff_ms = Some(remaining_backoff_ms(item.next_attempt_at));
            return Ok(false);
        }
        let timestamp_ms = item.snapshot.system.timestamp_ms;
        let started = Instant::now();
        if let Some(deadline) = deadline {
            if let Err(error) = configure_connection_for_deadline(conn, deadline) {
                self.health.last_error = Some(error.to_string());
                self.refresh_front_health();
                return Err(error);
            }
        }
        match write(conn, &item.snapshot) {
            Ok(()) => {
                self.queue.pop_front();
                self.health.queue_depth = self.queue.len();
                self.health.writer_delay_ms = now_ms().saturating_sub(timestamp_ms).max(0);
                self.health.last_commit_duration_ms =
                    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                self.health.last_committed_timestamp_ms = Some(timestamp_ms);
                // last_error is the most recent observed write error and remains visible until a
                // newer error replaces it, including when a later frame commits successfully.
                self.refresh_front_health();
                Ok(true)
            }
            Err(error) => {
                let attempts = if let Some(item) = self.queue.front_mut() {
                    item.attempts = item.attempts.saturating_add(1);
                    item.next_attempt_at = Instant::now()
                        .checked_add(retry_backoff(item.attempts))
                        .unwrap_or_else(Instant::now);
                    item.attempts
                } else {
                    0
                };
                self.health.retry_count = self.health.retry_count.saturating_add(1);
                self.health.queue_depth = self.queue.len();
                self.health.front_retry_attempts = Some(attempts);
                self.health.front_retry_backoff_ms = self
                    .queue
                    .front()
                    .map(|queued| remaining_backoff_ms(queued.next_attempt_at));
                self.health.last_error = Some(error.to_string());
                if attempts > self.retry_limit {
                    self.queue.pop_front();
                    self.health.drop_count = self.health.drop_count.saturating_add(1);
                    self.health.terminal_failure_count =
                        self.health.terminal_failure_count.saturating_add(1);
                    self.refresh_front_health();
                }
                Err(error)
            }
        }
    }

    pub fn flush_all(&mut self, conn: &Connection) -> rusqlite::Result<usize> {
        self.flush_all_inner(conn, insert_resource_snapshot)
    }

    pub fn flush_until(&mut self, conn: &Connection, deadline: Instant) -> rusqlite::Result<usize> {
        self.flush_until_inner(conn, deadline, insert_resource_snapshot)
    }

    fn flush_all_inner<F>(&mut self, conn: &Connection, mut write: F) -> rusqlite::Result<usize>
    where
        F: FnMut(&Connection, &ResourceSnapshot) -> rusqlite::Result<()>,
    {
        let mut committed = 0;
        let mut last_error = None;
        let initial_terminal_failures = self.health.terminal_failure_count;
        while self.queue.front().is_some() {
            match self.write_next_inner(conn, true, None, &mut write) {
                Ok(true) => committed += 1,
                Ok(false) => break,
                Err(error) => last_error = Some(error),
            }
        }
        if self.queue.is_empty() && self.health.terminal_failure_count == initial_terminal_failures
        {
            return Ok(committed);
        }
        Err(last_error.unwrap_or_else(|| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(io::Error::other(
                "frame queue could not be drained",
            )))
        }))
    }

    fn flush_until_inner<F>(
        &mut self,
        conn: &Connection,
        deadline: Instant,
        mut write: F,
    ) -> rusqlite::Result<usize>
    where
        F: FnMut(&Connection, &ResourceSnapshot) -> rusqlite::Result<()>,
    {
        let mut committed = 0;
        let mut last_error = None;
        let initial_terminal_failures = self.health.terminal_failure_count;
        let mut deadline_exceeded_after_commit = false;
        while self.queue.front().is_some() {
            if Instant::now() >= deadline {
                let error = shutdown_timeout_error();
                self.health.last_error = Some(error.to_string());
                self.refresh_front_health();
                last_error = Some(error);
                break;
            }
            match self.write_next_inner(conn, true, Some(deadline), &mut write) {
                Ok(true) => {
                    committed += 1;
                    if Instant::now() >= deadline {
                        deadline_exceeded_after_commit = true;
                        break;
                    }
                }
                Ok(false) => break,
                Err(error) => {
                    last_error = Some(error);
                    if Instant::now() >= deadline {
                        break;
                    }
                }
            }
        }
        let timed_out = deadline_exceeded_after_commit
            || (Instant::now() >= deadline && self.queue.front().is_some());
        if self.queue.is_empty()
            && self.health.terminal_failure_count == initial_terminal_failures
            && !timed_out
        {
            return Ok(committed);
        }
        Err(last_error.unwrap_or_else(|| {
            if timed_out {
                shutdown_timeout_error()
            } else {
                rusqlite::Error::ToSqlConversionFailure(Box::new(io::Error::other(
                    "frame queue could not be drained before shutdown deadline",
                )))
            }
        }))
    }

    pub fn discard_for_explicit_clear(&mut self) {
        self.health.drop_count = self
            .health
            .drop_count
            .saturating_add(u64::try_from(self.queue.len()).unwrap_or(u64::MAX));
        self.queue.clear();
        self.health.queue_depth = 0;
        self.health.front_retry_attempts = None;
        self.health.front_retry_backoff_ms = None;
    }

    pub fn health(&self) -> WriterHealth {
        let mut health = self.health.clone();
        health.queue_depth = self.queue.len();
        if let Some(queued) = self.queue.front() {
            health.front_retry_attempts = Some(queued.attempts);
            health.front_retry_backoff_ms = Some(remaining_backoff_ms(queued.next_attempt_at));
        } else {
            health.front_retry_attempts = None;
            health.front_retry_backoff_ms = None;
        }
        health
    }

    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    #[cfg(test)]
    pub(crate) fn write_next_with<F>(
        &mut self,
        conn: &Connection,
        ignore_backoff: bool,
        write: F,
    ) -> rusqlite::Result<bool>
    where
        F: FnOnce(&Connection, &ResourceSnapshot) -> rusqlite::Result<()>,
    {
        self.write_next_inner(conn, ignore_backoff, None, write)
    }

    #[cfg(test)]
    pub(crate) fn flush_all_with<F>(
        &mut self,
        conn: &Connection,
        write: F,
    ) -> rusqlite::Result<usize>
    where
        F: FnMut(&Connection, &ResourceSnapshot) -> rusqlite::Result<()>,
    {
        self.flush_all_inner(conn, write)
    }

    #[cfg(test)]
    pub(crate) fn flush_until_with<F>(
        &mut self,
        conn: &Connection,
        deadline: Instant,
        write: F,
    ) -> rusqlite::Result<usize>
    where
        F: FnMut(&Connection, &ResourceSnapshot) -> rusqlite::Result<()>,
    {
        self.flush_until_inner(conn, deadline, write)
    }

    fn refresh_front_health(&mut self) {
        self.health.queue_depth = self.queue.len();
        if let Some(queued) = self.queue.front() {
            self.health.front_retry_attempts = Some(queued.attempts);
            self.health.front_retry_backoff_ms = Some(remaining_backoff_ms(queued.next_attempt_at));
        } else {
            self.health.front_retry_attempts = None;
            self.health.front_retry_backoff_ms = None;
        }
    }
}

pub fn recover_open_interval(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE foreground_interval SET end_time_ms = last_seen_time_ms, close_reason = 'recovery' WHERE end_time_ms IS NULL",
        [],
    )
}

pub fn start_runtime_session(conn: &Connection, now: i64) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    let boot_key = format!("runtime-{now}-{}", std::process::id());
    tx.execute(
        "INSERT INTO boot_session(boot_id, boot_time_ms, observed_start_ms, shutdown_kind, created_at_ms) VALUES (?1, ?2, ?2, NULL, ?2)",
        params![boot_key, now],
    )?;
    let boot_id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO collection_session(boot_session_id, started_at_ms, app_version, schema_version, config_hash) VALUES (?1, ?2, ?3, 7, NULL)",
        params![boot_id, now, env!("CARGO_PKG_VERSION")],
    )?;
    let collection_id = tx.last_insert_rowid();
    for (key, value) in [
        ("runtime_boot_session_id", boot_id),
        ("runtime_collection_session_id", collection_id),
    ] {
        tx.execute(
            "INSERT INTO settings(key, value, updated_at_ms) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            params![key, value.to_string(), now],
        )?;
    }
    tx.commit()
}

pub fn ensure_runtime_session(conn: &Connection, now: i64) -> rusqlite::Result<()> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_collection_session_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        start_runtime_session(conn, now)?;
    }
    Ok(())
}

pub fn finish_runtime_session(
    conn: &Connection,
    now: i64,
    shutdown_kind: &str,
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    let collection_id: Option<i64> = tx
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_collection_session_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(collection_id) = collection_id {
        let boot_id: Option<i64> = tx
            .query_row(
                "SELECT boot_session_id FROM collection_session WHERE id = ?1",
                [collection_id],
                |row| row.get(0),
            )
            .optional()?;
        tx.execute(
            "UPDATE collection_session
             SET ended_at_ms = MAX(started_at_ms, ?1)
             WHERE id = ?2 AND ended_at_ms IS NULL",
            params![now, collection_id],
        )?;
        if let Some(boot_id) = boot_id {
            tx.execute(
                "UPDATE boot_session
                 SET observed_end_ms = MAX(COALESCE(observed_end_ms, 0), ?1), shutdown_kind = ?2
                 WHERE id = ?3",
                params![now, shutdown_kind, boot_id],
            )?;
        }
    }
    tx.commit()
}

pub fn upsert_app(conn: &Connection, app: &ForegroundApp, now: i64) -> rusqlite::Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let app_id = upsert_app_tx(&tx, app, now, true)?;
    let executable_id = tx.query_row(
        "SELECT id FROM app_executable WHERE app_id = ?1 AND normalized_path = ?2",
        params![app_id, normalized_path(app)],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(executable_id)
}

pub fn begin_interval(
    conn: &Connection,
    app_executable_id: i64,
    at_ms: i64,
    state: &str,
) -> rusqlite::Result<i64> {
    ensure_runtime_session(conn, at_ms)?;
    let boot_id = current_boot_session_id(conn)?;
    conn.execute(
        "INSERT INTO foreground_interval(boot_session_id, app_executable_id, start_time_ms, last_seen_time_ms, activity_state) VALUES (?1, ?2, ?3, ?3, ?4)",
        params![boot_id, app_executable_id, at_ms, state],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn checkpoint_interval(
    conn: &Connection,
    interval_id: i64,
    at_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE foreground_interval SET last_seen_time_ms = MAX(last_seen_time_ms, ?1) WHERE id = ?2 AND end_time_ms IS NULL",
        params![at_ms, interval_id],
    )?;
    Ok(())
}

pub fn close_interval(
    conn: &Connection,
    interval_id: i64,
    at_ms: i64,
    reason: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE foreground_interval SET last_seen_time_ms = MAX(last_seen_time_ms, ?1), end_time_ms = MAX(start_time_ms, ?1), close_reason = ?2 WHERE id = ?3 AND end_time_ms IS NULL",
        params![at_ms, reason, interval_id],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn insert_system_sample(conn: &Connection, sample: &SystemSample) -> rusqlite::Result<()> {
    insert_snapshot(conn, sample, &[], sample.has_app_snapshot)
}

pub fn insert_resource_snapshot(
    conn: &Connection,
    snapshot: &ResourceSnapshot,
) -> rusqlite::Result<()> {
    insert_snapshot(conn, &snapshot.system, &snapshot.apps, true)
}

fn insert_snapshot(
    conn: &Connection,
    system: &SystemSample,
    apps: &[crate::models::AppResourceSample],
    process_snapshot_present: bool,
) -> rusqlite::Result<()> {
    ensure_runtime_session(conn, system.timestamp_ms)?;
    let tx = conn.unchecked_transaction()?;
    let collection_id: i64 = tx.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_collection_session_id'",
        [],
        |row| row.get(0),
    )?;
    let sequence: i64 = tx.query_row(
        "SELECT COALESCE(MAX(sequence), 0) + 1 FROM sample_frame WHERE collection_session_id = ?1",
        [collection_id],
        |row| row.get(0),
    )?;
    let writer_delay_ms = now_ms().saturating_sub(system.timestamp_ms).max(0);
    tx.execute(
        "INSERT INTO sample_frame(collection_session_id, ts, sequence, duration_ms, writer_delay_ms, process_snapshot_present) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![collection_id, system.timestamp_ms, sequence, system.sample_duration_ms.max(1), writer_delay_ms, process_snapshot_present as i64],
    )?;
    let frame_id = tx.last_insert_rowid();
    if system.cpu_percent.is_some() {
        tx.execute(
            "INSERT INTO cpu_sample(frame_id, usage_pct) VALUES (?1, ?2)",
            params![frame_id, system.cpu_percent],
        )?;
    }
    if system.memory_percent.is_some()
        || system.memory_used_bytes.is_some()
        || system.memory_total_bytes.is_some()
    {
        let available = system
            .memory_total_bytes
            .zip(system.memory_used_bytes)
            .map(|(total, used)| (total - used).max(0));
        tx.execute(
            "INSERT INTO memory_sample(frame_id, used_bytes, available_bytes, usage_pct) VALUES (?1, ?2, ?3, ?4)",
            params![frame_id, system.memory_used_bytes, available, system.memory_percent],
        )?;
    }
    if system.disk_read_bytes_per_sec.is_some() || system.disk_write_bytes_per_sec.is_some() {
        let device_id = ensure_device_tx(&tx, "runtime:disk-total", "disk", system.timestamp_ms)?;
        tx.execute(
            "INSERT INTO disk_sample(frame_id, device_id, read_bps, write_bps) VALUES (?1, ?2, ?3, ?4)",
            params![frame_id, device_id, system.disk_read_bytes_per_sec, system.disk_write_bytes_per_sec],
        )?;
    }
    for app in apps {
        let executable_id = upsert_resource_app_tx(&tx, app, system.timestamp_ms)?;
        let instance_key = format!("runtime:{}", app.app_key);
        tx.execute(
            "INSERT INTO process_instance(app_executable_id, stable_key, source) VALUES (?1, ?2, 'runtime') ON CONFLICT(stable_key) DO UPDATE SET app_executable_id = excluded.app_executable_id",
            params![executable_id, instance_key],
        )?;
        let process_id: i64 = tx.query_row(
            "SELECT id FROM process_instance WHERE stable_key = ?1",
            [&instance_key],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO process_sample(frame_id, process_instance_id, cpu_pct, working_set_bytes, process_count, read_bps, write_bps) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![frame_id, process_id, app.cpu_percent, app.memory_used_bytes, app.process_count, app.io_read_bytes_per_sec, app.io_write_bytes_per_sec],
        )?;
    }
    tx.commit()
}

fn upsert_app_tx(
    tx: &rusqlite::Transaction<'_>,
    app: &ForegroundApp,
    now: i64,
    overwrite_display_name: bool,
) -> rusqlite::Result<i64> {
    let process_name = app.process_name.trim();
    if process_name.is_empty() {
        return Err(rusqlite::Error::InvalidParameterName(
            "process_name must be present for an app identity".into(),
        ));
    }
    let display_name = if app.display_name.trim().is_empty() {
        process_name
    } else {
        app.display_name.trim()
    };
    tx.execute(
        "INSERT INTO app(stable_key, process_name, display_name, first_seen_at_ms, last_seen_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(stable_key) DO UPDATE SET
            process_name = COALESCE(excluded.process_name, app.process_name),
            display_name = CASE WHEN ?5 THEN excluded.display_name ELSE app.display_name END,
            last_seen_at_ms = excluded.last_seen_at_ms",
        params![
            app.identity_key,
            process_name,
            display_name,
            now,
            overwrite_display_name as i64
        ],
    )?;
    let app_id: i64 = tx.query_row(
        "SELECT id FROM app WHERE stable_key = ?1",
        [&app.identity_key],
        |row| row.get(0),
    )?;
    let path = normalized_path(app);
    tx.execute(
        "INSERT INTO app_executable(app_id, normalized_path, first_seen_at_ms, last_seen_at_ms) VALUES (?1, ?2, ?3, ?3) ON CONFLICT(app_id, normalized_path) DO UPDATE SET last_seen_at_ms = excluded.last_seen_at_ms",
        params![app_id, path, now],
    )?;
    Ok(app_id)
}

fn upsert_resource_app_tx(
    tx: &rusqlite::Transaction<'_>,
    app: &crate::models::AppResourceSample,
    now: i64,
) -> rusqlite::Result<i64> {
    let foreground = ForegroundApp {
        identity_key: app.app_key.clone(),
        process_name: app.process_name.clone(),
        exe_path: app.exe_path.clone(),
        display_name: app.process_name.clone(),
    };
    upsert_app_tx(tx, &foreground, now, false)?;
    tx.query_row(
        "SELECT e.id FROM app_executable e JOIN app a ON a.id = e.app_id WHERE a.stable_key = ?1 AND e.normalized_path = ?2",
        params![app.app_key, normalized_path(&foreground)],
        |row| row.get(0),
    )
}

fn normalized_path(app: &ForegroundApp) -> String {
    app.exe_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(|path| {
            format!(
                "path:{}",
                path.trim()
                    .trim_start_matches(r"\\?\")
                    .replace('/', "\\")
                    .to_lowercase()
            )
        })
        .unwrap_or_else(|| format!("legacy:{}", app.identity_key))
}

const RETRY_BASE_DELAY_MS: u64 = 25;
const RETRY_MAX_DELAY_MS: u64 = 5_000;
const MAX_RETRY_LIMIT: u32 = 32;
const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

fn retry_backoff(attempt: u32) -> Duration {
    if attempt == 0 {
        return Duration::ZERO;
    }
    let shift = attempt.saturating_sub(1).min(8);
    let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
    let delay_ms = RETRY_BASE_DELAY_MS
        .saturating_mul(multiplier)
        .min(RETRY_MAX_DELAY_MS);
    Duration::from_millis(delay_ms)
}

fn remaining_backoff_ms(next_attempt_at: Instant) -> u64 {
    u64::try_from(
        next_attempt_at
            .saturating_duration_since(Instant::now())
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

pub(crate) fn configure_connection_for_deadline(
    conn: &Connection,
    deadline: Instant,
) -> rusqlite::Result<()> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(shutdown_timeout_error());
    }
    conn.busy_timeout(remaining.min(SQLITE_BUSY_TIMEOUT))
}

fn shutdown_timeout_error() -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(io::Error::new(
        io::ErrorKind::TimedOut,
        "frame writer shutdown drain deadline expired",
    )))
}

fn ensure_device_tx(
    tx: &rusqlite::Transaction<'_>,
    stable_key: &str,
    category: &str,
    now: i64,
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO hardware_device(stable_key, category, first_seen_at_ms, last_seen_at_ms) VALUES (?1, ?2, ?3, ?3) ON CONFLICT(stable_key) DO UPDATE SET last_seen_at_ms = excluded.last_seen_at_ms",
        params![stable_key, category, now],
    )?;
    tx.query_row(
        "SELECT id FROM hardware_device WHERE stable_key = ?1",
        [stable_key],
        |row| row.get(0),
    )
}

fn current_boot_session_id(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_boot_session_id'",
        [],
        |row| row.get(0),
    )
}

pub fn tracked_app_keys(conn: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut statement = conn.prepare("SELECT stable_key FROM app")?;
    let rows = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn set_app_hidden(conn: &Connection, executable_id: i64, hidden: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE app SET is_hidden = ?1 WHERE id = (SELECT app_id FROM app_executable WHERE id = ?2)",
        params![hidden as i64, executable_id],
    )?;
    Ok(())
}

fn setting_u64(conn: &Connection, key: &str, fallback: u64) -> rusqlite::Result<u64> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(fallback))
}

pub fn collection_settings(conn: &Connection) -> rusqlite::Result<CollectionSettings> {
    Ok(CollectionSettings {
        foreground_poll_interval_ms: setting_u64(conn, "foreground_poll_interval_ms", 1_000)?,
        system_sample_interval_ms: setting_u64(conn, "system_sample_interval_ms", 5_000)?,
        idle_threshold_seconds: setting_u64(conn, "idle_threshold_seconds", 300)?,
        system_sample_retention_days: setting_u64(conn, "system_sample_retention_days", 7)?,
    })
}

pub fn save_collection_settings(
    conn: &Connection,
    settings: &CollectionSettings,
    updated_at_ms: i64,
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (key, value) in [
        (
            "foreground_poll_interval_ms",
            settings.foreground_poll_interval_ms,
        ),
        (
            "system_sample_interval_ms",
            settings.system_sample_interval_ms,
        ),
        ("idle_threshold_seconds", settings.idle_threshold_seconds),
        (
            "system_sample_retention_days",
            settings.system_sample_retention_days,
        ),
    ] {
        tx.execute(
            "INSERT INTO settings(key, value, updated_at_ms) VALUES (?1, ?2, ?3) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
            params![key, value.to_string(), updated_at_ms],
        )?;
    }
    tx.commit()
}

pub fn start_with_windows(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(setting_u64(conn, "start_with_windows", 1)? != 0)
}

pub fn save_start_with_windows(
    conn: &Connection,
    enabled: bool,
    updated_at_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value, updated_at_ms) VALUES ('start_with_windows', ?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
        params![if enabled { "1" } else { "0" }, updated_at_ms],
    )?;
    Ok(())
}

pub fn prune_system_samples(conn: &Connection, cutoff_ms: i64) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM sample_frame WHERE ts < ?1", [cutoff_ms])
}

pub fn clear_collected_data(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM foreground_interval", [])?;
    tx.execute("DELETE FROM sample_frame", [])?;
    tx.execute("DELETE FROM process_instance", [])?;
    tx.execute("DELETE FROM app_executable", [])?;
    tx.execute("DELETE FROM app", [])?;
    tx.commit()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::retry_backoff;
    use std::time::Duration;

    #[test]
    fn retry_backoff_is_bounded_and_exponential() {
        assert_eq!(retry_backoff(1), Duration::from_millis(25));
        assert_eq!(retry_backoff(2), Duration::from_millis(50));
        assert_eq!(retry_backoff(8), Duration::from_millis(3_200));
        assert_eq!(retry_backoff(u32::MAX), Duration::from_secs(5));
    }
}
