#![cfg(windows)]

use crate::model::{Capability, DeviceInfo, MachineInfo, SupportStatus};
use std::{collections::BTreeMap, mem::size_of, time::Duration};
use windows::{
    core::{HRESULT, PCWSTR},
    Win32::{
        Foundation::{
            CloseHandle, BOOL, ERROR_ACCESS_DENIED, ERROR_INVALID_HANDLE, ERROR_INVALID_PARAMETER,
            ERROR_PARTIAL_COPY, ERROR_PROCESS_ABORTED, FILETIME, HANDLE,
        },
        NetworkManagement::IpHelper::{
            FreeMibTable, GetIfTable2, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_PPP,
            IF_TYPE_SLIP, IF_TYPE_SOFTWARE_LOOPBACK, IF_TYPE_TUNNEL, MIB_IF_ROW2, MIB_IF_TABLE2,
        },
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            Memory::{
                MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
                MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS, MEM_COMMIT, PAGE_GUARD,
                PAGE_NOACCESS,
            },
            Performance::{
                PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData,
                PdhGetFormattedCounterValue, PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE,
            },
            Power::{
                CallNtPowerInformation, GetSystemPowerStatus, ProcessorInformation,
                PROCESSOR_POWER_INFORMATION, SYSTEM_POWER_STATUS,
            },
            ProcessStatus::{EnumProcesses, GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ},
            SystemInformation::{
                GetNativeSystemInfo, GetTickCount64, GetVersionExW, GlobalMemoryStatusEx,
                MEMORYSTATUSEX, OSVERSIONINFOW, SYSTEM_INFO,
            },
            Threading::{
                GetCurrentProcess, GetCurrentProcessId, GetProcessHandleCount,
                GetProcessIoCounters, GetProcessTimes, GetSystemTimes, OpenProcess,
                OpenProcessToken, IO_COUNTERS, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    },
};

pub const SOURCE_SYSTEM_INFO: &str = "Windows Win32 API";
pub const SOURCE_PDH: &str = "Windows PDH English counters";
pub const SOURCE_NVML: &str = "NVIDIA NVML dynamic runtime";

#[cfg(windows)]
pub use crate::nvml::{append_nvidia_inventory, NvmlProvider, TimedRead};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadStatus {
    Value,
    Unsupported,
    PermissionDenied,
    ProviderMissing,
    Failed,
    RuntimeFailed,
}

#[derive(Debug, Clone)]
pub struct ReadResult<T> {
    pub status: ReadStatus,
    pub reason_code: String,
    pub value: Option<T>,
}

impl<T> ReadResult<T> {
    fn value(value: T) -> Self {
        Self {
            status: ReadStatus::Value,
            reason_code: "ok".to_string(),
            value: Some(value),
        }
    }

    fn status(status: ReadStatus, reason_code: impl Into<String>) -> Self {
        Self {
            status,
            reason_code: reason_code.into(),
            value: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CpuTimes {
    pub idle_100ns: u64,
    pub kernel_100ns: u64,
    pub user_100ns: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct CpuFrequencyInfo {
    pub current_mhz: Option<f64>,
    pub max_mhz: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct PowerInfo {
    pub battery_present: bool,
    pub ac_line_status: Option<bool>,
    pub battery_percent: Option<u8>,
    pub saver_active: Option<bool>,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessAccessSummary {
    pub enumerated: u32,
    pub accessible: u32,
    pub restricted: u32,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct ProcessMetrics {
    pub cpu_time_100ns: ReadResult<u64>,
    pub working_set_bytes: ReadResult<u64>,
    pub private_bytes: ReadResult<u64>,
    pub read_bytes: ReadResult<u64>,
    pub write_bytes: ReadResult<u64>,
}

#[derive(Debug, Clone)]
struct ProcessFailure {
    status: ReadStatus,
    reason_code: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessDetailSummary {
    pub attempted: u32,
    pub readable_cpu_time: u32,
    pub readable_working_set: u32,
    pub readable_private_memory: u32,
    pub readable_io: u32,
    pub permission_denied: u32,
    pub probe_failed: u32,
    pub raced: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SelfMetrics {
    pub cpu_time_100ns: u64,
    pub working_set_bytes: u64,
    pub thread_count: u32,
    pub handle_count: u32,
}

#[derive(Debug, Clone)]
pub struct NetworkInterfaceSnapshot {
    pub device_key: String,
    pub category: String,
    pub classification: String,
    pub interface_type: u32,
    pub in_octets: u64,
    pub out_octets: u64,
}

#[derive(Debug, Clone)]
pub struct DiskCounters {
    pub read_bytes_per_sec: f64,
    pub write_bytes_per_sec: f64,
}

#[derive(Debug, Clone)]
pub struct DiskProvider {
    query: isize,
    read_counter: isize,
    write_counter: isize,
    warmed: bool,
}

impl DiskProvider {
    pub fn new() -> ReadResult<Self> {
        unsafe {
            let mut query = 0;
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                return ReadResult::status(ReadStatus::ProviderMissing, "pdh_open_query_failed");
            }
            let read_path = wide(r"\PhysicalDisk(_Total)\Disk Read Bytes/sec");
            let write_path = wide(r"\PhysicalDisk(_Total)\Disk Write Bytes/sec");
            let mut read_counter = 0;
            let mut write_counter = 0;
            if PdhAddEnglishCounterW(
                query,
                PCWSTR::from_raw(read_path.as_ptr()),
                0,
                &mut read_counter,
            ) != 0
                || PdhAddEnglishCounterW(
                    query,
                    PCWSTR::from_raw(write_path.as_ptr()),
                    0,
                    &mut write_counter,
                ) != 0
            {
                PdhCloseQuery(query);
                return ReadResult::status(ReadStatus::ProviderMissing, "pdh_counter_missing");
            }
            let status = PdhCollectQueryData(query);
            if status != 0 {
                PdhCloseQuery(query);
                return ReadResult::status(ReadStatus::Failed, "pdh_initial_collect_failed");
            }
            ReadResult::value(Self {
                query,
                read_counter,
                write_counter,
                warmed: false,
            })
        }
    }

    pub fn sample(&mut self) -> ReadResult<DiskCounters> {
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return ReadResult::status(ReadStatus::Failed, "pdh_collect_failed");
            }
            let read = match formatted_counter(self.read_counter) {
                Some(value) => value.max(0.0),
                None => return ReadResult::status(ReadStatus::Failed, "pdh_read_value_failed"),
            };
            let write = match formatted_counter(self.write_counter) {
                Some(value) => value.max(0.0),
                None => return ReadResult::status(ReadStatus::Failed, "pdh_write_value_failed"),
            };
            if !self.warmed {
                self.warmed = true;
                return ReadResult::status(ReadStatus::Failed, "pdh_warmup_sample");
            }
            ReadResult::value(DiskCounters {
                read_bytes_per_sec: read,
                write_bytes_per_sec: write,
            })
        }
    }
}

impl Drop for DiskProvider {
    fn drop(&mut self) {
        unsafe {
            if self.query != 0 {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CpuPerformanceCounters {
    pub processor_frequency_mhz: f64,
    pub processor_performance_percent: f64,
    pub processor_utility_percent: f64,
    pub percent_maximum_frequency: f64,
}

/// Windows Processor Information counters collected through one reusable PDH query.
///
/// This is deliberately an independent probe primitive. It is not used by the
/// production collector and does not turn an OS counter into a hardware sensor.
#[derive(Debug)]
pub struct CpuPerformanceProvider {
    query: isize,
    processor_frequency: isize,
    processor_performance: isize,
    processor_utility: isize,
    percent_maximum_frequency: isize,
    warmed: bool,
}

impl CpuPerformanceProvider {
    pub fn new() -> ReadResult<Self> {
        unsafe {
            let mut query = 0;
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                return ReadResult::status(
                    ReadStatus::ProviderMissing,
                    "pdh_processor_information_open_failed",
                );
            }
            let paths = [
                r"\Processor Information(_Total)\Processor Frequency",
                r"\Processor Information(_Total)\% Processor Performance",
                r"\Processor Information(_Total)\% Processor Utility",
                r"\Processor Information(_Total)\% of Maximum Frequency",
            ];
            let mut counters = [0_isize; 4];
            for (path, counter) in paths.iter().zip(counters.iter_mut()) {
                let path = wide(path);
                if PdhAddEnglishCounterW(query, PCWSTR::from_raw(path.as_ptr()), 0, counter) != 0 {
                    PdhCloseQuery(query);
                    return ReadResult::status(
                        ReadStatus::ProviderMissing,
                        "pdh_processor_information_counter_missing",
                    );
                }
            }
            if PdhCollectQueryData(query) != 0 {
                PdhCloseQuery(query);
                return ReadResult::status(
                    ReadStatus::Failed,
                    "pdh_processor_information_initial_collect_failed",
                );
            }
            ReadResult::value(Self {
                query,
                processor_frequency: counters[0],
                processor_performance: counters[1],
                processor_utility: counters[2],
                percent_maximum_frequency: counters[3],
                warmed: false,
            })
        }
    }

    pub fn sample(&mut self) -> ReadResult<CpuPerformanceCounters> {
        unsafe {
            if PdhCollectQueryData(self.query) != 0 {
                return ReadResult::status(
                    ReadStatus::Failed,
                    "pdh_processor_information_collect_failed",
                );
            }
            let values = [
                formatted_counter(self.processor_frequency),
                formatted_counter(self.processor_performance),
                formatted_counter(self.processor_utility),
                formatted_counter(self.percent_maximum_frequency),
            ];
            let [Some(processor_frequency_mhz), Some(processor_performance_percent), Some(processor_utility_percent), Some(percent_maximum_frequency)] =
                values
            else {
                return ReadResult::status(
                    ReadStatus::Failed,
                    "pdh_processor_information_read_failed",
                );
            };
            if !self.warmed {
                self.warmed = true;
                return ReadResult::status(
                    ReadStatus::Failed,
                    "pdh_processor_information_warmup_sample",
                );
            }
            ReadResult::value(CpuPerformanceCounters {
                processor_frequency_mhz,
                processor_performance_percent,
                processor_utility_percent,
                percent_maximum_frequency,
            })
        }
    }
}

impl Drop for CpuPerformanceProvider {
    fn drop(&mut self) {
        unsafe {
            if self.query != 0 {
                PdhCloseQuery(self.query);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AfterburnerSnapshot {
    pub source_timestamp_seconds: i64,
    pub cpu_temperature_celsius: Option<f64>,
    pub cpu_power_watts: Option<f64>,
    pub cpu_clock_mhz: Option<f64>,
}

#[repr(C)]
struct MahmSharedMemoryHeader {
    signature: u32,
    version: u32,
    header_size: u32,
    entry_count: u32,
    entry_size: u32,
    time: i32,
    gpu_entry_count: u32,
    gpu_entry_size: u32,
}

#[repr(C)]
struct MahmSharedMemoryEntry {
    source_name: [u8; 260],
    source_units: [u8; 260],
    localized_source_name: [u8; 260],
    localized_source_units: [u8; 260],
    recommended_format: [u8; 260],
    data: f32,
    min_limit: f32,
    max_limit: f32,
    flags: u32,
    gpu: u32,
    source_id: u32,
}

// The SDK documents the four-character literal `MAHM`; MSVC stores that
// multi-character constant as the byte sequence `MHAM` in this DWORD.
const MAHM_SIGNATURE: u32 = u32::from_le_bytes(*b"MHAM");
const MAHM_VERSION_2: u32 = 0x0002_0000;
const MAHM_GLOBAL_GPU: u32 = u32::MAX;
const MAHM_CPU_TEMPERATURE: u32 = 0x0000_0080;
const MAHM_CPU_CLOCK: u32 = 0x0000_00A0;
const MAHM_CPU_POWER: u32 = 0x0000_0100;
const MAHM_MAX_ENTRY_COUNT: usize = 1024;
const MAHM_MAX_MAPPING_LENGTH: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MappingValidationError {
    EmptyRange,
    AddressOverflow,
    QueryFailed,
    InvalidRegion,
    NotCommitted,
    NoAccess,
    GuardPage,
    MissingAllocationBase,
    AllocationBaseChanged,
    RangeNotCovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MappedMemoryRegion {
    base_address: usize,
    allocation_base: usize,
    region_size: usize,
    state: u32,
    protection: u32,
}

fn validate_mapped_regions(
    view_address: usize,
    length: usize,
    expected_allocation_base: Option<usize>,
    regions: &[MappedMemoryRegion],
) -> Result<usize, MappingValidationError> {
    if view_address == 0 || length == 0 {
        return Err(MappingValidationError::EmptyRange);
    }
    let range_end = view_address
        .checked_add(length)
        .ok_or(MappingValidationError::AddressOverflow)?;
    let mut cursor = view_address;
    let mut allocation_base = expected_allocation_base;

    for region in regions {
        if cursor >= range_end {
            break;
        }
        let region_end = region
            .base_address
            .checked_add(region.region_size)
            .ok_or(MappingValidationError::AddressOverflow)?;
        if region.region_size == 0 || region.base_address > cursor || region_end <= cursor {
            return Err(MappingValidationError::RangeNotCovered);
        }
        if region.state != MEM_COMMIT.0 {
            return Err(MappingValidationError::NotCommitted);
        }
        if region.protection & PAGE_NOACCESS.0 != 0 {
            return Err(MappingValidationError::NoAccess);
        }
        if region.protection & PAGE_GUARD.0 != 0 {
            return Err(MappingValidationError::GuardPage);
        }
        if region.allocation_base == 0 {
            return Err(MappingValidationError::MissingAllocationBase);
        }
        if let Some(expected) = allocation_base {
            if expected != region.allocation_base {
                return Err(MappingValidationError::AllocationBaseChanged);
            }
        } else {
            allocation_base = Some(region.allocation_base);
        }

        cursor = region_end.min(range_end);
    }

    if cursor != range_end {
        return Err(MappingValidationError::RangeNotCovered);
    }
    allocation_base.ok_or(MappingValidationError::MissingAllocationBase)
}

fn validate_virtual_mapping(
    view: *const u8,
    length: usize,
    expected_allocation_base: Option<usize>,
) -> Result<usize, MappingValidationError> {
    if view.is_null() || length == 0 {
        return Err(MappingValidationError::EmptyRange);
    }
    let view_address = view as usize;
    let range_end = view_address
        .checked_add(length)
        .ok_or(MappingValidationError::AddressOverflow)?;
    let mut cursor = view_address;
    let mut regions = Vec::new();
    while cursor < range_end {
        let mut region = MEMORY_BASIC_INFORMATION::default();
        let queried = unsafe {
            VirtualQuery(
                Some(cursor as *const std::ffi::c_void),
                &mut region,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if queried != size_of::<MEMORY_BASIC_INFORMATION>() {
            return Err(MappingValidationError::QueryFailed);
        }
        let region_start = region.BaseAddress as usize;
        let region_end = region_start
            .checked_add(region.RegionSize)
            .ok_or(MappingValidationError::AddressOverflow)?;
        if region.RegionSize == 0 || region_end <= cursor {
            return Err(MappingValidationError::InvalidRegion);
        }
        regions.push(MappedMemoryRegion {
            base_address: region_start,
            allocation_base: region.AllocationBase as usize,
            region_size: region.RegionSize,
            state: region.State.0,
            protection: region.Protect.0,
        });
        cursor = region_end;
    }
    validate_mapped_regions(view_address, length, expected_allocation_base, &regions)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MahmLayoutError {
    HeaderTooSmall,
    EntryTooSmall,
    EntryCountTooLarge,
    EntrySizeOverflow,
    MappingTooLarge,
}

fn validate_mahm_layout(
    header_size: usize,
    entry_size: usize,
    entry_count: usize,
) -> Result<usize, MahmLayoutError> {
    if header_size < size_of::<MahmSharedMemoryHeader>() {
        return Err(MahmLayoutError::HeaderTooSmall);
    }
    if entry_size < size_of::<MahmSharedMemoryEntry>() {
        return Err(MahmLayoutError::EntryTooSmall);
    }
    if entry_count > MAHM_MAX_ENTRY_COUNT {
        return Err(MahmLayoutError::EntryCountTooLarge);
    }
    let entries_size = entry_count
        .checked_mul(entry_size)
        .ok_or(MahmLayoutError::EntrySizeOverflow)?;
    let mapping_length = header_size
        .checked_add(entries_size)
        .ok_or(MahmLayoutError::EntrySizeOverflow)?;
    if mapping_length > MAHM_MAX_MAPPING_LENGTH {
        return Err(MahmLayoutError::MappingTooLarge);
    }
    Ok(mapping_length)
}

fn mahm_layout_matches(
    header_size: usize,
    entry_size: usize,
    entry_count: usize,
    validated_header_size: usize,
    validated_entry_size: usize,
    validated_entry_count: usize,
    validated_mapping_length: usize,
) -> bool {
    header_size == validated_header_size
        && entry_size == validated_entry_size
        && entry_count == validated_entry_count
        && validate_mahm_layout(header_size, entry_size, entry_count)
            .is_ok_and(|length| length == validated_mapping_length)
}

/// Read-only adapter for MSI Afterburner's documented MAHM shared-memory SDK.
///
/// The adapter is reference-only: it never starts Afterburner, writes to the
/// mapping, installs a driver, or treats the result as a production dependency.
pub struct AfterburnerSharedMemory {
    mapping: HANDLE,
    view: *mut std::ffi::c_void,
    validated_header_size: usize,
    validated_entry_count: usize,
    validated_entry_size: usize,
    validated_mapping_length: usize,
    allocation_base: usize,
}

impl std::fmt::Debug for AfterburnerSharedMemory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AfterburnerSharedMemory")
            .field("validated_header_size", &self.validated_header_size)
            .field("validated_entry_count", &self.validated_entry_count)
            .field("validated_entry_size", &self.validated_entry_size)
            .field("validated_mapping_length", &self.validated_mapping_length)
            .field("allocation_base", &self.allocation_base)
            .finish()
    }
}

impl AfterburnerSharedMemory {
    pub fn open() -> ReadResult<Self> {
        Self::open_named("MAHMSharedMemory")
    }

    fn open_named(name: &str) -> ReadResult<Self> {
        unsafe {
            let name = wide(name);
            let mapping =
                match OpenFileMappingW(FILE_MAP_READ.0, BOOL(0), PCWSTR::from_raw(name.as_ptr())) {
                    Ok(mapping) => mapping,
                    Err(error) => {
                        let access_denied =
                            error.code() == HRESULT::from_win32(ERROR_ACCESS_DENIED.0);
                        return ReadResult::status(
                            if access_denied {
                                ReadStatus::PermissionDenied
                            } else {
                                ReadStatus::ProviderMissing
                            },
                            if access_denied {
                                "afterburner_shared_memory_access_denied"
                            } else {
                                "afterburner_shared_memory_missing"
                            },
                        );
                    }
                };
            let view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0).Value;
            if view.is_null() {
                close_handle(mapping);
                return ReadResult::status(
                    ReadStatus::Failed,
                    "afterburner_shared_memory_map_failed",
                );
            }
            let header_allocation_base = match validate_virtual_mapping(
                view as *const u8,
                size_of::<MahmSharedMemoryHeader>(),
                None,
            ) {
                Ok(allocation_base) => allocation_base,
                Err(_) => {
                    let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view });
                    close_handle(mapping);
                    return ReadResult::status(
                        ReadStatus::Failed,
                        "afterburner_shared_memory_map_bounds_invalid",
                    );
                }
            };
            let header = &*(view as *const MahmSharedMemoryHeader);
            if header.signature != MAHM_SIGNATURE || header.version < MAHM_VERSION_2 {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view });
                close_handle(mapping);
                return ReadResult::status(
                    ReadStatus::Failed,
                    "afterburner_shared_memory_header_invalid",
                );
            }
            let header_size = header.header_size as usize;
            let entry_count = header.entry_count as usize;
            let entry_size = header.entry_size as usize;
            let mapped_size = match validate_mahm_layout(header_size, entry_size, entry_count) {
                Ok(size) => size,
                Err(_) => {
                    let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view });
                    close_handle(mapping);
                    return ReadResult::status(
                        ReadStatus::Failed,
                        "afterburner_shared_memory_header_invalid",
                    );
                }
            };
            let allocation_base = match validate_virtual_mapping(
                view as *const u8,
                mapped_size,
                Some(header_allocation_base),
            ) {
                Ok(allocation_base) => allocation_base,
                Err(_) => {
                    let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view });
                    close_handle(mapping);
                    return ReadResult::status(
                        ReadStatus::Failed,
                        "afterburner_shared_memory_map_bounds_invalid",
                    );
                }
            };
            if allocation_base != header_allocation_base {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view });
                close_handle(mapping);
                return ReadResult::status(
                    ReadStatus::Failed,
                    "afterburner_shared_memory_map_bounds_invalid",
                );
            }
            ReadResult::value(Self {
                mapping,
                view,
                validated_header_size: header_size,
                validated_entry_count: entry_count,
                validated_entry_size: entry_size,
                validated_mapping_length: mapped_size,
                allocation_base,
            })
        }
    }

    pub fn sample(&self) -> ReadResult<AfterburnerSnapshot> {
        unsafe {
            if validate_virtual_mapping(
                self.view as *const u8,
                self.validated_mapping_length,
                Some(self.allocation_base),
            )
            .is_err()
            {
                return ReadResult::status(
                    ReadStatus::Failed,
                    "afterburner_shared_memory_mapping_invalidated",
                );
            }
            let header = &*(self.view as *const MahmSharedMemoryHeader);
            if header.signature != MAHM_SIGNATURE || header.version < MAHM_VERSION_2 {
                return ReadResult::status(
                    ReadStatus::Failed,
                    "afterburner_shared_memory_invalidated",
                );
            }
            let current_header_size = header.header_size as usize;
            let current_entry_count = header.entry_count as usize;
            let current_entry_size = header.entry_size as usize;
            if !mahm_layout_matches(
                current_header_size,
                current_entry_size,
                current_entry_count,
                self.validated_header_size,
                self.validated_entry_size,
                self.validated_entry_count,
                self.validated_mapping_length,
            ) {
                return ReadResult::status(
                    ReadStatus::Failed,
                    "afterburner_shared_memory_layout_changed",
                );
            }
            let mut temperature = None;
            let mut power = None;
            let mut clock = None;
            for index in 0..self.validated_entry_count {
                let offset = match self
                    .validated_entry_size
                    .checked_mul(index)
                    .and_then(|entry_offset| self.validated_header_size.checked_add(entry_offset))
                {
                    Some(offset) => offset,
                    None => {
                        return ReadResult::status(
                            ReadStatus::Failed,
                            "afterburner_shared_memory_layout_changed",
                        )
                    }
                };
                match offset.checked_add(size_of::<MahmSharedMemoryEntry>()) {
                    Some(entry_end) if entry_end <= self.validated_mapping_length => {}
                    _ => {
                        return ReadResult::status(
                            ReadStatus::Failed,
                            "afterburner_shared_memory_layout_changed",
                        )
                    }
                }
                let entry =
                    &*((self.view as *const u8).add(offset) as *const MahmSharedMemoryEntry);
                let value = valid_sensor_value(entry.data);
                if entry.gpu != MAHM_GLOBAL_GPU {
                    continue;
                }
                match entry.source_id {
                    MAHM_CPU_TEMPERATURE => temperature = value,
                    MAHM_CPU_POWER => power = value,
                    MAHM_CPU_CLOCK => clock = value,
                    _ => {}
                }
            }
            if temperature.is_none() && power.is_none() && clock.is_none() {
                return ReadResult::status(
                    ReadStatus::Unsupported,
                    "afterburner_cpu_sources_missing",
                );
            }
            ReadResult::value(AfterburnerSnapshot {
                source_timestamp_seconds: header.time as i64,
                cpu_temperature_celsius: temperature,
                cpu_power_watts: power,
                cpu_clock_mhz: clock,
            })
        }
    }
}

impl Drop for AfterburnerSharedMemory {
    fn drop(&mut self) {
        unsafe {
            if !self.view.is_null() {
                let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: self.view });
            }
            close_handle(self.mapping);
        }
    }
}

fn valid_sensor_value(value: f32) -> Option<f64> {
    value
        .is_finite()
        .then_some(value as f64)
        .filter(|value| value.abs() < f32::MAX as f64)
}

pub fn machine_info() -> MachineInfo {
    let memory = memory_info();
    let mut system_info = SYSTEM_INFO::default();
    unsafe { GetNativeSystemInfo(&mut system_info) };
    let os = os_version();
    MachineInfo {
        os_name: "Windows".to_string(),
        os_display_version: registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "DisplayVersion",
        )
        .or_else(|| registry_string(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", "ReleaseId")),
        os_build: registry_string(
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuild",
        )
        .or_else(|| {
            registry_string(
                r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
                "CurrentBuildNumber",
            )
        })
        .or_else(|| os.map(|version| version.dwBuildNumber.to_string())),
        architecture: architecture_name(system_info),
        cpu_model: registry_string(
            r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
            "ProcessorNameString",
        ),
        logical_processor_count: Some(system_info.dwNumberOfProcessors),
        memory_total_bytes: memory.value.map(|value| value.total_bytes),
        elevated: elevated_status(),
    }
}

pub fn inventory() -> (Vec<DeviceInfo>, Vec<Capability>) {
    let (mut devices, mut capabilities) = inventory_with_options(true, true, true, true);
    match NvmlProvider::new() {
        ReadResult {
            status: ReadStatus::Value,
            value: Some(provider),
            ..
        } => append_nvidia_inventory(&mut devices, &mut capabilities, Some(&provider), None),
        result => append_nvidia_inventory(
            &mut devices,
            &mut capabilities,
            None,
            Some(&(result.status, result.reason_code)),
        ),
    }
    (devices, capabilities)
}

pub fn inventory_with_options(
    include_disk: bool,
    include_network: bool,
    include_power: bool,
    include_process: bool,
) -> (Vec<DeviceInfo>, Vec<Capability>) {
    let machine = machine_info();
    let mut devices = Vec::new();
    let mut capabilities = Vec::new();

    let mut cpu_details = BTreeMap::new();
    if let Some(model) = machine.cpu_model.clone() {
        cpu_details.insert("model".to_string(), model);
    }
    if let Some(count) = machine.logical_processor_count {
        cpu_details.insert("logical_processor_count".to_string(), count.to_string());
    }
    devices.push(DeviceInfo {
        device_key: "cpu:system".to_string(),
        category: "cpu".to_string(),
        present: Some(true),
        classification: "system_total".to_string(),
        details: cpu_details,
    });
    capabilities.push(capability(
        "cpu:system",
        "cpu",
        "win32-system-times",
        ReadStatus::Value,
        "ok",
        "system CPU time counters provide interval usage; frequency is reported only as OS-exposed information",
    ));

    devices.push(DeviceInfo {
        device_key: "memory:physical".to_string(),
        category: "memory".to_string(),
        present: Some(true),
        classification: "physical".to_string(),
        details: BTreeMap::from([(
            "total_bytes".to_string(),
            machine
                .memory_total_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        )]),
    });
    capabilities.push(capability(
        "memory:physical",
        "memory",
        "global-memory-status-ex",
        ReadStatus::Value,
        "ok",
        "available memory is Windows-reported available physical memory; used is total minus available",
    ));

    if include_disk {
        devices.push(DeviceInfo {
            device_key: "disk:physical-total".to_string(),
            category: "disk".to_string(),
            present: None,
            classification: "physical_or_unknown".to_string(),
            details: BTreeMap::new(),
        });
        let disk_probe = DiskProvider::new();
        let disk_status = disk_probe.status;
        let disk_reason = disk_probe.reason_code.clone();
        capabilities.push(capability(
            "disk:physical-total",
            "disk",
            "pdh-physical-disk-total",
            disk_status,
            disk_reason,
            "PDH PhysicalDisk(_Total) is intended as a physical-disk total; Windows provider classification is not independently enumerated",
        ));
    }

    if include_network {
        let network_probe = network_interfaces();
        let network_status = network_probe.status;
        let network_reason = network_probe.reason_code.clone();
        devices.push(DeviceInfo {
            device_key: "network:interfaces".to_string(),
            category: "network".to_string(),
            present: Some(network_status == ReadStatus::Value),
            classification: "per-interface-physicality-may-be-unknown".to_string(),
            details: BTreeMap::new(),
        });
        capabilities.push(capability(
            "network:interfaces",
            "network",
            "ip-helper-get-if-table2",
            network_status,
            network_reason,
            "interface octet counters are cumulative; interval throughput is derived from counter deltas and wall time",
        ));
    }

    if include_process {
        devices.push(DeviceInfo {
            device_key: "processes:system".to_string(),
            category: "process".to_string(),
            present: Some(true),
            classification: "system-enumeration".to_string(),
            details: BTreeMap::new(),
        });
        capabilities.push(capability(
            "processes:system",
            "process",
            "psapi-enum-processes",
            ReadStatus::Value,
            "ok",
            "process details are kept in memory only; protected and exited processes can be inaccessible",
        ));
    }

    if include_power {
        let power = power_info();
        let battery_present = power.value.as_ref().map(|value| value.battery_present);
        devices.push(DeviceInfo {
            device_key: "power:system".to_string(),
            category: "power".to_string(),
            present: Some(true),
            classification: "system-power-status".to_string(),
            details: BTreeMap::from([(
                "battery_present".to_string(),
                battery_present
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            )]),
        });
        devices.push(DeviceInfo {
            device_key: "battery:system".to_string(),
            category: "battery".to_string(),
            present: battery_present,
            classification: "windows-system-power-status".to_string(),
            details: BTreeMap::new(),
        });
        capabilities.push(capability(
            "power:system",
            "power",
            "get-system-power-status",
            power.status,
            power.reason_code,
            "battery percentage is omitted when no battery is present; system saver flag is exposed only when Windows provides it",
        ));
    }

    devices.push(DeviceInfo {
        device_key: "system:uptime".to_string(),
        category: "system".to_string(),
        present: Some(true),
        classification: "boot-time-derived".to_string(),
        details: BTreeMap::new(),
    });
    capabilities.push(capability(
        "system:uptime",
        "system",
        "get-tick-count-64",
        ReadStatus::Value,
        "ok",
        "uptime is monotonic time since Windows boot and does not provide a UTC boot timestamp",
    ));

    devices.push(DeviceInfo {
        device_key: "probe:current-process".to_string(),
        category: "probe".to_string(),
        present: Some(true),
        classification: "self-resource-accounting".to_string(),
        details: BTreeMap::new(),
    });
    capabilities.push(capability(
        "probe:current-process",
        "probe",
        "current-process-win32",
        ReadStatus::Value,
        "ok",
        "resource counters describe this probe process only and include API/reporting work performed by it",
    ));

    (devices, capabilities)
}

pub fn cpu_times() -> ReadResult<CpuTimes> {
    unsafe {
        let mut idle = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        match GetSystemTimes(
            Some(&mut idle as *mut _),
            Some(&mut kernel as *mut _),
            Some(&mut user as *mut _),
        ) {
            Ok(()) => ReadResult::value(CpuTimes {
                idle_100ns: filetime_to_u64(idle),
                kernel_100ns: filetime_to_u64(kernel),
                user_100ns: filetime_to_u64(user),
            }),
            Err(_) => ReadResult::status(ReadStatus::Failed, "get_system_times_failed"),
        }
    }
}

pub fn memory_info() -> ReadResult<MemoryInfo> {
    unsafe {
        let mut status = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        match GlobalMemoryStatusEx(&mut status) {
            Ok(()) => {
                let used = status.ullTotalPhys.saturating_sub(status.ullAvailPhys);
                let usage = if status.ullTotalPhys == 0 {
                    0.0
                } else {
                    used as f64 * 100.0 / status.ullTotalPhys as f64
                };
                ReadResult::value(MemoryInfo {
                    total_bytes: status.ullTotalPhys,
                    available_bytes: status.ullAvailPhys,
                    used_bytes: used,
                    usage_percent: usage,
                })
            }
            Err(_) => ReadResult::status(ReadStatus::Failed, "global_memory_status_failed"),
        }
    }
}

pub fn cpu_frequency_info() -> ReadResult<CpuFrequencyInfo> {
    let mut system_info = SYSTEM_INFO::default();
    unsafe { GetNativeSystemInfo(&mut system_info) };
    let processor_count = system_info.dwNumberOfProcessors.max(1);
    let mut values = vec![PROCESSOR_POWER_INFORMATION::default(); processor_count as usize];
    unsafe {
        let status = CallNtPowerInformation(
            ProcessorInformation,
            None,
            0,
            Some(values.as_mut_ptr() as *mut _),
            (values.len() * size_of::<PROCESSOR_POWER_INFORMATION>()) as u32,
        );
        if status.0 != 0 {
            return ReadResult::status(
                ReadStatus::Unsupported,
                "processor_frequency_api_unavailable",
            );
        }
    }
    let current: Vec<_> = values
        .iter()
        .map(|value| value.CurrentMhz)
        .filter(|value| *value > 0)
        .collect();
    let max: Vec<_> = values
        .iter()
        .map(|value| value.MaxMhz)
        .filter(|value| *value > 0)
        .collect();
    if current.is_empty() && max.is_empty() {
        return ReadResult::status(
            ReadStatus::Unsupported,
            "processor_frequency_values_unavailable",
        );
    }
    ReadResult::value(CpuFrequencyInfo {
        current_mhz: (!current.is_empty())
            .then(|| current.iter().map(|value| *value as f64).sum::<f64>() / current.len() as f64),
        max_mhz: (!max.is_empty())
            .then(|| max.iter().map(|value| *value as f64).sum::<f64>() / max.len() as f64),
    })
}

pub fn power_info() -> ReadResult<PowerInfo> {
    unsafe {
        let mut status = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut status).is_err() {
            return ReadResult::status(ReadStatus::Failed, "get_system_power_status_failed");
        }
        let battery_present = status.BatteryFlag != 128 && status.BatteryLifePercent != 255;
        let ac_line_status = match status.ACLineStatus {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        };
        let battery_percent = battery_present
            .then_some(status.BatteryLifePercent)
            .filter(|value| *value <= 100);
        ReadResult::value(PowerInfo {
            battery_present,
            ac_line_status,
            battery_percent,
            saver_active: Some(status.SystemStatusFlag != 0),
        })
    }
}

pub fn uptime_ms() -> u64 {
    unsafe { GetTickCount64() }
}

pub fn process_access_summary() -> ReadResult<ProcessAccessSummary> {
    let started = std::time::Instant::now();
    unsafe {
        let mut ids = vec![0_u32; 4096];
        let mut bytes_needed = 0_u32;
        if EnumProcesses(
            ids.as_mut_ptr(),
            (ids.len() * size_of::<u32>()) as u32,
            &mut bytes_needed,
        )
        .is_err()
        {
            return ReadResult::status(ReadStatus::Failed, "enum_processes_failed");
        }
        let process_count = (bytes_needed as usize / size_of::<u32>()).min(ids.len());
        let process_ids: Vec<u32> = ids
            .into_iter()
            .take(process_count)
            .filter(|pid| is_probeable_process_id(*pid))
            .collect();
        let enumerated = process_ids.len() as u32;
        let mut accessible = 0_u32;
        let mut restricted = 0_u32;
        for pid in process_ids {
            match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                Ok(handle) => {
                    accessible += 1;
                    close_handle(handle);
                }
                Err(_) => restricted += 1,
            }
        }
        ReadResult::value(ProcessAccessSummary {
            enumerated,
            accessible,
            restricted,
            elapsed: started.elapsed(),
        })
    }
}

pub fn process_detail_summary() -> ReadResult<ProcessDetailSummary> {
    unsafe {
        let mut ids = vec![0_u32; 4096];
        let mut bytes_needed = 0_u32;
        if EnumProcesses(
            ids.as_mut_ptr(),
            (ids.len() * size_of::<u32>()) as u32,
            &mut bytes_needed,
        )
        .is_err()
        {
            return ReadResult::status(ReadStatus::Failed, "enum_processes_for_detail_failed");
        }
        let mut summary = ProcessDetailSummary {
            attempted: 0,
            readable_cpu_time: 0,
            readable_working_set: 0,
            readable_private_memory: 0,
            readable_io: 0,
            permission_denied: 0,
            probe_failed: 0,
            raced: 0,
        };
        for pid in ids
            .into_iter()
            .take((bytes_needed as usize / size_of::<u32>()).min(4096))
            .filter(|pid| is_probeable_process_id(*pid))
        {
            summarize_process_detail(&mut summary, &process_metrics(pid));
        }
        ReadResult::value(summary)
    }
}

pub fn process_metrics(pid: u32) -> ReadResult<ProcessMetrics> {
    if !is_probeable_process_id(pid) {
        return ReadResult::status(ReadStatus::Unsupported, "pid_zero_excluded");
    }

    unsafe {
        // Windows 10/11 support all three detail APIs with limited query access. Keep one
        // handle per process, while preserving independent results for each child read.
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let cpu_time = read_process_cpu_time(handle);
                let memory = read_process_memory(handle);
                let (read_bytes, write_bytes) = read_process_io_counters(handle);
                close_handle(handle);
                ReadResult::value(ProcessMetrics {
                    cpu_time_100ns: cpu_time,
                    working_set_bytes: memory.0,
                    private_bytes: memory.1,
                    read_bytes,
                    write_bytes,
                })
            }
            Err(error) => {
                let failure = classify_process_error(&error, "open_process_limited_query");
                ReadResult::value(ProcessMetrics {
                    cpu_time_100ns: ReadResult::status(failure.status, failure.reason_code.clone()),
                    working_set_bytes: ReadResult::status(
                        failure.status,
                        failure.reason_code.clone(),
                    ),
                    private_bytes: ReadResult::status(failure.status, failure.reason_code.clone()),
                    read_bytes: ReadResult::status(failure.status, failure.reason_code.clone()),
                    write_bytes: ReadResult::status(failure.status, failure.reason_code),
                })
            }
        }
    }
}

