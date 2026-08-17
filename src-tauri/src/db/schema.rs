use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static MIGRATION_RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

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
CREATE UNIQUE INDEX idx_foreground_single_open ON foreground_interval((end_time_ms IS NULL)) WHERE end_time_ms IS NULL;
CREATE INDEX idx_foreground_interval_range ON foreground_interval(start_time_ms, end_time_ms, last_seen_time_ms);
CREATE INDEX idx_foreground_interval_app ON foreground_interval(app_id, start_time_ms);
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
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at_ms INTEGER NOT NULL);
INSERT INTO settings(key, value, updated_at_ms) VALUES
    ('idle_threshold_seconds', '300', 0), ('system_sample_retention_days', '7', 0);
"#;

const MIGRATION_JOURNAL_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS migration_journal (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    from_version INTEGER NOT NULL,
    to_version INTEGER NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('preflight','backup','create','identity_backfill','usage_backfill','resource_backfill','verify','commit','postflight')),
    status TEXT NOT NULL CHECK (status IN ('pending','started','completed','failed','interrupted')),
    started_at_ms INTEGER,
    completed_at_ms INTEGER,
    detail_json TEXT,
    error_text TEXT,
    UNIQUE(run_id, stage)
);
CREATE INDEX IF NOT EXISTS idx_migration_journal_run ON migration_journal(run_id, id);
CREATE INDEX IF NOT EXISTS idx_migration_journal_pending ON migration_journal(to_version, status, id);
"#;

