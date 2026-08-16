use crate::{
    collector::manager::Control,
    platform::{now_ms, stamp_platform_event, PlatformEvent, PlatformEventEnvelope},
};
use crossbeam_channel::Sender;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            RemoteDesktop::{
                WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
                NOTIFY_FOR_THIS_SESSION,
            },
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            RegisterClassExW, TranslateMessage, HMENU, MSG, PBT_APMRESUMEAUTOMATIC,
            PBT_APMRESUMECRITICAL, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND, WINDOW_EX_STYLE,
            WINDOW_STYLE, WM_APP, WM_ENDSESSION, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE,
            WNDCLASSEXW, WTS_CONSOLE_CONNECT, WTS_CONSOLE_DISCONNECT, WTS_REMOTE_CONNECT,
            WTS_REMOTE_DISCONNECT, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
        },
    },
};

static CONTROL: OnceLock<Sender<Control>> = OnceLock::new();
static CRITICAL_CONTROL: OnceLock<Sender<PlatformEventEnvelope>> = OnceLock::new();
static CRITICAL_FALLBACK: OnceLock<Mutex<VecDeque<PlatformEventEnvelope>>> = OnceLock::new();
static OBSERVER_DIRTY: AtomicBool = AtomicBool::new(false);
static CRITICAL_OVERFLOW: AtomicBool = AtomicBool::new(false);
pub const OPEN_WINDOW_MESSAGE: u32 = WM_APP + 42;
const CRITICAL_FALLBACK_CAPACITY: usize = 32;

pub fn start(tx: Sender<Control>, critical_tx: Sender<PlatformEventEnvelope>) {
    if CONTROL.set(tx).is_err() {
        return;
    }
    let _ = CRITICAL_CONTROL.set(critical_tx);
    let _ = CRITICAL_FALLBACK.set(Mutex::new(VecDeque::with_capacity(
        CRITICAL_FALLBACK_CAPACITY,
    )));
    std::thread::spawn(message_loop);
}

fn message_loop() {
    unsafe {
        let Ok(module) = GetModuleHandleW(PCWSTR::null()) else {
            return;
        };
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: w!("ResourceTimelineSessionObserver"),
            ..Default::default()
        };
        if RegisterClassExW(&class) == 0 {
            return;
        }
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class.lpszClassName,
            w!(""),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND::default(),
            HMENU::default(),
            instance,
            None,
        ) else {
            return;
        };
        if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION).is_err() {
            let _ = DestroyWindow(hwnd);
            return;
        }
        let foreground_hook = super::foreground::install_hook();

        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, HWND::default(), 0, 0).0;
            if result <= 0 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if let Some(hook) = foreground_hook {
            super::foreground::uninstall_hook(hook);
        }
        let _ = WTSUnRegisterSessionNotification(hwnd);
        let _ = DestroyWindow(hwnd);
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_WTSSESSION_CHANGE => match wparam.0 as u32 {
            WTS_SESSION_LOCK => send_critical(PlatformEvent::Locked { at_ms: now_ms() }),
            WTS_SESSION_UNLOCK => send_critical(PlatformEvent::Unlocked { at_ms: now_ms() }),
            WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT => {
                send_critical(PlatformEvent::Disconnected { at_ms: now_ms() })
            }
            WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT => {
                send_critical(PlatformEvent::Connected { at_ms: now_ms() })
            }
            _ => {}
        },
        WM_POWERBROADCAST => match wparam.0 as u32 {
            PBT_APMSUSPEND => send_critical(PlatformEvent::Suspended { at_ms: now_ms() }),
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMECRITICAL | PBT_APMRESUMESUSPEND => {
                send_critical(PlatformEvent::Resumed { at_ms: now_ms() })
            }
            _ => {}
        },
        WM_ENDSESSION if wparam.0 != 0 => {
            send_critical(PlatformEvent::WindowsShutdown { at_ms: now_ms() })
        }
        OPEN_WINDOW_MESSAGE => send(Control::OpenWindow),
        _ => {}
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

pub(crate) fn send(event: Control) {
    if let Some(tx) = CONTROL.get() {
        if tx.try_send(event).is_err() {
            OBSERVER_DIRTY.store(true, Ordering::Release);
        }
    }
}

pub(crate) fn send_platform(event: PlatformEvent) {
    send(Control::Platform(stamp_platform_event(event)));
}

pub(crate) fn send_critical(event: PlatformEvent) {
    let Some(tx) = CRITICAL_CONTROL.get() else {
        return;
    };
    let event = stamp_platform_event(event);
    if tx.try_send(event).is_ok() {
        return;
    }
    OBSERVER_DIRTY.store(true, Ordering::Release);
    let Some(fallback) = CRITICAL_FALLBACK.get() else {
        CRITICAL_OVERFLOW.store(true, Ordering::Release);
        return;
    };
    match fallback.try_lock() {
        Ok(mut pending) if pending.len() < CRITICAL_FALLBACK_CAPACITY => pending.push_back(event),
        _ => CRITICAL_OVERFLOW.store(true, Ordering::Release),
    }
}

pub fn take_dirty() -> bool {
    OBSERVER_DIRTY.swap(false, Ordering::AcqRel)
}

pub fn take_recovery() -> crate::platform::ObserverRecovery {
    let events = CRITICAL_FALLBACK
        .get()
        .and_then(|fallback| {
            fallback
                .try_lock()
                .ok()
                .map(|mut pending| pending.drain(..).collect())
        })
        .unwrap_or_default();
    crate::platform::ObserverRecovery {
        events,
        overflowed: CRITICAL_OVERFLOW.swap(false, Ordering::AcqRel),
    }
}
