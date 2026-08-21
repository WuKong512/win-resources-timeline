use crate::{
    collector::system_metrics::now_ms,
    db::{query, writer},
    error::{AppError, CommandError},
    models::{AppIdentity, CollectionSettings, CollectorStatus, StorageUsage},
    AppState,
};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub fn list_apps(state: State<'_, AppState>) -> Result<Vec<AppIdentity>, CommandError> {
    state.db.read(query::list_apps).map_err(Into::into)
}

#[tauri::command]
pub fn set_app_hidden(
    state: State<'_, AppState>,
    app_id: i64,
    hidden: bool,
) -> Result<(), CommandError> {
    state
        .db
        .with_writer(|conn| writer::set_app_hidden(conn, app_id, hidden))
        .map_err(Into::into)
}

#[tauri::command]
pub fn get_collector_status(state: State<'_, AppState>) -> CollectorStatus {
    state.collector.status(state.db.size_bytes())
}

#[tauri::command]
pub fn get_storage_usage(state: State<'_, AppState>) -> StorageUsage {
    state.db.storage_usage()
}

#[tauri::command]
pub fn set_collection_paused(state: State<'_, AppState>, paused: bool) -> Result<(), CommandError> {
    state
        .collector
        .set_paused(paused)
        .map_err(|e| AppError::Other(e).into())
}

#[tauri::command]
pub fn get_collection_settings(
    state: State<'_, AppState>,
) -> Result<CollectionSettings, CommandError> {
    state
        .db
        .read(writer::collection_settings)
        .map_err(Into::into)
}

#[tauri::command]
pub fn set_collection_settings(
    state: State<'_, AppState>,
    settings: CollectionSettings,
) -> Result<(), CommandError> {
    settings.validate().map_err(AppError::InvalidRequest)?;
    state
        .collector
        .update_settings(settings)
        .map_err(|error| AppError::Other(error).into())
}

#[tauri::command]
pub fn get_autostart_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, CommandError> {
    let preferred = state
        .db
        .with_writer(writer::start_with_windows)
        .map_err(CommandError::from)?;
    let autolaunch = app.autolaunch();
    let actual = autolaunch
        .is_enabled()
        .map_err(|e| AppError::Other(e.to_string()))?;
    if preferred != actual {
        let result = if preferred {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        result.map_err(|e| AppError::Other(e.to_string()))?;
    }
    if preferred {
        crate::platform::refresh_autostart_command().map_err(AppError::Other)?;
    }
    Ok(preferred)
}

#[tauri::command]
pub fn set_autostart_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), CommandError> {
    let result = if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    result.map_err(|e| AppError::Other(e.to_string()))?;
    if enabled {
        crate::platform::refresh_autostart_command().map_err(AppError::Other)?;
    }
    state
        .db
        .with_writer(|conn| writer::save_start_with_windows(conn, enabled, now_ms()))
        .map_err(Into::into)
}

#[tauri::command]
pub fn clear_collected_data(state: State<'_, AppState>) -> Result<(), CommandError> {
    state
        .collector
        .clear()
        .map_err(|e| AppError::Other(e).into())
}
