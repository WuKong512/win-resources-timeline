use crate::models::ForegroundApp;
use std::path::Path;
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::CloseHandle,
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    },
};

fn normalize_path(path: &str) -> String {
    path.trim()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}

pub fn foreground_app() -> Option<ForegroundApp> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok();
        let exe_path = handle.and_then(|handle| {
            let mut buffer = vec![0_u16; 32_768];
            let mut size = buffer.len() as u32;
            let result = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(buffer.as_mut_ptr()),
                &mut size,
            )
            .ok()
            .map(|_| String::from_utf16_lossy(&buffer[..size as usize]));
            let _ = CloseHandle(handle);
            result
        });
        let process_name = exe_path
            .as_deref()
            .and_then(|p| Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unresolved")
            .to_string();
        let display_name = Path::new(&process_name)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or(&process_name)
            .to_string();
        let identity_key = exe_path
            .as_deref()
            .map(|p| format!("path:{}", normalize_path(p)))
            .unwrap_or_else(|| {
                if process_name == "unresolved" {
                    "name:unresolved".into()
                } else {
                    format!("name:{}", process_name.to_lowercase())
                }
            });
        Some(ForegroundApp {
            identity_key,
            process_name,
            exe_path,
            display_name,
        })
    }
}
