use crate::models::{CollectionSettings, ForegroundApp, ResourceSnapshot, SystemSample};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{HashSet, VecDeque},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriterHealth {
    pub queue_depth: usize,
    pub writer_delay_ms: i64,
    pub drop_count: u64,
    pub retry_count: u64,
    pub front_retry_attempts: Option<u32>,
    pub last_commit_duration_ms: u64,
    pub last_committed_timestamp_ms: Option<i64>,
    pub last_error: Option<String>,
}

struct QueuedFrame {
    snapshot: ResourceSnapshot,
    attempts: u32,
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
            max_queue_depth: max_queue_depth.max(1),
            retry_limit: retry_limit.max(1),
            health: WriterHealth::default(),
        }
    }

    pub fn enqueue(&mut self, snapshot: ResourceSnapshot) -> bool {
        if self.queue.len() >= self.max_queue_depth {
            self.health.drop_count += 1;
            self.health.queue_depth = self.queue.len();
            return false;
        }
        self.queue.push_back(QueuedFrame {
            snapshot,
            attempts: 0,
        });
        self.health.queue_depth = self.queue.len();
        true
    }

    pub fn write_next(&mut self, conn: &Connection) -> rusqlite::Result<bool> {
        let Some(item) = self.queue.front() else {
            self.health.queue_depth = 0;
            return Ok(false);
        };
        let timestamp_ms = item.snapshot.system.timestamp_ms;
        let started = Instant::now();
        match insert_resource_snapshot(conn, &item.snapshot) {
            Ok(()) => {
                self.queue.pop_front();
                self.health.queue_depth = self.queue.len();
                self.health.writer_delay_ms = now_ms().saturating_sub(timestamp_ms).max(0);
                self.health.last_commit_duration_ms = started.elapsed().as_millis() as u64;
                self.health.last_committed_timestamp_ms = Some(timestamp_ms);
                self.health.front_retry_attempts = None;
                self.health.last_error = None;
                Ok(true)
            }
            Err(error) => {
                if let Some(item) = self.queue.front_mut() {
                    item.attempts = item.attempts.saturating_add(1).min(self.retry_limit);
                }
                self.health.retry_count += 1;
                self.health.queue_depth = self.queue.len();
                self.health.front_retry_attempts = self.queue.front().map(|queued| queued.attempts);
                self.health.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    pub fn flush_all(&mut self, conn: &Connection) -> rusqlite::Result<usize> {
        let mut committed = 0;
        while self.queue.front().is_some() {
            if self.write_next(conn)? {
                committed += 1;
            }
        }
        Ok(committed)
    }

    pub fn discard_for_explicit_clear(&mut self) {
        self.health.drop_count += self.queue.len() as u64;
        self.queue.clear();
        self.health.queue_depth = 0;
    }

    pub fn health(&self) -> WriterHealth {
        let mut health = self.health.clone();
        health.queue_depth = self.queue.len();
        health.front_retry_attempts = self.queue.front().map(|queued| queued.attempts);
        health
    }

    pub fn queue_depth(&self) -> usize {
        self.queue.len()
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

pub fn upsert_app(conn: &Connection, app: &ForegroundApp, now: i64) -> rusqlite::Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let app_id = upsert_app_tx(&tx, app, now)?;
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
) -> rusqlite::Result<i64> {
    tx.execute(
        "INSERT INTO app(stable_key, display_name, first_seen_at_ms, last_seen_at_ms) VALUES (?1, ?2, ?3, ?3) ON CONFLICT(stable_key) DO UPDATE SET display_name = excluded.display_name, last_seen_at_ms = excluded.last_seen_at_ms",
        params![app.identity_key, app.display_name, now],
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
    upsert_app_tx(tx, &foreground, now)?;
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
