#![cfg(windows)]

use crate::{
    model::{Capability, DeviceInfo, SupportStatus},
    windows::{ReadResult, ReadStatus, SOURCE_NVML},
};
use std::{
    collections::{BTreeMap, VecDeque},
    ffi::{c_char, c_void},
    mem,
    path::PathBuf,
    ptr,
    sync::{Arc, Mutex},
    time::Instant,
};
use windows::{
    core::{PCSTR, PCWSTR},
    Win32::{
        Foundation::{FreeLibrary, HANDLE, HMODULE},
        System::LibraryLoader::{
            GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_FLAGS, LOAD_LIBRARY_SEARCH_SYSTEM32,
        },
    },
};

const NVML_SUCCESS: u32 = 0;
const NVML_ERROR_UNINITIALIZED: u32 = 1;
const NVML_ERROR_INVALID_ARGUMENT: u32 = 2;
const NVML_ERROR_NOT_SUPPORTED: u32 = 3;
const NVML_ERROR_NO_PERMISSION: u32 = 4;
const NVML_ERROR_ALREADY_INITIALIZED: u32 = 5;
const NVML_ERROR_NOT_FOUND: u32 = 6;
const NVML_ERROR_INSUFFICIENT_SIZE: u32 = 7;
const NVML_ERROR_INSUFFICIENT_POWER: u32 = 8;
const NVML_ERROR_DRIVER_NOT_LOADED: u32 = 9;
const NVML_ERROR_TIMEOUT: u32 = 10;
const NVML_ERROR_IRQ_ISSUE: u32 = 11;
const NVML_ERROR_LIBRARY_NOT_FOUND: u32 = 12;
const NVML_ERROR_FUNCTION_NOT_FOUND: u32 = 13;
const NVML_ERROR_CORRUPTED_IN_USE: u32 = 14;
const NVML_ERROR_GPU_IS_LOST: u32 = 15;
const NVML_ERROR_RESET_REQUIRED: u32 = 16;
const NVML_ERROR_OPERATING_SYSTEM: u32 = 17;
const NVML_ERROR_LIB_RM_VERSION_MISMATCH: u32 = 18;
const NVML_ERROR_IN_USE: u32 = 19;
const NVML_ERROR_MEMORY: u32 = 20;
const NVML_ERROR_NO_DATA: u32 = 21;
const NVML_ERROR_VGPU_ECC_NOT_SUPPORTED: u32 = 22;
const NVML_ERROR_INSUFFICIENT_RESOURCES: u32 = 23;
const NVML_ERROR_FREQ_NOT_SUPPORTED: u32 = 24;
const NVML_ERROR_UNKNOWN: u32 = 999;

const NVML_TEMPERATURE_GPU: u32 = 0;
const NVML_CLOCK_GRAPHICS: u32 = 0;
const NVML_CLOCK_MEM: u32 = 2;
pub const NO_COMPATIBLE_GPU_REASON: &str = "no_compatible_nvidia_gpu";

type NvmlDevice = *mut c_void;
type NvmlReturn = u32;
type NvmlInitV2 = unsafe extern "C" fn() -> NvmlReturn;
type NvmlShutdown = unsafe extern "C" fn() -> NvmlReturn;
type NvmlSystemGetDriverVersion = unsafe extern "C" fn(*mut c_char, u32) -> NvmlReturn;
type NvmlDeviceGetCountV2 = unsafe extern "C" fn(*mut u32) -> NvmlReturn;
type NvmlDeviceGetHandleByIndexV2 = unsafe extern "C" fn(u32, *mut NvmlDevice) -> NvmlReturn;
type NvmlDeviceGetName = unsafe extern "C" fn(NvmlDevice, *mut c_char, u32) -> NvmlReturn;
type NvmlDeviceGetUtilizationRates =
    unsafe extern "C" fn(NvmlDevice, *mut NvmlUtilization) -> NvmlReturn;
