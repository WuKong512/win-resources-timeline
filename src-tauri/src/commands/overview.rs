use crate::{
    db::query,
    error::CommandError,
    models::{
        AppResourceHistoryPoint, AppResourceSample, DailyUsageSummary, ResourceApp, SystemSample,
        TodayOverview,
    },
    AppState,
};
use tauri::State;

#[tauri::command]
pub fn get_today_overview(
    state: State<'_, AppState>,
    start_ms: i64,
    end_ms: i64,
) -> Result<TodayOverview, CommandError> {
    state
        .db
        .read(|conn| query::today_overview(conn, start_ms, end_ms))
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_daily_usage_summary(
    state: State<'_, AppState>,
    local_date: String,
    include_hidden: bool,
) -> Result<Vec<DailyUsageSummary>, CommandError> {
    if local_date.trim().is_empty() {
        return Err(crate::error::AppError::InvalidRequest("localDate is required".into()).into());
    }
    state
        .db
        .read(|conn| query::daily_usage_summary(conn, &local_date, include_hidden))
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_resource_apps(state: State<'_, AppState>) -> Result<Vec<ResourceApp>, CommandError> {
    state.db.read(query::resource_apps).map_err(Into::into)
}

#[tauri::command]
pub fn get_overview_available_dates(
    state: State<'_, AppState>,
) -> Result<Vec<String>, CommandError> {
    state
        .db
        .read(query::overview_available_dates)
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_resource_available_dates(
    state: State<'_, AppState>,
) -> Result<Vec<String>, CommandError> {
    state
        .db
        .read(query::resource_available_dates)
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_app_resource_available_dates(
    state: State<'_, AppState>,
    app_key: String,
) -> Result<Vec<String>, CommandError> {
    if app_key.trim().is_empty() {
        return Err(crate::error::AppError::InvalidRequest("appKey is required".into()).into());
    }
    state
        .db
        .read(|conn| query::app_resource_available_dates(conn, &app_key))
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_app_resource_history(
    state: State<'_, AppState>,
    app_key: String,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> Result<Vec<AppResourceHistoryPoint>, CommandError> {
    if app_key.trim().is_empty() {
        return Err(crate::error::AppError::InvalidRequest("appKey is required".into()).into());
    }
    if !(500..=10_000).contains(&max_points) {
        return Err(crate::error::AppError::InvalidRequest(
            "maxPoints must be between 500 and 10000".into(),
        )
        .into());
    }
    state
        .db
        .read(|conn| query::app_resource_history(conn, &app_key, start_ms, end_ms, max_points))
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_app_resource_samples(
    state: State<'_, AppState>,
    timestamp_ms: i64,
) -> Result<Vec<AppResourceSample>, CommandError> {
    state
        .db
        .read(|conn| query::app_resource_samples(conn, timestamp_ms))
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_system_samples(
    state: State<'_, AppState>,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
) -> Result<Vec<SystemSample>, CommandError> {
    if !(500..=10_000).contains(&max_points) {
        return Err(crate::error::AppError::InvalidRequest(
            "maxPoints must be between 500 and 10000".into(),
        )
        .into());
    }
    state
        .db
        .read(|conn| query::system_samples(conn, start_ms, end_ms, max_points))
        .map_err(Into::into)
}