const SCHEMA_V7: &str = r#"
CREATE TABLE IF NOT EXISTS boot_session (
    id INTEGER PRIMARY KEY AUTOINCREMENT, boot_id TEXT NOT NULL UNIQUE, boot_time_ms INTEGER,
    observed_start_ms INTEGER, observed_end_ms INTEGER,
    shutdown_kind TEXT CHECK (shutdown_kind IS NULL OR shutdown_kind IN ('clean','crash','sleep','legacy-v6','unknown')),
    created_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS collection_session (
    id INTEGER PRIMARY KEY AUTOINCREMENT, boot_session_id INTEGER NOT NULL, started_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER, app_version TEXT, schema_version INTEGER NOT NULL, config_hash TEXT,
    FOREIGN KEY (boot_session_id) REFERENCES boot_session(id) ON DELETE CASCADE,
    CHECK (ended_at_ms IS NULL OR ended_at_ms >= started_at_ms), UNIQUE (boot_session_id, started_at_ms)
);
CREATE TABLE IF NOT EXISTS hardware_device (
    id INTEGER PRIMARY KEY AUTOINCREMENT, stable_key TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL CHECK (category IN ('cpu','gpu','memory','disk','network','battery','power','cooling','other')),
    vendor TEXT, model TEXT, capacity_bytes INTEGER, first_seen_at_ms INTEGER NOT NULL, last_seen_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS provider (
    id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, name TEXT NOT NULL, version TEXT, priority INTEGER NOT NULL DEFAULT 0,
    last_status TEXT NOT NULL DEFAULT 'unknown' CHECK (last_status IN ('unknown','supported','unsupported','permission_denied','provider_missing','probe_failed','failed')),
    UNIQUE(kind, name, version)
);
CREATE TABLE IF NOT EXISTS collection_session_metric (
    session_id INTEGER NOT NULL, metric_key TEXT NOT NULL, device_id INTEGER, enabled INTEGER NOT NULL CHECK (enabled IN (0,1)),
    support_status TEXT NOT NULL CHECK (support_status IN ('supported','unsupported','permission_denied','provider_missing','probe_failed','failed')),
    provider_id INTEGER, interval_ms INTEGER, PRIMARY KEY(session_id, metric_key, device_id),
    FOREIGN KEY(session_id) REFERENCES collection_session(id) ON DELETE CASCADE,
    FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE SET NULL,
    FOREIGN KEY(provider_id) REFERENCES provider(id) ON DELETE SET NULL,
    CHECK(interval_ms IS NULL OR interval_ms > 0)
);
CREATE TABLE IF NOT EXISTS app (
    id INTEGER PRIMARY KEY AUTOINCREMENT, stable_key TEXT NOT NULL UNIQUE, process_name TEXT NOT NULL, display_name TEXT NOT NULL, publisher TEXT, category TEXT,
    is_hidden INTEGER NOT NULL DEFAULT 0 CHECK(is_hidden IN (0,1)), first_seen_at_ms INTEGER NOT NULL, last_seen_at_ms INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS app_executable (
    id INTEGER PRIMARY KEY AUTOINCREMENT, app_id INTEGER NOT NULL, normalized_path TEXT NOT NULL, file_identity TEXT,
    version TEXT, package_family TEXT, first_seen_at_ms INTEGER NOT NULL, last_seen_at_ms INTEGER NOT NULL,
    source TEXT NOT NULL DEFAULT 'runtime' CHECK(source IN ('legacy-v6','runtime','unknown')),
    FOREIGN KEY(app_id) REFERENCES app(id) ON DELETE CASCADE, UNIQUE(app_id, normalized_path)
);
CREATE INDEX IF NOT EXISTS idx_app_executable_path ON app_executable(normalized_path);
CREATE TABLE IF NOT EXISTS process_instance (
    id INTEGER PRIMARY KEY AUTOINCREMENT, app_executable_id INTEGER NOT NULL, stable_key TEXT NOT NULL UNIQUE, pid INTEGER,
    create_time_ms INTEGER, exit_time_ms INTEGER, exit_code INTEGER,
    source TEXT NOT NULL DEFAULT 'runtime' CHECK(source IN ('legacy-v6','runtime','unknown')),
    FOREIGN KEY(app_executable_id) REFERENCES app_executable(id) ON DELETE CASCADE,
    CHECK(exit_time_ms IS NULL OR create_time_ms IS NULL OR exit_time_ms >= create_time_ms)
);
CREATE INDEX IF NOT EXISTS idx_process_instance_executable ON process_instance(app_executable_id, create_time_ms);
CREATE TABLE IF NOT EXISTS computer_state_interval (
    id INTEGER PRIMARY KEY AUTOINCREMENT, boot_session_id INTEGER NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('active','idle','locked','sleep','disconnected','unknown')),
    start_ts INTEGER NOT NULL, end_ts INTEGER, source TEXT NOT NULL, quality INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(boot_session_id) REFERENCES boot_session(id) ON DELETE CASCADE, CHECK(end_ts IS NULL OR end_ts >= start_ts)
);
CREATE INDEX IF NOT EXISTS idx_computer_state_range ON computer_state_interval(boot_session_id, start_ts, end_ts);
CREATE TABLE IF NOT EXISTS foreground_interval (
    id INTEGER PRIMARY KEY AUTOINCREMENT, boot_session_id INTEGER NOT NULL, app_executable_id INTEGER NOT NULL,
    start_time_ms INTEGER NOT NULL, end_time_ms INTEGER, last_seen_time_ms INTEGER NOT NULL,
    activity_state TEXT NOT NULL CHECK(activity_state IN ('active','idle')), close_reason TEXT, context_id INTEGER, legacy_v6_id INTEGER UNIQUE,
    FOREIGN KEY(boot_session_id) REFERENCES boot_session(id) ON DELETE CASCADE,
    FOREIGN KEY(app_executable_id) REFERENCES app_executable(id) ON DELETE CASCADE,
    CHECK(end_time_ms IS NULL OR end_time_ms >= start_time_ms), CHECK(last_seen_time_ms >= start_time_ms)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_foreground_single_open ON foreground_interval((end_time_ms IS NULL)) WHERE end_time_ms IS NULL;
CREATE INDEX IF NOT EXISTS idx_foreground_interval_range ON foreground_interval(start_time_ms, end_time_ms, last_seen_time_ms);
CREATE INDEX IF NOT EXISTS idx_foreground_interval_app ON foreground_interval(app_executable_id, start_time_ms);
CREATE TABLE IF NOT EXISTS sample_frame (
    id INTEGER PRIMARY KEY AUTOINCREMENT, collection_session_id INTEGER NOT NULL, ts INTEGER NOT NULL, sequence INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL, writer_delay_ms INTEGER, process_snapshot_present INTEGER NOT NULL DEFAULT 0 CHECK(process_snapshot_present IN (0,1)),
    source TEXT NOT NULL DEFAULT 'runtime' CHECK(source IN ('legacy-v6','runtime','unknown')), legacy_v6_system_sample_id INTEGER UNIQUE,
    FOREIGN KEY(collection_session_id) REFERENCES collection_session(id) ON DELETE CASCADE,
    CHECK(duration_ms > 0), CHECK(writer_delay_ms >= 0), UNIQUE(collection_session_id, sequence)
);
CREATE INDEX IF NOT EXISTS idx_sample_frame_time ON sample_frame(ts);
CREATE INDEX IF NOT EXISTS idx_sample_frame_session_time ON sample_frame(collection_session_id, ts);
CREATE TABLE IF NOT EXISTS cpu_sample (
    frame_id INTEGER PRIMARY KEY, usage_pct REAL, temp_c REAL, package_power_w REAL, effective_clock_mhz REAL,
    quality_mask INTEGER NOT NULL DEFAULT 0, source TEXT NOT NULL DEFAULT 'runtime' CHECK(source IN ('legacy-v6','runtime','unknown')),
    FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS gpu_sample (
    frame_id INTEGER NOT NULL, device_id INTEGER NOT NULL, usage_pct REAL, temp_c REAL, board_power_w REAL, core_clock_mhz REAL,
    vram_used_bytes INTEGER, quality_mask INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(frame_id, device_id),
    FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE, FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS memory_sample (
    frame_id INTEGER PRIMARY KEY, used_bytes INTEGER, available_bytes INTEGER, usage_pct REAL, quality_mask INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'runtime' CHECK(source IN ('legacy-v6','runtime','unknown')),
    FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS disk_sample (
    frame_id INTEGER NOT NULL, device_id INTEGER NOT NULL, read_bps INTEGER, write_bps INTEGER, active_pct REAL, temp_c REAL,
    quality_mask INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(frame_id, device_id),
    FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE, FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS network_sample (
    frame_id INTEGER NOT NULL, device_id INTEGER NOT NULL, rx_bps INTEGER, tx_bps INTEGER, signal REAL, rx_phy_bps INTEGER, tx_phy_bps INTEGER,
    quality_mask INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(frame_id, device_id),
    FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE, FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS battery_sample (
    frame_id INTEGER NOT NULL, device_id INTEGER NOT NULL, remaining_pct REAL, charge_rate_w REAL, state TEXT, quality_mask INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(frame_id, device_id), FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE, FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS power_sample (
    frame_id INTEGER NOT NULL, device_id INTEGER NOT NULL, power_w REAL, energy_counter_wh REAL, power_scope TEXT NOT NULL, quality_mask INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(frame_id, device_id), FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE, FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS cooling_sample (
    frame_id INTEGER NOT NULL, device_id INTEGER NOT NULL, rpm REAL, sensor_kind TEXT NOT NULL, quality_mask INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(frame_id, device_id), FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE, FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS process_sample (
    frame_id INTEGER NOT NULL, process_instance_id INTEGER NOT NULL, cpu_pct REAL, cpu_time_delta_us INTEGER, working_set_bytes INTEGER,
    private_bytes INTEGER, gpu_pct REAL, vram_bytes INTEGER, process_count INTEGER NOT NULL DEFAULT 1, read_bps INTEGER, write_bps INTEGER, network_bps INTEGER,
    selection_reason INTEGER NOT NULL DEFAULT 0, quality_mask INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'runtime' CHECK(source IN ('legacy-v6','runtime','unknown')), legacy_v6_id INTEGER UNIQUE,
    PRIMARY KEY(frame_id, process_instance_id), FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE,
    FOREIGN KEY(process_instance_id) REFERENCES process_instance(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_process_sample_process_time ON process_sample(process_instance_id, frame_id);
CREATE TABLE IF NOT EXISTS system_rollup_1m (
    bucket_start_ms INTEGER NOT NULL, metric_key TEXT NOT NULL, device_id INTEGER, avg_value REAL, min_value REAL, max_value REAL,
    sample_count INTEGER NOT NULL CHECK(sample_count >= 0), quality_count INTEGER NOT NULL CHECK(quality_count >= 0), coverage REAL,
    source_start_ms INTEGER, source_end_ms INTEGER, processing_version TEXT NOT NULL, PRIMARY KEY(bucket_start_ms, metric_key, device_id)
);
CREATE TABLE IF NOT EXISTS process_rollup_1m (
    bucket_start_ms INTEGER NOT NULL, app_id INTEGER NOT NULL, weighted_cpu_pct REAL, max_working_set_bytes INTEGER,
    cpu_time_us INTEGER NOT NULL DEFAULT 0, read_bytes INTEGER NOT NULL DEFAULT 0, write_bytes INTEGER NOT NULL DEFAULT 0, gpu_active_ms INTEGER NOT NULL DEFAULT 0,
    sample_count INTEGER NOT NULL CHECK(sample_count >= 0), coverage REAL, selection_reason_mask INTEGER NOT NULL DEFAULT 0,
    source_start_ms INTEGER, source_end_ms INTEGER, processing_version TEXT NOT NULL, PRIMARY KEY(bucket_start_ms, app_id), FOREIGN KEY(app_id) REFERENCES app(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS process_rollup_1h (
    bucket_start_ms INTEGER NOT NULL, app_id INTEGER NOT NULL, weighted_cpu_pct REAL, max_working_set_bytes INTEGER,
    cpu_time_us INTEGER NOT NULL DEFAULT 0, read_bytes INTEGER NOT NULL DEFAULT 0, write_bytes INTEGER NOT NULL DEFAULT 0, gpu_active_ms INTEGER NOT NULL DEFAULT 0,
    sample_count INTEGER NOT NULL CHECK(sample_count >= 0), coverage REAL, selection_reason_mask INTEGER NOT NULL DEFAULT 0,
    source_start_ms INTEGER, source_end_ms INTEGER, processing_version TEXT NOT NULL,
    PRIMARY KEY(bucket_start_ms, app_id), FOREIGN KEY(app_id) REFERENCES app(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS app_usage_daily (
    local_date TEXT NOT NULL, app_id INTEGER NOT NULL, foreground_total_ms INTEGER NOT NULL DEFAULT 0, active_usage_ms INTEGER,
    idle_foreground_ms INTEGER NOT NULL DEFAULT 0, launch_count INTEGER NOT NULL DEFAULT 0, processing_version TEXT NOT NULL,
    PRIMARY KEY(local_date, app_id), FOREIGN KEY(app_id) REFERENCES app(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS app_resource_daily (
    local_date TEXT NOT NULL, app_id INTEGER NOT NULL, cpu_time_us INTEGER, memory_peak_bytes INTEGER, gpu_active_ms INTEGER,
    read_bytes INTEGER, write_bytes INTEGER, crash_count INTEGER NOT NULL DEFAULT 0, hang_count INTEGER NOT NULL DEFAULT 0, coverage REAL,
    processing_version TEXT NOT NULL, PRIMARY KEY(local_date, app_id), FOREIGN KEY(app_id) REFERENCES app(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS energy_rollup_daily (
    local_date TEXT NOT NULL, device_id INTEGER NOT NULL, power_scope TEXT NOT NULL, energy_wh REAL, covered_duration_ms INTEGER,
    expected_duration_ms INTEGER, provider_id INTEGER, component_json TEXT, processing_version TEXT NOT NULL,
    PRIMARY KEY(local_date, device_id, power_scope), FOREIGN KEY(device_id) REFERENCES hardware_device(id) ON DELETE CASCADE, FOREIGN KEY(provider_id) REFERENCES provider(id) ON DELETE SET NULL
);
CREATE TABLE IF NOT EXISTS system_event (
    id INTEGER PRIMARY KEY AUTOINCREMENT, channel TEXT NOT NULL, event_id TEXT NOT NULL, record_id TEXT NOT NULL, event_time_ms INTEGER NOT NULL,
    provider TEXT, payload_summary TEXT, UNIQUE(channel, record_id)
);
CREATE INDEX IF NOT EXISTS idx_system_event_time ON system_event(event_time_ms);
CREATE TABLE IF NOT EXISTS crash_case (
    id INTEGER PRIMARY KEY AUTOINCREMENT, stable_key TEXT NOT NULL UNIQUE, anchor_time_ms INTEGER NOT NULL, classification TEXT NOT NULL,
    window_start_ms INTEGER NOT NULL, window_end_ms INTEGER NOT NULL, evidence_status TEXT NOT NULL, processing_version TEXT NOT NULL,
    CHECK(window_end_ms >= window_start_ms)
);
CREATE INDEX IF NOT EXISTS idx_crash_case_anchor ON crash_case(anchor_time_ms);
CREATE TABLE IF NOT EXISTS retention_hold (
    id INTEGER PRIMARY KEY AUTOINCREMENT, crash_case_id INTEGER NOT NULL, start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL,
    expires_at_ms INTEGER, released_at_ms INTEGER, FOREIGN KEY(crash_case_id) REFERENCES crash_case(id) ON DELETE CASCADE, CHECK(end_ms >= start_ms)
);
CREATE TABLE IF NOT EXISTS crash_evidence_summary (
    id INTEGER PRIMARY KEY AUTOINCREMENT, crash_case_id INTEGER NOT NULL, metric_key TEXT NOT NULL, window_start_ms INTEGER NOT NULL, window_end_ms INTEGER NOT NULL,
    avg_value REAL, min_value REAL, max_value REAL, delta_value REAL, peak_time_ms INTEGER, coverage REAL, evidence_ref TEXT, processing_version TEXT NOT NULL,
    FOREIGN KEY(crash_case_id) REFERENCES crash_case(id) ON DELETE CASCADE, UNIQUE(crash_case_id, metric_key), CHECK(window_end_ms >= window_start_ms)
);
CREATE TABLE IF NOT EXISTS v7_legacy_identity_map (
    legacy_app_id INTEGER PRIMARY KEY, app_id INTEGER NOT NULL, app_executable_id INTEGER NOT NULL,
    FOREIGN KEY(app_id) REFERENCES app(id) ON DELETE CASCADE, FOREIGN KEY(app_executable_id) REFERENCES app_executable(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS v7_legacy_frame_map (
    legacy_system_sample_id INTEGER PRIMARY KEY, frame_id INTEGER NOT NULL UNIQUE,
    FOREIGN KEY(frame_id) REFERENCES sample_frame(id) ON DELETE CASCADE
);
"#;

const V7_COMPATIBILITY_DDL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_app_process_name ON app(process_name);
CREATE TRIGGER IF NOT EXISTS trg_provider_null_version_insert
    BEFORE INSERT ON provider
    WHEN NEW.version IS NULL AND EXISTS(
        SELECT 1 FROM provider WHERE kind = NEW.kind AND name = NEW.name AND version IS NULL
    )
BEGIN
    SELECT RAISE(ABORT, 'duplicate provider with NULL version');
END;
CREATE TRIGGER IF NOT EXISTS trg_provider_null_version_update
    BEFORE UPDATE OF kind, name, version ON provider
    WHEN NEW.version IS NULL AND EXISTS(
        SELECT 1 FROM provider WHERE id <> OLD.id AND kind = NEW.kind AND name = NEW.name AND version IS NULL
    )
BEGIN
    SELECT RAISE(ABORT, 'duplicate provider with NULL version');
END;
CREATE TRIGGER IF NOT EXISTS trg_collection_session_metric_system_insert
    BEFORE INSERT ON collection_session_metric
    WHEN NEW.device_id IS NULL AND EXISTS(
        SELECT 1 FROM collection_session_metric
        WHERE session_id = NEW.session_id AND metric_key = NEW.metric_key AND device_id IS NULL
    )
BEGIN
    SELECT RAISE(ABORT, 'duplicate system collection metric');
END;
CREATE TRIGGER IF NOT EXISTS trg_collection_session_metric_system_update
    BEFORE UPDATE OF session_id, metric_key, device_id ON collection_session_metric
    WHEN NEW.device_id IS NULL AND EXISTS(
        SELECT 1 FROM collection_session_metric
        WHERE rowid <> OLD.rowid AND session_id = NEW.session_id AND metric_key = NEW.metric_key AND device_id IS NULL
    )
BEGIN
    SELECT RAISE(ABORT, 'duplicate system collection metric');
END;
CREATE TRIGGER IF NOT EXISTS trg_system_rollup_1m_system_insert
    BEFORE INSERT ON system_rollup_1m
    WHEN NEW.device_id IS NULL AND EXISTS(
        SELECT 1 FROM system_rollup_1m
        WHERE bucket_start_ms = NEW.bucket_start_ms AND metric_key = NEW.metric_key AND device_id IS NULL
    )
BEGIN
    SELECT RAISE(ABORT, 'duplicate system rollup');
END;
CREATE TRIGGER IF NOT EXISTS trg_system_rollup_1m_system_update
    BEFORE UPDATE OF bucket_start_ms, metric_key, device_id ON system_rollup_1m
    WHEN NEW.device_id IS NULL AND EXISTS(
        SELECT 1 FROM system_rollup_1m
        WHERE rowid <> OLD.rowid AND bucket_start_ms = NEW.bucket_start_ms AND metric_key = NEW.metric_key AND device_id IS NULL
    )
BEGIN
    SELECT RAISE(ABORT, 'duplicate system rollup');
END;
"#;

const V8_COMPATIBILITY_DDL: &str = r#"
CREATE INDEX IF NOT EXISTS idx_gpu_sample_device_frame ON gpu_sample(device_id, frame_id);
CREATE TRIGGER IF NOT EXISTS trg_gpu_sample_power_scope_insert
    BEFORE INSERT ON gpu_sample
    WHEN NOT (
        (NEW.board_power_w IS NULL AND NEW.power_scope IS NULL)
        OR (NEW.board_power_w IS NOT NULL AND NEW.power_scope = 'gpu_board')
    )
BEGIN
    SELECT RAISE(ABORT, 'gpu board power must use gpu_board scope');
END;
CREATE TRIGGER IF NOT EXISTS trg_gpu_sample_power_scope_update
    BEFORE UPDATE OF board_power_w, power_scope ON gpu_sample
    WHEN NOT (
        (NEW.board_power_w IS NULL AND NEW.power_scope IS NULL)
        OR (NEW.board_power_w IS NOT NULL AND NEW.power_scope = 'gpu_board')
    )
BEGIN
    SELECT RAISE(ABORT, 'gpu board power must use gpu_board scope');
END;
"#;

const STAGES: &[&str] = &[
    "preflight",
    "backup",
    "create",
    "identity_backfill",
    "usage_backfill",
    "resource_backfill",
    "verify",
    "commit",
    "postflight",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPreflight {
    pub user_version: i64,
    pub database_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
    pub backup_parent_exists: bool,
    pub available_bytes: Option<u64>,
    pub required_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationVerification {
    pub foreground_rows: i64,
    pub frame_rows: i64,
    pub process_rows: i64,
    pub earliest_foreground_ms: Option<i64>,
    pub latest_foreground_ms: Option<i64>,
    pub earliest_frame_ms: Option<i64>,
    pub latest_frame_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct LegacySummary {
    app_rows: i64,
    foreground_rows: i64,
    frame_rows: i64,
    process_rows: i64,
    earliest_foreground_ms: Option<i64>,
    latest_foreground_ms: Option<i64>,
    earliest_frame_ms: Option<i64>,
    latest_frame_ms: Option<i64>,
    memory_total: Option<i64>,
    disk_read_total: Option<i64>,
    disk_write_total: Option<i64>,
    process_cpu_total: Option<f64>,
    process_memory_total: Option<i64>,
    process_read_total: Option<i64>,
    process_write_total: Option<i64>,
}

#[allow(dead_code)]
pub fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    migrate_with_path(conn, None)
}

pub fn migrate_with_path(
    conn: &mut Connection,
    database_path: Option<&Path>,
) -> rusqlite::Result<()> {
    configure_connection(conn)?;
    let original_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if original_version > 8 {
        return Err(invalid_schema(
            "database schema is newer than this application",
        ));
    }
    if original_version < 1 {
        let has_legacy = table_exists(conn, "app_identity")?;
        let tx = conn.transaction()?;
        if has_legacy {
            tx.execute_batch("ALTER TABLE app_identity RENAME TO legacy_app_identity; ALTER TABLE foreground_interval RENAME TO legacy_foreground_interval; ALTER TABLE system_sample RENAME TO legacy_system_sample; DROP TABLE IF EXISTS app_display_rule;")?;
        }
        tx.execute_batch(SCHEMA_V1)?;
        if has_legacy {
            tx.execute_batch(r#"
                INSERT OR IGNORE INTO app_identity(identity_key, process_name, exe_path, display_name, publisher, is_hidden, first_seen_at_ms, last_seen_at_ms)
                SELECT CASE WHEN exe_path IS NOT NULL AND trim(exe_path) <> '' THEN 'path:' || lower(replace(trim(exe_path), '/', '\\')) WHEN lower(app_name) LIKE 'pid-%' THEN 'name:unresolved' ELSE 'name:' || lower(app_name) END,
                       CASE WHEN lower(app_name) LIKE 'pid-%' THEN 'unresolved' ELSE app_name END, exe_path, COALESCE(display_name, app_name), publisher,
                       COALESCE(is_hidden, 0), COALESCE(first_seen_at, 0) * 1000, COALESCE(last_seen_at, first_seen_at, 0) * 1000
                FROM legacy_app_identity;
                INSERT INTO foreground_interval(app_id, start_time_ms, end_time_ms, last_seen_time_ms, activity_state, end_reason)
                SELECT target.id, old.start_time * 1000, old.end_time * 1000, old.end_time * 1000, 'active', 'recovery'
                FROM legacy_foreground_interval old JOIN legacy_app_identity old_app ON old_app.id = old.app_id
                JOIN app_identity target ON target.identity_key = CASE WHEN old_app.exe_path IS NOT NULL AND trim(old_app.exe_path) <> '' THEN 'path:' || lower(replace(trim(old_app.exe_path), '/', '\\')) WHEN lower(old_app.app_name) LIKE 'pid-%' THEN 'name:unresolved' ELSE 'name:' || lower(old_app.app_name) END
                WHERE old.end_time >= old.start_time;
                INSERT INTO system_sample(timestamp_ms, sample_duration_ms, cpu_percent, memory_percent, memory_used_bytes, memory_total_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec)
                SELECT timestamp * 1000, 5000, cpu_percent, memory_percent, memory_used_bytes, memory_total_bytes, disk_read_bytes_per_sec, disk_write_bytes_per_sec FROM legacy_system_sample;
                DROP TABLE legacy_foreground_interval; DROP TABLE legacy_system_sample; DROP TABLE legacy_app_identity;
            "#)?;
        }
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    }
    if original_version < 2 {
        let tx = conn.transaction()?;
        tx.execute_batch("INSERT OR IGNORE INTO settings(key,value,updated_at_ms) VALUES ('foreground_poll_interval_ms','1000',0),('system_sample_interval_ms','5000',0),('idle_threshold_seconds','300',0),('system_sample_retention_days','7',0);")?;
        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
    }
    if original_version < 3 {
        let tx = conn.transaction()?;
        tx.execute_batch("CREATE TABLE app_resource_sample (id INTEGER PRIMARY KEY AUTOINCREMENT, system_sample_id INTEGER NOT NULL, app_key TEXT NOT NULL, process_name TEXT NOT NULL, exe_path TEXT, process_count INTEGER NOT NULL, cpu_percent REAL NOT NULL, memory_used_bytes INTEGER NOT NULL, io_read_bytes_per_sec INTEGER NOT NULL, io_write_bytes_per_sec INTEGER NOT NULL, FOREIGN KEY(system_sample_id) REFERENCES system_sample(id) ON DELETE CASCADE, UNIQUE(system_sample_id,app_key)); CREATE INDEX idx_app_resource_sample_system ON app_resource_sample(system_sample_id);")?;
        tx.pragma_update(None, "user_version", 3)?;
        tx.commit()?;
    }
    if original_version < 4 {
        let tx = conn.transaction()?;
        tx.execute_batch("CREATE TABLE app_resource_snapshot (system_sample_id INTEGER PRIMARY KEY, FOREIGN KEY(system_sample_id) REFERENCES system_sample(id) ON DELETE CASCADE); INSERT OR IGNORE INTO app_resource_snapshot(system_sample_id) SELECT DISTINCT system_sample_id FROM app_resource_sample;")?;
        tx.pragma_update(None, "user_version", 4)?;
        tx.commit()?;
    }
    if original_version < 5 {
        let poll = conn.query_row("SELECT CAST(value AS INTEGER) FROM settings WHERE key='foreground_poll_interval_ms'", [], |row| row.get::<_,i64>(0)).unwrap_or(1000).clamp(1000,10000);
        let tx = conn.transaction()?;
        if poll > 2500 {
            tx.execute(r#"UPDATE foreground_interval AS current SET end_time_ms=(SELECT MIN(next.start_time_ms) FROM foreground_interval next WHERE next.start_time_ms > current.start_time_ms), last_seen_time_ms=(SELECT MIN(next.start_time_ms) FROM foreground_interval next WHERE next.start_time_ms > current.start_time_ms), end_reason='sampling_interval_repair' WHERE current.end_reason='clock_gap' AND current.end_time_ms-current.start_time_ms BETWEEN 0 AND 1500 AND (SELECT MIN(next.start_time_ms) FROM foreground_interval next WHERE next.start_time_ms > current.start_time_ms)-current.start_time_ms BETWEEN ?1 AND ?2"#, params![poll/2,poll*5/2])?;
        }
        tx.pragma_update(None, "user_version", 5)?;
        tx.commit()?;
    }
    if original_version < 6 {
        let tx = conn.transaction()?;
        tx.execute("INSERT OR IGNORE INTO settings(key,value,updated_at_ms) VALUES ('start_with_windows','1',0)", [])?;
        tx.pragma_update(None, "user_version", 6)?;
        tx.commit()?;
    }
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 7 {
        migrate_v6_to_v7(conn, database_path)?;
    }
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 8 {
        migrate_v7_to_v8(conn, database_path)?;
    }
    let final_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match final_version {
        7 => validate_v7_open(conn)?,
        8 => validate_v8_open(conn)?,
        _ => {}
    }
    Ok(())
}

#[allow(dead_code)]
pub fn preflight(
    conn: &Connection,
    database_path: Option<&Path>,
) -> rusqlite::Result<MigrationPreflight> {
    preflight_with_available_space(conn, database_path, None)
}

fn preflight_with_available_space(
    conn: &Connection,
    database_path: Option<&Path>,
    available_override: Option<u64>,
) -> rusqlite::Result<MigrationPreflight> {
    check_pragma_ok(conn, "quick_check")?;
    check_pragma_ok(conn, "integrity_check")?;
    if let Some(error) = foreign_key_error(conn)? {
        return Err(invalid_schema(&format!(
            "foreign_key_check failed: {error}"
        )));
    }
    let backup_parent_exists = database_path
        .and_then(database_directory)
        .map(Path::exists)
        .unwrap_or(true);
    if !backup_parent_exists {
        return Err(invalid_schema("database backup directory does not exist"));
    }
    let database_bytes = database_path.map(file_size).unwrap_or(0);
    let wal_bytes = database_path
        .map(|p| file_size(&sidecar(p, "-wal")))
        .unwrap_or(0);
    let shm_bytes = database_path
        .map(|p| file_size(&sidecar(p, "-shm")))
        .unwrap_or(0);
    let required_bytes = required_migration_space(database_bytes, wal_bytes, shm_bytes);
    let available_bytes = available_override.or_else(|| {
        database_path
            .and_then(database_directory)
            .and_then(available_space_bytes)
    });
    if let Some(available) = available_bytes {
        if available < required_bytes {
            return Err(invalid_schema(&format!(
                "insufficient free space for v6 to v7 migration: available={available}, required={required_bytes}"
            )));
        }
    }
    Ok(MigrationPreflight {
        user_version: conn.pragma_query_value(None, "user_version", |row| row.get(0))?,
        database_bytes,
        wal_bytes,
        shm_bytes,
        backup_parent_exists,
        available_bytes,
        required_bytes,
    })
}

pub fn create_consistent_backup(conn: &Connection, destination: &Path) -> rusqlite::Result<()> {
    if connection_main_path(conn).as_deref() == Some(destination) {
        return Err(invalid_schema("backup destination is the live database"));
    }
    if destination.exists() {
        return Err(invalid_schema("backup destination already exists"));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    conn.execute("VACUUM INTO ?1", [destination.to_string_lossy().as_ref()])?;
    Ok(())
}

fn migrate_v6_to_v7(conn: &mut Connection, database_path: Option<&Path>) -> rusqlite::Result<()> {
    migrate_v6_to_v7_with_options(conn, database_path, None, None)
}

fn migrate_v7_to_v8(conn: &mut Connection, database_path: Option<&Path>) -> rusqlite::Result<()> {
    migrate_v7_to_v8_with_options(conn, database_path, None, None)
}

#[cfg(test)]
pub(crate) fn migrate_v6_to_v7_fail_at(
    conn: &mut Connection,
    database_path: Option<&Path>,
    stage: &str,
) -> rusqlite::Result<()> {
    migrate_v6_to_v7_with_options(conn, database_path, Some(stage), None)
}

#[cfg(test)]
pub(crate) fn migrate_v6_to_v7_with_available_space(
    conn: &mut Connection,
    database_path: Option<&Path>,
    available_bytes: u64,
) -> rusqlite::Result<()> {
    migrate_v6_to_v7_with_options(conn, database_path, None, Some(available_bytes))
}

fn migrate_v6_to_v7_with_options(
    conn: &mut Connection,
    database_path: Option<&Path>,
    failure_stage: Option<&str>,
    available_override: Option<u64>,
) -> rusqlite::Result<()> {
    ensure_migration_journal_schema(conn)?;
    let run_id = format!(
        "v6-v7-{}-{}-{}",
        now_ms(),
        std::process::id(),
        MIGRATION_RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    conn.execute("UPDATE migration_journal SET status='interrupted', completed_at_ms=?1, error_text='previous migration was interrupted' WHERE to_version=7 AND status = 'started'", [now_ms()])?;
    start_run(conn, &run_id, 6, 7)?;
    mark_stage(conn, &run_id, "preflight", "started", None, None)?;
    let preflight = match preflight_with_available_space(conn, database_path, available_override) {
        Ok(v) => {
            mark_stage(conn, &run_id, "preflight", "completed", None, None)?;
            v
        }
        Err(e) => {
            mark_stage(
                conn,
                &run_id,
                "preflight",
                "failed",
                None,
                Some(&e.to_string()),
            )?;
            return Err(e);
        }
    };
    mark_stage(conn, &run_id, "backup", "started", None, None)?;
    let backup_path = database_path.map(|p| {
        p.parent().unwrap_or_else(|| Path::new(".")).join(format!(
            "{}.v7-backup-{}-{}.sqlite3",
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("resource-timeline.sqlite3"),
            now_ms(),
            std::process::id()
        ))
    });
    if let Some(path) = backup_path.as_deref() {
        if let Err(e) = create_consistent_backup(conn, path) {
            mark_stage(
                conn,
                &run_id,
                "backup",
                "failed",
                None,
                Some(&e.to_string()),
            )?;
            return Err(e);
        }
    }
    mark_stage(
        conn,
        &run_id,
        "backup",
        "completed",
        Some(&format!(
            "{{\"database_bytes\":{},\"wal_bytes\":{},\"shm_bytes\":{},\"available_bytes\":{},\"required_bytes\":{}}}",
            preflight.database_bytes,
            preflight.wal_bytes,
            preflight.shm_bytes,
            preflight
                .available_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".into()),
            preflight.required_bytes
        )),
        None,
    )?;
    let legacy = legacy_summary(conn)?;
    mark_stage(conn, &run_id, "create", "started", None, None)?;
    let mut current_stage = "create";
    let mut completed_stages = Vec::new();
    let result = (|| {
        let tx = conn.transaction()?;
        mark_stage_tx(&tx, &run_id, "create", "started", None, None)?;
        inject_failure(failure_stage, "create")?;
        rename_legacy_tables(&tx)?;
        tx.execute_batch(SCHEMA_V7)?;
        tx.execute_batch(V7_COMPATIBILITY_DDL)?;
        repair_open_computer_state_intervals(&tx)?;
        install_v7_nullable_constraints(&tx)?;
        let (boot, collection) = ensure_legacy_session(&tx, &legacy)?;
        mark_stage_tx(&tx, &run_id, "create", "completed", None, None)?;
        completed_stages.push("create");

        current_stage = "identity_backfill";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        backfill_identity(&tx)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);

        current_stage = "usage_backfill";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        backfill_usage(&tx, boot)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);

        current_stage = "resource_backfill";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        backfill_resources(&tx, collection)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);

        current_stage = "verify";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        let verification = verify_v7(&tx, &legacy)?;
        verify_database_integrity(&tx)?;
        mark_stage_tx(
            &tx,
            &run_id,
            current_stage,
            "completed",
            Some(&format!(
                "{{\"foreground_rows\":{},\"frame_rows\":{},\"process_rows\":{}}}",
                verification.foreground_rows, verification.frame_rows, verification.process_rows
            )),
            None,
        )?;
        completed_stages.push(current_stage);

        current_stage = "commit";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        tx.pragma_update(None, "user_version", 7)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);
        tx.commit()?;
        Ok::<_, rusqlite::Error>(verification)
    })();
    let verification = match result {
        Ok(v) => v,
        Err(e) => {
            for stage in &completed_stages {
                mark_stage(conn, &run_id, stage, "completed", None, None)?;
            }
            mark_stage(
                conn,
                &run_id,
                current_stage,
                "failed",
                None,
                Some(&e.to_string()),
            )?;
            return Err(e);
        }
    };
    debug_assert_eq!(verification.frame_rows, legacy.frame_rows);
    mark_stage(conn, &run_id, "postflight", "started", None, None)?;
    if let Err(e) = inject_failure(failure_stage, "postflight") {
        mark_stage(
            conn,
            &run_id,
            "postflight",
            "failed",
            None,
            Some(&e.to_string()),
        )?;
        return Err(e);
    }
    if let Err(e) = postflight(conn) {
        mark_stage(
            conn,
            &run_id,
            "postflight",
            "failed",
            None,
            Some(&e.to_string()),
        )?;
        return Err(e);
    }
    mark_stage(conn, &run_id, "postflight", "completed", None, None)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn migrate_v7_to_v8_fail_at(
    conn: &mut Connection,
    database_path: Option<&Path>,
    stage: &str,
) -> rusqlite::Result<()> {
    migrate_v7_to_v8_with_options(conn, database_path, Some(stage), None)
}

fn migrate_v7_to_v8_with_options(
    conn: &mut Connection,
    database_path: Option<&Path>,
    failure_stage: Option<&str>,
    available_override: Option<u64>,
) -> rusqlite::Result<()> {
    ensure_migration_journal_schema(conn)?;
    let run_id = format!(
        "v7-v8-{}-{}-{}",
        now_ms(),
        std::process::id(),
        MIGRATION_RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    conn.execute("UPDATE migration_journal SET status='interrupted', completed_at_ms=?1, error_text='previous migration was interrupted' WHERE to_version=8 AND status = 'started'", [now_ms()])?;
    start_run(conn, &run_id, 7, 8)?;
    mark_stage(conn, &run_id, "preflight", "started", None, None)?;
    let preflight = match preflight_with_available_space(conn, database_path, available_override) {
        Ok(value) => {
            mark_stage(conn, &run_id, "preflight", "completed", None, None)?;
            value
        }
        Err(error) => {
            mark_stage(
                conn,
                &run_id,
                "preflight",
                "failed",
                None,
                Some(&error.to_string()),
            )?;
            return Err(error);
        }
    };
    mark_stage(conn, &run_id, "backup", "started", None, None)?;
    let backup_path = database_path.map(|path| {
        path.parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(
                "{}.v8-backup-{}-{}.sqlite3",
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("resource-timeline.sqlite3"),
                now_ms(),
                std::process::id()
            ))
    });
    if let Some(path) = backup_path.as_deref() {
        if let Err(error) = create_consistent_backup(conn, path) {
            mark_stage(
                conn,
                &run_id,
                "backup",
                "failed",
                None,
                Some(&error.to_string()),
            )?;
            return Err(error);
        }
    }
    mark_stage(
        conn,
        &run_id,
        "backup",
        "completed",
        Some(&format!(
            "{{\"database_bytes\":{},\"wal_bytes\":{},\"shm_bytes\":{},\"available_bytes\":{},\"required_bytes\":{}}}",
            preflight.database_bytes,
            preflight.wal_bytes,
            preflight.shm_bytes,
            preflight
                .available_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".into()),
            preflight.required_bytes
        )),
        None,
    )?;

    let old_gpu_rows: i64 =
        conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| row.get(0))?;
    let mut current_stage = "create";
    let mut completed_stages = Vec::new();
    let result = (|| {
        let tx = conn.transaction()?;
        mark_stage_tx(&tx, &run_id, "create", "started", None, None)?;
        inject_failure(failure_stage, "create")?;
        ensure_v8_columns(&tx)?;
        tx.execute(
            "UPDATE gpu_sample SET power_scope = 'gpu_board' WHERE board_power_w IS NOT NULL AND power_scope IS NULL",
            [],
        )?;
        tx.execute_batch(V8_COMPATIBILITY_DDL)?;
        mark_stage_tx(&tx, &run_id, "create", "completed", None, None)?;
        completed_stages.push("create");

        current_stage = "identity_backfill";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);

        current_stage = "usage_backfill";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);

        current_stage = "resource_backfill";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);

        current_stage = "verify";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        verify_v8(&tx, old_gpu_rows)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);

        current_stage = "commit";
        mark_stage_tx(&tx, &run_id, current_stage, "started", None, None)?;
        inject_failure(failure_stage, current_stage)?;
        tx.pragma_update(None, "user_version", 8)?;
        mark_stage_tx(&tx, &run_id, current_stage, "completed", None, None)?;
        completed_stages.push(current_stage);
        tx.commit()?;
        Ok::<_, rusqlite::Error>(())
    })();
    if let Err(error) = result {
        for stage in &completed_stages {
            mark_stage(conn, &run_id, stage, "completed", None, None)?;
        }
        mark_stage(
            conn,
            &run_id,
            current_stage,
            "failed",
            None,
            Some(&error.to_string()),
        )?;
        return Err(error);
    }

    mark_stage(conn, &run_id, "postflight", "started", None, None)?;
    if let Err(error) = inject_failure(failure_stage, "postflight") {
        mark_stage(
            conn,
            &run_id,
            "postflight",
            "failed",
            None,
            Some(&error.to_string()),
        )?;
        return Err(error);
    }
    if let Err(error) = postflight_v8(conn) {
        mark_stage(
            conn,
            &run_id,
            "postflight",
            "failed",
            None,
            Some(&error.to_string()),
        )?;
        return Err(error);
    }
    mark_stage(conn, &run_id, "postflight", "completed", None, None)?;
    Ok(())
}

fn start_run(
    conn: &Connection,
    run_id: &str,
    from_version: i64,
    to_version: i64,
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    for stage in STAGES {
        tx.execute("INSERT INTO migration_journal(run_id,from_version,to_version,stage,status) VALUES (?1,?2,?3,?4,'pending')", params![run_id, from_version, to_version, *stage])?;
    }
    tx.commit()
}

fn ensure_migration_journal_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(MIGRATION_JOURNAL_DDL)?;
    let started_at_not_null = {
        let mut statement = conn.prepare("PRAGMA table_info(migration_journal)")?;
        let mut rows = statement.query([])?;
        let mut not_null = None;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == "started_at_ms" {
                not_null = Some(row.get::<_, i64>(3)? != 0);
                break;
            }
        }
        not_null.ok_or_else(|| invalid_schema("migration journal is missing started_at_ms"))?
    };
    if !started_at_not_null {
        return Ok(());
    }

    // Early PR-01 builds made pending stages carry a non-null start timestamp. Rebuild only
    // this internal journal table while preserving every row so a failed v6 migration remains
    // retryable without inventing timestamps for stages that never started.
    let legacy_name = format!(
        "migration_journal_legacy_{}_{}",
        std::process::id(),
        MIGRATION_RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "DROP INDEX IF EXISTS idx_migration_journal_run;
         DROP INDEX IF EXISTS idx_migration_journal_pending;",
    )?;
    tx.execute_batch(&format!(
        "ALTER TABLE migration_journal RENAME TO {legacy_name};"
    ))?;
    tx.execute_batch(MIGRATION_JOURNAL_DDL)?;
    tx.execute(
        &format!(
            "INSERT INTO migration_journal(
                 id,run_id,from_version,to_version,stage,status,started_at_ms,completed_at_ms,detail_json,error_text
             )
             SELECT id,run_id,from_version,to_version,stage,status,started_at_ms,completed_at_ms,detail_json,error_text
             FROM {legacy_name}"
        ),
        [],
    )?;
    tx.execute_batch(&format!("DROP TABLE {legacy_name};"))?;
    tx.commit()
}

fn mark_stage(
    conn: &Connection,
    run_id: &str,
    stage: &str,
    status: &str,
    detail: Option<&str>,
    error: Option<&str>,
) -> rusqlite::Result<()> {
    let timestamp = now_ms();
    let changed = conn.execute("UPDATE migration_journal SET status=?1,started_at_ms=CASE WHEN ?1='started' THEN COALESCE(started_at_ms,?2) ELSE started_at_ms END,completed_at_ms=CASE WHEN ?1 IN ('completed','failed','interrupted') THEN ?2 ELSE completed_at_ms END,detail_json=COALESCE(?3,detail_json),error_text=COALESCE(?4,error_text) WHERE run_id=?5 AND stage=?6",params![status,timestamp,detail,error,run_id,stage])?;
    if changed != 1 {
        return Err(invalid_schema(&format!(
            "migration journal stage not found: run_id={run_id}, stage={stage}"
        )));
    }
    Ok(())
}

fn mark_stage_tx(
    tx: &Transaction<'_>,
    run_id: &str,
    stage: &str,
    status: &str,
    detail: Option<&str>,
    error: Option<&str>,
) -> rusqlite::Result<()> {
    let timestamp = now_ms();
    let changed = tx.execute("UPDATE migration_journal SET status=?1,started_at_ms=CASE WHEN ?1='started' THEN COALESCE(started_at_ms,?2) ELSE started_at_ms END,completed_at_ms=CASE WHEN ?1 IN ('completed','failed','interrupted') THEN ?2 ELSE completed_at_ms END,detail_json=COALESCE(?3,detail_json),error_text=COALESCE(?4,error_text) WHERE run_id=?5 AND stage=?6",params![status,timestamp,detail,error,run_id,stage])?;
    if changed != 1 {
        return Err(invalid_schema(&format!(
            "migration journal stage not found: run_id={run_id}, stage={stage}"
        )));
    }
    Ok(())
}

fn rename_legacy_tables(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    for index in [
        "idx_foreground_single_open",
        "idx_foreground_interval_range",
        "idx_foreground_interval_app",
        "idx_system_sample_timestamp",
        "idx_app_resource_sample_system",
    ] {
        tx.execute_batch(&format!("DROP INDEX IF EXISTS {index};"))?;
    }
    for (from, to) in [
        ("app_identity", "legacy_v6_app_identity"),
        ("foreground_interval", "legacy_v6_foreground_interval"),
        ("system_sample", "legacy_v6_system_sample"),
        ("app_resource_sample", "legacy_v6_app_resource_sample"),
        ("app_resource_snapshot", "legacy_v6_app_resource_snapshot"),
    ] {
        if table_exists(tx, from)? {
            tx.execute_batch(&format!("ALTER TABLE {from} RENAME TO {to};"))?;
        }
    }
    Ok(())
}

fn ensure_legacy_session(
    tx: &Transaction<'_>,
    legacy: &LegacySummary,
) -> rusqlite::Result<(Option<i64>, Option<i64>)> {
    if legacy.app_rows + legacy.foreground_rows + legacy.frame_rows + legacy.process_rows == 0 {
        return Ok((None, None));
    }
    let start = [legacy.earliest_foreground_ms, legacy.earliest_frame_ms]
        .into_iter()
        .flatten()
        .min();
    let end = [legacy.latest_foreground_ms, legacy.latest_frame_ms]
        .into_iter()
        .flatten()
        .max();
    tx.execute("INSERT INTO boot_session(boot_id,boot_time_ms,observed_start_ms,observed_end_ms,shutdown_kind,created_at_ms) VALUES('legacy-v6',?1,?1,?2,'legacy-v6',?3) ON CONFLICT(boot_id) DO UPDATE SET observed_start_ms=excluded.observed_start_ms,observed_end_ms=excluded.observed_end_ms",params![start,end,now_ms()])?;
    let boot: i64 = tx.query_row(
        "SELECT id FROM boot_session WHERE boot_id='legacy-v6'",
        [],
        |r| r.get(0),
    )?;
    tx.execute("INSERT INTO collection_session(boot_session_id,started_at_ms,ended_at_ms,app_version,schema_version,config_hash) VALUES(?1,?2,?3,'legacy-v6',6,'legacy-v6') ON CONFLICT(boot_session_id,started_at_ms) DO NOTHING",params![boot,start.unwrap_or(0),end])?;
    let collection:i64=tx.query_row("SELECT id FROM collection_session WHERE boot_session_id=?1 AND app_version='legacy-v6' ORDER BY id LIMIT 1",[boot],|r|r.get(0))?;
    Ok((Some(boot), Some(collection)))
}

fn backfill_identity(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    if table_exists(tx, "legacy_v6_app_identity")? {
        tx.execute_batch(r#"
        INSERT INTO app(stable_key,process_name,display_name,publisher,is_hidden,first_seen_at_ms,last_seen_at_ms)
        SELECT identity_key,COALESCE(NULLIF(TRIM(process_name),''),'unresolved'),COALESCE(NULLIF(TRIM(display_name),''),'Unresolved'),publisher,COALESCE(is_hidden,0),first_seen_at_ms,last_seen_at_ms FROM legacy_v6_app_identity WHERE 1
        ON CONFLICT(stable_key) DO UPDATE SET process_name=COALESCE(excluded.process_name,app.process_name),publisher=excluded.publisher,is_hidden=excluded.is_hidden,first_seen_at_ms=MIN(app.first_seen_at_ms,excluded.first_seen_at_ms),last_seen_at_ms=MAX(app.last_seen_at_ms,excluded.last_seen_at_ms);
        INSERT INTO app_executable(app_id,normalized_path,first_seen_at_ms,last_seen_at_ms,source)
        SELECT a.id,CASE WHEN old.exe_path IS NOT NULL AND trim(old.exe_path)<>'' THEN 'path:'||lower(replace(trim(old.exe_path),'/','\\')) ELSE 'legacy:'||old.identity_key END,old.first_seen_at_ms,old.last_seen_at_ms,'legacy-v6'
        FROM legacy_v6_app_identity old JOIN app a ON a.stable_key=old.identity_key WHERE 1
        ON CONFLICT(app_id,normalized_path) DO UPDATE SET first_seen_at_ms=MIN(app_executable.first_seen_at_ms,excluded.first_seen_at_ms),last_seen_at_ms=MAX(app_executable.last_seen_at_ms,excluded.last_seen_at_ms);
        INSERT OR REPLACE INTO v7_legacy_identity_map(legacy_app_id,app_id,app_executable_id)
        SELECT old.id,a.id,e.id FROM legacy_v6_app_identity old JOIN app a ON a.stable_key=old.identity_key JOIN app_executable e ON e.app_id=a.id AND e.normalized_path=CASE WHEN old.exe_path IS NOT NULL AND trim(old.exe_path)<>'' THEN 'path:'||lower(replace(trim(old.exe_path),'/','\\')) ELSE 'legacy:'||old.identity_key END;
    "#)?;
    }
    if table_exists(tx, "legacy_v6_app_resource_sample")?
        && table_exists(tx, "legacy_v6_system_sample")?
    {
        tx.execute_batch(r#"
        INSERT INTO app(stable_key,process_name,display_name,first_seen_at_ms,last_seen_at_ms)
        SELECT old.app_key,COALESCE(MAX(NULLIF(TRIM(old.process_name),'')),'unresolved'),COALESCE(MAX(NULLIF(TRIM(old.process_name),'')),'Unresolved'),COALESCE(MIN(s.timestamp_ms),0),COALESCE(MAX(s.timestamp_ms),0) FROM legacy_v6_app_resource_sample old LEFT JOIN legacy_v6_system_sample s ON s.id=old.system_sample_id WHERE 1 GROUP BY old.app_key
        ON CONFLICT(stable_key) DO UPDATE SET process_name=COALESCE(excluded.process_name,app.process_name),first_seen_at_ms=MIN(app.first_seen_at_ms,excluded.first_seen_at_ms),last_seen_at_ms=MAX(app.last_seen_at_ms,excluded.last_seen_at_ms);
        INSERT INTO app_executable(app_id,normalized_path,first_seen_at_ms,last_seen_at_ms,source)
        SELECT a.id,CASE WHEN old.exe_path IS NOT NULL AND trim(old.exe_path)<>'' THEN 'path:'||lower(replace(trim(old.exe_path),'/','\\')) ELSE 'legacy:'||old.app_key END,COALESCE(MIN(s.timestamp_ms),0),COALESCE(MAX(s.timestamp_ms),0),'legacy-v6'
        FROM legacy_v6_app_resource_sample old JOIN app a ON a.stable_key=old.app_key LEFT JOIN legacy_v6_system_sample s ON s.id=old.system_sample_id WHERE 1 GROUP BY a.id,old.app_key,old.exe_path
        ON CONFLICT(app_id,normalized_path) DO UPDATE SET first_seen_at_ms=MIN(app_executable.first_seen_at_ms,excluded.first_seen_at_ms),last_seen_at_ms=MAX(app_executable.last_seen_at_ms,excluded.last_seen_at_ms);
    "#)?;
    }
    Ok(())
}

fn backfill_usage(tx: &Transaction<'_>, boot: Option<i64>) -> rusqlite::Result<()> {
    let Some(boot) = boot else {
        return Ok(());
    };
    if !table_exists(tx, "legacy_v6_foreground_interval")? {
        return Ok(());
    }
    tx.execute(r#"INSERT INTO foreground_interval(boot_session_id,app_executable_id,start_time_ms,end_time_ms,last_seen_time_ms,activity_state,close_reason,legacy_v6_id) SELECT ?1,m.app_executable_id,o.start_time_ms,o.end_time_ms,o.last_seen_time_ms,o.activity_state,o.end_reason,o.id FROM legacy_v6_foreground_interval o JOIN v7_legacy_identity_map m ON m.legacy_app_id=o.app_id WHERE o.end_time_ms IS NULL OR o.end_time_ms>=o.start_time_ms"#,[boot])?;
    Ok(())
}

fn backfill_resources(tx: &Transaction<'_>, collection: Option<i64>) -> rusqlite::Result<()> {
    let Some(collection) = collection else {
        return Ok(());
    };
    if !table_exists(tx, "legacy_v6_system_sample")? {
        return Ok(());
    }
    tx.execute(r#"INSERT INTO sample_frame(collection_session_id,ts,sequence,duration_ms,writer_delay_ms,process_snapshot_present,source,legacy_v6_system_sample_id) SELECT ?1,o.timestamp_ms,ROW_NUMBER() OVER(ORDER BY o.timestamp_ms,o.id),o.sample_duration_ms,NULL,0,'legacy-v6',o.id FROM legacy_v6_system_sample o WHERE 1 ON CONFLICT(legacy_v6_system_sample_id) DO NOTHING"#,[collection])?;
    tx.execute_batch(r#"INSERT OR REPLACE INTO v7_legacy_frame_map(legacy_system_sample_id,frame_id) SELECT o.id,f.id FROM legacy_v6_system_sample o JOIN sample_frame f ON f.legacy_v6_system_sample_id=o.id;
        INSERT INTO cpu_sample(frame_id,usage_pct,source) SELECT m.frame_id,o.cpu_percent,'legacy-v6' FROM legacy_v6_system_sample o JOIN v7_legacy_frame_map m ON m.legacy_system_sample_id=o.id WHERE o.cpu_percent IS NOT NULL;
        INSERT INTO memory_sample(frame_id,used_bytes,available_bytes,usage_pct,source) SELECT m.frame_id,o.memory_used_bytes,CASE WHEN o.memory_total_bytes IS NOT NULL AND o.memory_used_bytes IS NOT NULL THEN MAX(o.memory_total_bytes-o.memory_used_bytes,0) END,o.memory_percent,'legacy-v6' FROM legacy_v6_system_sample o JOIN v7_legacy_frame_map m ON m.legacy_system_sample_id=o.id WHERE o.memory_percent IS NOT NULL OR o.memory_used_bytes IS NOT NULL OR o.memory_total_bytes IS NOT NULL;
    "#)?;
    let has_disk_data: i64 = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM legacy_v6_system_sample WHERE disk_read_bytes_per_sec IS NOT NULL OR disk_write_bytes_per_sec IS NOT NULL)",
        [],
        |row| row.get(0),
    )?;
    if has_disk_data != 0 {
        tx.execute("INSERT INTO hardware_device(stable_key,category,first_seen_at_ms,last_seen_at_ms) SELECT 'legacy-v6:disk-total','disk',MIN(ts),MAX(ts) FROM sample_frame WHERE source='legacy-v6' ON CONFLICT(stable_key) DO NOTHING",[])?;
        tx.execute_batch(r#"INSERT OR IGNORE INTO disk_sample(frame_id,device_id,read_bps,write_bps) SELECT m.frame_id,d.id,o.disk_read_bytes_per_sec,o.disk_write_bytes_per_sec FROM legacy_v6_system_sample o JOIN v7_legacy_frame_map m ON m.legacy_system_sample_id=o.id JOIN hardware_device d ON d.stable_key='legacy-v6:disk-total' WHERE o.disk_read_bytes_per_sec IS NOT NULL OR o.disk_write_bytes_per_sec IS NOT NULL;"#)?;
    }
    if table_exists(tx, "legacy_v6_app_resource_snapshot")? {
        tx.execute("UPDATE sample_frame SET process_snapshot_present=1 WHERE legacy_v6_system_sample_id IN (SELECT system_sample_id FROM legacy_v6_app_resource_snapshot)",[])?;
    }
    if table_exists(tx, "legacy_v6_app_resource_sample")? {
        tx.execute_batch(r#"
        INSERT INTO process_instance(app_executable_id,stable_key,source)
        SELECT e.id,'legacy-v6:'||o.app_key||':'||CASE WHEN o.exe_path IS NOT NULL AND trim(o.exe_path)<>'' THEN 'path:'||lower(replace(trim(o.exe_path),'/','\\')) ELSE 'legacy:'||o.app_key END,'legacy-v6' FROM (SELECT DISTINCT app_key,exe_path FROM legacy_v6_app_resource_sample) o JOIN app a ON a.stable_key=o.app_key JOIN app_executable e ON e.app_id=a.id AND e.normalized_path=CASE WHEN o.exe_path IS NOT NULL AND trim(o.exe_path)<>'' THEN 'path:'||lower(replace(trim(o.exe_path),'/','\\')) ELSE 'legacy:'||o.app_key END WHERE 1 ON CONFLICT(stable_key) DO NOTHING;
        INSERT INTO process_sample(frame_id,process_instance_id,cpu_pct,working_set_bytes,process_count,read_bps,write_bps,source,legacy_v6_id)
        SELECT m.frame_id,i.id,o.cpu_percent,o.memory_used_bytes,o.process_count,o.io_read_bytes_per_sec,o.io_write_bytes_per_sec,'legacy-v6',o.id FROM legacy_v6_app_resource_sample o JOIN v7_legacy_frame_map m ON m.legacy_system_sample_id=o.system_sample_id JOIN process_instance i ON i.stable_key='legacy-v6:'||o.app_key||':'||CASE WHEN o.exe_path IS NOT NULL AND trim(o.exe_path)<>'' THEN 'path:'||lower(replace(trim(o.exe_path),'/','\\')) ELSE 'legacy:'||o.app_key END WHERE 1 ON CONFLICT(legacy_v6_id) DO NOTHING;
        UPDATE sample_frame SET process_snapshot_present=1 WHERE legacy_v6_system_sample_id IN (SELECT system_sample_id FROM legacy_v6_app_resource_sample);
    "#)?;
    }
    Ok(())
}

fn verify_v7(
    tx: &Transaction<'_>,
    legacy: &LegacySummary,
) -> rusqlite::Result<MigrationVerification> {
    if let Some(e) = foreign_key_error(tx)? {
        return Err(invalid_schema(&format!(
            "foreign_key_check failed after backfill: {e}"
        )));
    }
    let foreground_rows: i64 =
        tx.query_row("SELECT COUNT(*) FROM foreground_interval", [], |r| r.get(0))?;
    let frame_rows: i64 = tx.query_row("SELECT COUNT(*) FROM sample_frame", [], |r| r.get(0))?;
    let process_rows: i64 =
        tx.query_row("SELECT COUNT(*) FROM process_sample", [], |r| r.get(0))?;
    let app_rows: i64 = tx.query_row("SELECT COUNT(*) FROM app", [], |r| r.get(0))?;
    if app_rows < legacy.app_rows
        || foreground_rows != legacy.foreground_rows
        || frame_rows != legacy.frame_rows
        || process_rows != legacy.process_rows
    {
        return Err(invalid_schema("v6 to v7 row-count verification failed"));
    }
    let fg = range(
        tx,
        "foreground_interval",
        "start_time_ms",
        "COALESCE(end_time_ms,last_seen_time_ms)",
    )?;
    let frames = range(tx, "sample_frame", "ts", "ts")?;
    if fg != (legacy.earliest_foreground_ms, legacy.latest_foreground_ms)
        || frames != (legacy.earliest_frame_ms, legacy.latest_frame_ms)
    {
        return Err(invalid_schema("v6 to v7 time-range verification failed"));
    }
    let process_cpu_total = sum_f64(tx, "process_sample", "cpu_pct")?;
    if sum_i64(tx, "memory_sample", "used_bytes")? != legacy.memory_total
        || sum_i64(tx, "disk_sample", "read_bps")? != legacy.disk_read_total
        || sum_i64(tx, "disk_sample", "write_bps")? != legacy.disk_write_total
        || !same_optional_f64(process_cpu_total, legacy.process_cpu_total)
        || sum_i64(tx, "process_sample", "working_set_bytes")? != legacy.process_memory_total
        || sum_i64(tx, "process_sample", "read_bps")? != legacy.process_read_total
        || sum_i64(tx, "process_sample", "write_bps")? != legacy.process_write_total
    {
        return Err(invalid_schema("v6 to v7 total verification failed"));
    }
    Ok(MigrationVerification {
        foreground_rows,
        frame_rows,
        process_rows,
        earliest_foreground_ms: fg.0,
        latest_foreground_ms: fg.1,
        earliest_frame_ms: frames.0,
        latest_frame_ms: frames.1,
    })
}

fn postflight(conn: &Connection) -> rusqlite::Result<()> {
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if v != 7 {
        return Err(invalid_schema("postflight user_version is not 7"));
    }
    verify_database_integrity(conn)
}

fn postflight_v8(conn: &Connection) -> rusqlite::Result<()> {
    let v: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if v != 8 {
        return Err(invalid_schema("postflight user_version is not 8"));
    }
    verify_database_integrity(conn)
}

fn validate_v7_open(conn: &mut Connection) -> rusqlite::Result<()> {
    ensure_v7_compatibility(conn)?;
    let latest_postflight = latest_postflight(conn, 7)?;
    match verify_database_integrity(conn) {
        Ok(()) => {
            if let Some((run_id, status)) = latest_postflight {
                if status != "completed" {
                    mark_stage(
                        conn,
                        &run_id,
                        "postflight",
                        "completed",
                        Some("revalidated successfully on database open"),
                        None,
                    )?;
                }
            }
            Ok(())
        }
        Err(error) => {
            if let Some((run_id, _)) = latest_postflight {
                mark_stage(
                    conn,
                    &run_id,
                    "postflight",
                    "failed",
                    None,
                    Some(&error.to_string()),
                )?;
            }
            Err(error)
        }
    }
}

fn validate_v8_open(conn: &mut Connection) -> rusqlite::Result<()> {
    ensure_v7_compatibility(conn)?;
    ensure_v8_compatibility(conn)?;
    let latest_postflight = latest_postflight(conn, 8)?;
    match verify_database_integrity(conn) {
        Ok(()) => {
            if let Some((run_id, status)) = latest_postflight {
                if status != "completed" {
                    mark_stage(
                        conn,
                        &run_id,
                        "postflight",
                        "completed",
                        Some("revalidated successfully on database open"),
                        None,
                    )?;
                }
            }
            Ok(())
        }
        Err(error) => {
            if let Some((run_id, _)) = latest_postflight {
                mark_stage(
                    conn,
                    &run_id,
                    "postflight",
                    "failed",
                    None,
                    Some(&error.to_string()),
                )?;
            }
            Err(error)
        }
    }
}

fn ensure_v7_compatibility(conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    if table_exists(&tx, "app")? && !has_column(&tx, "app", "process_name")? {
        tx.execute("ALTER TABLE app ADD COLUMN process_name TEXT", [])?;
    }
    if table_exists(&tx, "app")?
        && table_exists(&tx, "legacy_v6_app_identity")?
        && table_exists(&tx, "v7_legacy_identity_map")?
    {
        tx.execute(
            "UPDATE app
             SET process_name = COALESCE(process_name, (
                 SELECT NULLIF(TRIM(old.process_name), '')
                 FROM legacy_v6_app_identity old
                 JOIN v7_legacy_identity_map map ON map.legacy_app_id = old.id
                 WHERE map.app_id = app.id
                 ORDER BY old.id
                 LIMIT 1
             ))
             WHERE process_name IS NULL",
            [],
        )?;
    }
    if table_exists(&tx, "app")? && table_exists(&tx, "legacy_v6_app_resource_sample")? {
        tx.execute(
            "UPDATE app
             SET process_name = COALESCE(process_name, (
                 SELECT NULLIF(TRIM(old.process_name), '')
                 FROM legacy_v6_app_resource_sample old
                 WHERE old.app_key = app.stable_key
                 ORDER BY old.id
                 LIMIT 1
             ))
             WHERE process_name IS NULL",
            [],
        )?;
    }
    if table_exists(&tx, "app")? {
        tx.execute(
            "UPDATE app SET process_name = NULLIF(substr(stable_key, 6), '')
             WHERE process_name IS NULL AND stable_key LIKE 'name:%'",
            [],
        )?;
    }
    let missing_process_name: i64 = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM app WHERE process_name IS NULL OR trim(process_name) = '')",
        [],
        |row| row.get(0),
    )?;
    if missing_process_name != 0 {
        return Err(invalid_schema(
            "v7 app rows are missing process_name and no authoritative legacy source is available",
        ));
    }
    tx.execute_batch(V7_COMPATIBILITY_DDL)?;
    repair_open_computer_state_intervals(&tx)?;
    install_v7_nullable_constraints(&tx)?;
    tx.commit()
}

fn ensure_v8_compatibility(conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    ensure_v8_columns(&tx)?;
    tx.execute(
        "UPDATE gpu_sample SET power_scope = 'gpu_board' WHERE board_power_w IS NOT NULL AND power_scope IS NULL",
        [],
    )?;
    tx.execute_batch(V8_COMPATIBILITY_DDL)?;
    tx.commit()
}

fn ensure_v8_columns(conn: &Connection) -> rusqlite::Result<()> {
    if !table_exists(conn, "gpu_sample")? {
        return Err(invalid_schema("schema v8 is missing gpu_sample"));
    }
    for (column, definition) in [
        ("memory_controller_usage_pct", "REAL"),
        ("memory_clock_mhz", "REAL"),
        ("vram_total_bytes", "INTEGER"),
        ("power_scope", "TEXT"),
    ] {
        if !has_column(conn, "gpu_sample", column)? {
            conn.execute(
                &format!("ALTER TABLE gpu_sample ADD COLUMN {column} {definition}"),
                [],
            )?;
        }
    }
    Ok(())
}

fn verify_v8(tx: &Transaction<'_>, expected_gpu_rows: i64) -> rusqlite::Result<()> {
    if let Some(error) = foreign_key_error(tx)? {
        return Err(invalid_schema(&format!(
            "foreign_key_check failed during v7 to v8 migration: {error}"
        )));
    }
    for column in [
        "memory_controller_usage_pct",
        "memory_clock_mhz",
        "vram_total_bytes",
        "power_scope",
    ] {
        if !has_column(tx, "gpu_sample", column)? {
            return Err(invalid_schema(&format!(
                "gpu_sample is missing v8 column {column}"
            )));
        }
    }
    let gpu_rows: i64 = tx.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| row.get(0))?;
    if gpu_rows != expected_gpu_rows {
        return Err(invalid_schema("v7 to v8 GPU row-count verification failed"));
    }
    let invalid_power_scope: i64 = tx.query_row(
        "SELECT COUNT(*) FROM gpu_sample WHERE NOT ((board_power_w IS NULL AND power_scope IS NULL) OR (board_power_w IS NOT NULL AND power_scope = 'gpu_board'))",
        [],
        |row| row.get(0),
    )?;
    if invalid_power_scope != 0 {
        return Err(invalid_schema(
            "v7 to v8 GPU power scope verification failed",
        ));
    }
    Ok(())
}

fn repair_open_computer_state_intervals(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE computer_state_interval AS current
         SET end_ts = MAX(
             current.start_ts,
             (
                 SELECT MIN(next.start_ts)
                 FROM computer_state_interval AS next
                 WHERE next.boot_session_id = current.boot_session_id
                   AND next.end_ts IS NULL
                   AND next.id > current.id
             )
         )
         WHERE current.end_ts IS NULL
           AND EXISTS (
               SELECT 1
               FROM computer_state_interval AS next
               WHERE next.boot_session_id = current.boot_session_id
                 AND next.end_ts IS NULL
                 AND next.id > current.id
           )",
        [],
    )?;
    tx.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_computer_state_single_open
         ON computer_state_interval(boot_session_id)
         WHERE end_ts IS NULL;",
    )?;
    Ok(())
}

fn install_v7_nullable_constraints(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    let provider_has_duplicates: i64 = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM provider WHERE version IS NULL GROUP BY kind, name HAVING COUNT(*) > 1)",
        [],
        |row| row.get(0),
    )?;
    if provider_has_duplicates != 0 {
        return Err(invalid_schema(
            "provider contains duplicate rows with NULL version",
        ));
    }
    let metric_has_duplicates: i64 = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM collection_session_metric WHERE device_id IS NULL GROUP BY session_id, metric_key HAVING COUNT(*) > 1)",
        [],
        |row| row.get(0),
    )?;
    if metric_has_duplicates != 0 {
        return Err(invalid_schema(
            "collection_session_metric contains duplicate system-level rows",
        ));
    }
    let rollup_has_duplicates: i64 = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM system_rollup_1m WHERE device_id IS NULL GROUP BY bucket_start_ms, metric_key HAVING COUNT(*) > 1)",
        [],
        |row| row.get(0),
    )?;
    if rollup_has_duplicates != 0 {
        return Err(invalid_schema(
            "system_rollup_1m contains duplicate system-level rows",
        ));
    }
    tx.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_kind_name_version ON provider(kind, name, version) WHERE version IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_provider_kind_name_without_version ON provider(kind, name) WHERE version IS NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_collection_session_metric_device ON collection_session_metric(session_id, metric_key, device_id) WHERE device_id IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_collection_session_metric_system ON collection_session_metric(session_id, metric_key) WHERE device_id IS NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_system_rollup_1m_device ON system_rollup_1m(bucket_start_ms, metric_key, device_id) WHERE device_id IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_system_rollup_1m_system ON system_rollup_1m(bucket_start_ms, metric_key) WHERE device_id IS NULL;",
    )?;
    Ok(())
}

fn latest_postflight(
    conn: &Connection,
    to_version: i64,
) -> rusqlite::Result<Option<(String, String)>> {
    if !table_exists(conn, "migration_journal")? {
        return Ok(None);
    }
    conn.query_row(
        "SELECT run_id, status
         FROM migration_journal
         WHERE to_version = ?1 AND stage = 'postflight'
         ORDER BY id DESC
         LIMIT 1",
        [to_version],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
}

fn has_column(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    let mut statement = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn inject_failure(failure_stage: Option<&str>, stage: &str) -> rusqlite::Result<()> {
    if failure_stage == Some(stage) {
        return Err(invalid_schema(&format!(
            "injected migration failure at stage {stage}"
        )));
    }
    Ok(())
}

fn verify_database_integrity(conn: &Connection) -> rusqlite::Result<()> {
    check_pragma_ok(conn, "quick_check")?;
    check_pragma_ok(conn, "integrity_check")?;
    if let Some(e) = foreign_key_error(conn)? {
        return Err(invalid_schema(&format!("foreign_key_check failed: {e}")));
    }
    Ok(())
}

fn legacy_summary(conn: &Connection) -> rusqlite::Result<LegacySummary> {
    let mut s = LegacySummary::default();
    if table_exists(conn, "app_identity")? {
        s.app_rows = count_rows(conn, "app_identity")?;
    }
    if table_exists(conn, "foreground_interval")? {
        s.foreground_rows = count_rows(conn, "foreground_interval")?;
        (s.earliest_foreground_ms, s.latest_foreground_ms) = range(
            conn,
            "foreground_interval",
            "start_time_ms",
            "COALESCE(end_time_ms,last_seen_time_ms)",
        )?;
    }
    if table_exists(conn, "system_sample")? {
        s.frame_rows = count_rows(conn, "system_sample")?;
        (s.earliest_frame_ms, s.latest_frame_ms) =
            range(conn, "system_sample", "timestamp_ms", "timestamp_ms")?;
        s.memory_total = sum_i64(conn, "system_sample", "memory_used_bytes")?;
        s.disk_read_total = sum_i64(conn, "system_sample", "disk_read_bytes_per_sec")?;
        s.disk_write_total = sum_i64(conn, "system_sample", "disk_write_bytes_per_sec")?;
    }
    if table_exists(conn, "app_resource_sample")? {
        s.process_rows = count_rows(conn, "app_resource_sample")?;
        s.process_cpu_total = sum_f64(conn, "app_resource_sample", "cpu_percent")?;
        s.process_memory_total = sum_i64(conn, "app_resource_sample", "memory_used_bytes")?;
        s.process_read_total = sum_i64(conn, "app_resource_sample", "io_read_bytes_per_sec")?;
        s.process_write_total = sum_i64(conn, "app_resource_sample", "io_write_bytes_per_sec")?;
    }
    Ok(s)
}
fn check_pragma_ok(conn: &Connection, pragma: &str) -> rusqlite::Result<()> {
    let mut st = conn.prepare(&format!("PRAGMA {pragma}"))?;
    let mut rows = st.query([])?;
    let value: Option<String> = rows.next()?.map(|r| r.get(0)).transpose()?;
    if value.as_deref() != Some("ok") {
        return Err(invalid_schema(&format!(
            "PRAGMA {pragma} failed: {}",
            value.unwrap_or_default()
        )));
    }
    Ok(())
}
fn foreign_key_error(conn: &Connection) -> rusqlite::Result<Option<String>> {
    let mut st = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = st.query([])?;
    if let Some(row) = rows.next()? {
        let table: String = row.get(0)?;
        let id: Option<i64> = row.get(1).ok();
        let parent: Option<String> = row.get(2).ok();
        return Ok(Some(format!(
            "table={table}, rowid={id:?}, parent={parent:?}"
        )));
    }
    Ok(None)
}
fn count_rows(conn: &Connection, table: &str) -> rusqlite::Result<i64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
}
fn range(
    conn: &Connection,
    table: &str,
    min_expr: &str,
    max_expr: &str,
) -> rusqlite::Result<(Option<i64>, Option<i64>)> {
    conn.query_row(
        &format!("SELECT MIN({min_expr}),MAX({max_expr}) FROM {table}"),
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
}
fn sum_i64(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(&format!("SELECT SUM({column}) FROM {table}"), [], |r| {
        r.get(0)
    })
}
fn sum_f64(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<Option<f64>> {
    conn.query_row(&format!("SELECT SUM({column}) FROM {table}"), [], |r| {
        r.get(0)
    })
}
fn same_optional_f64(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            let tolerance = 1e-9 * left.abs().max(right.abs()).max(1.0);
            (left - right).abs() <= tolerance
        }
        _ => false,
    }
}
fn table_exists(conn: &Connection, table: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |r| r.get(0),
    )
}
fn configure_connection(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}
fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub fn database_size_bytes(path: &Path) -> u64 {
    file_size(path)
        .saturating_add(file_size(&sidecar(path, "-wal")))
        .saturating_add(file_size(&sidecar(path, "-shm")))
}

fn required_migration_space(database_bytes: u64, wal_bytes: u64, shm_bytes: u64) -> u64 {
    const MINIMUM_MIGRATION_SPACE: u64 = 1 << 20;
    let live_bytes = database_bytes
        .saturating_add(wal_bytes)
        .saturating_add(shm_bytes);
    live_bytes
        .saturating_add(database_bytes.max(1))
        .max(MINIMUM_MIGRATION_SPACE)
}

#[cfg(windows)]
fn available_space_bytes(directory: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{core::PCWSTR, Win32::Storage::FileSystem::GetDiskFreeSpaceExW};

    let wide: Vec<u16> = directory
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free = 0;
    unsafe {
        GetDiskFreeSpaceExW(PCWSTR::from_raw(wide.as_ptr()), Some(&mut free), None, None)
            .ok()
            .map(|_| free)
    }
}

#[cfg(not(windows))]
fn available_space_bytes(_directory: &Path) -> Option<u64> {
    None
}

pub(crate) fn database_directory(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .or_else(|| Some(Path::new(".")))
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", path.display(), suffix))
}
fn connection_main_path(conn: &Connection) -> Option<PathBuf> {
    let mut st = conn.prepare("PRAGMA database_list").ok()?;
    let mut rows = st.query([]).ok()?;
    while let Some(r) = rows.next().ok()? {
        if r.get::<_, String>(1).ok()? == "main" {
            let p: String = r.get(2).ok()?;
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}
fn invalid_schema(message: &str) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.to_string(),
    )))
}
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GPU_BOARD_POWER_SCOPE;
    use rusqlite::{params, Connection};

    fn test_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!(
            "resource-timeline-schema-{name}-{}-{nonce}.sqlite3",
            std::process::id(),
        ))
    }

    fn cleanup(path: &Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = fs::remove_file(PathBuf::from(format!("{}{}", path.display(), suffix)));
        }
        let Some(parent) = path.parent() else {
            return;
        };
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            return;
        };
        let prefixes = [format!("{name}.v8-backup-")];
        if let Ok(entries) = fs::read_dir(parent) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|value| prefixes.iter().any(|prefix| value.starts_with(prefix)))
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }

    fn create_v7_gpu_fixture(path: &Path) {
        let conn = Connection::open(path).unwrap();
        configure_connection(&conn).unwrap();
        conn.execute_batch(SCHEMA_V7).unwrap();
        conn.execute_batch(V7_COMPATIBILITY_DDL).unwrap();
        conn.execute(
            "INSERT INTO boot_session(boot_id, boot_time_ms, created_at_ms) VALUES ('test-boot', 1000, 1000)",
            [],
        )
        .unwrap();
        let boot_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO collection_session(boot_session_id, started_at_ms, app_version, schema_version) VALUES (?1, 1000, 'test', 7)",
            [boot_id],
        )
        .unwrap();
        let session_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO hardware_device(stable_key, category, vendor, model, capacity_bytes, first_seen_at_ms, last_seen_at_ms) VALUES ('runtime:gpu:legacy', 'gpu', 'NVIDIA', 'Legacy GPU', 1024, 1000, 1000)",
            [],
        )
        .unwrap();
        let device_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO sample_frame(collection_session_id, ts, sequence, duration_ms) VALUES (?1, 1000, 1, 2000)",
            [session_id],
        )
        .unwrap();
        let frame_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO gpu_sample(frame_id, device_id, usage_pct, temp_c, board_power_w, core_clock_mhz, vram_used_bytes) VALUES (?1, ?2, 0.0, 40.0, 100.0, 2000.0, 0)",
            params![frame_id, device_id],
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 7).unwrap();
    }

    #[test]
    fn v7_to_v8_gpu_migration_preserves_rows_and_adds_contract() {
        let path = test_path("gpu-migration");
        cleanup(&path);
        create_v7_gpu_fixture(&path);

        let mut conn = Connection::open(&path).unwrap();
        migrate_v7_to_v8(&mut conn, Some(&path)).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            8
        );
        for column in [
            "memory_controller_usage_pct",
            "memory_clock_mhz",
            "vram_total_bytes",
            "power_scope",
        ] {
            assert!(has_column(&conn, "gpu_sample", column).unwrap());
        }
        assert_eq!(
            conn.query_row(
                "SELECT usage_pct, vram_used_bytes, power_scope FROM gpu_sample",
                [],
                |row| {
                    Ok((
                        row.get::<_, f64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap(),
            (0.0, 0, GPU_BOARD_POWER_SCOPE.to_string())
        );
        assert_eq!(
            conn.query_row("SELECT schema_version FROM collection_session", [], |row| {
                row.get::<_, i64>(0)
            },)
                .unwrap(),
            7
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_gpu_sample_device_frame'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
            1
        );
        verify_database_integrity(&conn).unwrap();
        drop(conn);
        cleanup(&path);
    }

    #[test]
    fn v7_to_v8_gpu_migration_failure_rolls_back_and_retries() {
        let path = test_path("gpu-migration-retry");
        cleanup(&path);
        create_v7_gpu_fixture(&path);

        let mut conn = Connection::open(&path).unwrap();
        assert!(migrate_v7_to_v8_fail_at(&mut conn, Some(&path), "create").is_err());
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            7
        );
        assert!(!has_column(&conn, "gpu_sample", "memory_controller_usage_pct").unwrap());
        assert_eq!(
            conn.query_row(
                "SELECT status FROM migration_journal WHERE to_version = 8 AND stage = 'create' ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
            "failed"
        );

        migrate_v7_to_v8(&mut conn, Some(&path)).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            8
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        verify_database_integrity(&conn).unwrap();
        drop(conn);
        cleanup(&path);
    }
}