type NvmlDeviceGetTemperature = unsafe extern "C" fn(NvmlDevice, u32, *mut u32) -> NvmlReturn;
type NvmlDeviceGetPowerUsage = unsafe extern "C" fn(NvmlDevice, *mut u32) -> NvmlReturn;
type NvmlDeviceGetClockInfo = unsafe extern "C" fn(NvmlDevice, u32, *mut u32) -> NvmlReturn;
type NvmlDeviceGetMemoryInfo = unsafe extern "C" fn(NvmlDevice, *mut NvmlMemory) -> NvmlReturn;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct NvmlUtilization {
    gpu: u32,
    memory: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct NvmlMemory {
    total: u64,
    free: u64,
    used: u64,
}

#[derive(Debug, Clone)]
pub struct TimedRead<T> {
    pub result: ReadResult<T>,
    pub latency_ms: f64,
}

#[derive(Debug, Clone)]
pub struct GpuDeviceSample {
    pub device_key: String,
    pub utilization_percent: TimedRead<f64>,
    pub memory_controller_utilization_percent: TimedRead<f64>,
    pub temperature_celsius: TimedRead<f64>,
    pub power_watts: TimedRead<f64>,
    pub graphics_clock_mhz: TimedRead<f64>,
    pub memory_clock_mhz: TimedRead<f64>,
    pub vram_used_bytes: TimedRead<f64>,
    pub vram_total_bytes: TimedRead<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NvmlInjection {
    MissingLibrary,
    PartialUnsupported,
    TransientMetricFailure,
    ProviderRuntimeFailure,
}

#[derive(Debug, Clone, Default)]
pub struct NvmlStats {
    pub library_load_attempts: u64,
    pub library_load_successes: u64,
    pub library_release_count: u64,
    pub init_calls: u64,
    pub init_successes: u64,
    pub shutdown_calls: u64,
    pub shutdown_successes: u64,
    pub sample_count: u64,
    pub failed_sample_count: u64,
    pub gpu_metric_call_count: u64,
    pub failed_gpu_metric_call_count: u64,
}

#[derive(Debug, Clone)]
pub struct NvmlStopResult {
    pub status: ReadStatus,
    pub reason_code: String,
    pub stats: NvmlStats,
}

#[derive(Debug, Clone)]
pub struct NvmlDeviceEntry {
    pub index: u32,
    pub product_name: String,
    pub handle_status: ReadStatus,
    pub handle_reason_code: String,
    handle: Option<NvmlDevice>,
}

impl NvmlDeviceEntry {
    pub fn device_key(&self) -> String {
        format!("gpu:nvidia:index-{}", self.index)
    }

    fn is_usable(&self) -> bool {
        self.handle.is_some()
    }
}

#[derive(Debug)]
struct NvmlLibrary {
    module: HMODULE,
    stats: Arc<Mutex<NvmlStats>>,
}

impl NvmlLibrary {
    fn load(stats: Arc<Mutex<NvmlStats>>) -> Result<Self, (ReadStatus, String)> {
        let mut candidates = vec![LoadCandidate::System32];
        if let Some(program_w6432) = std::env::var_os("ProgramW6432") {
            candidates.push(LoadCandidate::KnownPath(
                PathBuf::from(program_w6432)
                    .join("NVIDIA Corporation")
                    .join("NVSMI")
                    .join("nvml.dll"),
            ));
        }

        for candidate in candidates {
            with_stats(&stats, |stats| stats.library_load_attempts += 1);
            let (path, flags) = match candidate {
                LoadCandidate::System32 => {
                    (PathBuf::from("nvml.dll"), LOAD_LIBRARY_SEARCH_SYSTEM32)
                }
                LoadCandidate::KnownPath(path) => (path, LOAD_LIBRARY_FLAGS(0)),
            };
            let wide_path = path
                .as_os_str()
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let module = unsafe {
                LoadLibraryExW(
                    PCWSTR::from_raw(wide_path.as_ptr()),
                    HANDLE::default(),
                    flags,
                )
            };
            if let Ok(module) = module {
                with_stats(&stats, |stats| stats.library_load_successes += 1);
                return Ok(Self { module, stats });
            }
        }

        Err((
            ReadStatus::ProviderMissing,
            "nvml_runtime_missing".to_string(),
        ))
    }

    fn module(&self) -> HMODULE {
        self.module
    }
}

impl Drop for NvmlLibrary {
    fn drop(&mut self) {
        if !self.module.is_invalid() {
            unsafe {
                let _ = FreeLibrary(self.module);
            }
            with_stats(&self.stats, |stats| stats.library_release_count += 1);
        }
    }
}

enum LoadCandidate {
    System32,
    KnownPath(PathBuf),
}

#[derive(Debug, Clone, Copy)]
struct NvmlFunctions {
    init_v2: NvmlInitV2,
    shutdown: NvmlShutdown,
    system_get_driver_version: NvmlSystemGetDriverVersion,
    device_get_count_v2: NvmlDeviceGetCountV2,
    device_get_handle_by_index_v2: NvmlDeviceGetHandleByIndexV2,
    device_get_name: NvmlDeviceGetName,
    device_get_utilization_rates: NvmlDeviceGetUtilizationRates,
    device_get_temperature: NvmlDeviceGetTemperature,
    device_get_power_usage: NvmlDeviceGetPowerUsage,
    device_get_clock_info: NvmlDeviceGetClockInfo,
    device_get_memory_info: NvmlDeviceGetMemoryInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FakeMetric {
    Utilization,
    Temperature,
    Power,
    GraphicsClock,
    MemoryClock,
    Memory,
}

#[derive(Debug)]
struct FakeNvmlState {
    init_status: NvmlReturn,
    shutdown_status: NvmlReturn,
    driver_version: String,
    metric_statuses: BTreeMap<FakeMetric, VecDeque<NvmlReturn>>,
}

#[derive(Debug, Clone)]
struct FakeNvmlDispatch {
    state: Arc<Mutex<FakeNvmlState>>,
}

impl FakeNvmlDispatch {
    fn new(injection: NvmlInjection) -> Self {
        let mut metric_statuses = BTreeMap::new();
        match injection {
            NvmlInjection::PartialUnsupported => {
                metric_statuses.insert(
                    FakeMetric::Power,
                    VecDeque::from([NVML_ERROR_NOT_SUPPORTED]),
                );
                metric_statuses.insert(
                    FakeMetric::MemoryClock,
                    VecDeque::from([NVML_ERROR_FREQ_NOT_SUPPORTED]),
                );
            }
            NvmlInjection::TransientMetricFailure => {
                metric_statuses.insert(
                    FakeMetric::Power,
                    VecDeque::from([NVML_ERROR_TIMEOUT, NVML_SUCCESS]),
                );
            }
            NvmlInjection::MissingLibrary | NvmlInjection::ProviderRuntimeFailure => {}
        }
        Self {
            state: Arc::new(Mutex::new(FakeNvmlState {
                init_status: if matches!(injection, NvmlInjection::ProviderRuntimeFailure) {
                    NVML_ERROR_GPU_IS_LOST
                } else {
                    NVML_SUCCESS
                },
                shutdown_status: NVML_SUCCESS,
                driver_version: "injected-driver".to_string(),
                metric_statuses,
            })),
        }
    }

    fn next_status(&self, metric: FakeMetric) -> NvmlReturn {
        let mut state = self.state.lock().expect("fake NVML state lock poisoned");
        let Some(statuses) = state.metric_statuses.get_mut(&metric) else {
            return NVML_SUCCESS;
        };
        if statuses.len() > 1 {
            statuses.pop_front().unwrap_or(NVML_SUCCESS)
        } else {
            statuses.front().copied().unwrap_or(NVML_SUCCESS)
        }
    }

    fn init(&self) -> NvmlReturn {
        self.state
            .lock()
            .expect("fake NVML state lock poisoned")
            .init_status
    }

    fn shutdown(&self) -> NvmlReturn {
        self.state
            .lock()
            .expect("fake NVML state lock poisoned")
            .shutdown_status
    }

    fn driver_version(&self, buffer: *mut c_char, length: u32) -> NvmlReturn {
        let version = self
            .state
            .lock()
            .expect("fake NVML state lock poisoned")
            .driver_version
            .clone();
        write_c_buffer(buffer, length, &version)
    }

    fn device_count(&self, count: *mut u32) -> NvmlReturn {
        unsafe {
            *count = 1;
        }
        NVML_SUCCESS
    }

    fn device_handle(&self, _index: u32, handle: *mut NvmlDevice) -> NvmlReturn {
        unsafe {
            *handle = 1_usize as NvmlDevice;
        }
        NVML_SUCCESS
    }

    fn device_name(&self, buffer: *mut c_char, length: u32) -> NvmlReturn {
        write_c_buffer(buffer, length, "Injected NVIDIA GPU")
    }

    fn utilization(&self, utilization: *mut NvmlUtilization) -> NvmlReturn {
        let status = self.next_status(FakeMetric::Utilization);
        if status == NVML_SUCCESS {
            unsafe {
                (*utilization).gpu = 7;
                (*utilization).memory = 3;
            }
        }
        status
    }

    fn temperature(&self, temperature: *mut u32) -> NvmlReturn {
        let status = self.next_status(FakeMetric::Temperature);
        if status == NVML_SUCCESS {
            unsafe {
                *temperature = 47;
            }
        }
        status
    }

    fn power(&self, milliwatts: *mut u32) -> NvmlReturn {
        let status = self.next_status(FakeMetric::Power);
        if status == NVML_SUCCESS {
            unsafe {
                *milliwatts = 41_776;
            }
        }
        status
    }

    fn clock(&self, clock_type: u32, mhz: *mut u32) -> NvmlReturn {
        let metric = if clock_type == NVML_CLOCK_MEM {
            FakeMetric::MemoryClock
        } else {
            FakeMetric::GraphicsClock
        };
        let status = self.next_status(metric);
        if status == NVML_SUCCESS {
            unsafe {
                *mhz = if clock_type == NVML_CLOCK_MEM {
                    16_001
                } else {
                    2_535
                };
            }
        }
        status
    }

    fn memory(&self, memory: *mut NvmlMemory) -> NvmlReturn {
        let status = self.next_status(FakeMetric::Memory);
        if status == NVML_SUCCESS {
            unsafe {
                (*memory).total = 17_094_934_528;
                (*memory).free = 13_567_782_912;
                (*memory).used = 3_527_151_616;
            }
        }
        status
    }
}

#[derive(Debug)]
enum NvmlDispatch {
    Native(NvmlFunctions),
    Fake(FakeNvmlDispatch),
}

impl NvmlDispatch {
    fn init(&self) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe { (functions.init_v2)() },
            Self::Fake(dispatch) => dispatch.init(),
        }
    }

    fn shutdown(&self) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe { (functions.shutdown)() },
            Self::Fake(dispatch) => dispatch.shutdown(),
        }
    }

    fn driver_version(&self, buffer: *mut c_char, length: u32) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.system_get_driver_version)(buffer, length)
            },
            Self::Fake(dispatch) => dispatch.driver_version(buffer, length),
        }
    }

    fn device_count(&self, count: *mut u32) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe { (functions.device_get_count_v2)(count) },
            Self::Fake(dispatch) => dispatch.device_count(count),
        }
    }

    fn device_handle(&self, index: u32, handle: *mut NvmlDevice) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.device_get_handle_by_index_v2)(index, handle)
            },
            Self::Fake(dispatch) => dispatch.device_handle(index, handle),
        }
    }

    fn device_name(&self, device: NvmlDevice, buffer: *mut c_char, length: u32) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.device_get_name)(device, buffer, length)
            },
            Self::Fake(dispatch) => dispatch.device_name(buffer, length),
        }
    }

    fn utilization(&self, device: NvmlDevice, utilization: *mut NvmlUtilization) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.device_get_utilization_rates)(device, utilization)
            },
            Self::Fake(dispatch) => dispatch.utilization(utilization),
        }
    }

    fn temperature(&self, device: NvmlDevice, sensor: u32, temperature: *mut u32) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.device_get_temperature)(device, sensor, temperature)
            },
            Self::Fake(dispatch) => dispatch.temperature(temperature),
        }
    }

    fn power(&self, device: NvmlDevice, milliwatts: *mut u32) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.device_get_power_usage)(device, milliwatts)
            },
            Self::Fake(dispatch) => dispatch.power(milliwatts),
        }
    }

    fn clock(&self, device: NvmlDevice, clock_type: u32, mhz: *mut u32) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.device_get_clock_info)(device, clock_type, mhz)
            },
            Self::Fake(dispatch) => dispatch.clock(clock_type, mhz),
        }
    }

    fn memory(&self, device: NvmlDevice, memory: *mut NvmlMemory) -> NvmlReturn {
        match self {
            Self::Native(functions) => unsafe {
                (functions.device_get_memory_info)(device, memory)
            },
            Self::Fake(dispatch) => dispatch.memory(memory),
        }
    }
}

