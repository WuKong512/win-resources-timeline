use crate::collector::manager::Control;
use crossbeam_channel::Sender;
use std::sync::OnceLock;
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
            WNDCLASSEXW, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
        },
    },
};

static CONTROL: OnceLock<Sender<Control>> = OnceLock::new();
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

        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, HWND::default(), 0, 0).0;
            if result <= 0 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
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
            WTS_SESSION_LOCK => send(Control::SessionPause("lock")),
            WTS_SESSION_UNLOCK => send(Control::SessionResume),
            _ => {}
        },
        WM_POWERBROADCAST => match wparam.0 as u32 {
            PBT_APMSUSPEND => send(Control::SessionPause("suspend")),
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMECRITICAL | PBT_APMRESUMESUSPEND => {
                send(Control::SessionResume)
            }
            _ => {}
        },
        WM_ENDSESSION if wparam.0 != 0 => send(Control::SessionPause("shutdown")),
        OPEN_WINDOW_MESSAGE => send(Control::OpenWindow),
        _ => {}
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn send(event: Control) {
    if let Some(tx) = CONTROL.get() {
        let _ = tx.try_send(event);
    }
}
