use crate::{db::query, error::CommandError, models::ForegroundInterval, AppState};
use tauri::State;

#[tauri::command]
pub fn get_app_usage_timeline(
    state: State<'_, AppState>,
    start_ms: i64,
    end_ms: i64,
    include_hidden: bool,
    include_idle: bool,
) -> Result<Vec<ForegroundInterval>, CommandError> {
    state
        .db
        .read(|conn| {
            query::foreground_intervals(conn, start_ms, end_ms, include_hidden, include_idle)
        })
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_timeline_available_dates(
    state: State<'_, AppState>,
) -> Result<Vec<String>, CommandError> {
    state
        .db
        .read(query::timeline_available_dates)
        .map_err(Into::into)
}