fn read_process_cpu_time(handle: HANDLE) -> ReadResult<u64> {
    unsafe {
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        match GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) {
            Ok(()) => {
                ReadResult::value(filetime_to_u64(kernel).saturating_add(filetime_to_u64(user)))
            }
            Err(error) => process_read_failure(&error, "get_process_times"),
        }
    }
}

fn read_process_io_counters(handle: HANDLE) -> (ReadResult<u64>, ReadResult<u64>) {
    unsafe {
        let mut io = IO_COUNTERS::default();
        match GetProcessIoCounters(handle, &mut io) {
            Ok(()) => (
                ReadResult::value(io.ReadTransferCount),
                ReadResult::value(io.WriteTransferCount),
            ),
            Err(error) => {
                let failure = classify_process_error(&error, "get_process_io_counters");
                (
                    ReadResult::status(failure.status, failure.reason_code.clone()),
                    ReadResult::status(failure.status, failure.reason_code),
                )
            }
        }
    }
}

fn read_process_memory(handle: HANDLE) -> (ReadResult<u64>, ReadResult<u64>) {
    unsafe {
        let mut memory = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        match GetProcessMemoryInfo(
            handle,
            &mut memory as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
            size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ) {
            Ok(()) => (
                ReadResult::value(memory.WorkingSetSize as u64),
                ReadResult::value(memory.PrivateUsage as u64),
            ),
            Err(error) => {
                let failure = classify_process_error(&error, "get_process_memory_info");
                (
                    ReadResult::status(failure.status, failure.reason_code.clone()),
                    ReadResult::status(failure.status, failure.reason_code),
                )
            }
        }
    }
}

