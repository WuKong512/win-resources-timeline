use rusqlite::{params, Connection};

const SCHEMA_V1: &str = r#"
CREATE TABLE app_identity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    identity_key TEXT NOT NULL UNIQUE,
    process_name TEXT NOT NULL,
    exe_path TEXT,
    display_name TEXT NOT NULL,
    publisher TEXT,
    is_hidden INTEGER NOT NULL DEFAULT 0 CHECK (is_hidden IN (0, 1)),
    first_seen_at_ms INTEGER NOT NULL,
    last_seen_at_ms INTEGER NOT NULL
);

CREATE TABLE foreground_interval (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    app_id INTEGER NOT NULL,
    start_time_ms INTEGER NOT NULL,
    end_time_ms INTEGER,
    last_seen_time_ms INTEGER NOT NULL,
    activity_state TEXT NOT NULL CHECK (activity_state IN ('active', 'idle')),
    end_reason TEXT,
    FOREIGN KEY(app_id) REFERENCES app_identity(id),
    CHECK(end_time_ms IS NULL OR end_time_ms >= start_time_ms),
    CHECK(last_seen_time_ms >= start_time_ms)
);

CREATE UNIQUE INDEX idx_foreground_single_open
ON foreground_interval((end_time_ms IS NULL)) WHERE end_time_ms IS NULL;
CREATE INDEX idx_foreground_interval_range
ON foreground_interval(start_time_ms, end_time_ms, last_seen_time_ms);
CREATE INDEX idx_foreground_interval_app
ON foreground_interval(app_id, start_time_ms);

CREATE TABLE system_sample (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp_ms INTEGER NOT NULL,
    sample_duration_ms INTEGER NOT NULL,
    cpu_percent REAL,
    memory_percent REAL,
    memory_used_bytes INTEGER,
    memory_total_bytes INTEGER,
    disk_read_bytes_per_sec INTEGER,
    disk_write_bytes_per_sec INTEGER
);
CREATE INDEX idx_system_sample_timestamp ON system_sample(timestamp_ms);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
INSERT INTO settings(key, value, updated_at_ms) VALUES
    ('idle_threshold_seconds', '300', 0),
    ('system_sample_retention_days', '7', 0);
"#;

