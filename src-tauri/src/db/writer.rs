use crate::models::{CollectionSettings, ForegroundApp, ResourceSnapshot, SystemSample};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;

pub fn recover_open_interval(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE foreground_interval SET end_time_ms = last_seen_time_ms, end_reason = 'recovery' WHERE end_time_ms IS NULL",
        [],
    )
}

pub fn upsert_app(conn: &Connection, app: &ForegroundApp, now_ms: i64) -> rusqlite::Result<i64> {
    conn.execute(
        r#"INSERT INTO app_identity(identity_key, process_name, exe_path, display_name, first_seen_at_ms, last_seen_at_ms)
           VALUES (?1, ?2, ?3, ?4, ?5, ?5)
           ON CONFLICT(identity_key) DO UPDATE SET
             process_name = excluded.process_name,
             exe_path = COALESCE(excluded.exe_path, app_identity.exe_path),
             display_name = excluded.display_name,
             last_seen_at_ms = excluded.last_seen_at_ms"#,
        params![app.identity_key, app.process_name, app.exe_path, app.display_name, now_ms],
    )?;
    conn.query_row(
        "SELECT id FROM app_identity WHERE identity_key = ?1",
        [&app.identity_key],
        |r| r.get(0),
    )
}

pub fn begin_interval(
    conn: &Connection,
    app_id: i64,
    at_ms: i64,
    state: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO foreground_interval(app_id, start_time_ms, last_seen_time_ms, activity_state) VALUES (?1, ?2, ?2, ?3)",
        params![app_id, at_ms, state],
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
        r#"UPDATE foreground_interval
           SET last_seen_time_ms = MAX(last_seen_time_ms, ?1),
               end_time_ms = MAX(start_time_ms, ?1), end_reason = ?2
           WHERE id = ?3 AND end_time_ms IS NULL"#,
        params![at_ms, reason, interval_id],
    )?;
    Ok(())
}

pub fn insert_system_sample(conn: &Connection, sample: &SystemSample) -> rusqlite::Result<()> {
    conn.execute(
        r#"INSERT INTO system_sample(timestamp_ms, sample_duration_ms, cpu_percent, memory_percent,
             memory_used_bytes, memory_total_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        params![sample.timestamp_ms, sample.sample_duration_ms, sample.cpu_percent, sample.memory_percent,
            sample.memory_used_bytes, sample.memory_total_bytes, sample.disk_read_bytes_per_sec, sample.disk_write_bytes_per_sec],
    )?;
    Ok(())
}

pub fn insert_resource_snapshot(
    conn: &Connection,
    snapshot: &ResourceSnapshot,
) -> rusqlite::Result<()> {
    insert_system_sample(conn, &snapshot.system)?;
    let system_sample_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO app_resource_snapshot(system_sample_id) VALUES (?1)",
        [system_sample_id],
    )?;
    let mut statement = conn.prepare_cached(
        r#"INSERT INTO app_resource_sample(
             system_sample_id, app_key, process_name, exe_path, process_count, cpu_percent,
             memory_used_bytes, io_read_bytes_per_sec, io_write_bytes_per_sec)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
    )?;
    for app in &snapshot.apps {
        statement.execute(params![
            system_sample_id,
            app.app_key,
            app.process_name,
            app.exe_path,
            app.process_count,
            app.cpu_percent,
            app.memory_used_bytes,
            app.io_read_bytes_per_sec,
            app.io_write_bytes_per_sec,
        ])?;
    }
    Ok(())
}

pub fn tracked_app_keys(conn: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut statement = conn.prepare("SELECT identity_key FROM app_identity")?;
    let rows = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn set_app_hidden(conn: &Connection, app_id: i64, hidden: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE app_identity SET is_hidden = ?1 WHERE id = ?2",
        params![hidden as i64, app_id],
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
            r#"INSERT INTO settings(key, value, updated_at_ms) VALUES (?1, ?2, ?3)
               ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms"#,
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
        r#"INSERT INTO settings(key, value, updated_at_ms) VALUES ('start_with_windows', ?1, ?2)
           ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms"#,
        params![if enabled { "1" } else { "0" }, updated_at_ms],
    )?;
    Ok(())
}

pub fn prune_system_samples(conn: &Connection, cutoff_ms: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "DELETE FROM system_sample WHERE timestamp_ms < ?1",
        [cutoff_ms],
    )
}

pub fn clear_collected_data(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM foreground_interval", [])?;
    tx.execute("DELETE FROM system_sample", [])?;
    tx.execute("DELETE FROM app_identity", [])?;
    tx.commit()
}