fn summarize_process_detail(
    summary: &mut ProcessDetailSummary,
    result: &ReadResult<ProcessMetrics>,
) {
    summary.attempted += 1;
    let Some(metrics) = result.value.as_ref() else {
        record_process_failure(summary, result.status, &result.reason_code);
        return;
    };

    summary.readable_cpu_time += read_succeeded(&metrics.cpu_time_100ns) as u32;
    summary.readable_working_set += read_succeeded(&metrics.working_set_bytes) as u32;
    summary.readable_private_memory += read_succeeded(&metrics.private_bytes) as u32;
    summary.readable_io +=
        (read_succeeded(&metrics.read_bytes) && read_succeeded(&metrics.write_bytes)) as u32;

    let reads = [
        &metrics.cpu_time_100ns,
        &metrics.working_set_bytes,
        &metrics.private_bytes,
        &metrics.read_bytes,
        &metrics.write_bytes,
    ];
    let has_permission_denied = reads
        .iter()
        .any(|read| read.status == ReadStatus::PermissionDenied);
    let has_race = reads
        .iter()
        .any(|read| is_process_race_reason(&read.reason_code));
    let has_non_race_failure = reads.iter().any(|read| {
        read.status == ReadStatus::Failed && !is_process_race_reason(&read.reason_code)
    });
    if has_permission_denied {
        summary.permission_denied += 1;
    }
    if has_race {
        summary.raced += 1;
    }
    if has_non_race_failure {
        summary.probe_failed += 1;
    }
}

