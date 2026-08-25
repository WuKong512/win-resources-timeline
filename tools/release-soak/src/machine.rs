use serde::Serialize;

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineMetadata {
    pub os: String,
    pub windows_version: Option<String>,
    pub architecture: String,
    pub cpu_model: Option<String>,
    pub logical_processor_count: u32,
    pub physical_memory_bytes: Option<u64>,
    pub gpu_model: Option<String>,
    pub gpu_metadata_provider: Option<String>,
    pub battery_present: Option<bool>,
}

#[cfg(windows)]
mod windows_impl {
    use super::MachineMetadata;
    use std::{mem::size_of, os::windows::process::CommandExt, process::Command};
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::WIN32_ERROR,
            System::{
                Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
                Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ},
                SystemInformation::{
                    GetNativeSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
                },
            },
        },
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn collect() -> MachineMetadata {
        let mut system_info = SYSTEM_INFO::default();
        unsafe { GetNativeSystemInfo(&mut system_info) };
        let logical_processor_count = system_info.dwNumberOfProcessors.max(1);
        let physical_memory_bytes = unsafe {
            let mut memory = MEMORYSTATUSEX {
                dwLength: size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };
            GlobalMemoryStatusEx(&mut memory)
                .is_ok()
                .then_some(memory.ullTotalPhys)
        };
        let windows_version = registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "DisplayVersion",
        )
        .or_else(|| registry_string(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "ReleaseId"))
        .map(|display| {
            let build = registry_string(
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                "CurrentBuild",
            )
            .or_else(|| {
                registry_string(
                    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                    "CurrentBuildNumber",
                )
            });
            match build {
                Some(build) => format!("{display} build {build}"),
                None => display,
            }
        });
        let gpu_model = gpu_models_from_windows_metadata();
        MachineMetadata {
            os: "Windows".to_string(),
            windows_version,
            architecture: architecture_name(system_info),
            cpu_model: registry_string(
                r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
                "ProcessorNameString",
            ),
            logical_processor_count,
            physical_memory_bytes,
            gpu_model,
            gpu_metadata_provider: Some("Win32_VideoController".to_string()),
            battery_present: battery_present(),
        }
    }

    fn battery_present() -> Option<bool> {
        unsafe {
            let mut status = SYSTEM_POWER_STATUS::default();
            GetSystemPowerStatus(&mut status)
                .is_ok()
                .then_some(status.BatteryFlag != 128)
        }
    }

    fn gpu_models_from_windows_metadata() -> Option<String> {
        let output = Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "(Get-CimInstance -ClassName Win32_VideoController | Select-Object -ExpandProperty Name) -join '; '",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let value = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        (!value.is_empty()).then(|| value.chars().take(512).collect())
    }

    fn registry_string(subkey: &str, value_name: &str) -> Option<String> {
        unsafe {
            let subkey = wide(subkey);
            let value_name = wide(value_name);
            let mut buffer = vec![0_u16; 512];
            let mut bytes = (buffer.len() * size_of::<u16>()) as u32;
            let status: WIN32_ERROR = RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(subkey.as_ptr()),
                PCWSTR::from_raw(value_name.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr() as *mut _),
                Some(&mut bytes),
            );
            if status.0 != 0 {
                return None;
            }
            let length = (bytes as usize / size_of::<u16>()).min(buffer.len());
            String::from_utf16(&buffer[..length])
                .ok()
                .map(|value| value.trim_end_matches('\0').trim().to_string())
                .filter(|value| !value.is_empty())
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn architecture_name(system_info: SYSTEM_INFO) -> String {
        unsafe {
            match system_info.Anonymous.Anonymous.wProcessorArchitecture.0 {
                9 => "x64".to_string(),
                12 => "arm64".to_string(),
                0 => "x86".to_string(),
                _ => "unknown".to_string(),
            }
        }
    }
}

#[cfg(windows)]
pub use windows_impl::collect;

#[cfg(not(windows))]
pub fn collect() -> MachineMetadata {
    MachineMetadata {
        os: std::env::consts::OS.to_string(),
        windows_version: None,
        architecture: std::env::consts::ARCH.to_string(),
        cpu_model: None,
        logical_processor_count: std::thread::available_parallelism()
            .map(|value| value.get() as u32)
            .unwrap_or(1),
        physical_memory_bytes: None,
        gpu_model: None,
        gpu_metadata_provider: None,
        battery_present: None,
    }
}
