use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessObservation {
    pub alive: bool,
    pub cpu_percent: Option<f64>,
    pub cpu_basis: &'static str,
    pub working_set_bytes: Option<u64>,
    pub private_bytes: Option<u64>,
    pub thread_count: Option<u32>,
    pub handle_count: Option<u32>,
    pub restarted: bool,
}

#[cfg(windows)]
mod windows_impl {
    use super::ProcessObservation;
    use std::{mem::size_of, time::Instant};
    use windows::{
        core::HRESULT,
        Win32::{
            Foundation::{CloseHandle, FILETIME, HANDLE},
            System::{
                Diagnostics::ToolHelp::{
                    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                    THREADENTRY32,
                },
                ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
                Threading::{
                    GetProcessHandleCount, GetProcessTimes, OpenProcess,
                    PROCESS_QUERY_LIMITED_INFORMATION,
                },
            },
        },
    };

    const FILETIME_UNIX_EPOCH_OFFSET_100NS: u64 = 116_444_736_000_000_000;

    #[derive(Debug, Clone, Copy)]
    struct RawProcessObservation {
        creation_time_100ns: u64,
        cpu_time_100ns: u64,
        working_set_bytes: Option<u64>,
        private_bytes: Option<u64>,
        thread_count: Option<u32>,
        handle_count: Option<u32>,
    }

    pub struct ProcessSampler {
        pid: u32,
        logical_processors: u32,
        creation_time_100ns: u64,
        previous_cpu_time_100ns: u64,
        previous_sample_at: Instant,
    }

    impl ProcessSampler {
        pub fn attach(pid: u32, logical_processors: u32) -> Result<Self, String> {
            let now = Instant::now();
            let raw = read_process(pid).ok_or_else(|| {
                format!("target process {pid} is not running or is not queryable")
            })?;
            Ok(Self {
                pid,
                logical_processors: logical_processors.max(1),
                creation_time_100ns: raw.creation_time_100ns,
                previous_cpu_time_100ns: raw.cpu_time_100ns,
                previous_sample_at: now,
            })
        }

        pub fn observe(&mut self) -> ProcessObservation {
            let now = Instant::now();
            let Some(raw) = read_process(self.pid) else {
                return ProcessObservation {
                    alive: false,
                    cpu_percent: None,
                    cpu_basis: "whole_machine_percentage",
                    working_set_bytes: None,
                    private_bytes: None,
                    thread_count: None,
                    handle_count: None,
                    restarted: false,
                };
            };
            let restarted = raw.creation_time_100ns != self.creation_time_100ns;
            let elapsed = now.saturating_duration_since(self.previous_sample_at);
            let cpu_percent = if !restarted && elapsed.as_nanos() > 0 {
                let cpu_delta = raw
                    .cpu_time_100ns
                    .saturating_sub(self.previous_cpu_time_100ns);
                Some(
                    cpu_delta as f64
                        / 10_000_000.0
                        / elapsed.as_secs_f64()
                        / self.logical_processors as f64
                        * 100.0,
                )
            } else {
                None
            };
            self.previous_cpu_time_100ns = raw.cpu_time_100ns;
            self.previous_sample_at = now;
            ProcessObservation {
                alive: true,
                cpu_percent,
                cpu_basis: "whole_machine_percentage",
                working_set_bytes: raw.working_set_bytes,
                private_bytes: raw.private_bytes,
                thread_count: raw.thread_count,
                handle_count: raw.handle_count,
                restarted,
            }
        }

        pub fn creation_time_utc_ms(&self) -> Option<i64> {
            filetime_to_unix_ms(self.creation_time_100ns)
        }
    }

    fn read_process(pid: u32) -> Option<RawProcessObservation> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut creation = FILETIME::default();
            let mut exit = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user).is_err() {
                close_handle(process);
                return None;
            }
            let mut memory = PROCESS_MEMORY_COUNTERS_EX {
                cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
                ..Default::default()
            };
            let memory = GetProcessMemoryInfo(
                process,
                &mut memory as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
                size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            )
            .ok()
            .map(|_| (memory.WorkingSetSize as u64, memory.PrivateUsage as u64));
            let mut handle_count = 0_u32;
            let handle_count = GetProcessHandleCount(process, &mut handle_count)
                .is_ok()
                .then_some(handle_count);
            let thread_count = thread_count_for_process(pid);
            close_handle(process);
            Some(RawProcessObservation {
                creation_time_100ns: filetime_to_u64(creation),
                cpu_time_100ns: filetime_to_u64(kernel).saturating_add(filetime_to_u64(user)),
                working_set_bytes: memory.map(|value| value.0),
                private_bytes: memory.map(|value| value.1),
                thread_count,
                handle_count,
            })
        }
    }

    fn thread_count_for_process(pid: u32) -> Option<u32> {
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).ok()?;
            let mut entry = THREADENTRY32 {
                dwSize: size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };
            let mut count = 0_u32;
            if Thread32First(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32OwnerProcessID == pid {
                        count = count.saturating_add(1);
                    }
                    if Thread32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            close_handle(snapshot);
            Some(count)
        }
    }

    fn close_handle(handle: HANDLE) {
        unsafe {
            let _ = CloseHandle(handle);
        }
    }

    fn filetime_to_u64(filetime: FILETIME) -> u64 {
        ((filetime.dwHighDateTime as u64) << 32) | filetime.dwLowDateTime as u64
    }

    fn filetime_to_unix_ms(value: u64) -> Option<i64> {
        value
            .checked_sub(FILETIME_UNIX_EPOCH_OFFSET_100NS)
            .map(|ticks| (ticks / 10_000) as i64)
    }

    #[allow(dead_code)]
    fn _keep_error_types(_: HRESULT, _: u32, _: HRESULT) {}
}

#[cfg(windows)]
pub use windows_impl::ProcessSampler;

#[cfg(not(windows))]
pub struct ProcessSampler;

#[cfg(not(windows))]
impl ProcessSampler {
    pub fn attach(_: u32, _: u32) -> Result<Self, String> {
        Err("release-soak requires Windows".to_string())
    }

    pub fn observe(&mut self) -> ProcessObservation {
        ProcessObservation {
            alive: false,
            cpu_percent: None,
            cpu_basis: "whole_machine_percentage",
            working_set_bytes: None,
            private_bytes: None,
            thread_count: None,
            handle_count: None,
            restarted: false,
        }
    }

    pub fn creation_time_utc_ms(&self) -> Option<i64> {
        None
    }
}