fn read_succeeded<T>(result: &ReadResult<T>) -> bool {
    result.status == ReadStatus::Value && result.value.is_some()
}

fn record_process_failure(
    summary: &mut ProcessDetailSummary,
    status: ReadStatus,
    reason_code: &str,
) {
    match status {
        ReadStatus::PermissionDenied => summary.permission_denied += 1,
        ReadStatus::Failed if is_process_race_reason(reason_code) => summary.raced += 1,
        ReadStatus::Failed => summary.probe_failed += 1,
        ReadStatus::RuntimeFailed => summary.probe_failed += 1,
        ReadStatus::Value | ReadStatus::Unsupported | ReadStatus::ProviderMissing => {}
    }
}

fn is_process_race_reason(reason_code: &str) -> bool {
    reason_code.ends_with("_process_exited_or_raced") || reason_code == "process_exited_or_raced"
}

fn process_read_failure<T>(error: &windows::core::Error, operation: &str) -> ReadResult<T> {
    let failure = classify_process_error(error, operation);
    ReadResult::status(failure.status, failure.reason_code)
}

fn classify_process_error(error: &windows::core::Error, operation: &str) -> ProcessFailure {
    let code = error.code();
    let status = if code == HRESULT::from_win32(ERROR_ACCESS_DENIED.0) {
        ReadStatus::PermissionDenied
    } else {
        ReadStatus::Failed
    };
    let reason_code = if status == ReadStatus::PermissionDenied {
        format!("{operation}_access_denied")
    } else if [
        HRESULT::from_win32(ERROR_INVALID_HANDLE.0),
        HRESULT::from_win32(ERROR_INVALID_PARAMETER.0),
        HRESULT::from_win32(ERROR_PARTIAL_COPY.0),
        HRESULT::from_win32(ERROR_PROCESS_ABORTED.0),
    ]
    .contains(&code)
    {
        format!("{operation}_process_exited_or_raced")
    } else {
        format!("{operation}_failed")
    };
    ProcessFailure {
        status,
        reason_code,
    }
}