impl NvmlFunctions {
    fn load(module: HMODULE) -> Result<Self, (ReadStatus, String)> {
        {
            macro_rules! symbol {
                ($name:literal, $type:ty) => {
                    load_symbol::<$type>(module, concat!($name, "\0").as_bytes()).map_err(|_| {
                        (
                            ReadStatus::ProviderMissing,
                            "nvml_symbol_missing".to_string(),
                        )
                    })?
                };
            }

            Ok(Self {
                init_v2: symbol!("nvmlInit_v2", NvmlInitV2),
                shutdown: symbol!("nvmlShutdown", NvmlShutdown),
                system_get_driver_version: symbol!(
                    "nvmlSystemGetDriverVersion",
                    NvmlSystemGetDriverVersion
                ),
                device_get_count_v2: symbol!("nvmlDeviceGetCount_v2", NvmlDeviceGetCountV2),
                device_get_handle_by_index_v2: symbol!(
                    "nvmlDeviceGetHandleByIndex_v2",
                    NvmlDeviceGetHandleByIndexV2
                ),
                device_get_name: symbol!("nvmlDeviceGetName", NvmlDeviceGetName),
                device_get_utilization_rates: symbol!(
                    "nvmlDeviceGetUtilizationRates",
                    NvmlDeviceGetUtilizationRates
                ),
                device_get_temperature: symbol!(
                    "nvmlDeviceGetTemperature",
                    NvmlDeviceGetTemperature
                ),
                device_get_power_usage: symbol!("nvmlDeviceGetPowerUsage", NvmlDeviceGetPowerUsage),
                device_get_clock_info: symbol!("nvmlDeviceGetClockInfo", NvmlDeviceGetClockInfo),
                device_get_memory_info: symbol!("nvmlDeviceGetMemoryInfo", NvmlDeviceGetMemoryInfo),
            })
        }
    }
}

