mod app_lifecycle;
mod collector;
mod commands;
mod crash;
mod db;
mod error;
mod models;
mod platform;

use collector::manager::CollectorManager;
use crash::CrashDetectorHandle;
#[cfg(all(not(debug_assertions), not(feature = "qualification")))]
use db::writer;
use db::Database;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
#[cfg(all(not(debug_assertions), not(feature = "qualification")))]
use tauri_plugin_autostart::ManagerExt;

pub struct AppState {
    pub db: Arc<Database>,
    pub collector: CollectorManager,
    pub crash_detector: CrashDetectorHandle,
    _instance_guard: platform::InstanceGuard,
}

pub fn run() {
    let exiting = Arc::new(AtomicBool::new(false));
    let exit_guard = exiting.clone();
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            app_lifecycle::show_main_window(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .setup(move |app| {
            let Some(instance_guard) =
                platform::acquire_instance().map_err(std::io::Error::other)?
            else {
                std::process::exit(0);
            };
            let db_path = app
                .path()
                .app_local_data_dir()?
                .join("resource-timeline.sqlite3");
            let db = Arc::new(Database::open(db_path)?);
            #[cfg(all(not(debug_assertions), not(feature = "qualification")))]
            if db.with_writer(writer::start_with_windows).unwrap_or(true) {
                // Re-enable on every release startup so a moved/replaced portable binary
                // refreshes the Windows Run entry to the current stable executable path.
                let _ = app.autolaunch().enable();
                let _ = platform::refresh_autostart_command();
            }
            let collector = CollectorManager::start(db.clone(), app.handle().clone());
            let crash_detector = CrashDetectorHandle::start(db.clone());
            app.manage(AppState {
                db,
                collector,
                crash_detector,
                _instance_guard: instance_guard,
            });
            app_lifecycle::install_tray(app.handle(), exiting.clone())?;
            if std::env::args().any(|arg| arg == "--background") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.destroy();
                }
            } else {
                app_lifecycle::show_main_window(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::crash::get_crash_detector_status,
            commands::crash::list_crash_cases,
            commands::crash::get_crash_case_detail,
            commands::crash::rebuild_crash_case,
            commands::crash::release_crash_case_hold,
            commands::overview::get_today_overview,
            commands::overview::get_daily_usage_summary,
            commands::overview::get_usage_summary,
            commands::overview::get_overview_available_dates,
            commands::timeline::get_app_usage_timeline,
            commands::timeline::get_timeline_available_dates,
            commands::overview::get_system_samples,
            commands::overview::get_system_timeline,
            commands::overview::get_gpu_samples,
            commands::overview::get_resource_available_dates,
            commands::overview::get_app_resource_samples,
            commands::overview::get_resource_apps,
            commands::overview::get_app_resource_available_dates,
            commands::overview::get_app_resource_history,
            commands::settings::list_apps,
            commands::settings::set_app_hidden,
            commands::settings::get_collector_status,
            commands::settings::get_storage_usage,
            commands::settings::set_collection_paused,
            commands::settings::get_collection_settings,
            commands::settings::set_collection_settings,
            commands::settings::get_dashboard_config,
            commands::settings::set_dashboard_config,
            commands::settings::get_autostart_enabled,
            commands::settings::set_autostart_enabled,
            commands::settings::clear_collected_data,
        ]);

    builder
        .build(tauri::generate_context!())
        .expect("failed to build Resource Timeline")
        .run(move |_app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !exit_guard.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}
