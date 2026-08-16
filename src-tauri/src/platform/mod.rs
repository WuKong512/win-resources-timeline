#[cfg(windows)]
mod windows;
use crate::collector::manager::Control;
use crate::models::{BootIdentity, ForegroundApp};
use crossbeam_channel::Sender;

#[derive(Debug, Clone, Copy)]
pub enum PlatformEvent {
    ForegroundWindow { hwnd: isize, at_ms: i64 },
    Locked { at_ms: i64 },
    Unlocked { at_ms: i64 },
    Suspended { at_ms: i64 },
    Resumed { at_ms: i64 },
    Disconnected { at_ms: i64 },
    Connected { at_ms: i64 },
    WindowsShutdown { at_ms: i64 },
}

#[allow(dead_code)]
pub fn foreground_app() -> Option<ForegroundApp> {
    #[cfg(windows)]
    {
        current_foreground_window().and_then(resolve_foreground_window)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn current_foreground_window() -> Option<isize> {
    #[cfg(windows)]
    {
        windows::foreground::current_window()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn resolve_foreground_window(hwnd: isize) -> Option<ForegroundApp> {
    #[cfg(windows)]
    {
        windows::foreground::resolve_window(hwnd)
    }
    #[cfg(not(windows))]
    {
        let _ = hwnd;
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

pub fn take_observer_dirty() -> bool {
    #[cfg(windows)]
    {
        windows::session::take_dirty()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn boot_identity(now_ms: i64) -> BootIdentity {
    #[cfg(windows)]
    {
        windows::activity::boot_identity(now_ms)
    }
    #[cfg(not(windows))]
    {
        BootIdentity {
            boot_id: format!("runtime-boot-{now_ms}"),
            boot_time_ms: now_ms,
        }
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
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