#[derive(Debug)]
pub struct NvmlProvider {
    library: Option<NvmlLibrary>,
    dispatch: NvmlDispatch,
    stats: Arc<Mutex<NvmlStats>>,
    initialized: bool,
    driver_version: Option<String>,
    devices: Vec<NvmlDeviceEntry>,
}

impl NvmlProvider {
    pub fn new() -> ReadResult<Self> {
        let stats = Arc::new(Mutex::new(NvmlStats::default()));
        let library = match NvmlLibrary::load(stats.clone()) {
            Ok(library) => library,
            Err((status, reason_code)) => return read_status(status, reason_code),
        };
        let functions = match NvmlFunctions::load(library.module()) {
            Ok(functions) => functions,
            Err((status, reason_code)) => return read_status(status, reason_code),
        };
        Self::from_dispatch(Some(library), NvmlDispatch::Native(functions), stats)
    }

    pub fn new_injected(injection: NvmlInjection) -> ReadResult<Self> {
        if matches!(injection, NvmlInjection::MissingLibrary) {
            return read_status(
                ReadStatus::ProviderMissing,
                "nvml_runtime_missing".to_string(),
            );
        }
        let stats = Arc::new(Mutex::new(NvmlStats {
            library_load_attempts: 1,
            library_load_successes: 1,
            ..NvmlStats::default()
        }));
        Self::from_dispatch(
            None,
            NvmlDispatch::Fake(FakeNvmlDispatch::new(injection)),
            stats,
        )
    }

    fn from_dispatch(
        library: Option<NvmlLibrary>,
        dispatch: NvmlDispatch,
        stats: Arc<Mutex<NvmlStats>>,
    ) -> ReadResult<Self> {
        let init_status = dispatch.init();
        with_stats(&stats, |stats| {
            stats.init_calls += 1;
            if init_status == NVML_SUCCESS {
                stats.init_successes += 1;
            }
        });
        if init_status != NVML_SUCCESS {
            let (status, reason_code) = map_nvml_status(init_status);
            return read_status(status, reason_code.to_string());
        }

        let mut provider = Self {
            library,
            dispatch,
            stats,
            initialized: true,
            driver_version: None,
            devices: Vec::new(),
        };
        if let Err((status, reason_code)) = provider.enumerate() {
            drop(provider);
            return read_status(status, reason_code);
        }
        ReadResult {
            status: ReadStatus::Value,
            reason_code: "ok".to_string(),
            value: Some(provider),
        }
    }

    pub fn shutdown(mut self) -> NvmlStopResult {
        let (status, reason_code) = self.shutdown_internal();
        let stats = self.stats.clone();
        drop(self);
        NvmlStopResult {
            status,
            reason_code,
            stats: stats_snapshot(&stats),
        }
    }

    fn shutdown_internal(&mut self) -> (ReadStatus, String) {
        if !self.initialized {
            return (ReadStatus::Value, "already_shutdown".to_string());
        }
        let status = self.dispatch.shutdown();
        with_stats(&self.stats, |stats| {
            stats.shutdown_calls += 1;
            if status == NVML_SUCCESS {
                stats.shutdown_successes += 1;
            }
        });
        self.initialized = false;
        if self.library.is_none() {
            with_stats(&self.stats, |stats| stats.library_release_count += 1);
        }
        if status == NVML_SUCCESS {
            (ReadStatus::Value, "ok".to_string())
        } else {
            let (status, reason_code) = map_nvml_status(status);
            (status, reason_code.to_string())
        }
    }

