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
