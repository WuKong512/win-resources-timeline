use crate::{
    collector::manager::Control,
    models::ForegroundApp,
    platform::{now_ms, PlatformEvent},
};
use std::path::Path;
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::{CloseHandle, FILETIME, HWND},
        System::Threading::{
            GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, WINEVENTPROC},
            WindowsAndMessaging::{
                EVENT_SYSTEM_FOREGROUND, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
            },
        },
    },
};

const WINDOWS_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;

pub fn current_window() -> Option<isize> {
    unsafe {
        let hwnd = GetForegroundWindow();
        (!hwnd.0.is_null()).then_some(hwnd.0 as isize)
    }
}

pub fn resolve_window(raw_hwnd: isize) -> Option<ForegroundApp> {
    unsafe {
        let hwnd = HWND(raw_hwnd as *mut core::ffi::c_void);
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok();
        let Some(handle) = handle else {
            return Some(unknown_app(pid));
        };
        let exe_path = query_exe_path(handle);
        let process_creation_time_ms = query_process_creation_time(handle);
        let _ = CloseHandle(handle);
        let Some(exe_path) = exe_path else {
            return Some(unknown_app(pid));
        };
        let process_name = Path::new(&exe_path)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("unresolved")
            .to_string();
        let display_name = Path::new(&process_name)
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(&process_name)
            .to_string();
        Some(ForegroundApp {
            identity_key: format!("name:{}", process_name.to_lowercase()),
            process_name,
            exe_path: Some(exe_path),
            display_name,
            pid: Some(pid),
            process_creation_time_ms,
        })
    }
}

unsafe fn query_exe_path(handle: windows::Win32::Foundation::HANDLE) -> Option<String> {
    let mut buffer = vec![0_u16; 32_768];
    let mut size = buffer.len() as u32;
    QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_WIN32,
        PWSTR(buffer.as_mut_ptr()),
        &mut size,
    )
    .ok()
    .map(|_| String::from_utf16_lossy(&buffer[..size as usize]))
}

unsafe fn query_process_creation_time(handle: windows::Win32::Foundation::HANDLE) -> Option<i64> {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user)
        .ok()
        .and_then(|_| {
            let ticks =
                (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
            let epoch_ms = i64::try_from(ticks / 10_000).ok()? - WINDOWS_EPOCH_OFFSET_MS;
            (epoch_ms > 0).then_some(epoch_ms)
        })
}

fn unknown_app(pid: u32) -> ForegroundApp {
    ForegroundApp {
        identity_key: format!("unknown:foreground:{pid}"),
        process_name: "unresolved".into(),
        exe_path: None,
        display_name: "Unknown foreground".into(),
        pid: Some(pid),
        process_creation_time_ms: None,
    }
}

pub fn install_hook() -> Option<windows::Win32::UI::Accessibility::HWINEVENTHOOK> {
    unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            WINEVENTPROC::Some(foreground_event_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        (!hook.0.is_null()).then_some(hook)
    }
}

pub fn uninstall_hook(hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK) {
    unsafe {
        let _ = UnhookWinEvent(hook);
    }
}

unsafe extern "system" fn foreground_event_callback(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if event != EVENT_SYSTEM_FOREGROUND {
        return;
    }
    super::session::send(Control::Platform(PlatformEvent::ForegroundWindow {
        hwnd: hwnd.0 as isize,
        at_ms: now_ms(),
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_epoch_offset_is_positive_and_stable() {
        assert_eq!(super::WINDOWS_EPOCH_OFFSET_MS, 11_644_473_600_000);
    }
}
