use super::provider::{
    MetricProvider, ProviderCallContext, ProviderCapabilitySpec, ProviderDescriptor, ProviderError,
    ProviderHealthObservation, ProviderLifecycleOutcome, ProviderPlan, ProviderSample,
    ProviderSchedule,
};
use crate::models::{
    CapabilitySupportStatus, GpuSample, MetricCategory, MetricRuntimeSupportStatus,
    ProviderErrorCode, ProviderErrorSummary, ProviderMetricMetadata, RuntimeDeviceMetadata,
    GPU_BOARD_POWER_SCOPE,
};
use std::{
    collections::{BTreeSet, HashSet},
    ffi::c_void,
};

#[cfg(windows)]
use std::{ffi::c_char, mem, path::PathBuf, ptr};
#[cfg(windows)]
use windows::{
    core::{PCSTR, PCWSTR},
    Win32::{
        Foundation::{FreeLibrary, HANDLE, HMODULE},
        System::LibraryLoader::{
            GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_FLAGS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        },
    },
};

pub const NVIDIA_NVML_PROVIDER_ID: &str = "nvidia-nvml";

const NVIDIA_VENDOR: &str = "NVIDIA";
const NVML_SUCCESS: u32 = 0;
const NVML_ERROR_NOT_SUPPORTED: u32 = 3;
const NVML_ERROR_NO_PERMISSION: u32 = 4;
const NVML_ERROR_DRIVER_NOT_LOADED: u32 = 9;
const NVML_ERROR_TIMEOUT: u32 = 10;
const NVML_ERROR_LIBRARY_NOT_FOUND: u32 = 12;
const NVML_ERROR_FUNCTION_NOT_FOUND: u32 = 13;
const NVML_ERROR_GPU_IS_LOST: u32 = 15;
const NVML_ERROR_RESET_REQUIRED: u32 = 16;
const NVML_ERROR_LIB_RM_VERSION_MISMATCH: u32 = 18;
const NVML_ERROR_FREQ_NOT_SUPPORTED: u32 = 24;

const NVML_TEMPERATURE_GPU: u32 = 0;
const NVML_CLOCK_GRAPHICS: u32 = 0;
const NVML_CLOCK_MEM: u32 = 2;
const NVML_DEVICE_UUID_BUFFER_SIZE: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum NvmlMetric {
    Utilization,
    MemoryControllerUtilization,
    Temperature,
    Power,
    GraphicsClock,
    MemoryClock,
    VramUsed,
    VramTotal,
}

impl NvmlMetric {
    const ALL: [Self; 8] = [
        Self::Utilization,
        Self::MemoryControllerUtilization,
        Self::Temperature,
        Self::Power,
        Self::GraphicsClock,
        Self::MemoryClock,
        Self::VramUsed,
        Self::VramTotal,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::Utilization => "gpu.utilization_percent",
            Self::MemoryControllerUtilization => "gpu.memory_controller_utilization_percent",
            Self::Temperature => "gpu.temperature_celsius",
            Self::Power => "gpu.power_watts",
            Self::GraphicsClock => "gpu.graphics_clock_mhz",
            Self::MemoryClock => "gpu.memory_clock_mhz",
            Self::VramUsed => "gpu.vram_used_bytes",
            Self::VramTotal => "gpu.vram_total_bytes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NvmlFailure {
    support_status: MetricRuntimeSupportStatus,
    provider_error: ProviderErrorCode,
    reason: &'static str,
}

impl NvmlFailure {
    const fn provider_missing(reason: &'static str) -> Self {
        Self {
            support_status: MetricRuntimeSupportStatus::ProviderMissing,
            provider_error: ProviderErrorCode::ProviderMissing,
            reason,
        }
    }

    const fn unsupported(reason: &'static str) -> Self {
        Self {
            support_status: MetricRuntimeSupportStatus::Unsupported,
            provider_error: ProviderErrorCode::Unsupported,
            reason,
        }
    }

    fn is_runtime_fatal(&self) -> bool {
        self.provider_error == ProviderErrorCode::RuntimeFailed
    }

