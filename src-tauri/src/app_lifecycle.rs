use crate::AppState;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
};

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Resource Timeline")
        .inner_size(1280.0, 820.0)
        .min_inner_size(900.0, 640.0)
        .build();
}

pub fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.destroy();
    }
}

pub fn install_tray(app: &AppHandle, exiting: Arc<AtomicBool>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Resource Timeline", true, None::<&str>)?;
    let hide = MenuItem::with_id(
        app,
        "hide",
        "Hide window (keep collecting)",
        true,
        None::<&str>,
    )?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        "Pause / Resume collection",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Stop collection and exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &hide, &pause, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Resource Timeline - collecting in background")
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "hide" => hide_main_window(app),
            "pause" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let status = state.collector.status(state.db.size_bytes());
                    let _ = state.collector.set_paused(!status.paused);
                }
            }
            "quit" => {
                exiting.store(true, Ordering::SeqCst);
                if let Some(state) = app.try_state::<AppState>() {
                    state.collector.shutdown();
                }
                app.exit(0);
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
