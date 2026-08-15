pub mod query;
pub mod rollup;
pub mod schema;
pub mod writer;

use rusqlite::{Connection, OpenFlags};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Database {
    path: PathBuf,
    writer: Mutex<Connection>,
}

impl Database {
    pub fn open(path: PathBuf) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| rusqlite::Error::InvalidPath(path.clone()))?;
        }
        let mut conn = Connection::open(&path)?;
        schema::migrate_with_path(&mut conn, Some(&path))?;
        writer::recover_open_interval(&conn)?;
        writer::start_runtime_session(&conn, now_ms())?;
        Ok(Self {
            path,
            writer: Mutex::new(conn),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn with_writer<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = self.writer.lock().expect("database writer lock poisoned");
        f(&conn)
    }

    pub fn read<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        f(&conn)
    }

    pub fn size_bytes(&self) -> u64 {
        schema::database_size_bytes(&self.path)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{query, writer},
        models::{
            ActivityState, AppResourceSample, CollectionSettings, ForegroundApp, ResourceSnapshot,
            SystemSample,
        },
    };
    use rusqlite::params;
    use std::{cell::Cell, rc::Rc};

    fn test_path(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!(
            "resource-timeline-{name}-{}-{nonce}.sqlite3",
            std::process::id(),
        ))
    }

    fn cleanup_test_files(path: &Path) {
        for suffix in ["", "-wal", "-shm"] {
            let candidate = PathBuf::from(format!("{}{}", path.display(), suffix));
            let _ = std::fs::remove_file(candidate);
        }
        let Some(parent) = path.parent() else {
            return;
        };
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            return;
        };
        let prefix = format!("{name}.v7-backup-");
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|value| value.starts_with(&prefix))
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    fn pragma_text(conn: &Connection, pragma: &str) -> rusqlite::Result<String> {
        conn.query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
    }

    fn foreign_key_error_count(conn: &Connection) -> rusqlite::Result<i64> {
        let mut statement = conn.prepare("PRAGMA foreign_key_check")?;
        let mut rows = statement.query([])?;
        let mut count = 0;
        while rows.next()?.is_some() {
            count += 1;
        }
        Ok(count)
    }

    fn create_v6_fixture(path: &Path, populated: bool) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
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
            CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at_ms INTEGER NOT NULL);
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
             CREATE TABLE app_resource_snapshot (
                 system_sample_id INTEGER PRIMARY KEY,
                 FOREIGN KEY(system_sample_id) REFERENCES system_sample(id) ON DELETE CASCADE
             );
             CREATE UNIQUE INDEX idx_foreground_single_open ON foreground_interval((end_time_ms IS NULL)) WHERE end_time_ms IS NULL;
             CREATE INDEX idx_foreground_interval_range ON foreground_interval(start_time_ms, end_time_ms, last_seen_time_ms);
             CREATE INDEX idx_foreground_interval_app ON foreground_interval(app_id, start_time_ms);
            CREATE INDEX idx_system_sample_timestamp ON system_sample(timestamp_ms);
            CREATE INDEX idx_app_resource_sample_system ON app_resource_sample(system_sample_id);
            INSERT INTO settings(key, value, updated_at_ms) VALUES
                ('foreground_poll_interval_ms', '1000', 0),
                ('system_sample_interval_ms', '5000', 0),
                ('idle_threshold_seconds', '300', 0),
                ('system_sample_retention_days', '7', 0),
                ('start_with_windows', '1', 0);
            "#,
        )
        .unwrap();

        if populated {
            let app_id = conn
                .execute(
                    "INSERT INTO app_identity(identity_key, process_name, exe_path, display_name, first_seen_at_ms, last_seen_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        "path:c:\\editor.exe",
                        "Editor.exe",
                        "C:\\Editor.exe",
                        "Editor",
                        1_000_i64,
                        5_000_i64
                    ],
                )
                .map(|_| conn.last_insert_rowid())
                .unwrap();
            conn.execute(
                "INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason) VALUES (?1, ?2, ?3, ?3, ?4, ?5)",
                params![app_id, 1_000_i64, 2_500_i64, "active", "shutdown"],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason) VALUES (?1, ?2, ?3, ?3, ?4, ?5)",
                params![app_id, 3_000_i64, 5_000_i64, "idle", "shutdown"],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO system_sample(timestamp_ms, sample_duration_ms, cpu_percent, memory_percent, memory_used_bytes, memory_total_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![2_000_i64, 5_000_i64, 30.0_f64, 50.0_f64, 100_i64, 200_i64, 10_i64, 20_i64],
            )
            .unwrap();
            let first_sample_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO system_sample(timestamp_ms, sample_duration_ms, cpu_percent, memory_percent, memory_used_bytes, memory_total_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![7_000_i64, 5_000_i64, Option::<f64>::None, Option::<f64>::None, Option::<i64>::None, Option::<i64>::None, 3_i64, 4_i64],
            )
            .unwrap();
            let second_sample_id = conn.last_insert_rowid();
            for sample_id in [first_sample_id, second_sample_id] {
                conn.execute(
                    "INSERT INTO app_resource_snapshot(system_sample_id) VALUES (?1)",
                    [sample_id],
                )
                .unwrap();
            }
            for (sample_id, cpu, memory, read, write) in [
                (first_sample_id, 10.0_f64, 100_i64, 5_i64, 6_i64),
                (second_sample_id, 20.0_f64, 200_i64, 7_i64, 8_i64),
            ] {
                conn.execute(
                    "INSERT INTO app_resource_sample(system_sample_id, app_key, process_name, exe_path, process_count, cpu_percent, memory_used_bytes, io_read_bytes_per_sec, io_write_bytes_per_sec) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8)",
                    params![sample_id, "path:c:\\editor.exe", "Editor.exe", "C:\\Editor.exe", cpu, memory, read, write],
                )
                .unwrap();
            }
        }
        conn.pragma_update(None, "user_version", 6).unwrap();
    }

    #[test]
    fn migration_upsert_recovery_and_range_clipping() {
        let path = test_path("database");
        let _ = std::fs::remove_file(&path);
        {
            let db = Database::open(path.clone()).unwrap();
            let app = ForegroundApp {
                identity_key: "path:c:\\test.exe".into(),
                process_name: "test.exe".into(),
                exe_path: Some("C:\\test.exe".into()),
                display_name: "test".into(),
            };
            let app_id = db
                .with_writer(|c| writer::upsert_app(c, &app, 1_000))
                .unwrap();
            assert_eq!(
                app_id,
                db.with_writer(|c| writer::upsert_app(c, &app, 2_000))
                    .unwrap()
            );
            db.with_writer(|c| {
                writer::begin_interval(c, app_id, 1_000, ActivityState::Active.as_str())
            })
            .unwrap();
            db.with_writer(|c| writer::checkpoint_interval(c, 1, 5_000))
                .unwrap();
        }
        let db = Database::open(path.clone()).unwrap();
        let intervals = db
            .read(|c| query::foreground_intervals(c, 2_000, 4_000, true, true))
            .unwrap();
        assert_eq!(intervals.len(), 1);
        assert_eq!(
            (intervals[0].start_time_ms, intervals[0].end_time_ms),
            (2_000, 4_000)
        );
        let available_dates = db.read(query::timeline_available_dates).unwrap();
        assert_eq!(available_dates.len(), 1);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn empty_v6_database_migrates_to_v7() {
        let path = test_path("empty-v6");
        cleanup_test_files(&path);
        create_v6_fixture(&path, false);

        let db = Database::open(path.clone()).unwrap();
        db.read(|conn| {
            let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
            assert_eq!(version, 7);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM app", [], |row| row.get::<_, i64>(0))?, 0);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row.get::<_, i64>(0))?, 0);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM process_sample", [], |row| row.get::<_, i64>(0))?, 0);
            assert_eq!(foreign_key_error_count(conn)?, 0);
            assert_eq!(pragma_text(conn, "quick_check")?, "ok");
            assert_eq!(pragma_text(conn, "integrity_check")?, "ok");
            let (runs, completed): (i64, i64) = conn.query_row(
                "SELECT COUNT(DISTINCT run_id), SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) FROM migration_journal",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            assert_eq!((runs, completed), (1, 9));
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn populated_v6_migration_preserves_rows_ranges_totals_and_nulls() {
        let path = test_path("populated-v6");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);

        let db = Database::open(path.clone()).unwrap();
        db.read(|conn| {
            assert_eq!(conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?, 7);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM app", [], |row| row.get::<_, i64>(0))?, 1);
            assert_eq!(
                conn.query_row(
                    "SELECT process_name, display_name FROM app",
                    [],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )?,
                ("Editor.exe".to_string(), "Editor".to_string())
            );
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM foreground_interval", [], |row| row.get::<_, i64>(0))?, 2);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row.get::<_, i64>(0))?, 2);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM process_sample", [], |row| row.get::<_, i64>(0))?, 2);
            assert_eq!(conn.query_row("SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = 'idx_foreground_single_open'", [], |row| row.get::<_, String>(0))?, "foreground_interval");
            assert_eq!(conn.query_row("SELECT tbl_name FROM sqlite_master WHERE type = 'index' AND name = 'idx_foreground_interval_range'", [], |row| row.get::<_, String>(0))?, "foreground_interval");
            assert_eq!(conn.query_row("SELECT MIN(start_time_ms), MAX(COALESCE(end_time_ms, last_seen_time_ms)) FROM foreground_interval", [], |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)))?, (Some(1_000), Some(5_000)));
            assert_eq!(conn.query_row("SELECT MIN(ts), MAX(ts) FROM sample_frame", [], |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)))?, (Some(2_000), Some(7_000)));
            assert_eq!(conn.query_row("SELECT SUM(used_bytes) FROM memory_sample", [], |row| row.get::<_, Option<i64>>(0))?, Some(100));
            assert_eq!(conn.query_row("SELECT SUM(read_bps), SUM(write_bps) FROM disk_sample", [], |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)))?, (Some(13), Some(24)));
            assert_eq!(conn.query_row("SELECT SUM(cpu_pct), SUM(working_set_bytes), SUM(read_bps), SUM(write_bps) FROM process_sample", [], |row| Ok((row.get::<_, Option<f64>>(0)?, row.get::<_, Option<i64>>(1)?, row.get::<_, Option<i64>>(2)?, row.get::<_, Option<i64>>(3)?)))?, (Some(30.0), Some(300), Some(12), Some(14)));
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM cpu_sample", [], |row| row.get::<_, i64>(0))?, 1);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM memory_sample", [], |row| row.get::<_, i64>(0))?, 1);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM sample_frame WHERE source = 'legacy-v6' AND writer_delay_ms IS NULL", [], |row| row.get::<_, i64>(0))?, 2);
            assert_eq!(conn.query_row("SELECT normalized_path FROM app_executable", [], |row| row.get::<_, String>(0))?, "path:c:\\editor.exe");
            assert_eq!(foreign_key_error_count(conn)?, 0);
            assert_eq!(pragma_text(conn, "quick_check")?, "ok");
            assert_eq!(pragma_text(conn, "integrity_check")?, "ok");
            let settings = writer::collection_settings(conn)?;
            assert_eq!(settings.idle_threshold_seconds, 300);
            assert_eq!(settings.foreground_poll_interval_ms, 1_000);
            assert_eq!(settings.system_sample_interval_ms, 5_000);
            Ok(())
        })
        .unwrap();

        let backups: Vec<PathBuf> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .filter(|candidate| {
                candidate
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| {
                        value.starts_with(&format!(
                            "{}.v7-backup-",
                            path.file_name().unwrap().to_string_lossy()
                        ))
                    })
            })
            .collect();
        assert_eq!(backups.len(), 1);
        let backup = Connection::open(&backups[0]).unwrap();
        assert_eq!(
            backup
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            6
        );
        assert_eq!(
            backup
                .query_row("SELECT COUNT(*) FROM app_identity", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn resource_sampling_preserves_display_name_and_process_name() {
        let path = test_path("resource-display-process-name");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let foreground = ForegroundApp {
            identity_key: "path:c:\\friendly-editor.exe".into(),
            process_name: "editor.exe".into(),
            exe_path: Some("C:\\friendly-editor.exe".into()),
            display_name: "Friendly Editor".into(),
        };
        db.with_writer(|conn| writer::upsert_app(conn, &foreground, 1_000))
            .unwrap();
        db.with_writer(|conn| {
            writer::insert_resource_snapshot(
                conn,
                &ResourceSnapshot {
                    system: SystemSample {
                        timestamp_ms: 2_000,
                        sample_duration_ms: 1_000,
                        cpu_percent: Some(10.0),
                        memory_percent: None,
                        memory_used_bytes: None,
                        memory_total_bytes: None,
                        disk_read_bytes_per_sec: None,
                        disk_write_bytes_per_sec: None,
                        has_app_snapshot: true,
                    },
                    apps: vec![AppResourceSample {
                        app_key: foreground.identity_key.clone(),
                        process_name: foreground.process_name.clone(),
                        exe_path: foreground.exe_path.clone(),
                        process_count: 1,
                        cpu_percent: 5.0,
                        memory_used_bytes: 10,
                        io_read_bytes_per_sec: 1,
                        io_write_bytes_per_sec: 2,
                    }],
                },
            )
        })
        .unwrap();

        let listed = db.read(query::list_apps).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].process_name, "editor.exe");
        assert_eq!(listed[0].display_name, "Friendly Editor");
        let resources = db.read(query::resource_apps).unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].process_name, "editor.exe");
        assert_eq!(resources[0].display_name, "Friendly Editor");

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn opening_v7_again_does_not_backfill_legacy_rows_twice() {
        let path = test_path("repeat-v7-open");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);

        {
            let db = Database::open(path.clone()).unwrap();
            assert_eq!(
                db.read(|conn| conn.query_row(
                    "SELECT COUNT(*) FROM sample_frame WHERE source = 'legacy-v6'",
                    [],
                    |row| row.get::<_, i64>(0)
                ))
                .unwrap(),
                2
            );
        }
        {
            let db = Database::open(path.clone()).unwrap();
            db.read(|conn| {
                assert_eq!(
                    conn.query_row("SELECT COUNT(*) FROM app", [], |row| row.get::<_, i64>(0))?,
                    1
                );
                assert_eq!(
                    conn.query_row(
                        "SELECT COUNT(*) FROM sample_frame WHERE source = 'legacy-v6'",
                        [],
                        |row| row.get::<_, i64>(0)
                    )?,
                    2
                );
                assert_eq!(
                    conn.query_row(
                        "SELECT COUNT(*) FROM process_sample WHERE source = 'legacy-v6'",
                        [],
                        |row| row.get::<_, i64>(0)
                    )?,
                    2
                );
                assert_eq!(
                    conn.query_row("SELECT COUNT(*) FROM migration_journal", [], |row| row
                        .get::<_, i64>(0))?,
                    9
                );
                Ok(())
            })
            .unwrap();
        }
        cleanup_test_files(&path);
    }

    #[test]
    fn migration_journal_records_only_reached_failure_stage_and_recovers() {
        let path = test_path("migration-journal-recovery");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);

        let mut conn = Connection::open(&path).unwrap();
        assert!(
            schema::migrate_v6_to_v7_fail_at(&mut conn, Some(&path), "usage_backfill").is_err()
        );
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            6
        );
        let run_id: String = conn
            .query_row(
                "SELECT run_id FROM migration_journal ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let statuses: Vec<(String, String)> = conn
            .prepare("SELECT stage, status FROM migration_journal WHERE run_id = ?1 ORDER BY id")
            .unwrap()
            .query_map([&run_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            statuses,
            vec![
                ("preflight".into(), "completed".into()),
                ("backup".into(), "completed".into()),
                ("create".into(), "completed".into()),
                ("identity_backfill".into(), "completed".into()),
                ("usage_backfill".into(), "failed".into()),
                ("resource_backfill".into(), "pending".into()),
                ("verify".into(), "pending".into()),
                ("commit".into(), "pending".into()),
                ("postflight".into(), "pending".into()),
            ]
        );
        drop(conn);

        let db = Database::open(path.clone()).unwrap();
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM app", [], |row| row.get::<_, i64>(0))?,
                1
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row
                    .get::<_, i64>(0))?,
                2
            );
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn failed_postflight_blocks_open_until_integrity_is_repaired() {
        let path = test_path("postflight-repair");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);
        let mut conn = Connection::open(&path).unwrap();
        assert!(schema::migrate_v6_to_v7_fail_at(&mut conn, Some(&path), "postflight").is_err());
        assert_eq!(
            conn.query_row(
                "SELECT status FROM migration_journal WHERE stage = 'postflight' ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "failed"
        );
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute(
            "INSERT INTO process_sample(frame_id, process_instance_id, process_count) VALUES (999999, 999999, 1)",
            [],
        )
        .unwrap();
        drop(conn);

        assert!(Database::open(path.clone()).is_err());

        let repair = Connection::open(&path).unwrap();
        repair.pragma_update(None, "foreign_keys", "OFF").unwrap();
        repair
            .execute("DELETE FROM process_sample WHERE frame_id = 999999", [])
            .unwrap();
        drop(repair);
        let db = Database::open(path.clone()).unwrap();
        db.read(|conn| {
            assert_eq!(pragma_text(conn, "quick_check")?, "ok");
            assert_eq!(foreign_key_error_count(conn)?, 0);
            assert_eq!(
                conn.query_row(
                    "SELECT status FROM migration_journal WHERE stage = 'postflight' ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "completed"
            );
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn failed_v6_to_v7_migration_rolls_back_legacy_schema() {
        let path = test_path("failed-v6-migration");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE VIEW app AS SELECT 1 AS id;")
                .unwrap();
        }

        let mut conn = Connection::open(&path).unwrap();
        assert!(schema::migrate_with_path(&mut conn, Some(&path)).is_err());
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            6
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM app_identity", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM system_sample", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'legacy_v6_app_identity'", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'boot_session'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        cleanup_test_files(&path);
    }

    #[test]
    fn old_migration_journal_schema_is_upgraded_and_retryable() {
        let path = test_path("old-migration-journal");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE migration_journal (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    run_id TEXT NOT NULL,
                    from_version INTEGER NOT NULL,
                    to_version INTEGER NOT NULL,
                    stage TEXT NOT NULL CHECK (stage IN ('preflight','backup','create','identity_backfill','usage_backfill','resource_backfill','verify','commit','postflight')),
                    status TEXT NOT NULL CHECK (status IN ('pending','started','completed','failed','interrupted')),
                    started_at_ms INTEGER NOT NULL,
                    completed_at_ms INTEGER,
                    detail_json TEXT,
                    error_text TEXT,
                    UNIQUE(run_id, stage)
                );
                CREATE INDEX idx_migration_journal_run ON migration_journal(run_id, id);
                CREATE INDEX idx_migration_journal_pending ON migration_journal(to_version, status, id);
                INSERT INTO migration_journal(run_id,from_version,to_version,stage,status,started_at_ms)
                VALUES ('old-run',6,7,'preflight','pending',0);
                "#,
            )
            .unwrap();
        }

        let db = Database::open(path.clone()).unwrap();
        db.read(|conn| {
            assert_eq!(
                conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
                7
            );
            let started_at_not_null: i64 = conn.query_row(
                "SELECT \"notnull\" FROM pragma_table_info('migration_journal') WHERE name = 'started_at_ms'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(started_at_not_null, 0);
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM migration_journal WHERE run_id = 'old-run'",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn v5_repairs_false_clock_gaps_before_v7_backfill() {
        let path = test_path("v5-clock-gap-repair");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute("DELETE FROM foreground_interval", []).unwrap();
            conn.execute(
                "INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason) VALUES (1, 1000, 2000, 2000, 'active', 'clock_gap')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason) VALUES (1, 6000, 7000, 7000, 'active', 'shutdown')",
                [],
            )
            .unwrap();
            conn.execute(
                "UPDATE settings SET value = '5000' WHERE key = 'foreground_poll_interval_ms'",
                [],
            )
            .unwrap();
            conn.pragma_update(None, "user_version", 4).unwrap();
        }
        let db = Database::open(path.clone()).unwrap();
        let repaired = db
            .read(|conn| {
                conn.query_row(
                    "SELECT end_time_ms, last_seen_time_ms, close_reason FROM foreground_interval WHERE start_time_ms = 1000",
                    [],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?)),
                )
            })
            .unwrap();
        assert_eq!(repaired, (6_000, 6_000, "sampling_interval_repair".into()));
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn collection_settings_round_trip() {
        let path = test_path("settings-round-trip");
        let _ = std::fs::remove_file(&path);
        let db = Database::open(path.clone()).unwrap();
        let expected = CollectionSettings {
            foreground_poll_interval_ms: 5_000,
            system_sample_interval_ms: 30_000,
            idle_threshold_seconds: 600,
            system_sample_retention_days: 14,
        };
        db.with_writer(|conn| writer::save_collection_settings(conn, &expected, 123))
            .unwrap();
        assert_eq!(db.read(writer::collection_settings).unwrap(), expected);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn start_with_windows_defaults_on_and_round_trips() {
        let path = test_path("start-with-windows");
        let _ = std::fs::remove_file(&path);
        let db = Database::open(path.clone()).unwrap();
        assert!(db.read(writer::start_with_windows).unwrap());
        db.with_writer(|conn| writer::save_start_with_windows(conn, false, 123))
            .unwrap();
        assert!(!db.read(writer::start_with_windows).unwrap());
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resource_snapshot_round_trip_and_retention_cascade() {
        let path = test_path("resource-snapshot");
        let _ = std::fs::remove_file(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = ResourceSnapshot {
            system: SystemSample {
                timestamp_ms: 10_000,
                sample_duration_ms: 5_000,
                cpu_percent: Some(75.0),
                memory_percent: Some(50.0),
                memory_used_bytes: Some(1_000),
                memory_total_bytes: Some(2_000),
                disk_read_bytes_per_sec: Some(300),
                disk_write_bytes_per_sec: Some(400),
                has_app_snapshot: false,
            },
            apps: vec![AppResourceSample {
                app_key: "path:c:\\app.exe".into(),
                process_name: "app.exe".into(),
                exe_path: Some("C:\\app.exe".into()),
                process_count: 2,
                cpu_percent: 70.0,
                memory_used_bytes: 900,
                io_read_bytes_per_sec: 250,
                io_write_bytes_per_sec: 350,
            }],
        };
        db.with_writer(|conn| writer::insert_resource_snapshot(conn, &snapshot))
            .unwrap();
        let apps = db
            .read(|conn| query::app_resource_samples(conn, 10_000))
            .unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].process_count, 2);
        let listed = db.read(query::resource_apps).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].process_name, "app.exe");
        let history = db
            .read(|conn| query::app_resource_history(conn, "path:c:\\app.exe", 1, 20_000, 500))
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].cpu_percent, Some(70.0));
        db.with_writer(|conn| writer::prune_system_samples(conn, 10_001))
            .unwrap();
        assert!(db
            .read(|conn| query::app_resource_samples(conn, 10_000))
            .unwrap()
            .is_empty());
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn resource_apps_merge_executable_versions_by_process_name() {
        let path = test_path("resource-app-version-merge");
        let _ = std::fs::remove_file(&path);
        let db = Database::open(path.clone()).unwrap();
        db.with_writer(|conn| {
            writer::upsert_app(
                conn,
                &ForegroundApp {
                    identity_key: r"path:c:\program files\windowsapps\openai_26.1\chatgpt.exe"
                        .into(),
                    process_name: "ChatGPT.exe".into(),
                    exe_path: Some(r"C:\Program Files\WindowsApps\OpenAI_26.1\ChatGPT.exe".into()),
                    display_name: "ChatGPT Desktop".into(),
                },
                9_000,
            )
            .map(|_| ())
        })
        .unwrap();
        for (timestamp_ms, version, cpu_percent) in [(10_000, "26.1", 10.0), (20_000, "26.2", 20.0)]
        {
            let exe_path = format!(r"C:\Program Files\WindowsApps\OpenAI_{version}\ChatGPT.exe");
            let snapshot = ResourceSnapshot {
                system: SystemSample {
                    timestamp_ms,
                    sample_duration_ms: 5_000,
                    cpu_percent: Some(cpu_percent),
                    memory_percent: Some(50.0),
                    memory_used_bytes: Some(1_000),
                    memory_total_bytes: Some(2_000),
                    disk_read_bytes_per_sec: Some(300),
                    disk_write_bytes_per_sec: Some(400),
                    has_app_snapshot: false,
                },
                apps: vec![AppResourceSample {
                    app_key: format!("path:{}", exe_path.to_lowercase()),
                    process_name: "ChatGPT.exe".into(),
                    exe_path: Some(exe_path),
                    process_count: 1,
                    cpu_percent,
                    memory_used_bytes: 900,
                    io_read_bytes_per_sec: 250,
                    io_write_bytes_per_sec: 350,
                }],
            };
            db.with_writer(|conn| writer::insert_resource_snapshot(conn, &snapshot))
                .unwrap();
        }

        let apps = db.read(query::resource_apps).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].app_key, "process:chatgpt.exe");
        assert_eq!(apps[0].display_name, "ChatGPT Desktop");
        assert!(apps[0]
            .exe_path
            .as_deref()
            .is_some_and(|path| path.contains("26.2")));

        let history = db
            .read(|conn| query::app_resource_history(conn, "process:chatgpt.exe", 1, 30_000, 500))
            .unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].cpu_percent, Some(10.0));
        assert_eq!(history[1].cpu_percent, Some(20.0));

        let dates = db
            .read(|conn| query::app_resource_available_dates(conn, "process:chatgpt.exe"))
            .unwrap();
        assert_eq!(dates.len(), 1);
        assert_eq!(db.read(query::resource_available_dates).unwrap(), dates);
        assert_eq!(db.read(query::overview_available_dates).unwrap(), dates);

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn nullable_logical_keys_are_unique_for_system_and_provider_rows() {
        let path = test_path("nullable-keys");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();

        db.with_writer(|conn| {
            let session_id: i64 = conn.query_row(
                "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_collection_session_id'",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO provider(kind, name, version) VALUES (?1, ?2, NULL)",
                params!["system", "builtin"],
            )?;
            assert!(conn
                .execute(
                    "INSERT INTO provider(kind, name, version) VALUES (?1, ?2, NULL)",
                    params!["system", "builtin"],
                )
                .is_err());
            conn.execute(
                "INSERT INTO provider(kind, name, version) VALUES (?1, ?2, ?3)",
                params!["system", "builtin", "1"],
            )?;
            conn.execute(
                "INSERT INTO provider(kind, name, version) VALUES (?1, ?2, ?3)",
                params!["system", "builtin", "2"],
            )?;

            conn.execute(
                "INSERT INTO collection_session_metric(session_id, metric_key, device_id, enabled, support_status, interval_ms) VALUES (?1, ?2, NULL, 1, 'supported', 1000)",
                params![session_id, "cpu"],
            )?;
            assert!(conn
                .execute(
                    "INSERT INTO collection_session_metric(session_id, metric_key, device_id, enabled, support_status, interval_ms) VALUES (?1, ?2, NULL, 1, 'supported', 1000)",
                    params![session_id, "cpu"],
                )
                .is_err());

            conn.execute(
                "INSERT INTO hardware_device(stable_key, category, first_seen_at_ms, last_seen_at_ms) VALUES (?1, 'cpu', 0, 0)",
                ["test:cpu:one"],
            )?;
            let device_one = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO hardware_device(stable_key, category, first_seen_at_ms, last_seen_at_ms) VALUES (?1, 'cpu', 0, 0)",
                ["test:cpu:two"],
            )?;
            let device_two = conn.last_insert_rowid();
            for device_id in [device_one, device_two] {
                conn.execute(
                    "INSERT INTO collection_session_metric(session_id, metric_key, device_id, enabled, support_status, interval_ms) VALUES (?1, ?2, ?3, 1, 'supported', 1000)",
                    params![session_id, "cpu", device_id],
                )?;
            }

            conn.execute(
                "INSERT INTO system_rollup_1m(bucket_start_ms, metric_key, device_id, avg_value, min_value, max_value, sample_count, quality_count, processing_version) VALUES (1000, 'cpu', NULL, 1.0, 1.0, 1.0, 1, 0, 'test')",
                [],
            )?;
            assert!(conn
                .execute(
                    "INSERT INTO system_rollup_1m(bucket_start_ms, metric_key, device_id, avg_value, min_value, max_value, sample_count, quality_count, processing_version) VALUES (1000, 'cpu', NULL, 1.0, 1.0, 1.0, 1, 0, 'test')",
                    [],
                )
                .is_err());
            for device_id in [device_one, device_two] {
                conn.execute(
                    "INSERT INTO system_rollup_1m(bucket_start_ms, metric_key, device_id, avg_value, min_value, max_value, sample_count, quality_count, processing_version) VALUES (1000, 'cpu', ?1, 1.0, 1.0, 1.0, 1, 0, 'test')",
                    [device_id],
                )?;
            }
            Ok(())
        })
        .unwrap();

        drop(db);
        let db = Database::open(path.clone()).unwrap();
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM provider", [], |row| row
                    .get::<_, i64>(0))?,
                3
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM collection_session_metric",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                3
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM system_rollup_1m", [], |row| row
                    .get::<_, i64>(0))?,
                3
            );
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn database_size_includes_main_wal_and_shm_and_handles_missing_files() {
        let path = test_path("database-size");
        cleanup_test_files(&path);
        std::fs::write(&path, b"abc").unwrap();
        std::fs::write(format!("{}-wal", path.display()), b"12345").unwrap();
        std::fs::write(format!("{}-shm", path.display()), b"1234567").unwrap();
        assert_eq!(schema::database_size_bytes(&path), 15);
        std::fs::remove_file(format!("{}-wal", path.display())).unwrap();
        std::fs::remove_file(format!("{}-shm", path.display())).unwrap();
        assert_eq!(schema::database_size_bytes(&path), 3);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(schema::database_size_bytes(&path), 0);
    }

    #[test]
    fn insufficient_space_preflight_leaves_v6_data_untouched_and_journaled() {
        let path = test_path("migration-space-preflight");
        cleanup_test_files(&path);
        create_v6_fixture(&path, true);
        let mut conn = Connection::open(&path).unwrap();
        assert!(schema::migrate_v6_to_v7_with_available_space(&mut conn, Some(&path), 0).is_err());
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            6
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'app_identity'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'legacy_v6_app_identity'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT status FROM migration_journal WHERE stage = 'preflight' ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "failed"
        );
        assert_eq!(
            conn.query_row(
                "SELECT status FROM migration_journal WHERE stage = 'backup' ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "pending"
        );
        drop(conn);
        cleanup_test_files(&path);
    }

    #[test]
    fn runtime_session_is_closed_after_final_flush_boundary() {
        let path = test_path("runtime-session-close");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let (collection_id, started_at): (i64, i64) = db
            .read(|conn| {
                conn.query_row(
                    "SELECT CAST(value AS INTEGER), started_at_ms FROM settings JOIN collection_session ON collection_session.id = CAST(settings.value AS INTEGER) WHERE settings.key = 'runtime_collection_session_id'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
            })
            .unwrap();
        db.with_writer(|conn| writer::finish_runtime_session(conn, started_at + 1_000, "clean"))
            .unwrap();
        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT ended_at_ms FROM collection_session WHERE id = ?1",
                    [collection_id],
                    |row| row.get::<_, Option<i64>>(0)
                )?,
                Some(started_at + 1_000)
            );
            assert_eq!(
                conn.query_row(
                    "SELECT shutdown_kind FROM boot_session WHERE id = (SELECT boot_session_id FROM collection_session WHERE id = ?1)",
                    [collection_id],
                    |row| row.get::<_, String>(0)
                )?,
                "clean"
            );
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn frame_writer_keeps_failed_frames_until_commit_and_bounds_queue() {
        let path = test_path("frame-writer");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = |timestamp_ms| ResourceSnapshot {
            system: SystemSample {
                timestamp_ms,
                sample_duration_ms: 5_000,
                cpu_percent: Some(25.0),
                memory_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };
        let mut frame_writer = writer::FrameWriter::new(2, 2);
        assert!(frame_writer.enqueue(snapshot(10_000)));
        assert!(frame_writer.enqueue(snapshot(20_000)));
        assert!(!frame_writer.enqueue(snapshot(30_000)));
        assert_eq!(frame_writer.health().queue_depth, 2);
        assert_eq!(frame_writer.health().drop_count, 1);

        let remaining_failures = Rc::new(Cell::new(2_u32));
        let first_attempt = remaining_failures.clone();
        assert!(db
            .with_writer(
                |conn| frame_writer.write_next_with(conn, false, move |conn, snapshot| {
                    let failures = first_attempt.get();
                    if failures > 0 {
                        first_attempt.set(failures - 1);
                        Err(rusqlite::Error::InvalidQuery)
                    } else {
                        writer::insert_resource_snapshot(conn, snapshot)
                    }
                })
            )
            .is_err());
        assert_eq!(frame_writer.health().queue_depth, 2);
        assert_eq!(frame_writer.health().front_retry_attempts, Some(1));
        assert!(frame_writer
            .health()
            .front_retry_backoff_ms
            .is_some_and(|delay| delay > 0));
        assert!(frame_writer.health().last_error.is_some());

        assert!(!db
            .with_writer(|conn| {
                frame_writer.write_next_with(conn, false, |_conn, _snapshot| {
                    Err(rusqlite::Error::InvalidQuery)
                })
            })
            .unwrap());
        assert_eq!(frame_writer.health().front_retry_attempts, Some(1));

        let second_attempt = remaining_failures.clone();
        assert!(db
            .with_writer(
                |conn| frame_writer.write_next_with(conn, true, move |conn, snapshot| {
                    let failures = second_attempt.get();
                    if failures > 0 {
                        second_attempt.set(failures - 1);
                        Err(rusqlite::Error::InvalidQuery)
                    } else {
                        writer::insert_resource_snapshot(conn, snapshot)
                    }
                })
            )
            .is_err());
        assert_eq!(frame_writer.health().front_retry_attempts, Some(2));

        let final_attempt = remaining_failures.clone();
        assert!(db
            .with_writer(
                |conn| frame_writer.write_next_with(conn, true, move |conn, snapshot| {
                    let failures = final_attempt.get();
                    if failures > 0 {
                        final_attempt.set(failures - 1);
                        Err(rusqlite::Error::InvalidQuery)
                    } else {
                        writer::insert_resource_snapshot(conn, snapshot)
                    }
                })
            )
            .unwrap());

        assert_eq!(frame_writer.health().queue_depth, 1);
        assert_eq!(
            db.read(
                |conn| conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row
                    .get::<_, i64>(0))
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db.with_writer(|conn| frame_writer.flush_all(conn)).unwrap(),
            1
        );
        assert_eq!(frame_writer.health().queue_depth, 0);
        assert_eq!(
            db.read(
                |conn| conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row
                    .get::<_, i64>(0))
            )
            .unwrap(),
            2
        );
        assert_eq!(
            frame_writer.health().last_committed_timestamp_ms,
            Some(20_000)
        );
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn frame_writer_retry_limits_zero_and_one_are_terminal_and_observable() {
        let path = test_path("frame-writer-limits");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = |timestamp_ms| ResourceSnapshot {
            system: SystemSample {
                timestamp_ms,
                sample_duration_ms: 5_000,
                cpu_percent: Some(25.0),
                memory_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };

        let mut zero = writer::FrameWriter::new(2, 0);
        assert!(zero.enqueue(snapshot(10_000)));
        assert!(db
            .with_writer(|conn| zero.write_next_with(conn, true, |_conn, _snapshot| {
                Err(rusqlite::Error::InvalidQuery)
            }))
            .is_err());
        let zero_health = zero.health();
        assert_eq!(zero_health.queue_depth, 0);
        assert_eq!(zero_health.drop_count, 1);
        assert_eq!(zero_health.terminal_failure_count, 1);
        assert!(zero_health.last_error.is_some());

        let mut one = writer::FrameWriter::new(2, 1);
        assert!(one.enqueue(snapshot(20_000)));
        assert!(db
            .with_writer(|conn| one.write_next_with(conn, true, |_conn, _snapshot| {
                Err(rusqlite::Error::InvalidQuery)
            }))
            .is_err());
        assert_eq!(one.health().front_retry_attempts, Some(1));
        assert!(db
            .with_writer(|conn| one.write_next_with(conn, true, |_conn, _snapshot| {
                Err(rusqlite::Error::InvalidQuery)
            }))
            .is_err());
        let one_health = one.health();
        assert_eq!(one_health.queue_depth, 0);
        assert_eq!(one_health.drop_count, 1);
        assert_eq!(one_health.terminal_failure_count, 1);
        assert!(one_health.last_error.is_some());

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn frame_writer_flush_continues_after_terminal_failure_and_reports_error() {
        let path = test_path("frame-writer-flush-failure");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = |timestamp_ms| ResourceSnapshot {
            system: SystemSample {
                timestamp_ms,
                sample_duration_ms: 5_000,
                cpu_percent: Some(25.0),
                memory_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };
        let mut frame_writer = writer::FrameWriter::new(4, 0);
        assert!(frame_writer.enqueue(snapshot(10_000)));
        assert!(frame_writer.enqueue(snapshot(20_000)));

        let result = db.with_writer(|conn| {
            frame_writer.flush_all_with(conn, |conn, snapshot| {
                if snapshot.system.timestamp_ms == 10_000 {
                    Err(rusqlite::Error::InvalidQuery)
                } else {
                    writer::insert_resource_snapshot(conn, snapshot)
                }
            })
        });
        assert!(result.is_err());
        let health = frame_writer.health();
        assert_eq!(health.queue_depth, 0);
        assert_eq!(health.drop_count, 1);
        assert_eq!(health.terminal_failure_count, 1);
        assert!(health.last_error.is_some());
        assert_eq!(health.last_committed_timestamp_ms, Some(20_000));
        assert_eq!(
            db.read(
                |conn| conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row
                    .get::<_, i64>(0))
            )
            .unwrap(),
            1
        );

        drop(db);
        cleanup_test_files(&path);
    }
}