fn is_probeable_process_id(pid: u32) -> bool {
    pid != 0
}

pub fn self_metrics() -> ReadResult<SelfMetrics> {
    unsafe {
        let process = GetCurrentProcess();
        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        if GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user).is_err() {
            return ReadResult::status(ReadStatus::Failed, "self_process_times_failed");
        }
        let mut memory = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        if GetProcessMemoryInfo(
            process,
            &mut memory as *mut PROCESS_MEMORY_COUNTERS_EX as *mut _,
            size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
        .is_err()
        {
            return ReadResult::status(ReadStatus::Failed, "self_memory_info_failed");
        }
        let mut handle_count = 0_u32;
        if GetProcessHandleCount(process, &mut handle_count).is_err() {
            return ReadResult::status(ReadStatus::Failed, "self_handle_count_failed");
        }
        let thread_count = thread_count_for_process(GetCurrentProcessId());
        ReadResult::value(SelfMetrics {
            cpu_time_100ns: filetime_to_u64(kernel).saturating_add(filetime_to_u64(user)),
            working_set_bytes: memory.WorkingSetSize as u64,
            thread_count,
            handle_count,
        })
    }
}

pub fn network_interfaces() -> ReadResult<Vec<NetworkInterfaceSnapshot>> {
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        let result = GetIfTable2(&mut table);
        if result.0 != 0 || table.is_null() {
            return ReadResult::status(ReadStatus::Failed, "get_if_table_failed");
        }
        let entries =
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize);
        let snapshots = entries.iter().map(network_snapshot).collect();
        FreeMibTable(table as *const _);
        ReadResult::value(snapshots)
    }
}

