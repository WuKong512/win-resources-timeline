#![allow(dead_code)]

use rusqlite::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RollupWindow {
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RollupMaintenanceStatus {
    pub last_run_at_ms: Option<i64>,
    pub last_duration_ms: Option<u64>,
    pub last_system_rows: u64,
    pub last_process_rows: u64,
}

pub const ROLLUP_TABLES: &[&str] = &[
    "system_rollup_1m",
    "process_rollup_1m",
    "process_rollup_1h",
    "app_usage_daily",
    "app_resource_daily",
    "energy_rollup_daily",
];

pub fn pending_frame_window(
    conn: &Connection,
    after_ms: Option<i64>,
) -> rusqlite::Result<Option<RollupWindow>> {
    let (start, end): (Option<i64>, Option<i64>) = conn.query_row(
        "SELECT MIN(ts), MAX(ts) FROM sample_frame WHERE (?1 IS NULL OR ts > ?1)",
        [after_ms],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(start
        .zip(end)
        .map(|(start_ms, end_ms)| RollupWindow { start_ms, end_ms }))
}

pub fn maintenance_status() -> RollupMaintenanceStatus {
    RollupMaintenanceStatus::default()
}
