use crate::models::MetricCategory;
use crate::models::{AppResourceSample, ProviderErrorCode, ResourceSnapshot, SystemSample};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    time::Instant,
};
use sysinfo::System;

const TOP_APPS_PER_RESOURCE: usize = 5;

pub struct SystemSampler {
    system: System,
    last_sample: Instant,
    warmed_up: bool,
    disk: DiskIoSampler,
}

impl SystemSampler {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let categories: BTreeSet<_> = MetricCategory::ALL.into_iter().collect();
        Self::new_for_categories(&categories)
    }

    pub fn new_for_categories(categories: &BTreeSet<MetricCategory>) -> Self {
        Self {
            system: System::new(),
            last_sample: Instant::now(),
            warmed_up: false,
            disk: DiskIoSampler::new_if(categories.contains(&MetricCategory::Disk)),
        }
    }

    pub fn disk_available(&self) -> bool {
        self.disk.is_available()
    }
    #[allow(dead_code)]
    pub fn sample(
        &mut self,
        timestamp_ms: i64,
        tracked_app_keys: &HashSet<String>,
    ) -> Option<ResourceSnapshot> {
        let categories: BTreeSet<_> = MetricCategory::ALL.into_iter().collect();
        self.sample_with_categories(timestamp_ms, tracked_app_keys, &categories)
    }

    pub fn sample_with_categories(
        &mut self,
        timestamp_ms: i64,
        tracked_app_keys: &HashSet<String>,
        categories: &BTreeSet<MetricCategory>,
    ) -> Option<ResourceSnapshot> {
        let elapsed = self.last_sample.elapsed();
        self.last_sample = Instant::now();
        let cpu_enabled = categories.contains(&MetricCategory::Cpu);
        let memory_enabled = categories.contains(&MetricCategory::Memory);
        let disk_enabled = categories.contains(&MetricCategory::Disk);
        let process_enabled = categories.contains(&MetricCategory::Process);
        if cpu_enabled {
            self.system.refresh_cpu();
        }
        if memory_enabled {
            self.system.refresh_memory();
        }
        if process_enabled {
            self.system.refresh_processes();
        }
        if !self.warmed_up {
            self.warmed_up = true;
            return None;
        }
        let memory =
            memory_enabled.then(|| (self.system.total_memory(), self.system.used_memory()));
        let disk = disk_enabled.then(|| self.disk.sample()).flatten();
        let sample_duration_ms = elapsed.as_millis().max(1) as i64;
        let system = SystemSample {
            timestamp_ms,
            sample_duration_ms,
            cpu_percent: cpu_enabled.then(|| self.system.global_cpu_info().cpu_usage() as f64),
            memory_percent: memory.and_then(|(total, used)| {
                (total > 0).then_some(used as f64 * 100.0 / total as f64)
            }),
            memory_used_bytes: memory.map(|(_, used)| used as i64),
            memory_total_bytes: memory.map(|(total, _)| total as i64),
            disk_read_bytes_per_sec: disk.map(|value| value.0),
            disk_write_bytes_per_sec: disk.map(|value| value.1),
            gpus: Vec::new(),
            has_app_snapshot: process_enabled,
        };
        let apps = if process_enabled {
            collect_app_samples(&self.system, sample_duration_ms, tracked_app_keys)
        } else {
            Vec::new()
        };
        Some(ResourceSnapshot { system, apps })
    }
}

