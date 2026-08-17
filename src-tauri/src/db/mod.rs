pub mod query;
pub mod rollup;
pub mod schema;
pub mod usage;
pub mod writer;

use rusqlite::{Connection, OpenFlags};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, TryLockError},
    thread,
    time::{Duration, Instant},
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
        let now = now_ms();
        let boot_identity = crate::platform::boot_identity(now);
        writer::recover_open_intervals(&conn, now)?;
        writer::start_runtime_session_with_identity(&conn, now, &boot_identity)?;
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

    pub fn with_writer_until<T>(
        &self,
        deadline: Instant,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = loop {
            if Instant::now() >= deadline {
                return Err(writer_lock_timeout_error());
            }
            match self.writer.try_lock() {
                Ok(conn) => {
                    if Instant::now() >= deadline {
                        return Err(writer_lock_timeout_error());
                    }
                    break conn;
                }
                Err(TryLockError::Poisoned(_)) => {
                    return Err(writer_lock_poisoned_error());
                }
                Err(TryLockError::WouldBlock) => {
                    let remaining = deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(1));
                    if remaining.is_zero() {
                        return Err(writer_lock_timeout_error());
                    }
                    thread::sleep(remaining);
                }
            }
        };
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

fn writer_lock_timeout_error() -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "database writer lock deadline expired",
    )))
}

