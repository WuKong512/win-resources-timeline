use super::session::OPEN_WINDOW_MESSAGE;
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, LPARAM, WPARAM},
        System::Threading::CreateMutexW,
        UI::WindowsAndMessaging::{FindWindowW, PostMessageW},
    },
};

pub struct Guard(isize);

unsafe impl Send for Guard {}
unsafe impl Sync for Guard {}

pub fn acquire() -> Result<Option<Guard>, String> {
    unsafe {
        let name = guard_name();
        let handle = CreateMutexW(None, true, PCWSTR::from_raw(name.as_ptr()))
            .map_err(|error| error.to_string())?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            notify_existing_instance();
            let _ = CloseHandle(handle);
            return Ok(None);
        }
        Ok(Some(Guard(handle.0 as isize)))
    }
}

#[cfg(feature = "qualification")]
fn guard_name() -> Vec<u16> {
    "Local\\ResourceTimelineQualificationGuard"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(not(feature = "qualification"))]
fn guard_name() -> Vec<u16> {
    "Local\\ResourceTimelineMvpGuard"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

fn notify_existing_instance() {
    unsafe {
        if let Ok(hwnd) = FindWindowW(w!("ResourceTimelineSessionObserver"), PCWSTR::null()) {
            let _ = PostMessageW(hwnd, OPEN_WINDOW_MESSAGE, WPARAM(0), LPARAM(0));
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(HANDLE(self.0 as _));
        }
    }
}