    fn as_provider_error(&self, fallback: ProviderErrorCode) -> ProviderError {
        let code = if self.provider_error == ProviderErrorCode::RuntimeFailed {
            ProviderErrorCode::RuntimeFailed
        } else {
            fallback
        };
        ProviderError::new(code, self.reason)
    }
}

fn nvml_failure(status: u32) -> NvmlFailure {
    match status {
        NVML_ERROR_NOT_SUPPORTED | NVML_ERROR_FREQ_NOT_SUPPORTED => {
            NvmlFailure::unsupported("nvml_not_supported")
        }
        NVML_ERROR_NO_PERMISSION => NvmlFailure {
            support_status: MetricRuntimeSupportStatus::PermissionDenied,
            provider_error: ProviderErrorCode::PermissionDenied,
            reason: "nvml_no_permission",
        },
        NVML_ERROR_DRIVER_NOT_LOADED => NvmlFailure::provider_missing("nvml_driver_missing"),
        NVML_ERROR_LIBRARY_NOT_FOUND => NvmlFailure::provider_missing("nvml_runtime_missing"),
        NVML_ERROR_FUNCTION_NOT_FOUND => NvmlFailure::provider_missing("nvml_symbol_missing"),
        NVML_ERROR_GPU_IS_LOST => NvmlFailure {
            support_status: MetricRuntimeSupportStatus::Failed,
            provider_error: ProviderErrorCode::RuntimeFailed,
            reason: "nvml_gpu_lost",
        },
        NVML_ERROR_RESET_REQUIRED => NvmlFailure {
            support_status: MetricRuntimeSupportStatus::Failed,
            provider_error: ProviderErrorCode::RuntimeFailed,
            reason: "nvml_reset_required",
        },
        NVML_ERROR_LIB_RM_VERSION_MISMATCH => NvmlFailure {
            support_status: MetricRuntimeSupportStatus::Failed,
            provider_error: ProviderErrorCode::RuntimeFailed,
            reason: "nvml_driver_library_mismatch",
        },
        NVML_ERROR_TIMEOUT => NvmlFailure {
            support_status: MetricRuntimeSupportStatus::Failed,
            provider_error: ProviderErrorCode::SampleFailed,
            reason: "nvml_timeout",
        },
        _ => NvmlFailure {
            support_status: MetricRuntimeSupportStatus::Failed,
            provider_error: ProviderErrorCode::SampleFailed,
            reason: "nvml_call_failed",
        },
    }
}

type NvmlResult<T> = Result<T, NvmlFailure>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NvmlDeviceHandle(usize);

impl NvmlDeviceHandle {
    fn as_ptr(self) -> *mut c_void {
        self.0 as *mut c_void
    }
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct NvmlUtilization {
    gpu: u32,
    memory: u32,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct NvmlMemory {
    total: u64,
    free: u64,
    used: u64,
}

trait NvmlDispatch: Send {
    fn init(&mut self) -> NvmlResult<()>;
    fn shutdown(&mut self) -> NvmlResult<()>;
    fn device_count(&mut self) -> NvmlResult<u32>;
    fn device_handle(&mut self, index: u32) -> NvmlResult<NvmlDeviceHandle>;
    fn device_uuid(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<String>;
    fn device_name(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<String>;
    fn utilization(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<NvmlUtilization>;
    fn temperature(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<u32>;
    fn power_usage_mw(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<u32>;
    fn clock_info(&mut self, handle: NvmlDeviceHandle, clock: u32) -> NvmlResult<u32>;
    fn memory_info(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<NvmlMemory>;
}

struct LoadedNvml {
    library: NvmlLibrary,
    dispatch: Box<dyn NvmlDispatch>,
}

trait NvmlLoader: Send {
    fn load(&self) -> NvmlResult<LoadedNvml>;
}

#[cfg(test)]
#[derive(Debug, Default)]
struct FakeStats {
    load_count: u64,
    init_count: u64,
    shutdown_count: u64,
    library_release_count: u64,
    metric_call_count: u64,
}

enum NvmlLibrary {
    #[cfg(windows)]
    Native(HMODULE),
    #[cfg(test)]
    Test(std::sync::Arc<std::sync::Mutex<FakeStats>>),
    #[cfg(not(windows))]
    Empty,
}

// Windows module handles are process-wide handles that can be moved between threads. The
// provider executor still creates, uses, and releases each handle on its single worker thread;
// this declaration only permits `MetricProvider: Send` to transfer the initially-empty provider
// into that worker.
#[cfg(windows)]
unsafe impl Send for NvmlLibrary {}

impl Drop for NvmlLibrary {
    fn drop(&mut self) {
        match self {
            #[cfg(windows)]
            Self::Native(module) if !module.is_invalid() => unsafe {
                let _ = FreeLibrary(*module);
            },
            #[cfg(test)]
            Self::Test(stats) => {
                stats
                    .lock()
                    .expect("fake NVML stats lock poisoned")
                    .library_release_count += 1;
            }
            _ => {}
        }
    }
}

#[cfg(windows)]
type NvmlInitV2 = unsafe extern "C" fn() -> u32;
#[cfg(windows)]
type NvmlShutdown = unsafe extern "C" fn() -> u32;
#[cfg(windows)]
type NvmlDeviceGetCountV2 = unsafe extern "C" fn(*mut u32) -> u32;
#[cfg(windows)]
type NvmlDeviceGetHandleByIndexV2 = unsafe extern "C" fn(u32, *mut *mut c_void) -> u32;
#[cfg(windows)]
type NvmlDeviceGetUuid = unsafe extern "C" fn(*mut c_void, *mut c_char, u32) -> u32;
#[cfg(windows)]
type NvmlDeviceGetName = unsafe extern "C" fn(*mut c_void, *mut c_char, u32) -> u32;
#[cfg(windows)]
type NvmlDeviceGetUtilizationRates = unsafe extern "C" fn(*mut c_void, *mut NvmlUtilization) -> u32;
#[cfg(windows)]
type NvmlDeviceGetTemperature = unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> u32;
#[cfg(windows)]
type NvmlDeviceGetPowerUsage = unsafe extern "C" fn(*mut c_void, *mut u32) -> u32;
#[cfg(windows)]
type NvmlDeviceGetClockInfo = unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> u32;
#[cfg(windows)]
type NvmlDeviceGetMemoryInfo = unsafe extern "C" fn(*mut c_void, *mut NvmlMemory) -> u32;

#[cfg(windows)]
#[derive(Clone, Copy)]
struct NativeNvmlFunctions {
    init_v2: NvmlInitV2,
    shutdown: NvmlShutdown,
    device_get_count_v2: NvmlDeviceGetCountV2,
    device_get_handle_by_index_v2: NvmlDeviceGetHandleByIndexV2,
    device_get_uuid: NvmlDeviceGetUuid,
    device_get_name: NvmlDeviceGetName,
    device_get_utilization_rates: NvmlDeviceGetUtilizationRates,
    device_get_temperature: NvmlDeviceGetTemperature,
    device_get_power_usage: NvmlDeviceGetPowerUsage,
    device_get_clock_info: NvmlDeviceGetClockInfo,
    device_get_memory_info: NvmlDeviceGetMemoryInfo,
}

#[cfg(windows)]
struct NativeNvmlLoader;

#[cfg(windows)]
impl NvmlLoader for NativeNvmlLoader {
    fn load(&self) -> NvmlResult<LoadedNvml> {
        let library = load_native_library()?;
        let functions = load_native_functions(library.module())?;
        Ok(LoadedNvml {
            library,
            dispatch: Box::new(NativeNvmlDispatch { functions }),
        })
    }
}

#[cfg(windows)]
impl NvmlLibrary {
    fn module(&self) -> HMODULE {
        match self {
            Self::Native(module) => *module,
            #[cfg(test)]
            Self::Test(_) => HMODULE::default(),
        }
    }
}

#[cfg(windows)]
fn load_native_library() -> NvmlResult<NvmlLibrary> {
    let mut candidates = vec![("nvml.dll".to_string(), LOAD_LIBRARY_SEARCH_SYSTEM32)];
    if let Some(program_w6432) = std::env::var_os("ProgramW6432") {
        let known_path = PathBuf::from(program_w6432)
            .join("NVIDIA Corporation")
            .join("NVSMI")
            .join("nvml.dll");
        candidates.push((
            known_path.to_string_lossy().into_owned(),
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        ));
    }

    for (candidate, flags) in candidates {
        let wide = candidate
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let module = unsafe {
            LoadLibraryExW(
                PCWSTR::from_raw(wide.as_ptr()),
                HANDLE::default(),
                LOAD_LIBRARY_FLAGS(flags.0),
            )
        };
        if let Ok(module) = module {
            return Ok(NvmlLibrary::Native(module));
        }
    }
    Err(NvmlFailure::provider_missing("nvml_runtime_missing"))
}

#[cfg(windows)]
fn load_native_functions(module: HMODULE) -> NvmlResult<NativeNvmlFunctions> {
    macro_rules! symbol {
        ($name:literal, $type:ty) => {
            load_symbol::<$type>(module, concat!($name, "\0").as_bytes())?
        };
    }
    Ok(NativeNvmlFunctions {
        init_v2: symbol!("nvmlInit_v2", NvmlInitV2),
        shutdown: symbol!("nvmlShutdown", NvmlShutdown),
        device_get_count_v2: symbol!("nvmlDeviceGetCount_v2", NvmlDeviceGetCountV2),
        device_get_handle_by_index_v2: symbol!(
            "nvmlDeviceGetHandleByIndex_v2",
            NvmlDeviceGetHandleByIndexV2
        ),
        device_get_uuid: symbol!("nvmlDeviceGetUUID", NvmlDeviceGetUuid),
        device_get_name: symbol!("nvmlDeviceGetName", NvmlDeviceGetName),
        device_get_utilization_rates: symbol!(
            "nvmlDeviceGetUtilizationRates",
            NvmlDeviceGetUtilizationRates
        ),
        device_get_temperature: symbol!("nvmlDeviceGetTemperature", NvmlDeviceGetTemperature),
        device_get_power_usage: symbol!("nvmlDeviceGetPowerUsage", NvmlDeviceGetPowerUsage),
        device_get_clock_info: symbol!("nvmlDeviceGetClockInfo", NvmlDeviceGetClockInfo),
        device_get_memory_info: symbol!("nvmlDeviceGetMemoryInfo", NvmlDeviceGetMemoryInfo),
    })
}

#[cfg(windows)]
fn load_symbol<T: Copy>(module: HMODULE, name: &[u8]) -> NvmlResult<T> {
    let symbol = unsafe { GetProcAddress(module, PCSTR::from_raw(name.as_ptr())) };
    let Some(symbol) = symbol else {
        return Err(NvmlFailure::provider_missing("nvml_symbol_missing"));
    };
    Ok(unsafe { mem::transmute_copy(&symbol) })
}

#[cfg(windows)]
struct NativeNvmlDispatch {
    functions: NativeNvmlFunctions,
}

#[cfg(windows)]
impl NvmlDispatch for NativeNvmlDispatch {
    fn init(&mut self) -> NvmlResult<()> {
        status_result(unsafe { (self.functions.init_v2)() })
    }

    fn shutdown(&mut self) -> NvmlResult<()> {
        status_result(unsafe { (self.functions.shutdown)() })
    }

    fn device_count(&mut self) -> NvmlResult<u32> {
        let mut count = 0;
        status_result(unsafe { (self.functions.device_get_count_v2)(&mut count) })?;
        Ok(count)
    }

    fn device_handle(&mut self, index: u32) -> NvmlResult<NvmlDeviceHandle> {
        let mut handle = ptr::null_mut();
        status_result(unsafe {
            (self.functions.device_get_handle_by_index_v2)(index, &mut handle)
        })?;
        (!handle.is_null())
            .then_some(NvmlDeviceHandle(handle as usize))
            .ok_or_else(|| NvmlFailure::unsupported("nvml_device_not_found"))
    }

    fn device_uuid(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<String> {
        let mut buffer = [0_i8; NVML_DEVICE_UUID_BUFFER_SIZE];
        status_result(unsafe {
            (self.functions.device_get_uuid)(
                handle.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        })?;
        c_string(&buffer).ok_or_else(|| NvmlFailure::unsupported("nvml_uuid_missing"))
    }

    fn device_name(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<String> {
        let mut buffer = [0_i8; 256];
        status_result(unsafe {
            (self.functions.device_get_name)(
                handle.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            )
        })?;
        c_string(&buffer).ok_or_else(|| NvmlFailure::unsupported("nvml_name_missing"))
    }

    fn utilization(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<NvmlUtilization> {
        let mut utilization = NvmlUtilization::default();
        status_result(unsafe {
            (self.functions.device_get_utilization_rates)(handle.as_ptr(), &mut utilization)
        })?;
        Ok(utilization)
    }

    fn temperature(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<u32> {
        let mut temperature = 0;
        status_result(unsafe {
            (self.functions.device_get_temperature)(
                handle.as_ptr(),
                NVML_TEMPERATURE_GPU,
                &mut temperature,
            )
        })?;
        Ok(temperature)
    }

    fn power_usage_mw(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<u32> {
        let mut milliwatts = 0;
        status_result(unsafe {
            (self.functions.device_get_power_usage)(handle.as_ptr(), &mut milliwatts)
        })?;
        Ok(milliwatts)
    }

    fn clock_info(&mut self, handle: NvmlDeviceHandle, clock: u32) -> NvmlResult<u32> {
        let mut mhz = 0;
        status_result(unsafe {
            (self.functions.device_get_clock_info)(handle.as_ptr(), clock, &mut mhz)
        })?;
        Ok(mhz)
    }

    fn memory_info(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<NvmlMemory> {
        let mut memory = NvmlMemory::default();
        status_result(unsafe {
            (self.functions.device_get_memory_info)(handle.as_ptr(), &mut memory)
        })?;
        Ok(memory)
    }
}

#[cfg(windows)]
fn status_result(status: u32) -> NvmlResult<()> {
    if status == NVML_SUCCESS {
        Ok(())
    } else {
        Err(nvml_failure(status))
    }
}

#[cfg(windows)]
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

#[cfg(not(windows))]
struct UnsupportedNvmlLoader;

#[cfg(not(windows))]
impl NvmlLoader for UnsupportedNvmlLoader {
    fn load(&self) -> NvmlResult<LoadedNvml> {
        Err(NvmlFailure::provider_missing("nvml_windows_only"))
    }
}

fn default_loader() -> Box<dyn NvmlLoader> {
    #[cfg(windows)]
    {
        Box::new(NativeNvmlLoader)
    }
    #[cfg(not(windows))]
    {
        Box::new(UnsupportedNvmlLoader)
    }
}

struct NvmlDevice {
    handle: NvmlDeviceHandle,
    metadata: RuntimeDeviceMetadata,
}

struct NvmlSession {
    _library: NvmlLibrary,
    dispatch: Box<dyn NvmlDispatch>,
    initialized: bool,
    devices: Vec<NvmlDevice>,
}

impl NvmlSession {
    fn open(loader: &dyn NvmlLoader) -> NvmlResult<Self> {
        let LoadedNvml {
            library,
            mut dispatch,
        } = loader.load()?;
        dispatch.init()?;
        let mut session = Self {
            _library: library,
            dispatch,
            initialized: true,
            devices: Vec::new(),
        };
        if let Err(error) = session.enumerate_devices() {
            let _ = session.shutdown();
            return Err(error);
        }
        Ok(session)
    }

    fn enumerate_devices(&mut self) -> NvmlResult<()> {
        let count = self.dispatch.device_count()?;
        if count == 0 {
            return Err(NvmlFailure::unsupported("no_compatible_nvidia_gpu"));
        }
        let mut last_failure = None;
        for index in 0..count {
            let handle = match self.dispatch.device_handle(index) {
                Ok(handle) => handle,
                Err(error) => {
                    last_failure = Some(error);
                    continue;
                }
            };
            let uuid = match self.dispatch.device_uuid(handle) {
                Ok(uuid) => uuid,
                Err(error) => {
                    // NVML enumeration order is deliberately never persisted as identity.
                    last_failure = Some(error);
                    continue;
                }
            };
            let stable_key = stable_device_key(&uuid)?;
            let model = self.dispatch.device_name(handle).ok();
            self.devices.push(NvmlDevice {
                handle,
                metadata: RuntimeDeviceMetadata {
                    stable_key,
                    category: MetricCategory::Gpu,
                    vendor: Some(NVIDIA_VENDOR.to_string()),
                    model,
                    capacity_bytes: None,
                },
            });
        }
        if self.devices.is_empty() {
            return Err(last_failure
                .unwrap_or_else(|| NvmlFailure::unsupported("no_compatible_nvidia_gpu")));
        }
        Ok(())
    }

    fn sample_all(&mut self) -> SessionSample {
        let mut samples = Vec::with_capacity(self.devices.len());
        let mut metadata = Vec::with_capacity(self.devices.len() * NvmlMetric::ALL.len());
        let mut first_failure = None;
        let mut fatal_failure = None;
        let mut has_value = false;

        for device in &mut self.devices {
            let outcome = sample_device(self.dispatch.as_mut(), device);
            has_value |= outcome.has_value;
            if first_failure.is_none() {
                first_failure = outcome.first_failure.clone();
            }
            if fatal_failure.is_none() {
                fatal_failure = outcome.fatal_failure.clone();
            }
            metadata.extend(outcome.metadata);
            samples.push(outcome.sample);
        }
        SessionSample {
            samples,
            metadata,
            first_failure,
            fatal_failure,
            has_value,
        }
    }

    fn shutdown(&mut self) -> NvmlResult<()> {
        if !self.initialized {
            return Ok(());
        }
        self.initialized = false;
        self.dispatch.shutdown()
    }
}

impl Drop for NvmlSession {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

struct SessionSample {
    samples: Vec<GpuSample>,
    metadata: Vec<ProviderMetricMetadata>,
    first_failure: Option<NvmlFailure>,
    fatal_failure: Option<NvmlFailure>,
    has_value: bool,
}

struct DeviceSample {
    sample: GpuSample,
    metadata: Vec<ProviderMetricMetadata>,
    first_failure: Option<NvmlFailure>,
    fatal_failure: Option<NvmlFailure>,
    has_value: bool,
}

#[derive(Clone)]
struct MetricRead<T> {
    value: Option<T>,
    status: MetricRuntimeSupportStatus,
    failure: Option<NvmlFailure>,
}

fn metric_read<T>(result: NvmlResult<T>) -> MetricRead<T> {
    match result {
        Ok(value) => MetricRead {
            value: Some(value),
            status: MetricRuntimeSupportStatus::Supported,
            failure: None,
        },
        Err(failure) => MetricRead {
            value: None,
            status: failure.support_status,
            failure: Some(failure),
        },
    }
}

fn sample_device(dispatch: &mut dyn NvmlDispatch, device: &mut NvmlDevice) -> DeviceSample {
    let utilization = metric_read(dispatch.utilization(device.handle));
    let temperature = metric_read(dispatch.temperature(device.handle));
    let power = metric_read(dispatch.power_usage_mw(device.handle));
    let graphics_clock = metric_read(dispatch.clock_info(device.handle, NVML_CLOCK_GRAPHICS));
    let memory_clock = metric_read(dispatch.clock_info(device.handle, NVML_CLOCK_MEM));
    let memory = metric_read(dispatch.memory_info(device.handle));

    let vram_total_bytes = memory
        .value
        .as_ref()
        .and_then(|memory| bytes_to_i64(memory.total));
    if vram_total_bytes.is_some() {
        device.metadata.capacity_bytes = vram_total_bytes;
    }

    let mut metadata = Vec::with_capacity(NvmlMetric::ALL.len());
    let mut first_failure = None;
    let mut fatal_failure = None;
    let mut record = |metric: NvmlMetric,
                      status: MetricRuntimeSupportStatus,
                      failure: &Option<NvmlFailure>| {
        metadata.push(metric_metadata(Some(&device.metadata), metric, status));
        if first_failure.is_none()
            && failure.as_ref().is_some_and(|failure| {
                failure.support_status != MetricRuntimeSupportStatus::Unsupported
            })
        {
            first_failure = failure.clone();
        }
        if fatal_failure.is_none() && failure.as_ref().is_some_and(NvmlFailure::is_runtime_fatal) {
            fatal_failure = failure.clone();
        }
    };
    record(
        NvmlMetric::Utilization,
        utilization.status,
        &utilization.failure,
    );
    record(
        NvmlMetric::MemoryControllerUtilization,
        utilization.status,
        &utilization.failure,
    );
    record(
        NvmlMetric::Temperature,
        temperature.status,
        &temperature.failure,
    );
    record(NvmlMetric::Power, power.status, &power.failure);
    record(
        NvmlMetric::GraphicsClock,
        graphics_clock.status,
        &graphics_clock.failure,
    );
    record(
        NvmlMetric::MemoryClock,
        memory_clock.status,
        &memory_clock.failure,
    );
    record(NvmlMetric::VramUsed, memory.status, &memory.failure);
    record(NvmlMetric::VramTotal, memory.status, &memory.failure);

    let has_value = utilization.value.is_some()
        || temperature.value.is_some()
        || power.value.is_some()
        || graphics_clock.value.is_some()
        || memory_clock.value.is_some()
        || memory.value.is_some();
    DeviceSample {
        sample: GpuSample {
            device_key: device.metadata.stable_key.clone(),
            vendor: device.metadata.vendor.clone(),
            model: device.metadata.model.clone(),
            capacity_bytes: device.metadata.capacity_bytes,
            utilization_percent: utilization.value.as_ref().map(|value| value.gpu as f64),
            memory_controller_utilization_percent: utilization
                .value
                .as_ref()
                .map(|value| value.memory as f64),
            temperature_celsius: temperature.value.map(|value| value as f64),
            power_watts: power.value.map(|value| value as f64 / 1_000.0),
            graphics_clock_mhz: graphics_clock.value.map(|value| value as f64),
            memory_clock_mhz: memory_clock.value.map(|value| value as f64),
            vram_used_bytes: memory
                .value
                .as_ref()
                .and_then(|value| bytes_to_i64(value.used)),
            vram_total_bytes,
            power_scope: power
                .value
                .is_some()
                .then(|| GPU_BOARD_POWER_SCOPE.to_string()),
            quality_mask: 0,
        },
        metadata,
        first_failure,
        fatal_failure,
        has_value,
    }
}

fn metric_metadata(
    device: Option<&RuntimeDeviceMetadata>,
    metric: NvmlMetric,
    support_status: MetricRuntimeSupportStatus,
) -> ProviderMetricMetadata {
    ProviderMetricMetadata {
        category: MetricCategory::Gpu,
        metric_key: metric.key().to_string(),
        device: device.cloned(),
        support_status,
    }
}

fn system_metric_metadata(status: MetricRuntimeSupportStatus) -> Vec<ProviderMetricMetadata> {
    NvmlMetric::ALL
        .into_iter()
        .map(|metric| metric_metadata(None, metric, status))
        .collect()
}

fn bytes_to_i64(value: u64) -> Option<i64> {
    i64::try_from(value).ok()
}

fn stable_device_key(uuid: &str) -> NvmlResult<String> {
    let uuid = uuid.trim();
    if uuid.is_empty() {
        return Err(NvmlFailure::unsupported("nvml_uuid_missing"));
    }
    Ok(format!("gpu:nvidia:uuid:{}", uuid.to_ascii_lowercase()))
}

pub struct NvidiaNvmlProvider {
    descriptor: ProviderDescriptor,
    loader: Box<dyn NvmlLoader>,
    session: Option<NvmlSession>,
    metadata: Vec<ProviderMetricMetadata>,
    health: ProviderHealthObservation,
}

impl NvidiaNvmlProvider {
    pub fn new() -> Self {
        Self::with_loader(default_loader())
    }

    fn with_loader(loader: Box<dyn NvmlLoader>) -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: NVIDIA_NVML_PROVIDER_ID.to_string(),
                display_name: "NVIDIA NVML".to_string(),
                schedule: ProviderSchedule::System,
                capabilities: vec![ProviderCapabilitySpec::supported(MetricCategory::Gpu)],
            },
            loader,
            session: None,
            metadata: Vec::new(),
            health: ProviderHealthObservation::default(),
        }
    }

    fn set_capability(
        &mut self,
        support_status: CapabilitySupportStatus,
        reason: Option<ProviderErrorCode>,
    ) {
        self.descriptor.capabilities = vec![ProviderCapabilitySpec {
            category: MetricCategory::Gpu,
            support_status,
            reason_code: reason,
        }];
    }

    fn set_unavailable(&mut self, failure: &NvmlFailure, probe_stage: bool) {
        let status = if probe_stage
            && failure.support_status == MetricRuntimeSupportStatus::Failed
            && !failure.is_runtime_fatal()
        {
            MetricRuntimeSupportStatus::ProbeFailed
        } else {
            failure.support_status
        };
        self.metadata = system_metric_metadata(status);
        self.set_capability(
            CapabilitySupportStatus::Unsupported,
            Some(failure.provider_error),
        );
    }

    fn update_capability_from_metadata(&mut self) {
        if self
            .metadata
            .iter()
            .any(|metric| metric.support_status == MetricRuntimeSupportStatus::Supported)
        {
            self.set_capability(CapabilitySupportStatus::Supported, None);
        } else {
            let reason = self
                .metadata
                .iter()
                .find_map(|metric| match metric.support_status {
                    MetricRuntimeSupportStatus::PermissionDenied => {
                        Some(ProviderErrorCode::PermissionDenied)
                    }
                    MetricRuntimeSupportStatus::ProviderMissing => {
                        Some(ProviderErrorCode::ProviderMissing)
                    }
                    MetricRuntimeSupportStatus::Unsupported => Some(ProviderErrorCode::Unsupported),
                    MetricRuntimeSupportStatus::ProbeFailed => {
                        Some(ProviderErrorCode::StartupFailed)
                    }
                    MetricRuntimeSupportStatus::Failed => Some(ProviderErrorCode::RuntimeFailed),
                    MetricRuntimeSupportStatus::Supported => None,
                })
                .or(Some(ProviderErrorCode::Unsupported));
            self.set_capability(CapabilitySupportStatus::Unsupported, reason);
        }
    }

    fn record_failure(&mut self, failure: &NvmlFailure, code: ProviderErrorCode) {
        self.health.failure_count = self.health.failure_count.saturating_add(1);
        self.health.last_error = Some(ProviderErrorSummary {
            code,
            message: Some(failure.reason.to_string()),
        });
    }

    fn release_session(&mut self) -> NvmlResult<()> {
        let Some(mut session) = self.session.take() else {
            return Ok(());
        };
        session.shutdown()
    }

    fn probe_session(&mut self, context: &ProviderCallContext) -> Result<(), ProviderError> {
        context.check()?;
        if self.session.is_some() {
            // A running provider already owns one session. Do not create a second NVML session
            // while reconciling settings; its next scheduled sample refreshes runtime truth.
            return Ok(());
        }
        let mut session = match NvmlSession::open(self.loader.as_ref()) {
            Ok(session) => session,
            Err(failure) => {
                self.set_unavailable(&failure, true);
                self.record_failure(&failure, ProviderErrorCode::StartupFailed);
                return Ok(());
            }
        };
        let outcome = session.sample_all();
        self.metadata = outcome.metadata;
        self.update_capability_from_metadata();
        if let Some(failure) = outcome.first_failure.as_ref() {
            self.record_failure(failure, ProviderErrorCode::SampleFailed);
        } else {
            self.health.last_error = None;
        }
        if let Some(failure) = outcome.fatal_failure.as_ref() {
            self.set_unavailable(failure, true);
            self.record_failure(failure, ProviderErrorCode::RuntimeFailed);
        }
        let shutdown = session.shutdown();
        if let Err(failure) = shutdown {
            self.record_failure(&failure, ProviderErrorCode::StopFailed);
        }
        context.check()
    }
}

impl Default for NvidiaNvmlProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricProvider for NvidiaNvmlProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn probe(
        &mut self,
        context: &ProviderCallContext,
        requested_categories: &BTreeSet<MetricCategory>,
    ) -> Result<Vec<ProviderCapabilitySpec>, ProviderError> {
        context.check()?;
        if requested_categories.contains(&MetricCategory::Gpu) {
            self.probe_session(context)?;
        }
        Ok(self.descriptor.capabilities.clone())
    }

    fn start(
        &mut self,
        plan: &ProviderPlan,
        context: &ProviderCallContext,
    ) -> Result<ProviderLifecycleOutcome, ProviderError> {
        context.check()?;
        if !plan.enabled || !plan.enabled_categories.contains(&MetricCategory::Gpu) {
            self.release_session()
                .map_err(|failure| failure.as_provider_error(ProviderErrorCode::StopFailed))?;
            return Ok(ProviderLifecycleOutcome {
                capabilities: Some(self.descriptor.capabilities.clone()),
            });
        }
        self.release_session()
            .map_err(|failure| failure.as_provider_error(ProviderErrorCode::StopFailed))?;
        let session = match NvmlSession::open(self.loader.as_ref()) {
            Ok(session) => session,
            Err(failure)
                if matches!(
                    failure.support_status,
                    MetricRuntimeSupportStatus::Unsupported
                        | MetricRuntimeSupportStatus::PermissionDenied
                        | MetricRuntimeSupportStatus::ProviderMissing
                ) =>
            {
                self.set_unavailable(&failure, false);
                self.record_failure(&failure, ProviderErrorCode::StartupFailed);
                return Ok(ProviderLifecycleOutcome {
                    capabilities: Some(self.descriptor.capabilities.clone()),
                });
            }
            Err(failure) => {
                self.metadata = system_metric_metadata(MetricRuntimeSupportStatus::Failed);
                self.set_capability(CapabilitySupportStatus::Supported, None);
                let code = if failure.is_runtime_fatal() {
                    ProviderErrorCode::RuntimeFailed
                } else {
                    ProviderErrorCode::StartupFailed
                };
                self.record_failure(&failure, code);
                return Err(failure.as_provider_error(code));
            }
        };
        // Probe has already established per-metric support. Start only owns the session and
        // stable device enumeration so it does not add a second full NVML metric sweep before
        // the first interval-driven sample.
        context.check()?;
        self.session = Some(session);
        Ok(ProviderLifecycleOutcome {
            capabilities: Some(self.descriptor.capabilities.clone()),
        })
    }

    fn reconfigure(
        &mut self,
        plan: &ProviderPlan,
        context: &ProviderCallContext,
    ) -> Result<ProviderLifecycleOutcome, ProviderError> {
        // Keep reconfiguration inside the canonical ProviderHost lifecycle: close the old
        // session before start so a disable/re-enable or recovery cannot create a double session.
        self.stop(context)?;
        self.start(plan, context).map_err(|error| {
            if error.code == ProviderErrorCode::StartupFailed {
                ProviderError {
                    code: ProviderErrorCode::ReconfigureFailed,
                    message: error.message,
                }
            } else {
                error
            }
        })
    }

    fn sample(
        &mut self,
        context: &ProviderCallContext,
        timestamp_ms: i64,
        _tracked_app_keys: &HashSet<String>,
    ) -> Result<Option<ProviderSample>, ProviderError> {
        context.check()?;
        let Some(session) = self.session.as_mut() else {
            return Err(ProviderError::without_message(
                ProviderErrorCode::StartupFailed,
            ));
        };
        let outcome = session.sample_all();
        self.metadata = outcome.metadata;
        if let Some(failure) = outcome.fatal_failure.as_ref() {
            self.record_failure(failure, ProviderErrorCode::RuntimeFailed);
            let _ = self.release_session();
            return Err(failure.as_provider_error(ProviderErrorCode::RuntimeFailed));
        }
        if let Some(failure) = outcome.first_failure.as_ref() {
            self.record_failure(failure, ProviderErrorCode::SampleFailed);
        } else {
            self.health.last_error = None;
        }
        if outcome.has_value {
            self.health.last_success_at_ms = Some(timestamp_ms);
        }
        context.check()?;
        Ok(outcome
            .has_value
            .then_some(ProviderSample::GpuSamples(outcome.samples)))
    }

    fn stop(&mut self, context: &ProviderCallContext) -> Result<(), ProviderError> {
        context.check()?;
        let result = self.release_session();
        match result {
            Ok(()) => {
                context.check()?;
                Ok(())
            }
            Err(failure) => {
                self.record_failure(&failure, ProviderErrorCode::StopFailed);
                Err(failure.as_provider_error(ProviderErrorCode::StopFailed))
            }
        }
    }

    fn health(&self) -> ProviderHealthObservation {
        self.health.clone()
    }

    fn metric_metadata(&self) -> Vec<ProviderMetricMetadata> {
        self.metadata.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        collector::provider::{CollectionPlan, ProviderHost},
        models::{CapabilityState, CollectionSettings, ProviderLifecycleState},
    };
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    #[derive(Debug, Clone)]
    struct FakeDevice {
        uuid: String,
        model: String,
        utilization: (u32, u32),
        temperature: u32,
        power_mw: u32,
        graphics_clock: u32,
        memory_clock: u32,
        memory: NvmlMemory,
        handle_status: u32,
        uuid_status: u32,
    }

    impl FakeDevice {
        fn new(uuid: &str, model: &str) -> Self {
            Self {
                uuid: uuid.to_string(),
                model: model.to_string(),
                utilization: (7, 3),
                temperature: 47,
                power_mw: 41_776,
                graphics_clock: 2_535,
                memory_clock: 16_001,
                memory: NvmlMemory {
                    total: 17_094_934_528,
                    free: 13_567_782_912,
                    used: 3_527_151_616,
                },
                handle_status: NVML_SUCCESS,
                uuid_status: NVML_SUCCESS,
            }
        }
    }

    #[derive(Debug, Clone)]
    struct FakeSessionConfig {
        devices: Vec<FakeDevice>,
        init_status: u32,
        shutdown_status: u32,
        metric_statuses: BTreeMap<(String, NvmlMetric), VecDeque<u32>>,
        delay_after_metric_call: Option<(u64, Duration)>,
    }

    impl FakeSessionConfig {
        fn with_devices(devices: Vec<FakeDevice>) -> Self {
            Self {
                devices,
                init_status: NVML_SUCCESS,
                shutdown_status: NVML_SUCCESS,
                metric_statuses: BTreeMap::new(),
                delay_after_metric_call: None,
            }
        }

        fn status(mut self, uuid: &str, metric: NvmlMetric, values: &[u32]) -> Self {
            self.metric_statuses
                .insert((uuid.to_string(), metric), values.iter().copied().collect());
            self
        }
    }

    #[derive(Clone)]
    struct FakeLoader {
        loads: Arc<Mutex<VecDeque<Result<FakeSessionConfig, NvmlFailure>>>>,
        stats: Arc<Mutex<FakeStats>>,
    }

    impl FakeLoader {
        fn new(
            loads: Vec<Result<FakeSessionConfig, NvmlFailure>>,
        ) -> (Self, Arc<Mutex<FakeStats>>) {
            let stats = Arc::new(Mutex::new(FakeStats::default()));
            (
                Self {
                    loads: Arc::new(Mutex::new(loads.into())),
                    stats: stats.clone(),
                },
                stats,
            )
        }
    }

    impl NvmlLoader for FakeLoader {
        fn load(&self) -> NvmlResult<LoadedNvml> {
            self.stats
                .lock()
                .expect("fake NVML stats lock poisoned")
                .load_count += 1;
            let config = self
                .loads
                .lock()
                .expect("fake NVML loader lock poisoned")
                .pop_front()
                .unwrap_or_else(|| Err(NvmlFailure::provider_missing("fake_loader_exhausted")))?;
            Ok(LoadedNvml {
                library: NvmlLibrary::Test(self.stats.clone()),
                dispatch: Box::new(FakeDispatch {
                    config,
                    stats: self.stats.clone(),
                }),
            })
        }
    }

    struct FakeDispatch {
        config: FakeSessionConfig,
        stats: Arc<Mutex<FakeStats>>,
    }

    impl FakeDispatch {
        fn device_index(handle: NvmlDeviceHandle) -> NvmlResult<usize> {
            handle
                .0
                .checked_sub(1)
                .ok_or_else(|| NvmlFailure::unsupported("nvml_device_not_found"))
        }

        fn device(&self, handle: NvmlDeviceHandle) -> NvmlResult<&FakeDevice> {
            self.config
                .devices
                .get(Self::device_index(handle)?)
                .ok_or_else(|| NvmlFailure::unsupported("nvml_device_not_found"))
        }

        fn metric_status(
            &mut self,
            handle: NvmlDeviceHandle,
            metric: NvmlMetric,
        ) -> NvmlResult<()> {
            let uuid = self.device(handle)?.uuid.clone();
            let metric_call_count = {
                let mut stats = self.stats.lock().expect("fake NVML stats lock poisoned");
                stats.metric_call_count += 1;
                stats.metric_call_count
            };
            if let Some((after, delay)) = self.config.delay_after_metric_call {
                if metric_call_count >= after {
                    thread::sleep(delay);
                }
            }
            let key = (uuid, metric);
            let status = self
                .config
                .metric_statuses
                .get_mut(&key)
                .and_then(|statuses| {
                    if statuses.len() > 1 {
                        statuses.pop_front()
                    } else {
                        statuses.front().copied()
                    }
                })
                .unwrap_or(NVML_SUCCESS);
            if status == NVML_SUCCESS {
                Ok(())
            } else {
                Err(nvml_failure(status))
            }
        }
    }

    impl NvmlDispatch for FakeDispatch {
        fn init(&mut self) -> NvmlResult<()> {
            self.stats
                .lock()
                .expect("fake NVML stats lock poisoned")
                .init_count += 1;
            if self.config.init_status == NVML_SUCCESS {
                Ok(())
            } else {
                Err(nvml_failure(self.config.init_status))
            }
        }

        fn shutdown(&mut self) -> NvmlResult<()> {
            self.stats
                .lock()
                .expect("fake NVML stats lock poisoned")
                .shutdown_count += 1;
            if self.config.shutdown_status == NVML_SUCCESS {
                Ok(())
            } else {
                Err(nvml_failure(self.config.shutdown_status))
            }
        }

        fn device_count(&mut self) -> NvmlResult<u32> {
            Ok(self.config.devices.len() as u32)
        }

        fn device_handle(&mut self, index: u32) -> NvmlResult<NvmlDeviceHandle> {
            let device = self
                .config
                .devices
                .get(index as usize)
                .ok_or_else(|| NvmlFailure::unsupported("nvml_device_not_found"))?;
            if device.handle_status == NVML_SUCCESS {
                Ok(NvmlDeviceHandle(index as usize + 1))
            } else {
                Err(nvml_failure(device.handle_status))
            }
        }

        fn device_uuid(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<String> {
            let device = self.device(handle)?;
            if device.uuid_status == NVML_SUCCESS {
                Ok(device.uuid.clone())
            } else {
                Err(nvml_failure(device.uuid_status))
            }
        }

        fn device_name(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<String> {
            Ok(self.device(handle)?.model.clone())
        }

        fn utilization(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<NvmlUtilization> {
            self.metric_status(handle, NvmlMetric::Utilization)?;
            let device = self.device(handle)?;
            Ok(NvmlUtilization {
                gpu: device.utilization.0,
                memory: device.utilization.1,
            })
        }

        fn temperature(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<u32> {
            self.metric_status(handle, NvmlMetric::Temperature)?;
            Ok(self.device(handle)?.temperature)
        }

        fn power_usage_mw(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<u32> {
            self.metric_status(handle, NvmlMetric::Power)?;
            Ok(self.device(handle)?.power_mw)
        }

        fn clock_info(&mut self, handle: NvmlDeviceHandle, clock: u32) -> NvmlResult<u32> {
            let metric = if clock == NVML_CLOCK_MEM {
                NvmlMetric::MemoryClock
            } else {
                NvmlMetric::GraphicsClock
            };
            self.metric_status(handle, metric)?;
            let device = self.device(handle)?;
            Ok(if clock == NVML_CLOCK_MEM {
                device.memory_clock
            } else {
                device.graphics_clock
            })
        }

        fn memory_info(&mut self, handle: NvmlDeviceHandle) -> NvmlResult<NvmlMemory> {
            self.metric_status(handle, NvmlMetric::VramUsed)?;
            Ok(self.device(handle)?.memory)
        }
    }

    fn gpu_settings() -> CollectionSettings {
        CollectionSettings {
            enabled_categories: vec![MetricCategory::Gpu],
            ..CollectionSettings::default()
        }
    }

    fn host_with(provider: NvidiaNvmlProvider, settings: &CollectionSettings) -> ProviderHost {
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        host.probe_all_for_settings(settings);
        host.apply_desired_plan(
            CollectionPlan::build_desired(settings, &host.descriptors()),
            Instant::now(),
        );
        host
    }

    fn gpu_samples(samples: Vec<ProviderSample>) -> Vec<GpuSample> {
        samples
            .into_iter()
            .flat_map(|sample| match sample {
                ProviderSample::GpuSamples(gpus) => gpus,
                ProviderSample::ResourceSnapshot(_) => Vec::new(),
            })
            .collect()
    }

    #[test]
    fn disabled_gpu_category_does_no_nvml_work() {
        let config = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")]);
        let (loader, stats) = FakeLoader::new(vec![Ok(config)]);
        let provider = NvidiaNvmlProvider::with_loader(Box::new(loader));
        let settings = CollectionSettings::default();
        let mut host = host_with(provider, &settings);

        assert!(host
            .sample_due(Instant::now(), 1_000, &HashSet::new())
            .is_empty());
        {
            let stats = stats.lock().unwrap();
            assert_eq!(stats.load_count, 0);
            assert_eq!(stats.init_count, 0);
            assert_eq!(stats.metric_call_count, 0);
        }
        drop(host);

        let config = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")]);
        let (loader, stats) = FakeLoader::new(vec![Ok(config)]);
        let explicitly_disabled = CollectionSettings {
            enabled_categories: vec![MetricCategory::Gpu],
            disabled_providers: vec![NVIDIA_NVML_PROVIDER_ID.into()],
            ..CollectionSettings::default()
        };
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &explicitly_disabled,
        );
        assert!(host
            .sample_due(Instant::now(), 1_000, &HashSet::new())
            .is_empty());
        let stats = stats.lock().unwrap();
        assert_eq!(stats.load_count, 0);
        assert_eq!(stats.init_count, 0);
        assert_eq!(stats.metric_call_count, 0);
    }

    #[test]
    fn missing_runtime_is_capability_not_crash() {
        let (loader, _) = FakeLoader::new(vec![Err(NvmlFailure::provider_missing(
            "nvml_runtime_missing",
        ))]);
        let host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        let status = &host.statuses()[0];
        assert!(!status.supported);
        assert_eq!(status.capabilities[0].state, CapabilityState::Unsupported);
        assert_eq!(
            status.capabilities[0].reason_code,
            Some(ProviderErrorCode::ProviderMissing)
        );
        assert!(host
            .collection_session_metric_metadata()
            .iter()
            .all(|metric| {
                metric.support_status == MetricRuntimeSupportStatus::ProviderMissing
                    && metric.enabled
                    && metric.device.is_none()
            }));
    }

    #[test]
    fn no_device_and_permission_denied_are_distinct() {
        let no_device = FakeSessionConfig::with_devices(Vec::new());
        let (loader, _) = FakeLoader::new(vec![Ok(no_device)]);
        let host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        assert_eq!(
            host.statuses()[0].capabilities[0].reason_code,
            Some(ProviderErrorCode::Unsupported)
        );

        let denied = FakeSessionConfig {
            init_status: NVML_ERROR_NO_PERMISSION,
            ..FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")])
        };
        let (loader, _) = FakeLoader::new(vec![Ok(denied)]);
        let host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        assert!(host
            .collection_session_metric_metadata()
            .iter()
            .all(|metric| {
                metric.support_status == MetricRuntimeSupportStatus::PermissionDenied
            }));
    }

    #[test]
    fn complete_sample_preserves_all_eight_metrics_and_board_power_scope() {
        let config = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")]);
        let (loader, _) = FakeLoader::new(vec![Ok(config.clone()), Ok(config)]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        let samples = gpu_samples(host.sample_due(Instant::now(), 2_000, &HashSet::new()));
        assert_eq!(samples.len(), 1);
        let gpu = &samples[0];
        assert_eq!(gpu.device_key, "gpu:nvidia:uuid:gpu-a");
        assert_eq!(gpu.utilization_percent, Some(7.0));
        assert_eq!(gpu.memory_controller_utilization_percent, Some(3.0));
        assert_eq!(gpu.temperature_celsius, Some(47.0));
        assert_eq!(gpu.power_watts, Some(41.776));
        assert_eq!(gpu.graphics_clock_mhz, Some(2_535.0));
        assert_eq!(gpu.memory_clock_mhz, Some(16_001.0));
        assert_eq!(gpu.vram_used_bytes, Some(3_527_151_616));
        assert_eq!(gpu.vram_total_bytes, Some(17_094_934_528));
        assert_eq!(gpu.power_scope.as_deref(), Some(GPU_BOARD_POWER_SCOPE));
    }

    #[test]
    fn legal_zero_is_not_unsupported() {
        let mut device = FakeDevice::new("GPU-A", "GPU A");
        device.utilization = (0, 0);
        device.power_mw = 0;
        device.memory.used = 0;
        let config = FakeSessionConfig::with_devices(vec![device]);
        let (loader, _) = FakeLoader::new(vec![Ok(config.clone()), Ok(config)]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        let gpu = gpu_samples(host.sample_due(Instant::now(), 2_000, &HashSet::new()))
            .pop()
            .unwrap();
        assert_eq!(gpu.utilization_percent, Some(0.0));
        assert_eq!(gpu.memory_controller_utilization_percent, Some(0.0));
        assert_eq!(gpu.power_watts, Some(0.0));
        assert_eq!(gpu.vram_used_bytes, Some(0));
        assert_eq!(gpu.power_scope.as_deref(), Some(GPU_BOARD_POWER_SCOPE));
    }

    #[test]
    fn partial_unsupported_and_transient_failure_keep_other_metrics() {
        let config = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")])
            .status("GPU-A", NvmlMetric::Power, &[NVML_ERROR_NOT_SUPPORTED])
            .status(
                "GPU-A",
                NvmlMetric::MemoryClock,
                &[NVML_ERROR_FREQ_NOT_SUPPORTED],
            );
        let (loader, _) = FakeLoader::new(vec![Ok(config.clone()), Ok(config)]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        let gpu = gpu_samples(host.sample_due(Instant::now(), 2_000, &HashSet::new()))
            .pop()
            .unwrap();
        assert_eq!(gpu.power_watts, None);
        assert_eq!(gpu.memory_clock_mhz, None);
        assert_eq!(gpu.utilization_percent, Some(7.0));
        assert_eq!(gpu.temperature_celsius, Some(47.0));
        let metadata = host.collection_session_metric_metadata();
        assert!(metadata.iter().any(|metric| {
            metric.metric_key == "gpu.power_watts"
                && metric.support_status == MetricRuntimeSupportStatus::Unsupported
        }));

        let transient_probe =
            FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-B", "GPU B")]);
        let transient_session =
            FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-B", "GPU B")]).status(
                "GPU-B",
                NvmlMetric::Power,
                &[NVML_ERROR_TIMEOUT, NVML_SUCCESS],
            );
        let (loader, stats) = FakeLoader::new(vec![Ok(transient_probe), Ok(transient_session)]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        let first = gpu_samples(host.sample_due(Instant::now(), 2_000, &HashSet::new()))
            .pop()
            .unwrap();
        assert_eq!(first.power_watts, None);
        let second = gpu_samples(host.sample_due(
            Instant::now() + Duration::from_secs(6),
            8_000,
            &HashSet::new(),
        ))
        .pop()
        .unwrap();
        assert_eq!(second.power_watts, Some(41.776));
        // Six native calls form the eight storage fields; no internal retry added a seventh call.
        let metric_call_count = stats.lock().unwrap().metric_call_count;
        assert_eq!(metric_call_count, 18);
    }

    #[test]
    fn fatal_runtime_failure_reenters_host_reconfigure_path() {
        let probe = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")]);
        let failing = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")])
            .status("GPU-A", NvmlMetric::Utilization, &[NVML_ERROR_GPU_IS_LOST]);
        let recovered = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")]);
        let (loader, stats) = FakeLoader::new(vec![
            Ok(probe),
            Ok(failing),
            Ok(recovered.clone()),
            Ok(recovered),
        ]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        assert!(host
            .sample_due(Instant::now(), 2_000, &HashSet::new())
            .is_empty());
        assert_eq!(host.statuses()[0].lifecycle, ProviderLifecycleState::Failed);
        assert!(host
            .collection_session_metric_metadata()
            .iter()
            .any(|metric| {
                metric.metric_key == "gpu.utilization_percent"
                    && metric.support_status == MetricRuntimeSupportStatus::Failed
            }));
        let recovered_samples = gpu_samples(host.sample_due(
            Instant::now() + Duration::from_secs(6),
            8_000,
            &HashSet::new(),
        ));
        assert_eq!(recovered_samples.len(), 1);
        assert!(stats.lock().unwrap().init_count >= 3);
    }

    #[test]
    fn disable_reenable_releases_and_recreates_the_session() {
        let config = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")]);
        let (loader, stats) = FakeLoader::new(vec![
            Ok(config.clone()),
            Ok(config.clone()),
            Ok(config.clone()),
            Ok(config),
        ]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        assert_eq!(
            gpu_samples(host.sample_due(Instant::now(), 2_000, &HashSet::new())).len(),
            1
        );

        let disabled = CollectionSettings::default();
        host.probe_all_for_settings(&disabled);
        host.apply_desired_plan(
            CollectionPlan::build_desired(&disabled, &host.descriptors()),
            Instant::now(),
        );
        assert!(host
            .sample_due(
                Instant::now() + Duration::from_secs(6),
                8_000,
                &HashSet::new()
            )
            .is_empty());
        let after_disable = stats.lock().unwrap().shutdown_count;
        assert!(after_disable >= 2);

        let enabled = gpu_settings();
        host.probe_all_for_settings(&enabled);
        host.apply_desired_plan(
            CollectionPlan::build_desired(&enabled, &host.descriptors()),
            Instant::now(),
        );
        assert_eq!(
            gpu_samples(host.sample_due(Instant::now(), 10_000, &HashSet::new())).len(),
            1
        );
        host.stop_all(Instant::now() + Duration::from_secs(1))
            .unwrap();
        let stats = stats.lock().unwrap();
        assert!(stats.init_count >= 4);
        assert!(stats.shutdown_count >= 4);
        assert!(stats.library_release_count >= 4);
    }

    #[test]
    fn stable_uuid_identity_survives_enumeration_reorder() {
        let a = FakeDevice::new("GPU-A", "GPU A");
        let b = FakeDevice::new("GPU-B", "GPU B");
        let run_one = FakeSessionConfig::with_devices(vec![a.clone(), b.clone()]);
        let run_two = FakeSessionConfig::with_devices(vec![b, a]);
        let (loader, _) = FakeLoader::new(vec![
            Ok(run_one.clone()),
            Ok(run_one),
            Ok(run_two.clone()),
            Ok(run_two),
        ]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        let first: BTreeSet<_> =
            gpu_samples(host.sample_due(Instant::now(), 2_000, &HashSet::new()))
                .into_iter()
                .map(|gpu| gpu.device_key)
                .collect();

        let disabled = CollectionSettings::default();
        host.probe_all_for_settings(&disabled);
        host.apply_desired_plan(
            CollectionPlan::build_desired(&disabled, &host.descriptors()),
            Instant::now(),
        );
        let enabled = gpu_settings();
        host.probe_all_for_settings(&enabled);
        host.apply_desired_plan(
            CollectionPlan::build_desired(&enabled, &host.descriptors()),
            Instant::now(),
        );
        let second: BTreeSet<_> =
            gpu_samples(host.sample_due(Instant::now(), 4_000, &HashSet::new()))
                .into_iter()
                .map(|gpu| gpu.device_key)
                .collect();
        assert_eq!(first, second);
        assert_eq!(
            first,
            BTreeSet::from([
                "gpu:nvidia:uuid:gpu-a".to_string(),
                "gpu:nvidia:uuid:gpu-b".to_string(),
            ])
        );
    }

    #[test]
    fn an_unavailable_device_does_not_discard_the_other_gpu() {
        let available = FakeDevice::new("GPU-A", "GPU A");
        let mut unavailable = FakeDevice::new("GPU-B", "GPU B");
        unavailable.handle_status = NVML_ERROR_GPU_IS_LOST;
        let config = FakeSessionConfig::with_devices(vec![available, unavailable]);
        let (loader, _) = FakeLoader::new(vec![Ok(config.clone()), Ok(config)]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );

        let samples = gpu_samples(host.sample_due(Instant::now(), 2_000, &HashSet::new()));
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].device_key, "gpu:nvidia:uuid:gpu-a");
        assert_eq!(samples[0].power_watts, Some(41.776));
    }

    #[test]
    fn sample_timeout_is_bounded_by_the_existing_executor() {
        let mut config = FakeSessionConfig::with_devices(vec![FakeDevice::new("GPU-A", "GPU A")]);
        // Probe consumes six metric calls. Delay the first interval-driven call to simulate a
        // blocked native driver boundary; start itself must not do another metric sweep.
        config.delay_after_metric_call = Some((7, Duration::from_millis(350)));
        let (loader, _) = FakeLoader::new(vec![Ok(config.clone()), Ok(config)]);
        let mut host = host_with(
            NvidiaNvmlProvider::with_loader(Box::new(loader)),
            &gpu_settings(),
        );
        let started = Instant::now();
        assert!(host.sample_due(started, 2_000, &HashSet::new()).is_empty());
        assert!(started.elapsed() < Duration::from_millis(330));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires a physical NVIDIA GPU and NVML runtime"]
    fn native_production_path_smoke() {
        let settings = gpu_settings();
        let mut host = host_with(NvidiaNvmlProvider::new(), &settings);
        let provider_status = host
            .statuses()
            .into_iter()
            .find(|status| status.provider_id == NVIDIA_NVML_PROVIDER_ID)
            .expect("NVIDIA provider status");
        assert!(provider_status.supported, "{provider_status:?}");

        let samples = gpu_samples(host.sample_due(Instant::now(), 60_000, &HashSet::new()));
        assert!(!samples.is_empty(), "NVML produced no GPU sample");
        assert!(samples.iter().all(|sample| {
            sample.device_key.starts_with("gpu:nvidia:uuid:")
                && !sample.device_key.contains("index-")
        }));
        let supported_metrics: Vec<_> = host
            .collection_session_metric_metadata()
            .into_iter()
            .filter(|metric| metric.support_status == MetricRuntimeSupportStatus::Supported)
            .map(|metric| metric.metric_key)
            .collect();
        eprintln!(
            "native NVIDIA NVML smoke: devices={}, supported_metrics={supported_metrics:?}",
            samples.len()
        );

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let directory = std::env::temp_dir().join(format!(
            "resource-timeline-nvidia-smoke-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let database = crate::db::Database::open(directory.join("smoke.sqlite3")).unwrap();
        let metadata = host.collection_session_metric_metadata();
        database
            .with_writer(|conn| {
                crate::db::writer::sync_collection_session_metrics(conn, &metadata, 61_000)
            })
            .unwrap();
        let mut frame_writer = crate::db::writer::FrameWriter::new(8, 0);
        let frame_samples = crate::collector::manager::merge_provider_samples(
            host.sample_due(
                Instant::now() + Duration::from_secs(6),
                62_000,
                &HashSet::new(),
            ),
            62_000,
            2_000,
        );
        let emitted_gpu_count: usize = frame_samples
            .iter()
            .map(|snapshot| snapshot.system.gpus.len())
            .sum();
        assert!(
            emitted_gpu_count >= samples.len(),
            "second production sample did not reach the frame contract"
        );
        for snapshot in frame_samples {
            frame_writer.enqueue(snapshot);
        }
        database
            .with_writer(|conn| frame_writer.flush_all(conn))
            .unwrap();
        let gpu_row_count = database
            .read(|conn| {
                conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap();
        eprintln!(
            "native NVIDIA NVML smoke: emitted_gpu_samples={emitted_gpu_count}, gpu_rows={gpu_row_count}"
        );
        assert!(gpu_row_count >= samples.len() as i64);

        let disabled_settings = CollectionSettings::default();
        host.probe_all_for_settings(&disabled_settings);
        host.apply_desired_plan(
            CollectionPlan::build_desired(&disabled_settings, &host.descriptors()),
            Instant::now(),
        );
        assert!(host
            .sample_due(
                Instant::now() + Duration::from_secs(6),
                64_000,
                &HashSet::new(),
            )
            .is_empty());
        database
            .with_writer(|conn| {
                crate::db::writer::sync_collection_session_metrics(
                    conn,
                    &host.collection_session_metric_metadata(),
                    64_000,
                )
            })
            .unwrap();
        let rows_after_disable = database
            .read(|conn| {
                conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap();
        assert_eq!(rows_after_disable, gpu_row_count);

        host.probe_all_for_settings(&settings);
        host.apply_desired_plan(
            CollectionPlan::build_desired(&settings, &host.descriptors()),
            Instant::now(),
        );
        let reenabled_frames = crate::collector::manager::merge_provider_samples(
            host.sample_due(
                Instant::now() + Duration::from_secs(6),
                66_000,
                &HashSet::new(),
            ),
            66_000,
            2_000,
        );
        assert!(reenabled_frames
            .iter()
            .any(|frame| !frame.system.gpus.is_empty()));
        database
            .with_writer(|conn| {
                crate::db::writer::sync_collection_session_metrics(
                    conn,
                    &host.collection_session_metric_metadata(),
                    66_000,
                )
            })
            .unwrap();
        for snapshot in reenabled_frames {
            frame_writer.enqueue(snapshot);
        }
        database
            .with_writer(|conn| frame_writer.flush_all(conn))
            .unwrap();
        let rows_after_reenable = database
            .read(|conn| {
                conn.query_row("SELECT COUNT(*) FROM gpu_sample", [], |row| {
                    row.get::<_, i64>(0)
                })
            })
            .unwrap();
        assert!(rows_after_reenable > rows_after_disable);
        host.stop_all(Instant::now() + Duration::from_secs(2))
            .unwrap();
        drop(database);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