fn writer_lock_poisoned_error() -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
        "database writer lock poisoned",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{query, writer},
        models::{
            ActivityState, AppResourceSample, BootIdentity, CollectionSettings, ComputerState,
            ForegroundApp, GpuSample, ResourceSnapshot, SystemSample, GPU_BOARD_POWER_SCOPE,
        },
    };
    use rusqlite::params;
    use std::{
        cell::Cell,
        rc::Rc,
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        },
        thread,
        time::{Duration, Instant},
    };

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
        let prefixes = [format!("{name}.v7-backup-"), format!("{name}.v8-backup-")];
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|value| prefixes.iter().any(|prefix| value.starts_with(prefix)))
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
                pid: None,
                process_creation_time_ms: None,
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
    fn empty_v6_database_migrates_to_v8() {
        let path = test_path("empty-v6");
        cleanup_test_files(&path);
        create_v6_fixture(&path, false);

        let db = Database::open(path.clone()).unwrap();
        db.read(|conn| {
            let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
            assert_eq!(version, 8);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM app", [], |row| row.get::<_, i64>(0))?, 0);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row.get::<_, i64>(0))?, 0);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM process_sample", [], |row| row.get::<_, i64>(0))?, 0);
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_computer_state_single_open'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                1
            );
            assert_eq!(foreign_key_error_count(conn)?, 0);
            assert_eq!(pragma_text(conn, "quick_check")?, "ok");
            assert_eq!(pragma_text(conn, "integrity_check")?, "ok");
            let (runs, completed): (i64, i64) = conn.query_row(
                "SELECT COUNT(DISTINCT run_id), SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) FROM migration_journal",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            assert_eq!((runs, completed), (2, 18));
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
            assert_eq!(conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?, 8);
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
            pid: None,
            process_creation_time_ms: None,
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
                        gpus: Vec::new(),
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
                    conn.query_row(
                        "SELECT COUNT(*) FROM migration_journal WHERE to_version = 7",
                        [],
                        |row| row.get::<_, i64>(0)
                    )?,
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
                8
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
            ..CollectionSettings::default()
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
                gpus: Vec::new(),
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
                    pid: None,
                    process_creation_time_ms: None,
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
                    gpus: Vec::new(),
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
    fn app_resource_history_preserves_process_snapshot_gaps_as_nulls() {
        let path = test_path("resource-history-gaps");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = |timestamp_ms, apps| ResourceSnapshot {
            system: SystemSample {
                timestamp_ms,
                sample_duration_ms: 5_000,
                cpu_percent: Some(25.0),
                memory_percent: Some(50.0),
                memory_used_bytes: Some(1_000),
                memory_total_bytes: Some(2_000),
                disk_read_bytes_per_sec: Some(300),
                disk_write_bytes_per_sec: Some(400),
                gpus: Vec::new(),
                has_app_snapshot: true,
            },
            apps,
        };
        let app = |path: &str, cpu_percent: f64| AppResourceSample {
            app_key: format!("path:{}", path.to_lowercase()),
            process_name: "ChatGPT.exe".into(),
            exe_path: Some(path.into()),
            process_count: 1,
            cpu_percent,
            memory_used_bytes: 900,
            io_read_bytes_per_sec: 250,
            io_write_bytes_per_sec: 350,
        };
        for (timestamp_ms, apps) in [
            (10_000, vec![app(r"C:\ChatGPT\v1\ChatGPT.exe", 10.0)]),
            (15_000, Vec::new()),
            (20_000, vec![app(r"C:\ChatGPT\v2\ChatGPT.exe", 20.0)]),
        ] {
            db.with_writer(|conn| {
                writer::insert_resource_snapshot(conn, &snapshot(timestamp_ms, apps))
            })
            .unwrap();
        }

        let history = db
            .read(|conn| query::app_resource_history(conn, "process:chatgpt.exe", 1, 30_000, 500))
            .unwrap();
        assert_eq!(
            history
                .iter()
                .map(|point| point.timestamp_ms)
                .collect::<Vec<_>>(),
            vec![10_000, 15_000, 20_000]
        );
        assert_eq!(history[0].cpu_percent, Some(10.0));
        assert_eq!(history[1].cpu_percent, None);
        assert_eq!(history[1].memory_used_bytes, None);
        assert_eq!(history[1].io_read_bytes_per_sec, None);
        assert_eq!(history[1].io_write_bytes_per_sec, None);
        assert_eq!(history[2].cpu_percent, Some(20.0));

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
    fn database_directory_resolves_the_database_parent_once() {
        let path = PathBuf::from("root").join("data").join("timeline.sqlite3");
        assert_eq!(schema::database_directory(&path), path.parent());
        assert_eq!(
            schema::database_directory(Path::new("timeline.sqlite3")),
            Some(Path::new("."))
        );
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
    fn writer_lock_deadline_timeout_does_not_run_closure() {
        let path = test_path("writer-lock-deadline");
        cleanup_test_files(&path);
        let db = Arc::new(Database::open(path.clone()).unwrap());
        let (locked_tx, locked_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let holder_db = Arc::clone(&db);
        let holder = thread::spawn(move || {
            let _guard = holder_db.writer.lock().unwrap();
            locked_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        locked_rx.recv().unwrap();

        let called = Arc::new(AtomicBool::new(false));
        let called_by_closure = Arc::clone(&called);
        let result = db.with_writer_until(Instant::now() + Duration::from_millis(25), |_| {
            called_by_closure.store(true, Ordering::SeqCst);
            Ok(())
        });
        let closure_ran = called.load(Ordering::SeqCst);
        release_tx.send(()).unwrap();
        holder.join().unwrap();

        let error = result.unwrap_err().to_string();
        assert!(error.contains("database writer lock deadline expired"));
        assert!(!closure_ran);

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn writer_lock_deadline_success_runs_closure() {
        let path = test_path("writer-lock-success");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let called = Cell::new(false);
        let result = db
            .with_writer_until(Instant::now() + Duration::from_secs(1), |conn| {
                called.set(true);
                conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
            })
            .unwrap();

        assert_eq!(result, 1);
        assert!(called.get());
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
                gpus: Vec::new(),
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
                gpus: Vec::new(),
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
                gpus: Vec::new(),
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

    #[test]
    fn frame_writer_transient_error_then_success_returns_ok() {
        let path = test_path("frame-writer-transient-success");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = ResourceSnapshot {
            system: SystemSample {
                timestamp_ms: 10_000,
                sample_duration_ms: 5_000,
                cpu_percent: Some(25.0),
                memory_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                gpus: Vec::new(),
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };
        let mut frame_writer = writer::FrameWriter::new(2, 2);
        assert!(frame_writer.enqueue(snapshot));
        let attempts = Rc::new(Cell::new(0_u32));
        let attempts_for_write = attempts.clone();
        let result = db.with_writer(|conn| {
            frame_writer.flush_all_with(conn, move |conn, snapshot| {
                let attempt = attempts_for_write.get();
                attempts_for_write.set(attempt + 1);
                if attempt == 0 {
                    Err(rusqlite::Error::InvalidQuery)
                } else {
                    writer::insert_resource_snapshot(conn, snapshot)
                }
            })
        });
        assert_eq!(result.unwrap(), 1);
        let health = frame_writer.health();
        assert_eq!(health.queue_depth, 0);
        assert_eq!(health.retry_count, 1);
        assert_eq!(health.terminal_failure_count, 0);
        assert!(health.last_error.is_some());
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

    #[test]
    fn frame_writer_deadline_failure_preserves_queue_without_retrying_forever() {
        let path = test_path("frame-writer-deadline");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = ResourceSnapshot {
            system: SystemSample {
                timestamp_ms: 10_000,
                sample_duration_ms: 5_000,
                cpu_percent: Some(25.0),
                memory_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                gpus: Vec::new(),
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };
        let mut frame_writer = writer::FrameWriter::new(2, 32);
        assert!(frame_writer.enqueue(snapshot));
        let result = db.with_writer(|conn| {
            frame_writer.flush_until_with(conn, std::time::Instant::now(), |_conn, _snapshot| {
                panic!("deadline should prevent another write attempt")
            })
        });
        assert!(result.is_err());
        let health = frame_writer.health();
        assert_eq!(health.queue_depth, 1);
        assert_eq!(health.retry_count, 0);
        assert_eq!(health.terminal_failure_count, 0);
        assert!(health
            .last_error
            .is_some_and(|error| error.contains("deadline")));

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_daily_usage_rollup_uses_interval_intersection_and_is_idempotent() {
        let path = test_path("pr02-daily-rollup");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let base = 1_700_000_000_000_i64;
        let app = ForegroundApp {
            identity_key: "name:editor.exe".into(),
            process_name: "editor.exe".into(),
            exe_path: Some("C:\\Editor.exe".into()),
            display_name: "Editor".into(),
            pid: Some(101),
            process_creation_time_ms: Some(base),
        };

        db.with_writer(|conn| {
            let executable_id = writer::resolve_foreground_app(conn, &app, base)?.app_executable_id;
            writer::begin_foreground_interval(conn, executable_id, base)?;

            let active_before_idle = writer::begin_computer_state_interval(
                conn,
                ComputerState::Active,
                base,
                "test",
                1,
            )?;
            writer::close_computer_state_interval(
                conn,
                active_before_idle,
                base + 300_000,
                "test",
            )?;
            let idle = writer::begin_computer_state_interval(
                conn,
                ComputerState::Idle,
                base + 300_000,
                "test",
                1,
            )?;
            writer::close_computer_state_interval(conn, idle, base + 900_000, "test")?;
            let active_after_idle = writer::begin_computer_state_interval(
                conn,
                ComputerState::Active,
                base + 900_000,
                "test",
                1,
            )?;
            writer::close_computer_state_interval(
                conn,
                active_after_idle,
                base + 1_200_000,
                "test",
            )?;
            writer::close_foreground_interval(conn, 1, base + 1_200_000, "test")?;
            writer::rebuild_daily_usage(conn, base, base + 1_300_000)
        })
        .unwrap();

        let local_date = db
            .read(|conn| {
                conn.query_row(
                    "SELECT date(?1 / 1000.0, 'unixepoch', 'localtime')",
                    [base],
                    |row| row.get::<_, String>(0),
                )
            })
            .unwrap();
        let summary = db
            .read(|conn| query::usage_summary(conn, base, base + 1_300_000, true))
            .unwrap();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].foreground_total_ms, 1_200_000);
        assert_eq!(summary[0].active_usage_ms, 600_000);
        assert_eq!(summary[0].idle_foreground_ms, 600_000);
        assert_eq!(
            db.read(|conn| query::computer_active_time(conn, base, base + 1_300_000))
                .unwrap(),
            600_000
        );
        assert!(
            summary[0].active_usage_ms
                <= db
                    .read(|conn| query::computer_active_time(conn, base, base + 1_300_000))
                    .unwrap()
        );

        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM foreground_interval WHERE end_time_ms IS NOT NULL",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                1
            );
            assert_eq!(
                conn.query_row(
                    "SELECT activity_state FROM foreground_interval",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                "active"
            );
            let overlap_count: i64 = conn.query_row(
                "SELECT COUNT(*)
                 FROM computer_state_interval first_state
                 JOIN computer_state_interval second_state
                   ON first_state.boot_session_id = second_state.boot_session_id
                  AND first_state.id < second_state.id
                  AND first_state.start_ts < COALESCE(second_state.end_ts, 9223372036854775807)
                  AND second_state.start_ts < COALESCE(first_state.end_ts, 9223372036854775807)",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(overlap_count, 0);
            Ok(())
        })
        .unwrap();

        let daily = db
            .read(|conn| query::daily_usage_summary(conn, &local_date, true))
            .unwrap();
        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].foreground_total_ms, 1_200_000);
        assert_eq!(daily[0].active_usage_ms, 600_000);
        assert_eq!(daily[0].idle_foreground_ms, 600_000);
        assert_eq!(daily[0].launch_count, 1);

        db.with_writer(|conn| writer::rebuild_daily_usage(conn, base, base + 1_300_000))
            .unwrap();
        let daily_again = db
            .read(|conn| query::daily_usage_summary(conn, &local_date, true))
            .unwrap();
        assert_eq!(daily_again.len(), 1);
        assert_eq!(
            (
                daily_again[0].foreground_total_ms,
                daily_again[0].active_usage_ms,
                daily_again[0].idle_foreground_ms,
                daily_again[0].launch_count,
            ),
            (
                daily[0].foreground_total_ms,
                daily[0].active_usage_ms,
                daily[0].idle_foreground_ms,
                daily[0].launch_count,
            )
        );

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_daily_usage_rollup_splits_at_local_midnight() {
        let path = test_path("pr02-midnight-rollup");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let app = ForegroundApp {
            identity_key: "name:midnight.exe".into(),
            process_name: "midnight.exe".into(),
            exe_path: Some("C:\\Midnight.exe".into()),
            display_name: "Midnight".into(),
            pid: Some(202),
            process_creation_time_ms: Some(1_000),
        };

        let (local_date, next_date, day_start, next_day_start) = db
            .read(|conn| {
                conn.query_row(
                    "SELECT date(?1 / 1000.0, 'unixepoch', 'localtime'),
                            date(date(?1 / 1000.0, 'unixepoch', 'localtime'), '+1 day'),
                            CAST(strftime('%s', date(?1 / 1000.0, 'unixepoch', 'localtime') || ' 00:00:00', 'utc') AS INTEGER) * 1000,
                            CAST(strftime('%s', date(date(?1 / 1000.0, 'unixepoch', 'localtime'), '+1 day') || ' 00:00:00', 'utc') AS INTEGER) * 1000",
                    [1_700_000_000_000_i64],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
            })
            .unwrap();
        let foreground_start = day_start + 23 * 60 * 60 * 1_000 + 55 * 60 * 1_000;
        let foreground_end = next_day_start + 10 * 60 * 1_000;

        db.with_writer(|conn| {
            let executable_id =
                writer::resolve_foreground_app(conn, &app, foreground_start)?.app_executable_id;
            writer::begin_foreground_interval(conn, executable_id, foreground_start)?;
            let state_id = writer::begin_computer_state_interval(
                conn,
                ComputerState::Active,
                foreground_start,
                "test",
                1,
            )?;
            writer::close_computer_state_interval(conn, state_id, foreground_end, "test")?;
            writer::close_foreground_interval(conn, 1, foreground_end, "test")?;
            writer::rebuild_daily_usage(conn, foreground_start, foreground_end)
        })
        .unwrap();

        let first_day = db
            .read(|conn| query::daily_usage_summary(conn, &local_date, true))
            .unwrap();
        let second_day = db
            .read(|conn| query::daily_usage_summary(conn, &next_date, true))
            .unwrap();
        assert_eq!(first_day[0].foreground_total_ms, 5 * 60 * 1_000);
        assert_eq!(second_day[0].foreground_total_ms, 10 * 60 * 1_000);
        assert_eq!(
            first_day[0].active_usage_ms,
            first_day[0].foreground_total_ms
        );
        assert_eq!(
            second_day[0].active_usage_ms,
            second_day[0].foreground_total_ms
        );

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_restart_recovery_ends_at_last_trusted_heartbeat() {
        let path = test_path("pr02-restart-recovery");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let identity = BootIdentity {
            boot_id: "test-boot-recovery".into(),
            boot_time_ms: 1_000,
        };
        let app = ForegroundApp {
            identity_key: "name:recovery.exe".into(),
            process_name: "recovery.exe".into(),
            exe_path: Some("C:\\Recovery.exe".into()),
            display_name: "Recovery".into(),
            pid: Some(303),
            process_creation_time_ms: Some(1_000),
        };

        db.with_writer(|conn| {
            writer::start_runtime_session_with_identity(conn, 10_000, &identity)?;
            let executable_id =
                writer::resolve_foreground_app(conn, &app, 11_000)?.app_executable_id;
            writer::begin_foreground_interval(conn, executable_id, 11_000)?;
            writer::checkpoint_foreground_interval(conn, 1, 15_000)?;
            writer::recover_open_intervals(conn, 100_000)?;
            writer::start_runtime_session_with_identity(conn, 200_000, &identity)?;
            writer::begin_foreground_interval(conn, executable_id, 200_000)?;
            Ok(())
        })
        .unwrap();

        db.read(|conn| {
            let mut statement = conn.prepare(
                "SELECT start_time_ms, end_time_ms
                 FROM foreground_interval
                 WHERE app_executable_id = (SELECT id FROM app_executable WHERE normalized_path = 'path:c:\\recovery.exe')
                 ORDER BY start_time_ms",
            )?;
            let rows: Vec<(i64, Option<i64>)> = statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            assert_eq!(rows, vec![(11_000, Some(15_000)), (200_000, None)]);
            let boot_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM boot_session WHERE boot_id = ?1",
                [&identity.boot_id],
                |row| row.get(0),
            )?;
            let collection_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM collection_session WHERE boot_session_id = (SELECT id FROM boot_session WHERE boot_id = ?1)",
                [&identity.boot_id],
                |row| row.get(0),
            )?;
            assert_eq!(boot_count, 1);
            assert_eq!(collection_count, 2);
            let sessions: Vec<(i64, Option<i64>)> = conn
                .prepare(
                    "SELECT started_at_ms, ended_at_ms
                     FROM collection_session
                     WHERE boot_session_id = (SELECT id FROM boot_session WHERE boot_id = ?1)
                     ORDER BY started_at_ms",
                )?
                .query_map([&identity.boot_id], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            assert_eq!(sessions.len(), 2);
            assert_eq!(sessions[0], (10_000, Some(15_000)));
            assert_eq!(sessions[1], (200_000, None));
            assert!(sessions[0].1.unwrap() < sessions[1].0);
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*)
                     FROM collection_session
                     WHERE boot_session_id = (SELECT id FROM boot_session WHERE boot_id = ?1)
                       AND started_at_ms < 200000
                       AND COALESCE(ended_at_ms, 200000) > 15000",
                    [&identity.boot_id],
                    |row| row.get::<_, i64>(0),
                )?,
                0
            );
            Ok(())
        })
        .unwrap();

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_boot_reconciliation_reuses_boot_within_tolerance() {
        let path = test_path("pr02-boot-reconciliation");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let first = BootIdentity {
            boot_id: "test-boot-exact".into(),
            boot_time_ms: 10_000,
        };
        let drifted = BootIdentity {
            boot_id: "test-boot-drifted-key".into(),
            boot_time_ms: 10_004,
        };
        let rebooted = BootIdentity {
            boot_id: "test-boot-rebooted".into(),
            boot_time_ms: 20_000,
        };

        db.with_writer(|conn| {
            writer::start_runtime_session_with_identity(conn, 30_000, &first)?;
            writer::start_runtime_session_with_identity(conn, 40_000, &drifted)?;
            writer::start_runtime_session_with_identity(conn, 50_000, &rebooted)
        })
        .unwrap();
        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM boot_session WHERE boot_id LIKE 'test-boot-%'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                2
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM collection_session WHERE boot_session_id = (SELECT id FROM boot_session WHERE boot_id = ?1)",
                    [&first.boot_id],
                    |row| row.get::<_, i64>(0),
                )?,
                2
            );
            Ok(())
        })
        .unwrap();

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_launch_count_uses_process_instance_not_foreground_activation() {
        let path = test_path("pr02-launch-count");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let base = 1_700_000_000_000_i64;
        let app_a = ForegroundApp {
            identity_key: "name:alpha.exe".into(),
            process_name: "alpha.exe".into(),
            exe_path: Some("C:\\Alpha.exe".into()),
            display_name: "Alpha".into(),
            pid: Some(404),
            process_creation_time_ms: Some(base),
        };
        let app_b = ForegroundApp {
            identity_key: "name:beta.exe".into(),
            process_name: "beta.exe".into(),
            exe_path: Some("C:\\Beta.exe".into()),
            display_name: "Beta".into(),
            pid: Some(405),
            process_creation_time_ms: Some(base),
        };

        db.with_writer(|conn| {
            let alpha_id = writer::resolve_foreground_app(conn, &app_a, base)?.app_executable_id;
            let beta_id = writer::resolve_foreground_app(conn, &app_b, base)?.app_executable_id;
            let state_id = writer::begin_computer_state_interval(
                conn,
                ComputerState::Active,
                base,
                "test",
                1,
            )?;
            let first_alpha = writer::begin_foreground_interval(conn, alpha_id, base)?;
            writer::close_foreground_interval(conn, first_alpha, base + 1_000, "app-switch")?;
            let beta = writer::begin_foreground_interval(conn, beta_id, base + 1_000)?;
            writer::close_foreground_interval(conn, beta, base + 2_000, "app-switch")?;
            let second_alpha = writer::begin_foreground_interval(conn, alpha_id, base + 2_000)?;
            writer::close_foreground_interval(conn, second_alpha, base + 3_000, "test")?;
            writer::close_computer_state_interval(conn, state_id, base + 3_000, "test")?;
            writer::rebuild_daily_usage(conn, base, base + 4_000)
        })
        .unwrap();

        let local_date = db
            .read(|conn| {
                conn.query_row(
                    "SELECT date(?1 / 1000.0, 'unixepoch', 'localtime')",
                    [base],
                    |row| row.get::<_, String>(0),
                )
            })
            .unwrap();
        let daily = db
            .read(|conn| query::daily_usage_summary(conn, &local_date, true))
            .unwrap();
        let alpha = daily
            .iter()
            .find(|item| item.app_name == "alpha.exe")
            .unwrap();
        assert_eq!(alpha.foreground_total_ms, 2_000);
        assert_eq!(alpha.launch_count, 1);

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_usage_transaction_rolls_back_close_when_start_fails() {
        let path = test_path("pr02-usage-transaction-rollback");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let app = ForegroundApp {
            identity_key: "name:transaction.exe".into(),
            process_name: "transaction.exe".into(),
            exe_path: Some("C:\\Transaction.exe".into()),
            display_name: "Transaction".into(),
            pid: Some(505),
            process_creation_time_ms: Some(1_000),
        };

        let (interval_id, executable_id) = db
            .with_writer(|conn| {
                let executable_id =
                    writer::resolve_foreground_app(conn, &app, 1_000)?.app_executable_id;
                let interval_id = writer::begin_foreground_interval(conn, executable_id, 1_000)?;
                Ok((interval_id, executable_id))
            })
            .unwrap();
        let current = writer::UsagePersistenceState {
            open_foreground_id: Some(interval_id),
            ..Default::default()
        };
        let actions = [
            writer::UsageWriteAction::CloseForeground {
                at_ms: 2_000,
                reason: "app-switch",
            },
            writer::UsageWriteAction::StartForeground {
                app_executable_id: executable_id + 1_000_000,
                at_ms: 2_000,
            },
        ];
        let result = db
            .with_writer(|conn| {
                writer::apply_usage_actions_with_retry(
                    conn,
                    &actions,
                    current,
                    15_000,
                    Some(Instant::now() + Duration::from_secs(1)),
                )
            })
            .unwrap_err();
        assert!(!writer::is_transient_usage_error(&result));

        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT end_time_ms FROM foreground_interval WHERE id = ?1",
                    [interval_id],
                    |row| row.get::<_, Option<i64>>(0),
                )?,
                None
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM foreground_interval WHERE start_time_ms = 2000",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                0
            );
            Ok(())
        })
        .unwrap();

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_usage_transaction_preserves_engine_state_when_close_fails() {
        let path = test_path("pr02-usage-close-failure");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let actions = [writer::UsageWriteAction::CloseForeground {
            at_ms: 2_000,
            reason: "test",
        }];
        let current = writer::UsagePersistenceState {
            open_foreground_id: Some(999_999),
            ..Default::default()
        };
        let result = db
            .with_writer(|conn| {
                writer::apply_usage_actions_with_retry(
                    conn,
                    &actions,
                    current,
                    15_000,
                    Some(Instant::now() + Duration::from_secs(1)),
                )
            })
            .unwrap_err();
        assert!(!writer::is_transient_usage_error(&result));
        assert_eq!(
            db.read(|conn| conn.query_row(
                "SELECT COUNT(*) FROM foreground_interval WHERE end_time_ms IS NULL",
                [],
                |row| row.get::<_, i64>(0),
            ))
            .unwrap(),
            0
        );
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_computer_state_transition_rolls_back_on_injected_failure() {
        let path = test_path("pr02-computer-state-rollback");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let state_id = db
            .with_writer(|conn| {
                let state_id = writer::begin_computer_state_interval(
                    conn,
                    ComputerState::Active,
                    1_000,
                    "test",
                    1,
                )?;
                conn.execute_batch(
                    "CREATE TRIGGER test_fail_idle_state
                     BEFORE INSERT ON computer_state_interval
                     WHEN NEW.state = 'idle'
                     BEGIN
                         SELECT RAISE(ABORT, 'injected computer state failure');
                     END;",
                )?;
                Ok(state_id)
            })
            .unwrap();
        let current = writer::UsagePersistenceState {
            open_computer_state_id: Some(state_id),
            ..Default::default()
        };
        let actions = [
            writer::UsageWriteAction::CloseComputerState {
                at_ms: 2_000,
                reason: "state-change",
            },
            writer::UsageWriteAction::StartComputerState {
                state: ComputerState::Idle,
                at_ms: 2_000,
                source: "test",
                quality: 1,
            },
        ];
        let result = db
            .with_writer(|conn| {
                writer::apply_usage_actions_with_retry(
                    conn,
                    &actions,
                    current,
                    15_000,
                    Some(Instant::now() + Duration::from_secs(1)),
                )
            })
            .unwrap_err();
        assert!(!writer::is_transient_usage_error(&result));
        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT end_ts FROM computer_state_interval WHERE id = ?1",
                    [state_id],
                    |row| row.get::<_, Option<i64>>(0),
                )?,
                None
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM computer_state_interval WHERE state = 'idle'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                0
            );
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_busy_usage_write_retries_and_recovers() {
        let path = test_path("pr02-usage-busy-retry");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let (ready_tx, ready_rx) = mpsc::sync_channel(0);
        let holder_path = path.clone();
        let holder = thread::spawn(move || {
            let holder = Connection::open(holder_path).unwrap();
            holder.execute_batch("BEGIN IMMEDIATE;").unwrap();
            ready_tx.send(()).unwrap();
            thread::sleep(Duration::from_millis(100));
            holder.execute_batch("ROLLBACK;").unwrap();
        });
        ready_rx.recv().unwrap();

        let actions = [writer::UsageWriteAction::StartComputerState {
            state: ComputerState::Active,
            at_ms: 2_000,
            source: "test",
            quality: 1,
        }];
        let result = db.with_writer(|conn| {
            writer::apply_usage_actions_with_retry_for_test(
                conn,
                &actions,
                Default::default(),
                15_000,
                Some(Instant::now() + Duration::from_secs(1)),
                8,
                Duration::from_millis(10),
            )
        });
        holder.join().unwrap();
        let (_, retries) = result.unwrap();
        assert!(retries > 0);
        assert_eq!(
            db.read(|conn| conn.query_row(
                "SELECT COUNT(*) FROM computer_state_interval WHERE start_ts = 2000",
                [],
                |row| row.get::<_, i64>(0),
            ))
            .unwrap(),
            1
        );

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn pr02_v7_state_open_index_repairs_existing_duplicates() {
        let path = test_path("pr02-state-repair");
        cleanup_test_files(&path);
        {
            let db = Database::open(path.clone()).unwrap();
            db.with_writer(|conn| {
                assert_eq!(
                    conn.query_row(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_computer_state_single_open'",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?,
                    1
                );
                let boot_id: i64 = conn.query_row(
                    "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_boot_session_id'",
                    [],
                    |row| row.get(0),
                )?;
                conn.execute("DROP INDEX idx_computer_state_single_open", [])?;
                conn.execute(
                    "INSERT INTO computer_state_interval(boot_session_id, state, start_ts, source, quality) VALUES (?1, 'active', 1000, 'test', 1)",
                    [boot_id],
                )?;
                conn.execute(
                    "INSERT INTO computer_state_interval(boot_session_id, state, start_ts, source, quality) VALUES (?1, 'idle', 2000, 'test', 1)",
                    [boot_id],
                )?;
                Ok(())
            })
            .unwrap();
            drop(db);
        }

        let mut conn = Connection::open(&path).unwrap();
        schema::migrate_with_path(&mut conn, Some(&path)).unwrap();
        let repaired: Vec<(i64, Option<i64>)> = {
            let mut statement = conn
                .prepare(
                    "SELECT start_ts, end_ts FROM computer_state_interval ORDER BY id DESC LIMIT 2",
                )
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        assert_eq!(repaired, vec![(2000, None), (1000, Some(2000))]);
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_computer_state_single_open'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        drop(conn);
        cleanup_test_files(&path);
    }

    #[test]
    fn gpu_storage_round_trips_multiple_devices_partial_metrics_and_zero() {
        let path = test_path("gpu-round-trip");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = ResourceSnapshot {
            system: SystemSample {
                timestamp_ms: 30_000,
                sample_duration_ms: 2_000,
                cpu_percent: Some(10.0),
                memory_percent: Some(20.0),
                memory_used_bytes: Some(2_000),
                memory_total_bytes: Some(8_000),
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                gpus: vec![
                    GpuSample {
                        device_key: "runtime:gpu:primary".into(),
                        vendor: Some("NVIDIA".into()),
                        model: Some("Test GPU A".into()),
                        capacity_bytes: Some(16 * 1024 * 1024 * 1024),
                        utilization_percent: Some(0.0),
                        memory_controller_utilization_percent: Some(0.0),
                        temperature_celsius: Some(42.5),
                        power_watts: Some(150.25),
                        graphics_clock_mhz: Some(2_535.0),
                        memory_clock_mhz: Some(16_001.0),
                        vram_used_bytes: Some(0),
                        vram_total_bytes: Some(16 * 1024 * 1024 * 1024),
                        power_scope: Some(GPU_BOARD_POWER_SCOPE.into()),
                    },
                    GpuSample {
                        device_key: "runtime:gpu:secondary".into(),
                        vendor: Some("NVIDIA".into()),
                        model: Some("Test GPU B".into()),
                        capacity_bytes: Some(8 * 1024 * 1024 * 1024),
                        utilization_percent: Some(80.0),
                        memory_controller_utilization_percent: None,
                        temperature_celsius: None,
                        power_watts: None,
                        graphics_clock_mhz: None,
                        memory_clock_mhz: None,
                        vram_used_bytes: Some(0),
                        vram_total_bytes: Some(8 * 1024 * 1024 * 1024),
                        power_scope: None,
                    },
                ],
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };
        db.with_writer(|conn| writer::insert_resource_snapshot(conn, &snapshot))
            .unwrap();
        assert_eq!(
            db.read(|conn| conn.query_row(
                "SELECT schema_version FROM collection_session WHERE ended_at_ms IS NULL",
                [],
                |row| row.get::<_, i64>(0),
            ))
            .unwrap(),
            8
        );

        db.with_writer(|conn| {
            let session_id: i64 = conn.query_row(
                "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'runtime_collection_session_id'",
                [],
                |row| row.get(0),
            )?;
            let provider_id = conn
                .execute(
                    "INSERT INTO provider(kind, name, version, last_status) VALUES ('gpu', 'test-provider', '1', 'supported')",
                    [],
                )
                .map(|_| conn.last_insert_rowid())?;
            let device_id: i64 = conn.query_row(
                "SELECT id FROM hardware_device WHERE stable_key = 'runtime:gpu:primary'",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO collection_session_metric(session_id, metric_key, device_id, enabled, support_status, provider_id, interval_ms) VALUES (?1, 'gpu.power_watts', ?2, 1, 'supported', ?3, 2000)",
                params![session_id, device_id, provider_id],
            )?;
            Ok(())
        })
        .unwrap();

        let samples = db
            .read(|conn| query::system_samples(conn, 1, 31_000, 100))
            .unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].gpus.len(), 2);
        assert_eq!(samples[0].gpus[0].device_key, "runtime:gpu:primary");
        assert_eq!(samples[0].gpus[0].utilization_percent, Some(0.0));
        assert_eq!(
            samples[0].gpus[0].memory_controller_utilization_percent,
            Some(0.0)
        );
        assert_eq!(samples[0].gpus[0].temperature_celsius, Some(42.5));
        assert_eq!(samples[0].gpus[0].power_watts, Some(150.25));
        assert_eq!(samples[0].gpus[0].graphics_clock_mhz, Some(2_535.0));
        assert_eq!(samples[0].gpus[0].memory_clock_mhz, Some(16_001.0));
        assert_eq!(samples[0].gpus[0].vram_used_bytes, Some(0));
        assert_eq!(
            samples[0].gpus[0].vram_total_bytes,
            Some(16 * 1024 * 1024 * 1024)
        );
        assert_eq!(
            samples[0].gpus[0].power_scope.as_deref(),
            Some(GPU_BOARD_POWER_SCOPE)
        );
        assert_eq!(samples[0].gpus[1].device_key, "runtime:gpu:secondary");
        assert_eq!(
            samples[0].gpus[1].memory_controller_utilization_percent,
            None
        );
        assert_eq!(samples[0].gpus[1].temperature_celsius, None);
        assert_eq!(samples[0].gpus[1].power_watts, None);
        assert_eq!(samples[0].gpus[1].vram_used_bytes, Some(0));
        assert_eq!(
            samples[0].gpus[1].vram_total_bytes,
            Some(8 * 1024 * 1024 * 1024)
        );
        let primary = db
            .read(|conn| query::gpu_samples(conn, 1, 31_000, Some("runtime:gpu:primary")))
            .unwrap();
        assert_eq!(primary.len(), 1);
        assert_eq!(primary[0].timestamp_ms, 30_000);
        assert_eq!(primary[0].gpu.device_key, "runtime:gpu:primary");
        assert_eq!(primary[0].gpu.utilization_percent, Some(0.0));
        assert_eq!(
            primary[0].gpu.memory_controller_utilization_percent,
            Some(0.0)
        );
        assert_eq!(primary[0].gpu.temperature_celsius, Some(42.5));
        assert_eq!(primary[0].gpu.power_watts, Some(150.25));
        assert_eq!(primary[0].gpu.graphics_clock_mhz, Some(2_535.0));
        assert_eq!(primary[0].gpu.memory_clock_mhz, Some(16_001.0));
        assert_eq!(primary[0].gpu.vram_used_bytes, Some(0));
        assert_eq!(
            primary[0].gpu.vram_total_bytes,
            Some(16 * 1024 * 1024 * 1024)
        );
        assert_eq!(
            primary[0].gpu.power_scope.as_deref(),
            Some(GPU_BOARD_POWER_SCOPE)
        );

        db.read(|conn| {
            let metadata: (String, i64, String) = conn.query_row(
                "SELECT p.name, csm.interval_ms, csm.support_status
                 FROM collection_session_metric csm
                 JOIN provider p ON p.id = csm.provider_id
                 WHERE csm.metric_key = 'gpu.power_watts'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            assert_eq!(
                metadata,
                ("test-provider".into(), 2_000, "supported".into())
            );
            Ok(())
        })
        .unwrap();

        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn gpu_storage_zero_devices_writes_no_gpu_rows() {
        let path = test_path("gpu-zero-devices");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = ResourceSnapshot {
            system: SystemSample {
                timestamp_ms: 40_000,
                sample_duration_ms: 2_000,
                cpu_percent: Some(1.0),
                memory_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                gpus: Vec::new(),
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };
        db.with_writer(|conn| writer::insert_resource_snapshot(conn, &snapshot))
            .unwrap();
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM hardware_device WHERE category = 'gpu'",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                0
            );
            let samples = query::system_samples(conn, 1, 41_000, 100)?;
            assert_eq!(samples.len(), 1);
            assert!(samples[0].gpus.is_empty());
            Ok(())
        })
        .unwrap();
        drop(db);
        cleanup_test_files(&path);
    }

    #[test]
    fn gpu_storage_power_scope_error_rolls_back_frame_and_retry_is_clean() {
        let path = test_path("gpu-power-scope-rollback");
        cleanup_test_files(&path);
        let db = Database::open(path.clone()).unwrap();
        let snapshot = |power_scope: Option<&str>| ResourceSnapshot {
            system: SystemSample {
                timestamp_ms: 50_000,
                sample_duration_ms: 2_000,
                cpu_percent: None,
                memory_percent: None,
                memory_used_bytes: None,
                memory_total_bytes: None,
                disk_read_bytes_per_sec: None,
                disk_write_bytes_per_sec: None,
                gpus: vec![GpuSample {
                    device_key: "runtime:gpu:rollback".into(),
                    vendor: None,
                    model: None,
                    capacity_bytes: None,
                    utilization_percent: Some(25.0),
                    memory_controller_utilization_percent: None,
                    temperature_celsius: None,
                    power_watts: Some(10.0),
                    graphics_clock_mhz: None,
                    memory_clock_mhz: None,
                    vram_used_bytes: None,
                    vram_total_bytes: None,
                    power_scope: power_scope.map(str::to_owned),
                }],
                has_app_snapshot: false,
            },
            apps: Vec::new(),
        };

        assert!(db
            .with_writer(|conn| writer::insert_resource_snapshot(conn, &snapshot(None)))
            .is_err());
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM sample_frame", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM hardware_device", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();

        db.with_writer(|conn| {
            writer::insert_resource_snapshot(conn, &snapshot(Some(GPU_BOARD_POWER_SCOPE)))
        })
        .unwrap();
        assert_eq!(
            db.read(
                |conn| conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| row
                    .get::<_, i64>(0))
            )
            .unwrap(),
            1
        );
        drop(db);
        cleanup_test_files(&path);
    }
}
