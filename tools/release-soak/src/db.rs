use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension};
use serde::Serialize;
use std::{fs, path::Path};

const ALLOWED_CATEGORIES: [&str; 8] = [
    "cpu", "gpu", "memory", "disk", "network", "power", "battery", "process",
];

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectionConfiguration {
    pub enabled_categories: Vec<String>,
    pub disabled_provider_count: u64,
    pub foreground_poll_interval_ms: Option<u64>,
    pub system_sample_interval_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderObservation {
    pub provider: String,
    pub persisted_status: String,
    pub enabled_metric_count: u64,
    pub failed_metric_count: u64,
    pub unsupported_metric_count: u64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DbObservation {
    pub present: bool,
    pub query_ok: bool,
    pub query_error_code: Option<String>,
    pub user_version: Option<i64>,
    pub main_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
    pub total_bytes: u64,
    pub current_session_id: Option<i64>,
    pub current_session_schema_version: Option<i64>,
    pub committed_frame_count: Option<u64>,
    pub first_frame_ts_ms: Option<i64>,
    pub last_frame_ts_ms: Option<i64>,
    pub writer_delay_average_ms: Option<f64>,
    pub writer_delay_max_ms: Option<i64>,
    pub sleep_interval_count: Option<u64>,
    pub active_retention_hold_count: Option<u64>,
    pub configuration: CollectionConfiguration,
    pub providers: Vec<ProviderObservation>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaCheck {
    pub present: bool,
    pub user_version: Option<i64>,
    pub quick_check: Option<String>,
    pub foreign_key_error_count: Option<u64>,
    pub passed: bool,
}

pub fn observe(path: &Path, since_ms: i64) -> DbObservation {
    let (main_bytes, wal_bytes, shm_bytes) = file_sizes(path);
    let mut result = DbObservation {
        present: main_bytes > 0,
        main_bytes,
        wal_bytes,
        shm_bytes,
        total_bytes: main_bytes
            .saturating_add(wal_bytes)
            .saturating_add(shm_bytes),
        ..DbObservation::default()
    };
    if !result.present {
        result.query_error_code = Some("database_missing".to_string());
        return result;
    }
    let conn = match Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(error) => {
            result.query_error_code = Some(error_code(&error));
            return result;
        }
    };
    if conn
        .busy_timeout(std::time::Duration::from_millis(750))
        .is_err()
    {
        result.query_error_code = Some("busy_timeout_setup_failed".to_string());
        return result;
    }
    result.user_version = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .ok();
    if table_exists(&conn, "settings") {
        result.configuration = read_configuration(&conn);
    }
    if table_exists(&conn, "collection_session") {
        result.current_session_id = conn
            .query_row(
                "SELECT id FROM collection_session WHERE ended_at_ms IS NULL ORDER BY started_at_ms DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten();
        if let Some(session_id) = result.current_session_id {
            result.current_session_schema_version = conn
                .query_row(
                    "SELECT schema_version FROM collection_session WHERE id = ?1",
                    [session_id],
                    |row| row.get(0),
                )
                .optional()
                .ok()
                .flatten();
        }
    }
    if table_exists(&conn, "sample_frame") {
        let _ = conn.query_row(
            "SELECT COUNT(*), MIN(ts), MAX(ts), AVG(writer_delay_ms), MAX(writer_delay_ms)
             FROM sample_frame WHERE ts >= ?1 AND source = 'runtime'",
            [since_ms],
            |row| {
                result.committed_frame_count = Some(row.get::<_, i64>(0)?.max(0) as u64);
                result.first_frame_ts_ms = row.get(1)?;
                result.last_frame_ts_ms = row.get(2)?;
                result.writer_delay_average_ms = row.get(3)?;
                result.writer_delay_max_ms = row.get(4)?;
                Ok::<_, rusqlite::Error>(())
            },
        );
    }
    if table_exists(&conn, "computer_state_interval") {
        result.sleep_interval_count = conn
            .query_row(
                "SELECT COUNT(*) FROM computer_state_interval
                 WHERE state = 'sleep' AND (end_ts IS NULL OR end_ts >= ?1)",
                [since_ms],
                |row| row.get::<_, i64>(0),
            )
            .ok()
            .map(|value| value.max(0) as u64);
    }
    if table_exists(&conn, "retention_hold") {
        result.active_retention_hold_count = conn
            .query_row(
                "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                [],
                |row| row.get::<_, i64>(0),
            )
            .ok()
            .map(|value| value.max(0) as u64);
    }
    if table_exists(&conn, "provider") && table_exists(&conn, "collection_session_metric") {
        result.providers = read_providers(&conn, result.current_session_id);
    }
    result.query_ok = true;
    result
}

pub fn schema_check(path: &Path) -> Result<SchemaCheck, String> {
    let (main_bytes, _, _) = file_sizes(path);
    if main_bytes == 0 {
        return Ok(SchemaCheck {
            present: false,
            user_version: None,
            quick_check: None,
            foreign_key_error_count: None,
            passed: false,
        });
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("open schema check failed: {}", error_code(&error)))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "busy_timeout_setup_failed".to_string())?;
    let user_version = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .map_err(|error| format!("read user_version failed: {}", error_code(&error)))?;
    let quick_check = conn
        .query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))
        .ok();
    let foreign_key_error_count = if table_exists(&conn, "sqlite_sequence") || user_version > 0 {
        let mut statement = conn
            .prepare("PRAGMA foreign_key_check")
            .map_err(|error| format!("foreign_key_check failed: {}", error_code(&error)))?;
        let mut rows = statement
            .query([])
            .map_err(|error| format!("foreign_key_check query failed: {}", error_code(&error)))?;
        let mut count = 0_u64;
        while rows
            .next()
            .map_err(|error| format!("foreign_key_check read failed: {}", error_code(&error)))?
            .is_some()
        {
            count = count.saturating_add(1);
        }
        Some(count)
    } else {
        Some(0)
    };
    Ok(SchemaCheck {
        present: true,
        user_version: Some(user_version),
        quick_check: quick_check.clone(),
        foreign_key_error_count,
        passed: user_version == 8
            && quick_check.as_deref() == Some("ok")
            && foreign_key_error_count == Some(0),
    })
}