pub fn elevated_status() -> Option<bool> {
    unsafe {
        let process = GetCurrentProcess();
        let mut token = HANDLE::default();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_err() {
            return None;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut return_length = 0_u32;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut TOKEN_ELEVATION as *mut _),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        )
        .is_ok();
        close_handle(token);
        result.then_some(elevation.TokenIsElevated != 0)
    }
}

fn capability(
    device_key: &str,
    category: &str,
    provider: &str,
    status: ReadStatus,
    reason_code: impl Into<String>,
    limitation: &str,
) -> Capability {
    Capability {
        device_key: device_key.to_string(),
        category: category.to_string(),
        provider: provider.to_string(),
        support_status: support_status(status),
        reason_code: reason_code.into(),
        details: BTreeMap::new(),
        source: SOURCE_SYSTEM_INFO.to_string(),
        known_semantic_limitations: vec![limitation.to_string()],
    }
}

fn support_status(status: ReadStatus) -> SupportStatus {
    match status {
        ReadStatus::Value => SupportStatus::Supported,
        ReadStatus::Unsupported => SupportStatus::Unsupported,
        ReadStatus::PermissionDenied => SupportStatus::PermissionDenied,
        ReadStatus::ProviderMissing => SupportStatus::ProviderMissing,
        ReadStatus::Failed => SupportStatus::ProbeFailed,
        ReadStatus::RuntimeFailed => SupportStatus::RuntimeFailed,
    }
}

