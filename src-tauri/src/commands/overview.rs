use crate::{
    db::query,
    error::CommandError,
    models::{
        AppResourceHistoryPoint, AppResourceSample, DailyUsageSummary, GpuSamplePoint, ResourceApp,
        SystemSample, TodayOverview,
    },
    AppState,
};
use tauri::State;

const GPU_QUERY_MIN_POINTS: usize = 500;
const GPU_QUERY_MAX_POINTS: usize = 10_000;

fn validate_gpu_max_points(max_points: usize) -> Result<(), CommandError> {
    if !(GPU_QUERY_MIN_POINTS..=GPU_QUERY_MAX_POINTS).contains(&max_points) {
        return Err(crate::error::AppError::InvalidRequest(
            "maxPoints must be between 500 and 10000".into(),
        )
        .into());
    }
    Ok(())
}

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

#[tauri::command]
pub fn get_gpu_samples(
    state: State<'_, AppState>,
    start_ms: i64,
    end_ms: i64,
    max_points: usize,
    device_key: Option<String>,
) -> Result<Vec<GpuSamplePoint>, CommandError> {
    validate_gpu_max_points(max_points)?;
    if device_key
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(crate::error::AppError::InvalidRequest(
            "deviceKey must be non-empty when provided".into(),
        )
        .into());
    }
    state
        .db
        .read(|conn| query::gpu_samples(conn, start_ms, end_ms, max_points, device_key.as_deref()))
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    #[test]
    fn gpu_query_max_points_is_bounded() {
        assert!(super::validate_gpu_max_points(0).is_err());
        assert!(super::validate_gpu_max_points(499).is_err());
        assert!(super::validate_gpu_max_points(500).is_ok());
        assert!(super::validate_gpu_max_points(10_000).is_ok());
        assert!(super::validate_gpu_max_points(10_001).is_err());
    }
}