    fn enumerate(&mut self) -> Result<(), (ReadStatus, String)> {
        let mut driver_version = [0_i8; 256];
        let driver_status = self
            .dispatch
            .driver_version(driver_version.as_mut_ptr(), driver_version.len() as u32);
        if driver_status != NVML_SUCCESS {
            let (status, reason_code) = map_nvml_status(driver_status);
            return Err((status, reason_code.to_string()));
        }
        self.driver_version = c_string(&driver_version);

        let mut count = 0_u32;
        let count_status = self.dispatch.device_count(&mut count);
        if count_status != NVML_SUCCESS {
            let (status, reason_code) = map_nvml_status(count_status);
            return Err((status, reason_code.to_string()));
        }

        for index in 0..count {
            let mut handle = ptr::null_mut();
            let handle_status = self.dispatch.device_handle(index, &mut handle);
            if handle_status != NVML_SUCCESS {
                let (status, reason_code) = map_nvml_status(handle_status);
                self.devices.push(NvmlDeviceEntry {
                    index,
                    product_name: "unknown".to_string(),
                    handle_status: status,
                    handle_reason_code: reason_code.to_string(),
                    handle: None,
                });
                continue;
            }

            let product_name = self
                .device_name(handle)
                .unwrap_or_else(|| "unknown".to_string());
            self.devices.push(NvmlDeviceEntry {
                index,
                product_name,
                handle_status: ReadStatus::Value,
                handle_reason_code: "ok".to_string(),
                handle: Some(handle),
            });
        }
        Ok(())
    }

    fn device_name(&self, device: NvmlDevice) -> Option<String> {
        let mut buffer = [0_i8; 256];
        let status = self
            .dispatch
            .device_name(device, buffer.as_mut_ptr(), buffer.len() as u32);
        (status == NVML_SUCCESS).then(|| c_string(&buffer).unwrap_or_else(|| "unknown".to_string()))
    }

    pub fn device_keys(&self) -> Vec<String> {
        self.devices
            .iter()
            .map(NvmlDeviceEntry::device_key)
            .collect()
    }

    pub fn device_statuses(&self) -> Vec<(String, ReadStatus, String)> {
        self.devices
            .iter()
            .map(|device| {
                (
                    device.device_key(),
                    device.handle_status,
                    device.handle_reason_code.clone(),
                )
            })
            .collect()
    }

    pub fn append_inventory(
        &self,
        devices: &mut Vec<DeviceInfo>,
        capabilities: &mut Vec<Capability>,
    ) {
        let provider_key = "gpu:nvidia:provider";
        let mut provider_details = BTreeMap::new();
        if let Some(driver_version) = &self.driver_version {
            provider_details.insert("driver_version".to_string(), driver_version.clone());
        }
        let usable_devices = self
            .devices
            .iter()
            .filter(|device| device.is_usable())
            .count();
        if self.devices.is_empty() {
            devices.push(DeviceInfo {
                device_key: provider_key.to_string(),
                category: "gpu".to_string(),
                present: Some(false),
                classification: NO_COMPATIBLE_GPU_REASON.to_string(),
                details: provider_details,
            });
            capabilities.push(Capability {
                device_key: provider_key.to_string(),
                category: "gpu".to_string(),
                provider: "nvidia-nvml".to_string(),
                support_status: SupportStatus::Unsupported,
                reason_code: NO_COMPATIBLE_GPU_REASON.to_string(),
                details: BTreeMap::new(),
                source: SOURCE_NVML.to_string(),
                known_semantic_limitations: vec![
                    "NVML initialized successfully but reported zero compatible devices"
                        .to_string(),
                ],
            });
            return;
        }

        capabilities.push(Capability {
            device_key: provider_key.to_string(),
            category: "gpu".to_string(),
            provider: "nvidia-nvml".to_string(),
            support_status: if usable_devices > 0 {
                SupportStatus::Supported
            } else {
                self.devices[0].handle_status.into()
            },
            reason_code: if usable_devices == self.devices.len() {
                "ok".to_string()
            } else if usable_devices > 0 {
                "partial_device_enumeration".to_string()
            } else {
                self.devices[0].handle_reason_code.clone()
            },
            details: provider_details,
            source: SOURCE_NVML.to_string(),
            known_semantic_limitations: vec![
                "NVML is loaded dynamically and reports board-level GPU values".to_string(),
            ],
        });

        for device in &self.devices {
            let mut details = BTreeMap::new();
            details.insert("product_name".to_string(), device.product_name.clone());
            if let Some(driver_version) = &self.driver_version {
                details.insert("driver_version".to_string(), driver_version.clone());
            }
            devices.push(DeviceInfo {
                device_key: device.device_key(),
                category: "gpu".to_string(),
                present: Some(true),
                classification: if device.is_usable() {
                    "nvidia_gpu".to_string()
                } else {
                    "nvidia_gpu_handle_unavailable".to_string()
                },
                details,
            });
            capabilities.push(Capability {
                device_key: device.device_key(),
                category: "gpu".to_string(),
                provider: "nvidia-nvml".to_string(),
                support_status: device.handle_status.into(),
                reason_code: device.handle_reason_code.clone(),
                details: BTreeMap::new(),
                source: SOURCE_NVML.to_string(),
                known_semantic_limitations: vec![
                    "GPU metrics are sampled per device; unsupported functions do not become zero values".to_string(),
                ],
            });
        }
    }