fn collect_app_samples(
    system: &System,
    sample_duration_ms: i64,
    tracked_app_keys: &HashSet<String>,
) -> Vec<AppResourceSample> {
    let cpu_count = system.cpus().len().max(1) as f64;
    let duration_ms = sample_duration_ms.max(1) as u64;
    let mut grouped = HashMap::<String, AppResourceSample>::new();
    for process in system.processes().values() {
        let process_name = process.name().trim();
        if process_name.is_empty() {
            continue;
        }
        let exe_path = process
            .exe()
            .filter(|path| !path.as_os_str().is_empty())
            .map(|path| path.to_string_lossy().into_owned());
        let app_key = exe_path
            .as_deref()
            .map(|path| {
                format!(
                    "path:{}",
                    path.trim()
                        .trim_start_matches(r"\\?\")
                        .replace('/', "\\")
                        .to_lowercase()
                )
            })
            .unwrap_or_else(|| format!("name:{}", process_name.to_lowercase()));
        let disk = process.disk_usage();
        let entry = grouped
            .entry(app_key.clone())
            .or_insert_with(|| AppResourceSample {
                app_key,
                process_name: process_name.to_string(),
                exe_path: exe_path.clone(),
                process_count: 0,
                cpu_percent: 0.0,
                memory_used_bytes: 0,
                io_read_bytes_per_sec: 0,
                io_write_bytes_per_sec: 0,
            });
        entry.process_count += 1;
        entry.cpu_percent += process.cpu_usage() as f64 / cpu_count;
        entry.memory_used_bytes = entry
            .memory_used_bytes
            .saturating_add(process.memory().min(i64::MAX as u64) as i64);
        entry.io_read_bytes_per_sec = entry
            .io_read_bytes_per_sec
            .saturating_add(rate_per_second(disk.read_bytes, duration_ms));
        entry.io_write_bytes_per_sec = entry
            .io_write_bytes_per_sec
            .saturating_add(rate_per_second(disk.written_bytes, duration_ms));
    }
    select_top_apps(
        grouped.into_values().collect(),
        TOP_APPS_PER_RESOURCE,
        tracked_app_keys,
    )
}

fn rate_per_second(bytes: u64, duration_ms: u64) -> i64 {
    bytes
        .saturating_mul(1_000)
        .checked_div(duration_ms.max(1))
        .unwrap_or(0)
        .min(i64::MAX as u64) as i64
}

fn select_top_apps(
    mut apps: Vec<AppResourceSample>,
    limit: usize,
    tracked_app_keys: &HashSet<String>,
) -> Vec<AppResourceSample> {
    let mut selected = HashSet::new();
    let mut add_top = |value: &dyn Fn(&AppResourceSample) -> f64| {
        let mut ranked: Vec<_> = apps.iter().collect();
        ranked.sort_by(|a, b| value(b).total_cmp(&value(a)));
        selected.extend(
            ranked
                .into_iter()
                .take(limit)
                .map(|app| app.app_key.clone()),
        );
    };
    add_top(&|app| app.cpu_percent);
    add_top(&|app| app.memory_used_bytes as f64);
    add_top(&|app| (app.io_read_bytes_per_sec + app.io_write_bytes_per_sec) as f64);
    selected.extend(tracked_app_keys.iter().cloned());
    apps.retain(|app| selected.contains(&app.app_key));
    apps.sort_by(|a, b| {
        b.cpu_percent
            .total_cmp(&a.cpu_percent)
            .then_with(|| b.memory_used_bytes.cmp(&a.memory_used_bytes))
    });
    apps
}

#[cfg(windows)]
struct DiskIoSampler {
    query: isize,
    read_counter: isize,
    write_counter: isize,
}

#[cfg(windows)]
impl DiskIoSampler {
    fn new_if(enabled: bool) -> Self {
        if enabled {
            Self::try_new().unwrap_or_else(|_| Self::unavailable())
        } else {
            Self::unavailable()
        }
    }

    fn try_new() -> Result<Self, ProviderErrorCode> {
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::{ERROR_ACCESS_DENIED, ERROR_SUCCESS},
                System::Performance::{PdhAddEnglishCounterW, PdhCollectQueryData, PdhOpenQueryW},
            },
        };

        unsafe {
            let mut query = 0;
            let status = PdhOpenQueryW(PCWSTR::null(), 0, &mut query);
            if status != ERROR_SUCCESS.0 {
                return Err(pdh_error_code(status, ERROR_ACCESS_DENIED.0));
            }
            let read_path = wide(r"\PhysicalDisk(_Total)\Disk Read Bytes/sec");
            let write_path = wide(r"\PhysicalDisk(_Total)\Disk Write Bytes/sec");
            let mut read_counter = 0;
            let mut write_counter = 0;
            let read_status = PdhAddEnglishCounterW(
                query,
                PCWSTR::from_raw(read_path.as_ptr()),
                0,
                &mut read_counter,
            );
            let write_status = PdhAddEnglishCounterW(
                query,
                PCWSTR::from_raw(write_path.as_ptr()),
                0,
                &mut write_counter,
            );
            if read_status != ERROR_SUCCESS.0 || write_status != ERROR_SUCCESS.0 {
                windows::Win32::System::Performance::PdhCloseQuery(query);
                let status = if read_status != ERROR_SUCCESS.0 {
                    read_status
                } else {
                    write_status
                };
                return Err(pdh_error_code(status, ERROR_ACCESS_DENIED.0));
            }
            let collect_status = PdhCollectQueryData(query);
            if collect_status != ERROR_SUCCESS.0 {
                windows::Win32::System::Performance::PdhCloseQuery(query);
                return Err(pdh_error_code(collect_status, ERROR_ACCESS_DENIED.0));
            }
            Ok(Self {
                query,
                read_counter,
                write_counter,
            })
        }
    }

    const fn unavailable() -> Self {
        Self {
            query: 0,
            read_counter: 0,
            write_counter: 0,
        }
    }

    fn sample(&mut self) -> Option<(i64, i64)> {
        use std::mem::MaybeUninit;
        use windows::Win32::{
            Foundation::ERROR_SUCCESS,
            System::Performance::{
                PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_FMT_COUNTERVALUE,
                PDH_FMT_DOUBLE,
            },
        };

        if self.query == 0 {
            return None;
        }
        unsafe {
            if PdhCollectQueryData(self.query) != ERROR_SUCCESS.0 {
                return None;
            }
            let read = formatted_value(self.read_counter)?;
            let write = formatted_value(self.write_counter)?;
            return Some((read.max(0.0) as i64, write.max(0.0) as i64));
        }

        unsafe fn formatted_value(counter: isize) -> Option<f64> {
            let mut value = MaybeUninit::<PDH_FMT_COUNTERVALUE>::uninit();
            if PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, None, value.as_mut_ptr())
                != ERROR_SUCCESS.0
            {
                return None;
            }
            let value = value.assume_init();
            (value.CStatus == ERROR_SUCCESS.0).then_some(value.Anonymous.doubleValue)
        }
    }

    fn is_available(&self) -> bool {
        self.query != 0 && self.read_counter != 0 && self.write_counter != 0
    }
}

