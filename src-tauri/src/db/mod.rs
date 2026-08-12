pub mod query;
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
        schema::migrate(&mut conn)?;
        writer::recover_open_interval(&conn)?;
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
        std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0)
    }
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

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "resource-timeline-{name}-{}.sqlite3",
            std::process::id()
        ))
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
    fn v1_to_v6_migration_preserves_data_and_existing_settings() {
        let path = test_path("v1-to-v6");
        let _ = std::fs::remove_file(&path);
        {
            let db = Database::open(path.clone()).unwrap();
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT INTO app_identity(identity_key, process_name, display_name, first_seen_at_ms, last_seen_at_ms) VALUES ('name:kept', 'kept.exe', 'Kept', 10, 20)",
                    [],
                )?;
                conn.execute(
                    "UPDATE settings SET value = '600' WHERE key = 'idle_threshold_seconds'",
                    [],
                )?;
                conn.execute(
                    "DELETE FROM settings WHERE key IN ('foreground_poll_interval_ms', 'system_sample_interval_ms')",
                    [],
                )?;
                conn.execute("DROP TABLE app_resource_sample", [])?;
                conn.execute("DROP TABLE app_resource_snapshot", [])?;
                conn.pragma_update(None, "user_version", 1)?;
                Ok(())
            })
            .unwrap();
        }

        let db = Database::open(path.clone()).unwrap();
        let (version, app_count, settings) = db
            .read(|conn| {
                Ok((
                    conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
                    conn.query_row("SELECT COUNT(*) FROM app_identity", [], |row| {
                        row.get::<_, i64>(0)
                    })?,
                    writer::collection_settings(conn)?,
                ))
            })
            .unwrap();
        assert_eq!(version, 6);
        assert_eq!(app_count, 1);
        assert_eq!(settings.idle_threshold_seconds, 600);
        assert_eq!(settings.foreground_poll_interval_ms, 1_000);
        assert_eq!(settings.system_sample_interval_ms, 5_000);
        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn v5_repairs_false_clock_gaps_from_slow_polling() {
        let path = test_path("v5-clock-gap-repair");
        let _ = std::fs::remove_file(&path);
        {
            let db = Database::open(path.clone()).unwrap();
            db.with_writer(|conn| {
                conn.execute(
                    "INSERT INTO app_identity(identity_key, process_name, display_name, first_seen_at_ms, last_seen_at_ms) VALUES ('name:test', 'test.exe', 'Test', 1000, 6000)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason) VALUES (1, 1000, 2000, 2000, 'active', 'clock_gap')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason) VALUES (1, 6000, 7000, 7000, 'active', 'shutdown')",
                    [],
                )?;
                conn.execute(
                    "UPDATE settings SET value = '5000' WHERE key = 'foreground_poll_interval_ms'",
                    [],
                )?;
                conn.pragma_update(None, "user_version", 4)?;
                Ok(())
            })
            .unwrap();
        }
        let db = Database::open(path.clone()).unwrap();
        let repaired = db
            .read(|conn| {
                conn.query_row(
                    "SELECT end_time_ms, last_seen_time_ms, end_reason FROM foreground_interval WHERE start_time_ms = 1000",
                    [],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?)),
                )
            })
            .unwrap();
        assert_eq!(repaired, (6_000, 6_000, "sampling_interval_repair".into()));
        drop(db);
        let _ = std::fs::remove_file(path);
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
}