pub fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;

    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        let has_legacy = table_exists(conn, "app_identity")?;
        let tx = conn.transaction()?;
        if has_legacy {
            tx.execute_batch(
                r#"
                ALTER TABLE app_identity RENAME TO legacy_app_identity;
                ALTER TABLE foreground_interval RENAME TO legacy_foreground_interval;
                ALTER TABLE system_sample RENAME TO legacy_system_sample;
                DROP TABLE IF EXISTS app_display_rule;
                "#,
            )?;
        }
        tx.execute_batch(SCHEMA_V1)?;
        if has_legacy {
            tx.execute_batch(
                r#"
                INSERT OR IGNORE INTO app_identity(
                    identity_key, process_name, exe_path, display_name, publisher,
                    is_hidden, first_seen_at_ms, last_seen_at_ms
                )
                SELECT
                    CASE
                      WHEN exe_path IS NOT NULL AND trim(exe_path) <> ''
                        THEN 'path:' || lower(replace(trim(exe_path), '/', '\'))
                      WHEN lower(app_name) LIKE 'pid-%' THEN 'name:unresolved'
                      ELSE 'name:' || lower(app_name)
                    END,
                    CASE WHEN lower(app_name) LIKE 'pid-%' THEN 'unresolved' ELSE app_name END,
                    exe_path,
                    COALESCE(display_name, app_name),
                    publisher,
                    COALESCE(is_hidden, 0),
                    COALESCE(first_seen_at, 0) * 1000,
                    COALESCE(last_seen_at, first_seen_at, 0) * 1000
                FROM legacy_app_identity;

                INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason)
                SELECT target.id, old_interval.start_time * 1000, old_interval.end_time * 1000,
                       old_interval.end_time * 1000, 'active', 'recovery'
                FROM legacy_foreground_interval old_interval
                JOIN legacy_app_identity old_app ON old_app.id = old_interval.app_id
                JOIN app_identity target ON target.identity_key = CASE
                    WHEN old_app.exe_path IS NOT NULL AND trim(old_app.exe_path) <> ''
                      THEN 'path:' || lower(replace(trim(old_app.exe_path), '/', '\'))
                    WHEN lower(old_app.app_name) LIKE 'pid-%' THEN 'name:unresolved'
                    ELSE 'name:' || lower(old_app.app_name)
                END
                WHERE old_interval.end_time >= old_interval.start_time;

                INSERT INTO system_sample(timestamp_ms, sample_duration_ms, cpu_percent, memory_percent,
                    memory_used_bytes, memory_total_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec)
                SELECT timestamp * 1000, 5000, cpu_percent, memory_percent, memory_used_bytes,
                    memory_total_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec
                FROM legacy_system_sample;

                DROP TABLE legacy_foreground_interval;
                DROP TABLE legacy_system_sample;
                DROP TABLE legacy_app_identity;
                "#,
            )?;
        }
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    }
    if version < 2 {
        let tx = conn.transaction()?;
        tx.execute_batch(
            r#"
            INSERT OR IGNORE INTO settings(key, value, updated_at_ms) VALUES
                ('foreground_poll_interval_ms', '1000', 0),
                ('system_sample_interval_ms', '5000', 0),
                ('idle_threshold_seconds', '300', 0),
                ('system_sample_retention_days', '7', 0);
            "#,
        )?;
        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
    }
    if version < 3 {
        let tx = conn.transaction()?;
        tx.execute_batch(
            r#"
            CREATE TABLE app_resource_sample (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                system_sample_id INTEGER NOT NULL,
                app_key TEXT NOT NULL,
                process_name TEXT NOT NULL,
                exe_path TEXT,
                process_count INTEGER NOT NULL,
                cpu_percent REAL NOT NULL,
                memory_used_bytes INTEGER NOT NULL,
                io_read_bytes_per_sec INTEGER NOT NULL,
                io_write_bytes_per_sec INTEGER NOT NULL,
                FOREIGN KEY(system_sample_id) REFERENCES system_sample(id) ON DELETE CASCADE,
                UNIQUE(system_sample_id, app_key)
            );
            CREATE INDEX idx_app_resource_sample_system
                ON app_resource_sample(system_sample_id);
            "#,
        )?;
        tx.pragma_update(None, "user_version", 3)?;
        tx.commit()?;
    }
    if version < 4 {
        let tx = conn.transaction()?;
        tx.execute_batch(
            r#"
            CREATE TABLE app_resource_snapshot (
                system_sample_id INTEGER PRIMARY KEY,
                FOREIGN KEY(system_sample_id) REFERENCES system_sample(id) ON DELETE CASCADE
            );
            INSERT OR IGNORE INTO app_resource_snapshot(system_sample_id)
                SELECT DISTINCT system_sample_id FROM app_resource_sample;
            "#,
        )?;
        tx.pragma_update(None, "user_version", 4)?;
        tx.commit()?;
    }
    if version < 5 {
        let foreground_poll_interval_ms = conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'foreground_poll_interval_ms'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(1_000)
            .clamp(1_000, 10_000);
        let tx = conn.transaction()?;
        if foreground_poll_interval_ms > 2_500 {
            // Versions through v4 used a fixed 2.5 s gap threshold even when foreground
            // polling was configured to 5 or 10 s. Repair only the characteristic 1 s
            // clock-gap fragments followed by another regular polling observation.
            tx.execute(
                r#"UPDATE foreground_interval AS current
                   SET end_time_ms = (
                         SELECT MIN(next.start_time_ms) FROM foreground_interval AS next
                         WHERE next.start_time_ms > current.start_time_ms
                       ),
                       last_seen_time_ms = (
                         SELECT MIN(next.start_time_ms) FROM foreground_interval AS next
                         WHERE next.start_time_ms > current.start_time_ms
                       ),
                       end_reason = 'sampling_interval_repair'
                   WHERE current.end_reason = 'clock_gap'
                     AND current.end_time_ms - current.start_time_ms BETWEEN 0 AND 1500
                     AND (
                       SELECT MIN(next.start_time_ms) FROM foreground_interval AS next
                       WHERE next.start_time_ms > current.start_time_ms
                     ) - current.start_time_ms BETWEEN ?1 AND ?2"#,
                params![
                    foreground_poll_interval_ms / 2,
                    foreground_poll_interval_ms * 5 / 2
                ],
            )?;
        }
        tx.pragma_update(None, "user_version", 5)?;
        tx.commit()?;
    }
    if version < 6 {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO settings(key, value, updated_at_ms) VALUES ('start_with_windows', '1', 0)",
            [],
        )?;
        tx.pragma_update(None, "user_version", 6)?;
        tx.commit()?;
    }
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get(0),
    )
}
