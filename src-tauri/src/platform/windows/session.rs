use crate::{
    collector::manager::Control,
    platform::{now_ms, PlatformEvent},
};
use crossbeam_channel::Sender;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
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
static OBSERVER_DIRTY: AtomicBool = AtomicBool::new(false);
pub const OPEN_WINDOW_MESSAGE: u32 = WM_APP + 42;

pub fn start(tx: Sender<Control>) {
    if CONTROL.set(tx).is_err() {
        return;
    }
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
            WTS_SESSION_LOCK => send(Control::Platform(PlatformEvent::Locked { at_ms: now_ms() })),
            WTS_SESSION_UNLOCK => send(Control::Platform(PlatformEvent::Unlocked {
                at_ms: now_ms(),
            })),
            WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT => {
                send(Control::Platform(PlatformEvent::Disconnected {
                    at_ms: now_ms(),
                }))
            }
            WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT => {
                send(Control::Platform(PlatformEvent::Connected {
                    at_ms: now_ms(),
                }))
            }
            _ => {}
        },
        WM_POWERBROADCAST => match wparam.0 as u32 {
            PBT_APMSUSPEND => send(Control::Platform(PlatformEvent::Suspended {
                at_ms: now_ms(),
            })),
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMECRITICAL | PBT_APMRESUMESUSPEND => {
                send(Control::Platform(PlatformEvent::Resumed {
                    at_ms: now_ms(),
                }))
            }
            _ => {}
        },
        WM_ENDSESSION if wparam.0 != 0 => send(Control::Platform(PlatformEvent::WindowsShutdown {
            at_ms: now_ms(),
        })),
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

pub fn take_dirty() -> bool {
    OBSERVER_DIRTY.swap(false, Ordering::AcqRel)
}