fn network_snapshot(row: &MIB_IF_ROW2) -> NetworkInterfaceSnapshot {
    NetworkInterfaceSnapshot {
        device_key: format!("network:interface:{:08x}", row.InterfaceIndex),
        category: "network_interface".to_string(),
        classification: network_classification(row),
        interface_type: row.Type,
        in_octets: row.InOctets,
        out_octets: row.OutOctets,
    }
}

fn network_classification(row: &MIB_IF_ROW2) -> String {
    match row.Type {
        IF_TYPE_ETHERNET_CSMACD => "physical_candidate_ethernet".to_string(),
        IF_TYPE_IEEE80211 => "physical_candidate_wifi".to_string(),
        IF_TYPE_SOFTWARE_LOOPBACK => "virtual_loopback".to_string(),
        IF_TYPE_TUNNEL | IF_TYPE_PPP | IF_TYPE_SLIP => "virtual_or_tunnel".to_string(),
        _ => "unknown".to_string(),
    }
}

fn thread_count_for_process(pid: u32) -> u32 {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            Ok(snapshot) => snapshot,
            Err(_) => return 0,
        };
        let mut entry = THREADENTRY32 {
            dwSize: size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut count = 0;
        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    count += 1;
                }
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        close_handle(snapshot);
        count
    }
}

fn filetime_to_u64(filetime: FILETIME) -> u64 {
    ((filetime.dwHighDateTime as u64) << 32) | filetime.dwLowDateTime as u64
}

fn close_handle(handle: HANDLE) {
    if !handle.is_invalid() {
        unsafe {
            let _ = CloseHandle(handle);
        }
    }
}

