#[cfg(windows)]
mod windows;
use crate::collector::manager::Control;
use crate::models::{BootIdentity, ComputerState, ForegroundApp};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PLATFORM_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Default)]
pub struct ObserverRecovery {
    pub events: Vec<PlatformEventEnvelope>,
    pub overflowed: bool,
    /// True when an observer send was rejected, including a recovered critical send.
    pub dirty: bool,
    /// The earliest sequence that may have been dropped, if the source could observe it.
    pub overflow_after_sequence: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformEventEnvelope {
    pub sequence: u64,
    pub event: PlatformEvent,
}

impl PlatformEventEnvelope {
    pub fn new(event: PlatformEvent) -> Self {
        Self {
            sequence: NEXT_PLATFORM_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            event,
        }
    }

    #[cfg(test)]
    pub fn with_sequence(sequence: u64, event: PlatformEvent) -> Self {
        Self { sequence, event }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn start_session_observer(tx: Sender<Control>, critical_tx: Sender<PlatformEventEnvelope>) {
    #[cfg(windows)]
    windows::session::start(tx, critical_tx);
    #[cfg(not(windows))]
    {
        drop(tx);
        drop(critical_tx);
    }
}

pub(crate) fn stamp_platform_event(event: PlatformEvent) -> PlatformEventEnvelope {
    PlatformEventEnvelope::new(event)
}

pub fn take_observer_recovery() -> ObserverRecovery {
    #[cfg(windows)]
    {
        windows::session::take_recovery()
    }
    #[cfg(not(windows))]
    {
        ObserverRecovery::default()
    }
}

pub fn current_computer_state(idle_threshold_ms: u64) -> Option<ComputerState> {
    #[cfg(windows)]
    {
        windows::activity::current_computer_state(idle_threshold_ms)
    }
    #[cfg(not(windows))]
    {
        let _ = idle_threshold_ms;
        None
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