    pub fn sample_all(&self) -> Vec<GpuDeviceSample> {
        let mut samples = Vec::new();
        for device in &self.devices {
            let Some(handle) = device.handle else {
                continue;
            };
            let utilization = timed(|| self.read_utilization(handle));
            let memory = timed(|| self.read_memory(handle));
            let temperature = timed(|| self.read_temperature(handle));
            let power = timed(|| self.read_power_watts(handle));
            let graphics_clock = timed(|| self.read_clock(handle, NVML_CLOCK_GRAPHICS));
            let memory_clock = timed(|| self.read_clock(handle, NVML_CLOCK_MEM));
            let failed = utilization.result.value.is_none()
                || memory.result.value.is_none()
                || temperature.result.value.is_none()
                || power.result.value.is_none()
                || graphics_clock.result.value.is_none()
                || memory_clock.result.value.is_none();
            with_stats(&self.stats, |stats| {
                stats.sample_count += 1;
                if failed {
                    stats.failed_sample_count += 1;
                }
            });
            samples.push(GpuDeviceSample {
                device_key: device.device_key(),
                utilization_percent: split_pair(utilization.clone(), true),
                memory_controller_utilization_percent: split_pair(utilization, false),
                temperature_celsius: temperature,
                power_watts: power,
                graphics_clock_mhz: graphics_clock,
                memory_clock_mhz: memory_clock,
                vram_used_bytes: split_pair(memory.clone(), true),
                vram_total_bytes: split_pair(memory, false),
            });
        }
        samples
    }

    fn read_utilization(&self, device: NvmlDevice) -> ReadResult<(f64, f64)> {
        let mut utilization = NvmlUtilization::default();
        let status = self.dispatch.utilization(device, &mut utilization);
        self.record_metric_call(status);
        if status != NVML_SUCCESS {
            return mapped_result(status);
        }
        ReadResult {
            status: ReadStatus::Value,
            reason_code: "ok".to_string(),
            value: Some((utilization.gpu as f64, utilization.memory as f64)),
        }
    }

    fn read_temperature(&self, device: NvmlDevice) -> ReadResult<f64> {
        let mut temperature = 0_u32;
        let status = self
            .dispatch
            .temperature(device, NVML_TEMPERATURE_GPU, &mut temperature);
        self.record_metric_call(status);
        if status != NVML_SUCCESS {
            return mapped_result(status);
        }
        ReadResult {
            status: ReadStatus::Value,
            reason_code: "ok".to_string(),
            value: Some(temperature as f64),
        }
    }

    fn read_power_watts(&self, device: NvmlDevice) -> ReadResult<f64> {
        let mut milliwatts = 0_u32;
        let status = self.dispatch.power(device, &mut milliwatts);
        self.record_metric_call(status);
        if status != NVML_SUCCESS {
            return mapped_result(status);
        }
        ReadResult {
            status: ReadStatus::Value,
            reason_code: "ok".to_string(),
            value: Some(milliwatts_to_watts(milliwatts)),
        }
    }

    fn read_clock(&self, device: NvmlDevice, clock_type: u32) -> ReadResult<f64> {
        let mut mhz = 0_u32;
        let status = self.dispatch.clock(device, clock_type, &mut mhz);
        self.record_metric_call(status);
        if status != NVML_SUCCESS {
            return mapped_result(status);
        }
        ReadResult {
            status: ReadStatus::Value,
            reason_code: "ok".to_string(),
            value: Some(mhz as f64),
        }
    }

    fn read_memory(&self, device: NvmlDevice) -> ReadResult<(f64, f64)> {
        let mut memory = NvmlMemory::default();
        let status = self.dispatch.memory(device, &mut memory);
        self.record_metric_call(status);
        if status != NVML_SUCCESS {
            return mapped_result(status);
        }
        ReadResult {
            status: ReadStatus::Value,
            reason_code: "ok".to_string(),
            value: Some((memory.used as f64, memory.total as f64)),
        }
    }

    fn record_metric_call(&self, status: NvmlReturn) {
        with_stats(&self.stats, |stats| {
            stats.gpu_metric_call_count += 1;
            if status != NVML_SUCCESS {
                stats.failed_gpu_metric_call_count += 1;
            }
        });
    }
}

impl Drop for NvmlProvider {
    fn drop(&mut self) {
        if self.initialized {
            let _ = self.shutdown_internal();
        }
    }
}

pub fn append_nvidia_inventory(
    devices: &mut Vec<DeviceInfo>,
    capabilities: &mut Vec<Capability>,
    provider: Option<&NvmlProvider>,
    init_status: Option<&(ReadStatus, String)>,
) {
    if let Some(provider) = provider {
        provider.append_inventory(devices, capabilities);
        return;
    }

    let (status, reason_code) = init_status.cloned().unwrap_or((
        ReadStatus::ProviderMissing,
        "nvml_runtime_missing".to_string(),
    ));
    let device_key = "gpu:nvidia:provider".to_string();
    devices.push(DeviceInfo {
        device_key: device_key.clone(),
        category: "gpu".to_string(),
        present: Some(false),
        classification: "nvidia_nvml_unavailable".to_string(),
        details: BTreeMap::new(),
    });
    capabilities.push(Capability {
        device_key,
        category: "gpu".to_string(),
        provider: "nvidia-nvml".to_string(),
        support_status: status.into(),
        reason_code,
        details: BTreeMap::new(),
        source: SOURCE_NVML.to_string(),
        known_semantic_limitations: vec![
            "NVML was not initialized; GPU metrics are omitted rather than represented as zero"
                .to_string(),
        ],
    });
}

