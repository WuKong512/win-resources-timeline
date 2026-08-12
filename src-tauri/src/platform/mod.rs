#[cfg(windows)]
mod windows;
use crate::collector::manager::Control;
use crate::models::ForegroundApp;
use crossbeam_channel::Sender;
pub fn foreground_app() -> Option<ForegroundApp> {
    #[cfg(windows)]
    {
        windows::foreground::foreground_app()
    }
    #[cfg(not(windows))]
    {
        None
    }
}
pub fn idle_for_ms() -> Option<u64> {
    #[cfg(windows)]
    {
        windows::activity::idle_for_ms()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn start_session_observer(tx: Sender<Control>) {
    #[cfg(windows)]
    windows::session::start(tx);
    #[cfg(not(windows))]
    drop(tx);
}

pub struct InstanceGuard {
    #[cfg(windows)]
    _inner: windows::instance::Guard,
}

pub fn acquire_instance() -> Result<Option<InstanceGuard>, String> {
    #[cfg(windows)]
    {
        windows::instance::acquire().map(|guard| guard.map(|inner| InstanceGuard { _inner: inner }))
    }
    #[cfg(not(windows))]
    {
        Ok(Some(InstanceGuard {}))
    }
}

pub fn refresh_autostart_command() -> Result<(), String> {
    #[cfg(windows)]
    {
        windows::autostart::refresh_command().map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    {
        Ok(())
    }
}
