use crate::models::BootIdentity;
use windows::Win32::{
    System::SystemInformation::GetTickCount64,
    UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
};
pub fn idle_for_ms() -> Option<u64> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        GetLastInputInfo(&mut info).ok().ok()?;
        Some(GetTickCount64().saturating_sub(info.dwTime as u64))
    }
}

pub fn boot_identity(now_ms: i64) -> BootIdentity {
    let uptime_ms = unsafe { GetTickCount64() };
    let boot_time_ms = now_ms.saturating_sub(i64::try_from(uptime_ms).unwrap_or(i64::MAX));
    BootIdentity {
        boot_id: format!("windows-boot-{boot_time_ms}"),
        boot_time_ms,
    }
}