pub fn map_nvml_status(status: NvmlReturn) -> (ReadStatus, &'static str) {
    match status {
        NVML_SUCCESS => (ReadStatus::Value, "ok"),
        NVML_ERROR_NOT_SUPPORTED | NVML_ERROR_FREQ_NOT_SUPPORTED => {
            (ReadStatus::Unsupported, "nvml_not_supported")
        }
        NVML_ERROR_NO_PERMISSION => (ReadStatus::PermissionDenied, "nvml_no_permission"),
        NVML_ERROR_DRIVER_NOT_LOADED => (ReadStatus::ProviderMissing, "nvml_driver_missing"),
        NVML_ERROR_LIBRARY_NOT_FOUND => (ReadStatus::ProviderMissing, "nvml_runtime_missing"),
        NVML_ERROR_FUNCTION_NOT_FOUND => (ReadStatus::ProviderMissing, "nvml_symbol_missing"),
        NVML_ERROR_GPU_IS_LOST => (ReadStatus::RuntimeFailed, "nvml_gpu_lost"),
        NVML_ERROR_LIB_RM_VERSION_MISMATCH => {
            (ReadStatus::RuntimeFailed, "nvml_driver_library_mismatch")
        }
        NVML_ERROR_UNINITIALIZED => (ReadStatus::Failed, "nvml_uninitialized"),
        NVML_ERROR_INVALID_ARGUMENT => (ReadStatus::Failed, "nvml_invalid_argument"),
        NVML_ERROR_ALREADY_INITIALIZED => (ReadStatus::Failed, "nvml_already_initialized"),
        NVML_ERROR_NOT_FOUND => (ReadStatus::Unsupported, "nvml_device_not_found"),
        NVML_ERROR_INSUFFICIENT_SIZE => (ReadStatus::Failed, "nvml_insufficient_size"),
        NVML_ERROR_INSUFFICIENT_POWER => (ReadStatus::Failed, "nvml_insufficient_power"),
        NVML_ERROR_TIMEOUT => (ReadStatus::Failed, "nvml_timeout"),
        NVML_ERROR_IRQ_ISSUE => (ReadStatus::Failed, "nvml_irq_issue"),
        NVML_ERROR_CORRUPTED_IN_USE => (ReadStatus::Failed, "nvml_corrupted_in_use"),
        NVML_ERROR_RESET_REQUIRED => (ReadStatus::RuntimeFailed, "nvml_reset_required"),
        NVML_ERROR_OPERATING_SYSTEM => (ReadStatus::Failed, "nvml_operating_system"),
        NVML_ERROR_IN_USE => (ReadStatus::Failed, "nvml_in_use"),
        NVML_ERROR_MEMORY => (ReadStatus::Failed, "nvml_memory_error"),
        NVML_ERROR_NO_DATA => (ReadStatus::Unsupported, "nvml_no_data"),
        NVML_ERROR_VGPU_ECC_NOT_SUPPORTED => {
            (ReadStatus::Unsupported, "nvml_vgpu_ecc_not_supported")
        }
        NVML_ERROR_INSUFFICIENT_RESOURCES => (ReadStatus::Failed, "nvml_insufficient_resources"),
        NVML_ERROR_UNKNOWN => (ReadStatus::Failed, "nvml_call_failed"),
        _ => (ReadStatus::Failed, "nvml_call_failed"),
    }
}

fn read_status<T>(status: ReadStatus, reason_code: String) -> ReadResult<T> {
    ReadResult {
        status,
        reason_code,
        value: None,
    }
}

fn mapped_result<T>(status: NvmlReturn) -> ReadResult<T> {
    let (status, reason_code) = map_nvml_status(status);
    read_status(status, reason_code.to_string())
}

fn split_pair<T: Copy>(timed: TimedRead<(T, T)>, first: bool) -> TimedRead<T> {
    let latency_ms = timed.latency_ms;
    let result = timed.result;
    let value = result.value.map(|pair| if first { pair.0 } else { pair.1 });
    TimedRead {
        result: ReadResult {
            status: result.status,
            reason_code: result.reason_code,
            value,
        },
        latency_ms,
    }
}

fn timed<T>(read: impl FnOnce() -> ReadResult<T>) -> TimedRead<T> {
    let started = Instant::now();
    let result = read();
    TimedRead {
        result,
        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
    }
}

fn c_string(buffer: &[i8]) -> Option<String> {
    let bytes = buffer
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect::<Vec<_>>();
    (!bytes.is_empty())
        .then(|| String::from_utf8_lossy(&bytes).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn load_symbol<T: Copy>(module: HMODULE, name: &[u8]) -> Result<T, ()> {
    let proc = unsafe { GetProcAddress(module, PCSTR::from_raw(name.as_ptr())) };
    let Some(proc) = proc else {
        return Err(());
    };
    Ok(unsafe { mem::transmute_copy(&proc) })
}

impl From<ReadStatus> for SupportStatus {
    fn from(status: ReadStatus) -> Self {
        match status {
            ReadStatus::Value => SupportStatus::Supported,
            ReadStatus::Unsupported => SupportStatus::Unsupported,
            ReadStatus::PermissionDenied => SupportStatus::PermissionDenied,
            ReadStatus::ProviderMissing => SupportStatus::ProviderMissing,
            ReadStatus::Failed => SupportStatus::ProbeFailed,
            ReadStatus::RuntimeFailed => SupportStatus::RuntimeFailed,
        }
    }
}

fn with_stats(stats: &Arc<Mutex<NvmlStats>>, update: impl FnOnce(&mut NvmlStats)) {
    update(&mut stats.lock().expect("NVML stats lock poisoned"));
}

fn stats_snapshot(stats: &Arc<Mutex<NvmlStats>>) -> NvmlStats {
    stats.lock().expect("NVML stats lock poisoned").clone()
}

fn write_c_buffer(buffer: *mut c_char, length: u32, value: &str) -> NvmlReturn {
    if buffer.is_null() || length == 0 {
        return NVML_ERROR_INVALID_ARGUMENT;
    }
    let bytes = value.as_bytes();
    let copy_length = bytes.len().min(length.saturating_sub(1) as usize);
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buffer, copy_length);
        *buffer.add(copy_length) = 0;
    }
    NVML_SUCCESS
}