fn formatted_counter(counter: isize) -> Option<f64> {
    let mut value = std::mem::MaybeUninit::<PDH_FMT_COUNTERVALUE>::uninit();
    unsafe {
        if PdhGetFormattedCounterValue(counter, PDH_FMT_DOUBLE, None, value.as_mut_ptr()) != 0 {
            return None;
        }
        let value = value.assume_init();
        (value.CStatus == 0).then_some(value.Anonymous.doubleValue)
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn registry_string(subkey: &str, value_name: &str) -> Option<String> {
    unsafe {
        let subkey = wide(subkey);
        let value_name = wide(value_name);
        let mut buffer = vec![0_u16; 512];
        let mut bytes = (buffer.len() * size_of::<u16>()) as u32;
        let status = RegGetValueW(
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

fn os_version() -> Option<OSVERSIONINFOW> {
    unsafe {
        let mut version = OSVERSIONINFOW {
            dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
            ..Default::default()
        };
        GetVersionExW(&mut version).ok().map(|_| version)
    }
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

#[cfg(test)]
mod tests {
    use super::{
        is_probeable_process_id, mahm_layout_matches, network_classification,
        summarize_process_detail, validate_mahm_layout, validate_mapped_regions,
        AfterburnerSharedMemory, MappedMemoryRegion, MappingValidationError, ProcessDetailSummary,
        ProcessMetrics, ReadResult, ReadStatus,
    };
    use windows::Win32::NetworkManagement::IpHelper::{
        IF_TYPE_IEEE80211, IF_TYPE_SOFTWARE_LOOPBACK, MIB_IF_ROW2,
    };
    use windows::Win32::System::Memory::{
        MEM_COMMIT, MEM_FREE, MEM_RESERVE, PAGE_GUARD, PAGE_NOACCESS, PAGE_READONLY,
    };

    fn empty_summary() -> ProcessDetailSummary {
        ProcessDetailSummary {
            attempted: 0,
            readable_cpu_time: 0,
            readable_working_set: 0,
            readable_private_memory: 0,
            readable_io: 0,
            permission_denied: 0,
            probe_failed: 0,
            raced: 0,
        }
    }

    fn metrics(
        cpu_time: ReadResult<u64>,
        working_set: ReadResult<u64>,
        private_memory: ReadResult<u64>,
        read_bytes: ReadResult<u64>,
        write_bytes: ReadResult<u64>,
    ) -> ReadResult<ProcessMetrics> {
        ReadResult::value(ProcessMetrics {
            cpu_time_100ns: cpu_time,
            working_set_bytes: working_set,
            private_bytes: private_memory,
            read_bytes,
            write_bytes,
        })
    }

    fn value(value: u64) -> ReadResult<u64> {
        ReadResult::value(value)
    }

    fn failed(reason_code: &str) -> ReadResult<u64> {
        ReadResult::status(ReadStatus::Failed, reason_code)
    }

    fn denied(reason_code: &str) -> ReadResult<u64> {
        ReadResult::status(ReadStatus::PermissionDenied, reason_code)
    }

    #[test]
    fn network_classification_preserves_unknowns() {
        let row = MIB_IF_ROW2 {
            Type: IF_TYPE_SOFTWARE_LOOPBACK,
            ..Default::default()
        };
        assert_eq!(network_classification(&row), "virtual_loopback");
        let row = MIB_IF_ROW2 {
            Type: IF_TYPE_IEEE80211,
            ..Default::default()
        };
        assert_eq!(network_classification(&row), "physical_candidate_wifi");
        let row = MIB_IF_ROW2 {
            Type: 999,
            ..Default::default()
        };
        assert_eq!(network_classification(&row), "unknown");
    }

    #[test]
    fn cpu_time_success_is_counted_when_memory_fails() {
        let mut summary = empty_summary();
        summarize_process_detail(
            &mut summary,
            &metrics(
                value(10),
                failed("memory_failed"),
                failed("memory_failed"),
                value(20),
                value(30),
            ),
        );
        assert_eq!(summary.attempted, 1);
        assert_eq!(summary.readable_cpu_time, 1);
        assert_eq!(summary.readable_working_set, 0);
        assert_eq!(summary.readable_private_memory, 0);
        assert_eq!(summary.readable_io, 1);
        assert_eq!(summary.probe_failed, 1);
    }

    #[test]
    fn memory_success_is_counted_when_io_fails() {
        let mut summary = empty_summary();
        summarize_process_detail(
            &mut summary,
            &metrics(
                value(10),
                value(20),
                value(30),
                failed("io_failed"),
                failed("io_failed"),
            ),
        );
        assert_eq!(summary.attempted, 1);
        assert_eq!(summary.readable_cpu_time, 1);
        assert_eq!(summary.readable_working_set, 1);
        assert_eq!(summary.readable_private_memory, 1);
        assert_eq!(summary.readable_io, 0);
        assert_eq!(summary.probe_failed, 1);
    }

    #[test]
    fn one_failed_metric_does_not_pollute_other_metric_counts() {
        let mut summary = empty_summary();
        summarize_process_detail(
            &mut summary,
            &metrics(
                denied("cpu_denied"),
                value(20),
                value(30),
                value(40),
                value(50),
            ),
        );
        assert_eq!(summary.readable_cpu_time, 0);
        assert_eq!(summary.readable_working_set, 1);
        assert_eq!(summary.readable_private_memory, 1);
        assert_eq!(summary.readable_io, 1);
        assert_eq!(summary.permission_denied, 1);
        assert_eq!(summary.probe_failed, 0);
    }

    #[test]
    fn multiple_denied_child_reads_count_once_per_process() {
        let mut summary = empty_summary();
        summarize_process_detail(
            &mut summary,
            &metrics(
                denied("cpu_denied"),
                denied("memory_denied"),
                denied("memory_denied"),
                denied("io_denied"),
                denied("io_denied"),
            ),
        );
        assert_eq!(summary.permission_denied, 1);
        assert_eq!(summary.readable_cpu_time, 0);
        assert_eq!(summary.readable_working_set, 0);
        assert_eq!(summary.readable_private_memory, 0);
        assert_eq!(summary.readable_io, 0);
    }

    #[test]
    fn failed_and_denied_results_do_not_become_zero_success_samples() {
        let mut summary = empty_summary();
        summarize_process_detail(
            &mut summary,
            &ReadResult::status(ReadStatus::PermissionDenied, "open_denied"),
        );
        summarize_process_detail(
            &mut summary,
            &ReadResult::status(ReadStatus::Failed, "process_process_exited_or_raced"),
        );
        assert_eq!(summary.attempted, 2);
        assert_eq!(summary.readable_cpu_time, 0);
        assert_eq!(summary.readable_working_set, 0);
        assert_eq!(summary.readable_private_memory, 0);
        assert_eq!(summary.readable_io, 0);
        assert_eq!(summary.permission_denied, 1);
        assert_eq!(summary.probe_failed, 0);
        assert_eq!(summary.raced, 1);
    }

    #[test]
    fn pid_zero_is_excluded_from_process_detail_attempts() {
        assert!(!is_probeable_process_id(0));
        assert!(is_probeable_process_id(1));
    }

    #[test]
    fn missing_reference_mapping_is_provider_missing() {
        let result = AfterburnerSharedMemory::open_named(
            "ResourceTimelineCpuSensorProbeMappingThatDoesNotExist",
        );
        assert_eq!(result.status, ReadStatus::ProviderMissing);
        assert_eq!(result.reason_code, "afterburner_shared_memory_missing");
        assert!(result.value.is_none());
    }

    #[test]
    fn invalid_reference_values_are_not_accepted() {
        assert!(super::valid_sensor_value(f32::NAN).is_none());
        assert!(super::valid_sensor_value(f32::INFINITY).is_none());
        assert!(super::valid_sensor_value(f32::MAX).is_none());
        assert_eq!(super::valid_sensor_value(42.5), Some(42.5));
    }

    fn region(
        base_address: usize,
        allocation_base: usize,
        region_size: usize,
        state: u32,
        protection: u32,
    ) -> MappedMemoryRegion {
        MappedMemoryRegion {
            base_address,
            allocation_base,
            region_size,
            state,
            protection,
        }
    }

    #[test]
    fn mapped_range_accepts_committed_readable_regions_with_one_allocation_base() {
        let allocation_base = 0x1000;
        let regions = [
            region(
                0x1000,
                allocation_base,
                0x1000,
                MEM_COMMIT.0,
                PAGE_READONLY.0,
            ),
            region(
                0x2000,
                allocation_base,
                0x1000,
                MEM_COMMIT.0,
                PAGE_READONLY.0,
            ),
        ];

        assert_eq!(
            validate_mapped_regions(0x1000, 0x1800, None, &regions),
            Ok(allocation_base)
        );
    }

    #[test]
    fn mapped_range_rejects_uncommitted_regions() {
        for state in [MEM_RESERVE.0, MEM_FREE.0] {
            let result = validate_mapped_regions(
                0x1000,
                0x100,
                None,
                &[region(0x1000, 0x1000, 0x1000, state, PAGE_READONLY.0)],
            );
            assert_eq!(result, Err(MappingValidationError::NotCommitted));
        }
    }

    #[test]
    fn mapped_range_rejects_noaccess_and_guard_pages() {
        let noaccess = validate_mapped_regions(
            0x1000,
            0x100,
            None,
            &[region(
                0x1000,
                0x1000,
                0x1000,
                MEM_COMMIT.0,
                PAGE_NOACCESS.0,
            )],
        );
        assert_eq!(noaccess, Err(MappingValidationError::NoAccess));

        let guard = validate_mapped_regions(
            0x1000,
            0x100,
            None,
            &[region(
                0x1000,
                0x1000,
                0x1000,
                MEM_COMMIT.0,
                PAGE_READONLY.0 | PAGE_GUARD.0,
            )],
        );
        assert_eq!(guard, Err(MappingValidationError::GuardPage));
    }

    #[test]
    fn mapped_range_rejects_allocation_base_changes_and_uncovered_ranges() {
        let allocation_change = validate_mapped_regions(
            0x1000,
            0x1800,
            None,
            &[
                region(0x1000, 0x1000, 0x1000, MEM_COMMIT.0, PAGE_READONLY.0),
                region(0x2000, 0x2000, 0x1000, MEM_COMMIT.0, PAGE_READONLY.0),
            ],
        );
        assert_eq!(
            allocation_change,
            Err(MappingValidationError::AllocationBaseChanged)
        );

        let uncovered = validate_mapped_regions(
            0x1000,
            0x1800,
            None,
            &[region(0x1000, 0x1000, 0x400, MEM_COMMIT.0, PAGE_READONLY.0)],
        );
        assert_eq!(uncovered, Err(MappingValidationError::RangeNotCovered));
    }

    #[test]
    fn mahm_layout_rejects_overflow_and_oversized_entry_count() {
        assert_eq!(
            validate_mahm_layout(
                usize::MAX - 1,
                super::size_of::<super::MahmSharedMemoryEntry>(),
                2
            ),
            Err(super::MahmLayoutError::EntrySizeOverflow)
        );
        assert_eq!(
            validate_mahm_layout(
                super::size_of::<super::MahmSharedMemoryHeader>(),
                super::size_of::<super::MahmSharedMemoryEntry>(),
                super::MAHM_MAX_ENTRY_COUNT + 1,
            ),
            Err(super::MahmLayoutError::EntryCountTooLarge)
        );
    }

    #[test]
    fn mahm_layout_changes_are_rejected_after_open_validation() {
        let header_size = super::size_of::<super::MahmSharedMemoryHeader>();
        let entry_size = super::size_of::<super::MahmSharedMemoryEntry>();
        let entry_count = 3;
        let mapping_length = validate_mahm_layout(header_size, entry_size, entry_count).unwrap();

        assert!(mahm_layout_matches(
            header_size,
            entry_size,
            entry_count,
            header_size,
            entry_size,
            entry_count,
            mapping_length,
        ));
        assert!(!mahm_layout_matches(
            header_size + 1,
            entry_size,
            entry_count,
            header_size,
            entry_size,
            entry_count,
            mapping_length,
        ));
        assert!(!mahm_layout_matches(
            header_size,
            entry_size + 1,
            entry_count,
            header_size,
            entry_size,
            entry_count,
            mapping_length,
        ));
        assert!(!mahm_layout_matches(
            header_size,
            entry_size,
            entry_count + 1,
            header_size,
            entry_size,
            entry_count,
            mapping_length,
        ));
    }
}
