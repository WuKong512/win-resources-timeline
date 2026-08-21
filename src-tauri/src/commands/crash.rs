use crate::{
    crash::{
        build_case_with_failure_status, get_crash_case_detail as load_crash_case_detail,
        list_crash_cases as load_crash_cases, release_case_hold, CrashCaseSummary,
        CrashDetectorStatus, CrashEvidenceDetail,
    },
    error::CommandError,
    AppState,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

#[tauri::command]
pub fn get_crash_detector_status(state: State<'_, AppState>) -> CrashDetectorStatus {
    state.crash_detector.status()
}

#[tauri::command]
pub fn list_crash_cases(state: State<'_, AppState>) -> Result<Vec<CrashCaseSummary>, CommandError> {
    state.db.read(load_crash_cases).map_err(Into::into)
}

#[tauri::command]
pub fn get_crash_case_detail(
    state: State<'_, AppState>,
    case_id: i64,
) -> Result<CrashEvidenceDetail, CommandError> {
    state
        .db
        .read(|conn| load_crash_case_detail(conn, case_id))
        .map_err(Into::into)
}

#[tauri::command]
pub fn rebuild_crash_case(state: State<'_, AppState>, case_id: i64) -> Result<(), CommandError> {
    build_case_with_failure_status(&state.db, case_id).map_err(Into::into)
}

#[tauri::command]
pub fn release_crash_case_hold(
    state: State<'_, AppState>,
    case_id: i64,
) -> Result<(), CommandError> {
    let released_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);
    state
        .db
        .with_writer(|conn| release_case_hold(conn, case_id, released_at_ms))
        .map(|_| ())
        .map_err(Into::into)
}