fn milliwatts_to_watts(milliwatts: u32) -> f64 {
    milliwatts as f64 / 1000.0
}

#[cfg(test)]
mod tests {
    use super::{
        map_nvml_status, milliwatts_to_watts, split_pair, NvmlDeviceEntry, NvmlInjection,
        NvmlProvider, TimedRead, NO_COMPATIBLE_GPU_REASON,
    };
    use crate::windows::{ReadResult, ReadStatus};

    #[test]
    fn maps_nvml_errors_to_stable_probe_states() {
        assert_eq!(
            map_nvml_status(3),
            (ReadStatus::Unsupported, "nvml_not_supported")
        );
        assert_eq!(
            map_nvml_status(4),
            (ReadStatus::PermissionDenied, "nvml_no_permission")
        );
        assert_eq!(
            map_nvml_status(15),
            (ReadStatus::RuntimeFailed, "nvml_gpu_lost")
        );
        assert_eq!(
            map_nvml_status(18),
            (ReadStatus::RuntimeFailed, "nvml_driver_library_mismatch")
        );
        assert_eq!(
            map_nvml_status(12),
            (ReadStatus::ProviderMissing, "nvml_runtime_missing")
        );
        assert_eq!(
            map_nvml_status(0xdead),
            (ReadStatus::Failed, "nvml_call_failed")
        );
    }

    #[test]
    fn converts_milliwatts_to_board_watts() {
        assert_eq!(milliwatts_to_watts(0), 0.0);
        assert_eq!(milliwatts_to_watts(12_345), 12.345);
    }

    #[test]
    fn no_device_and_multiple_device_keys_are_stable() {
        assert_eq!(NO_COMPATIBLE_GPU_REASON, "no_compatible_nvidia_gpu");
        let first = NvmlDeviceEntry {
            index: 0,
            product_name: "GPU A".to_string(),
            handle_status: ReadStatus::Value,
            handle_reason_code: "ok".to_string(),
            handle: None,
        };
        let second = NvmlDeviceEntry {
            index: 1,
            product_name: "GPU B".to_string(),
            handle_status: ReadStatus::Value,
            handle_reason_code: "ok".to_string(),
            handle: None,
        };
        assert_eq!(first.device_key(), "gpu:nvidia:index-0");
        assert_eq!(second.device_key(), "gpu:nvidia:index-1");
        assert_ne!(first.device_key(), second.device_key());
    }

    #[test]
    fn paired_memory_values_keep_used_and_total_distinct() {
        let timed = TimedRead {
            result: ReadResult {
                status: ReadStatus::Value,
                reason_code: "ok".to_string(),
                value: Some((3.0, 16.0)),
            },
            latency_ms: 0.2,
        };
        assert_eq!(split_pair(timed.clone(), true).result.value, Some(3.0));
        assert_eq!(split_pair(timed, false).result.value, Some(16.0));
    }

    #[test]
    fn injected_missing_library_is_safe_and_has_no_provider() {
        let result = NvmlProvider::new_injected(NvmlInjection::MissingLibrary);
        assert_eq!(result.status, ReadStatus::ProviderMissing);
        assert_eq!(result.reason_code, "nvml_runtime_missing");
        assert!(result.value.is_none());
    }

    #[test]
    fn injected_partial_unsupported_keeps_other_metrics_sampling() {
        let provider = NvmlProvider::new_injected(NvmlInjection::PartialUnsupported)
            .value
            .expect("injected provider should initialize");
        let sample = provider.sample_all().pop().expect("one fake GPU");
        assert_eq!(sample.utilization_percent.result.value, Some(7.0));
        assert_eq!(sample.power_watts.result.status, ReadStatus::Unsupported);
        assert!(sample.power_watts.result.value.is_none());
        assert_eq!(
            sample.memory_clock_mhz.result.status,
            ReadStatus::Unsupported
        );
        assert!(sample.memory_clock_mhz.result.value.is_none());
        let stop = provider.shutdown();
        assert_eq!(stop.status, ReadStatus::Value);
        assert_eq!(stop.stats.gpu_metric_call_count, 6);
        assert_eq!(stop.stats.failed_gpu_metric_call_count, 2);
        assert_eq!(stop.stats.library_release_count, 1);
    }

    #[test]
    fn injected_transient_metric_failure_recovers_without_retry_loop() {
        let provider = NvmlProvider::new_injected(NvmlInjection::TransientMetricFailure)
            .value
            .expect("injected provider should initialize");
        let first = provider.sample_all().pop().expect("one fake GPU");
        let second = provider.sample_all().pop().expect("one fake GPU");
        assert_eq!(first.power_watts.result.status, ReadStatus::Failed);
        assert_eq!(first.power_watts.result.reason_code, "nvml_timeout");
        assert_eq!(second.power_watts.result.value, Some(41.776));
        let stop = provider.shutdown();
        assert_eq!(stop.stats.sample_count, 2);
        assert_eq!(stop.stats.failed_sample_count, 1);
        assert_eq!(stop.stats.failed_gpu_metric_call_count, 1);
        assert_eq!(stop.stats.shutdown_calls, 1);
        assert_eq!(stop.stats.library_release_count, 1);
    }

    #[test]
    fn injected_provider_runtime_failure_is_distinct_from_probe_failure() {
        let result = NvmlProvider::new_injected(NvmlInjection::ProviderRuntimeFailure);
        assert_eq!(result.status, ReadStatus::RuntimeFailed);
        assert_eq!(result.reason_code, "nvml_gpu_lost");
        assert!(result.value.is_none());
    }
}
