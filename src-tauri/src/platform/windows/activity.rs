use crate::models::{BootIdentity, ComputerState};
use std::ffi::c_void;
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::HANDLE,
        System::{
            RemoteDesktop::{
                WTSConnectState, WTSDisconnected as WTS_DISCONNECTED, WTSDown as WTS_DOWN,
                WTSFreeMemory, WTSInit as WTS_INIT, WTSQuerySessionInformationW,
                WTS_CONNECTSTATE_CLASS, WTS_CURRENT_SERVER_HANDLE, WTS_CURRENT_SESSION,
            },
            StationsAndDesktops::{
                CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_ACCESS_FLAGS,
                DESKTOP_CONTROL_FLAGS, UOI_NAME,
            },
            SystemInformation::GetTickCount64,
        },
        UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
    },
};

const WINDOWS_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;
const SYSTEM_TIME_OF_DAY_INFORMATION: u32 = 3;

#[repr(C)]
struct SystemTimeOfDayInformation {
    boot_time: i64,
    _current_time: i64,
    _time_zone_bias: i64,
    _current_time_zone_id: u32,
    _reserved: u32,
    _boot_time_bias: i64,
    _sleep_time_bias: i64,
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtQuerySystemInformation(
        system_information_class: u32,
        system_information: *mut c_void,
        system_information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}
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
    boot_identity_from_observation(now_ms, uptime_ms, stable_boot_time_ms())
}

fn boot_identity_from_observation(
    now_ms: i64,
    uptime_ms: u64,
    stable_boot_time_ms: Option<i64>,
) -> BootIdentity {
    let boot_time_ms = stable_boot_time_ms
        .unwrap_or_else(|| now_ms.saturating_sub(i64::try_from(uptime_ms).unwrap_or(i64::MAX)));
    let boot_id = if stable_boot_time_ms.is_some() {
        format!("windows-boot-{boot_time_ms}")
    } else {
        format!("windows-boot-fallback-{boot_time_ms}")
    };
    BootIdentity {
        boot_id,
        boot_time_ms,
    }
}

pub fn current_computer_state(idle_threshold_ms: u64) -> Option<ComputerState> {
    if session_is_disconnected()? {
        return Some(ComputerState::Disconnected);
    }
    if workstation_is_locked()? {
        return Some(ComputerState::Locked);
    }
    let idle_ms = idle_for_ms()?;
    Some(if idle_ms >= idle_threshold_ms {
        ComputerState::Idle
    } else {
        ComputerState::Active
    })
}

fn stable_boot_time_ms() -> Option<i64> {
    let mut info = SystemTimeOfDayInformation {
        boot_time: 0,
        _current_time: 0,
        _time_zone_bias: 0,
        _current_time_zone_id: 0,
        _reserved: 0,
        _boot_time_bias: 0,
        _sleep_time_bias: 0,
    };
    let mut returned = 0_u32;
    let status = unsafe {
        NtQuerySystemInformation(
            SYSTEM_TIME_OF_DAY_INFORMATION,
            (&mut info as *mut SystemTimeOfDayInformation).cast(),
            std::mem::size_of::<SystemTimeOfDayInformation>() as u32,
            &mut returned,
        )
    };
    if status != 0 {
        return None;
    }
    Some(info.boot_time / 10_000 - WINDOWS_EPOCH_OFFSET_MS)
}

fn session_is_disconnected() -> Option<bool> {
    let mut buffer = PWSTR::null();
    let mut bytes_returned = 0_u32;
    unsafe {
        WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            WTS_CURRENT_SESSION,
            WTSConnectState,
            &mut buffer,
            &mut bytes_returned,
        )
        .ok()?;
        if bytes_returned < std::mem::size_of::<WTS_CONNECTSTATE_CLASS>() as u32 {
            WTSFreeMemory(buffer.0.cast());
            return None;
        }
        let state = std::ptr::read_unaligned(buffer.0.cast::<WTS_CONNECTSTATE_CLASS>());
        WTSFreeMemory(buffer.0.cast());
        Some(matches!(state, WTS_DISCONNECTED | WTS_DOWN | WTS_INIT))
    }
}

fn workstation_is_locked() -> Option<bool> {
    let desktop = unsafe {
        OpenInputDesktop(
            DESKTOP_CONTROL_FLAGS::default(),
            false,
            DESKTOP_ACCESS_FLAGS::default(),
        )
        .ok()?
    };
    let mut name = [0_u16; 64];
    let mut bytes_needed = 0_u32;
    let result = unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            (name.len() * std::mem::size_of::<u16>()) as u32,
            Some(&mut bytes_needed),
        )
    };
    let _ = unsafe { CloseDesktop(desktop) };
    result.ok()?;
    let units = (bytes_needed as usize / std::mem::size_of::<u16>()).saturating_sub(1);
    let desktop_name = String::from_utf16_lossy(&name[..units.min(name.len())]);
    Some(
        desktop_name.eq_ignore_ascii_case("winlogon")
            || desktop_name.eq_ignore_ascii_case("screen-saver"),
    )
}

#[cfg(test)]
mod tests {
    use super::boot_identity_from_observation;

    #[test]
    fn stable_boot_identity_survives_wall_clock_adjustment() {
        let before = boot_identity_from_observation(1_000_000, 250_000, Some(750_000));
        let after = boot_identity_from_observation(1_010_000, 260_000, Some(750_000));
        assert_eq!(before, after);
    }

    #[test]
    fn stable_boot_identity_changes_after_real_reboot() {
        let before = boot_identity_from_observation(1_000_000, 250_000, Some(750_000));
        let after = boot_identity_from_observation(1_010_000, 100, Some(1_009_900));
        assert_ne!(before, after);
    }
}