fn read_configuration(conn: &Connection) -> CollectionConfiguration {
    let enabled_categories = setting_text(conn, "enabled_categories")
        .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
        .unwrap_or_else(|| {
            vec![
                "cpu".to_string(),
                "memory".to_string(),
                "disk".to_string(),
                "process".to_string(),
            ]
        })
        .into_iter()
        .filter(|value| ALLOWED_CATEGORIES.contains(&value.as_str()))
        .collect();
    let disabled_provider_count = setting_text(conn, "disabled_providers")
        .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
        .map(|values| values.len() as u64)
        .unwrap_or(0);
    CollectionConfiguration {
        enabled_categories,
        disabled_provider_count,
        foreground_poll_interval_ms: setting_text(conn, "foreground_poll_interval_ms")
            .and_then(|value| value.parse().ok()),
        system_sample_interval_ms: setting_text(conn, "system_sample_interval_ms")
            .and_then(|value| value.parse().ok()),
    }
}

fn read_providers(conn: &Connection, session_id: Option<i64>) -> Vec<ProviderObservation> {
    let mut statement = match conn.prepare(
        "SELECT p.kind, p.name, p.last_status,
                COALESCE(SUM(CASE WHEN csm.enabled = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN csm.support_status = 'failed' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN csm.support_status = 'unsupported' THEN 1 ELSE 0 END), 0)
         FROM provider p
         LEFT JOIN collection_session_metric csm
           ON csm.provider_id = p.id
          AND (?1 IS NULL OR csm.session_id = ?1)
         GROUP BY p.id, p.kind, p.name, p.last_status
         ORDER BY p.kind, p.name",
    ) {
        Ok(statement) => statement,
        Err(_) => return Vec::new(),
    };
    let rows = statement.query_map([session_id], |row| {
        let kind: String = row.get(0)?;
        let name: String = row.get(1)?;
        Ok(ProviderObservation {
            provider: safe_token(&format!("{kind}:{name}")),
            persisted_status: safe_token(&row.get::<_, String>(2)?),
            enabled_metric_count: row.get::<_, i64>(3)?.max(0) as u64,
            failed_metric_count: row.get::<_, i64>(4)?.max(0) as u64,
            unsupported_metric_count: row.get::<_, i64>(5)?.max(0) as u64,
        })
    });
    match rows {
        Ok(rows) => rows.filter_map(Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

fn setting_text(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .optional()
    .ok()
    .flatten()
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [table],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn file_sizes(path: &Path) -> (u64, u64, u64) {
    let main = file_size(path);
    let wal = file_size(&sidecar(path, "-wal"));
    let shm = file_size(&sidecar(path, "-shm"));
    (main, wal, shm)
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn sidecar(path: &Path, suffix: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}{}", path.display(), suffix))
}

fn error_code(error: &rusqlite::Error) -> String {
    match error {
        rusqlite::Error::SqliteFailure(code, _) => match code.code {
            ErrorCode::DatabaseBusy => "database_busy".to_string(),
            ErrorCode::DatabaseLocked => "database_locked".to_string(),
            ErrorCode::NotADatabase => "not_a_database".to_string(),
            ErrorCode::ReadOnly => "read_only".to_string(),
            _ => "sqlite_error".to_string(),
        },
        rusqlite::Error::InvalidPath(_) => "invalid_path".to_string(),
        _ => "database_error".to_string(),
    }
}

fn safe_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
        })
        .take(128)
        .collect()
}