#[cfg(windows)]
fn pdh_error_code(status: u32, access_denied: u32) -> ProviderErrorCode {
    if status == access_denied {
        ProviderErrorCode::PermissionDenied
    } else {
        ProviderErrorCode::ProviderMissing
    }
}

#[cfg(windows)]
impl Drop for DiskIoSampler {
    fn drop(&mut self) {
        if self.query != 0 {
            unsafe {
                windows::Win32::System::Performance::PdhCloseQuery(self.query);
            }
        }
    }
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
struct DiskIoSampler;

#[cfg(not(windows))]
impl DiskIoSampler {
    fn new_if(_enabled: bool) -> Self {
        Self
    }

    fn sample(&mut self) -> Option<(i64, i64)> {
        None
    }

    fn is_available(&self) -> bool {
        false
    }
}

pub trait DiskCapabilityProbe: Send {
    fn probe(&self) -> Result<(), ProviderErrorCode>;
}

pub struct PdhDiskCapabilityProbe;

impl DiskCapabilityProbe for PdhDiskCapabilityProbe {
    fn probe(&self) -> Result<(), ProviderErrorCode> {
        #[cfg(windows)]
        {
            DiskIoSampler::try_new().map(drop)
        }
        #[cfg(not(windows))]
        {
            Err(ProviderErrorCode::ProviderMissing)
        }
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod app_sample_tests {
    use super::{select_top_apps, AppResourceSample};
    use std::collections::HashSet;

    fn app(name: &str, cpu: f64, memory: i64, disk: i64) -> AppResourceSample {
        AppResourceSample {
            app_key: format!("name:{name}"),
            process_name: name.into(),
            exe_path: None,
            process_count: 1,
            cpu_percent: cpu,
            memory_used_bytes: memory,
            io_read_bytes_per_sec: disk,
            io_write_bytes_per_sec: 0,
        }
    }

    #[test]
    fn retains_the_union_of_resource_leaders() {
        let selected = select_top_apps(
            vec![
                app("cpu", 80.0, 1, 0),
                app("memory", 0.0, 100, 0),
                app("disk", 0.0, 1, 1_000),
                app("small", 0.0, 0, 0),
            ],
            1,
            &HashSet::from(["name:small".to_string()]),
        );
        let keys: HashSet<_> = selected.into_iter().map(|item| item.app_key).collect();
        assert_eq!(keys.len(), 4);
        assert!(keys.contains("name:cpu"));
        assert!(keys.contains("name:memory"));
        assert!(keys.contains("name:disk"));
        assert!(keys.contains("name:small"));
    }
}
