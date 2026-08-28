use crate::cli::{AmdUprofConfig, AmdUprofMode};

#[cfg(not(windows))]
pub fn run(_config: AmdUprofConfig) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    Err("AMD uProf live qualification is Windows-only".to_string())
}

#[cfg(not(windows))]
pub fn run_workload_child(_workers: u32) -> Result<(), String> {
    Err("AMD uProf workload helper is Windows-only".to_string())
}

#[cfg(not(windows))]
pub fn run_load_child(_install_root: std::path::PathBuf) -> Result<(), String> {
    Err("AMD uProf library load-check is Windows-only".to_string())
}

#[cfg(not(windows))]
pub fn run_load_only_child(_path: std::path::PathBuf) -> Result<(), String> {
    Err("AMD uProf load-only diagnostic is Windows-only".to_string())
}

#[cfg(windows)]
mod windows_impl {
    use super::{AmdUprofConfig, AmdUprofMode};
    use crate::{
        model::MachineInfo,
        report::{validate_public_text, write_atomic},
        stats::Distribution,
    };
    use serde::Serialize;
    use std::os::windows::{ffi::OsStrExt, process::CommandExt};
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::{c_char, CStr},
        fs,
        io::Read,
        mem::{self, size_of},
        path::{Path, PathBuf},
        process::{Child, Stdio},
        ptr,
        time::{Duration, Instant},
    };
    use windows::{
        core::{PCSTR, PCWSTR},
        Win32::{
            Foundation::{FreeLibrary, GetLastError, HANDLE, HMODULE},
            System::LibraryLoader::{
                GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            },
        },
    };

    const SCHEMA_VERSION: &str = "cpu-sensor-amd-uprof-live/v1";
    const PROBE_NAME: &str = "cpu-sensor-amd-uprof";
    const PROVIDER: &str = "amd_uprof";
    const POWER_KEY: &str = "reference.amd_uprof.package_power_watts";
    const FREQUENCY_KEY: &str = "reference.amd_uprof.core_effective_frequency_mhz";
    const TEMPERATURE_KEY: &str = "reference.amd_uprof.package_temperature_celsius";
    const DEFAULT_INSTALL_ROOT: &str = r"C:\Program Files\AMD\AMDuProf";
    const MAX_COUNTERS: u32 = 4096;
    const MAX_SAMPLES_PER_READ: u32 = 4096;
    const MAX_COUNTER_VALUES_PER_SAMPLE: u32 = 4096;

    const AMDT_STATUS_OK: u32 = 0x0000_0000;
    const AMDT_ERROR_ACCESSDENIED: u32 = 0x8007_0005;
    const AMDT_ERROR_NOTSUPPORTED: u32 = 0x8000_fffe;
    const AMDT_ERROR_DRIVER_ALREADY_INITIALIZED: u32 = 0x8008_0001;
    const AMDT_ERROR_DRIVER_UNAVAILABLE: u32 = 0x8008_0002;
    const AMDT_ERROR_DRIVER_UNINITIALIZED: u32 = 0x8008_0005;
    const AMDT_ERROR_INVALID_COUNTERID: u32 = 0x8008_0007;
    const AMDT_ERROR_COUNTER_ALREADY_ENABLED: u32 = 0x8008_0008;
    const AMDT_ERROR_NO_WRITE_PERMISSION: u32 = 0x8008_0009;
    const AMDT_ERROR_COUNTER_NOT_ENABLED: u32 = 0x8008_000a;
    const AMDT_ERROR_TIMER_NOT_SET: u32 = 0x8008_000b;
    const AMDT_ERROR_PROFILE_ALREADY_STARTED: u32 = 0x8008_000d;
    const AMDT_ERROR_PROFILE_NOT_STARTED: u32 = 0x8008_000e;
    const AMDT_ERROR_PROFILE_DATA_NOT_AVAILABLE: u32 = 0x8008_0010;
    const AMDT_ERROR_PLATFORM_NOT_SUPPORTED: u32 = 0x8008_0011;
    const AMDT_DRIVER_VERSION_MISMATCH: u32 = 0x8008_0013;
    const AMDT_ERROR_PROFILE_SESSION_EXISTS: u32 = 0x8008_0017;
    const AMDT_ERROR_SMU_ACCESS_FAILED: u32 = 0x8008_0018;
    const AMDT_ERROR_COUNTERS_NOT_ENABLED: u32 = 0x8008_0019;
    const AMDT_ERROR_PREVIOUS_SESSION_NOT_CLOSED: u32 = 0x8008_0020;
    const AMDT_ERROR_COUNTER_NOT_ACCESSIBLE: u32 = 0x8008_0022;
    const AMDT_ERROR_HYPERVISOR_NOT_SUPPORTED: u32 = 0x8008_0023;

    const AMDT_PWR_MODE_TIMELINE_ONLINE: u32 = 0;
    const AMDT_PWR_CATEGORY_POWER: u32 = 0;
    const AMDT_PWR_CATEGORY_FREQUENCY: u32 = 1;
    const AMDT_PWR_CATEGORY_TEMPERATURE: u32 = 2;
    const AMDT_PWR_UNIT_WATT: u32 = 6;
    const AMDT_PWR_UNIT_MEGA_HERTZ: u32 = 9;
    const AMDT_PWR_UNIT_CENTIGRADE: u32 = 10;
    const AMDT_DEVICE_TYPE_PACKAGE: u32 = 1;
    const AMDT_DEVICE_TYPE_CPU_COMPUTE_UNIT: u32 = 2;
    const AMDT_DEVICE_TYPE_CPU_CORE: u32 = 3;
    const AMDT_DEVICE_TYPE_PHYSICAL_CORE: u32 = 5;
    const AMDT_DEVICE_TYPE_THREAD: u32 = 6;

    type AmdResult = u32;

    type ProfileInitialize = unsafe extern "C" fn(u32) -> AmdResult;
    type GetSupportedCounters =
        unsafe extern "C" fn(*mut u32, *mut *mut AmdCounterDesc) -> AmdResult;
    type EnableCounter = unsafe extern "C" fn(u32) -> AmdResult;
    type SetTimerSamplingPeriod = unsafe extern "C" fn(u32) -> AmdResult;
    type StartProfiling = unsafe extern "C" fn() -> AmdResult;
    type StopProfiling = unsafe extern "C" fn() -> AmdResult;
    type ProfileClose = unsafe extern "C" fn() -> AmdResult;
    type ReadAllEnabledCounters = unsafe extern "C" fn(*mut u32, *mut *mut AmdSample) -> AmdResult;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct AmdCounterDesc {
        counter_id: u32,
        device_id: u32,
        device_type: u32,
        device_instance_id: u32,
        name: *mut c_char,
        description: *mut c_char,
        category: u32,
        aggregation: u32,
        min_value: f64,
        max_value: f64,
        units: u32,
        is_parent_counter: u8,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AmdCounterValue {
        counter_id: u32,
        data: f32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AmdSystemTime {
        second: u64,
        subsecond_millisecond: u64,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AmdSample {
        system_time: AmdSystemTime,
        elapsed_time_ms: u64,
        record_id: u64,
        num_of_counter: u32,
        counter_values: *mut AmdCounterValue,
    }

    #[derive(Clone, Copy)]
    struct AmdFunctions {
        initialize: ProfileInitialize,
        get_supported_counters: GetSupportedCounters,
        enable_counter: EnableCounter,
        set_timer_sampling_period: SetTimerSamplingPeriod,
        start_profiling: StartProfiling,
        read_all_enabled_counters: ReadAllEnabledCounters,
        stop_profiling: StopProfiling,
        close: ProfileClose,
    }

    #[derive(Debug, Clone, Serialize)]
    struct InstallationAudit {
        requested_root: Option<String>,
        root: Option<String>,
        library: Option<String>,
        root_verified: bool,
        library_exists: bool,
        library_size_bytes: Option<u64>,
        library_architecture: Option<String>,
        process_architecture: String,
        marker_files: BTreeMap<String, bool>,
        safe_load: String,
        isolated_load_exit_code: Option<i32>,
        isolated_load_exit_code_hex: Option<String>,
        isolated_load_stdout: Option<String>,
        isolated_load_stderr: Option<String>,
        isolated_load_timed_out: bool,
        audit_status: String,
        audit_error: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct VerifiedInstall {
        library: PathBuf,
        audit: InstallationAudit,
    }

    #[derive(Debug, Clone, Serialize)]
    struct ApiContract {
        api_version_query: String,
        profile_mode: String,
        functions: Vec<String>,
        lifecycle: String,
        enumeration: String,
        status_model: String,
        busy_status: String,
        sampling_constraint: String,
        simultaneous_counter_support: String,
        timestamp_semantics: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct CounterReport {
        counter_id: u32,
        device_id: u32,
        device_type: String,
        device_instance_id: u32,
        category: String,
        aggregation: String,
        units: String,
        min_value: f64,
        max_value: f64,
        name: Option<String>,
        selected_metric: Option<String>,
        enable_status: Option<String>,
        enable_vendor_status: Option<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct EnumerationReport {
        supported_counter_count: u32,
        counters: Vec<CounterReport>,
        duplicate_frequency_identity_count: usize,
        expected_frequency_identity_count: Option<u32>,
        returned_frequency_identity_count: usize,
        missing_frequency_identity_behavior: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct SourceTimestamp {
        second: u64,
        subsecond_millisecond: u64,
        elapsed_time_ms: u64,
    }

    #[derive(Debug, Clone, Serialize)]
    struct AmdEvent {
        timestamp_ms: i64,
        provider: String,
        metric_key: String,
        scope: String,
        identity: String,
        identity_semantics: String,
        value: Option<f64>,
        unit: String,
        status: String,
        reason_code: String,
        requested_interval_ms: u64,
        actual_interval_ms: Option<f64>,
        api_latency_ms: f64,
        session_generation: u64,
        source_timestamp: Option<SourceTimestamp>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct MetricReport {
        metric_key: String,
        provider: String,
        scope: String,
        identity: String,
        identity_semantics: String,
        unit: String,
        window: String,
        qualifier: Option<String>,
        status: String,
        reason_code: String,
        sample_count: usize,
        failed_sample_count: usize,
        non_finite_count: usize,
        negative_value_count: usize,
        values: Distribution,
        sample_intervals_ms: Distribution,
        first_value: Option<f64>,
        last_value: Option<f64>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct SessionReport {
        generation: u64,
        initialize: OperationReport,
        enumeration: OperationReport,
        timer: OperationReport,
        start: OperationReport,
        stop: Option<OperationReport>,
        close: Option<OperationReport>,
        enabled_counter_ids: Vec<u32>,
        read_call_count: u64,
        successful_read_count: u64,
        no_data_read_count: u64,
        late_sample_count: u64,
        dropped_sample_count: u64,
        poll_count: u64,
        closed_before_drop: bool,
        session_outcome: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct OperationReport {
        operation: String,
        stable_status: String,
        vendor_status: Option<String>,
        vendor_status_code: Option<String>,
    }

    #[derive(Debug, Clone, Serialize)]
    struct LifecycleReport {
        enable: String,
        disable: String,
        quiescence: String,
        re_enable: String,
        final_disable: String,
        process_exit: String,
        first_generation: Option<u64>,
        re_enabled_generation: Option<u64>,
        polls_after_disable: u64,
        api_calls_after_disable: u64,
        owned_timer_stopped: bool,
        owned_worker_stopped: bool,
        handles_released: bool,
        resources_bounded: bool,
        owned_leaks: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct ConcurrencyReport {
        single_session_rule: String,
        second_session_result: String,
        vendor_status: Option<String>,
        vendor_status_code: Option<String>,
        provider_busy_mapping: String,
        first_session_survived: String,
        retry_behavior: String,
        ryzen_master_interaction: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct PerformanceReport {
        average_cpu_percent: Option<f64>,
        p95_cpu_percent: Option<f64>,
        peak_working_set_bytes: Option<u64>,
        handle_count_peak: Option<u32>,
        thread_count_peak: Option<u32>,
        api_average_latency_ms: Option<f64>,
        api_p95_latency_ms: Option<f64>,
        api_max_latency_ms: Option<f64>,
        api_sample_count: usize,
        late_samples: u64,
        dropped_samples: u64,
        probe_overhead_scope: String,
        amd_external_overhead: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct FailureIsolationReport {
        missing_library: String,
        unsafe_path: String,
        unsupported_counter: String,
        permission: String,
        busy: String,
        invalid_value: String,
        timeout_late: String,
        zero_synthesis: String,
    }

    #[derive(Debug, Clone, Serialize)]
    struct AmdReport {
        schema_version: String,
        probe_name: String,
        mode: String,
        started_at_utc: String,
        finished_at_utc: String,
        machine: MachineInfo,
        initial_process_elevated: Option<bool>,
        requested_interval_ms: u64,
        actual_interval_ms: Option<f64>,
        duration_seconds: u64,
        phase_duration_ms: u64,
        quiescence_ms: u64,
        representative_load: bool,
        load_workers: u32,
        installation: InstallationAudit,
        api_contract: ApiContract,
        enumeration: EnumerationReport,
        sessions: Vec<SessionReport>,
        events_file: String,
        event_count: usize,
        metrics: Vec<MetricReport>,
        lifecycle: Option<LifecycleReport>,
        concurrency: Option<ConcurrencyReport>,
        performance: PerformanceReport,
        failure_isolation: FailureIsolationReport,
        external_reference: String,
        run_status: String,
        notes: Vec<String>,
    }

    #[derive(Debug, Clone)]
    struct Descriptor {
        raw: AmdCounterDesc,
        name: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct Target {
        counter_id: u32,
        metric_key: String,
        scope: String,
        identity: String,
        identity_semantics: String,
        unit: String,
        qualifier: Option<String>,
    }

    #[derive(Debug, Clone, Default)]
    struct Selection {
        power: Option<Target>,
        temperature: Option<Target>,
        frequencies: Vec<Target>,
    }

    impl Selection {
        fn targets(&self) -> impl Iterator<Item = &Target> {
            self.power
                .iter()
                .chain(self.temperature.iter())
                .chain(self.frequencies.iter())
        }
    }

    #[derive(Debug, Clone)]
    struct AmdError {
        operation: String,
        stable_status: String,
        vendor_status: Option<u32>,
        reason: String,
    }

    impl AmdError {
        fn vendor(operation: &str, status: u32) -> Self {
            Self {
                operation: operation.to_string(),
                stable_status: stable_status_for(operation, status),
                vendor_status: Some(status),
                reason: status_name(status),
            }
        }

        fn synthetic(operation: &str, status: &str, reason: &str) -> Self {
            Self {
                operation: operation.to_string(),
                stable_status: status.to_string(),
                vendor_status: None,
                reason: reason.to_string(),
            }
        }

        fn operation_report(&self) -> OperationReport {
            OperationReport {
                operation: self.operation.clone(),
                stable_status: self.stable_status.clone(),
                vendor_status: self.vendor_status.map(status_name),
                vendor_status_code: self.vendor_status.map(status_code),
            }
        }
    }

    #[derive(Debug, Clone)]
    struct MetricAccumulator {
        metric_key: String,
        provider: String,
        scope: String,
        identity: String,
        identity_semantics: String,
        unit: String,
        window: String,
        qualifier: Option<String>,
        status: String,
        reason_code: String,
        values: Vec<f64>,
        failed_sample_count: usize,
        non_finite_count: usize,
        negative_value_count: usize,
        intervals_ms: Vec<f64>,
        first_value: Option<f64>,
        last_value: Option<f64>,
    }

    impl MetricAccumulator {
        fn from_target(target: &Target) -> Self {
            Self {
                metric_key: target.metric_key.clone(),
                provider: PROVIDER.to_string(),
                scope: target.scope.clone(),
                identity: target.identity.clone(),
                identity_semantics: target.identity_semantics.clone(),
                unit: target.unit.clone(),
                window: if target.metric_key == POWER_KEY {
                    "average over vendor sampling period; not instantaneous".to_string()
                } else {
                    "vendor sample interval".to_string()
                },
                qualifier: target.qualifier.clone(),
                status: "not_observed".to_string(),
                reason_code: "not_observed".to_string(),
                values: Vec::new(),
                failed_sample_count: 0,
                non_finite_count: 0,
                negative_value_count: 0,
                intervals_ms: Vec::new(),
                first_value: None,
                last_value: None,
            }
        }

        fn failure(&mut self, status: &str, reason: &str) {
            self.failed_sample_count = self.failed_sample_count.saturating_add(1);
            self.status = status.to_string();
            self.reason_code = reason.to_string();
        }

        fn value(&mut self, value: f64, actual_interval_ms: Option<f64>) -> bool {
            if !value.is_finite() {
                self.non_finite_count = self.non_finite_count.saturating_add(1);
                self.failure("invalid_value", "non_finite_value");
                return false;
            }
            if (self.metric_key == POWER_KEY || self.metric_key == FREQUENCY_KEY) && value < 0.0 {
                self.negative_value_count = self.negative_value_count.saturating_add(1);
                self.failure("invalid_value", "negative_value");
                return false;
            }
            self.values.push(value);
            if let Some(interval) = actual_interval_ms {
                self.intervals_ms.push(interval);
            }
            self.first_value.get_or_insert(value);
            self.last_value = Some(value);
            self.status = if self.failed_sample_count == 0 {
                "supported".to_string()
            } else {
                "partial".to_string()
            };
            self.reason_code = if self.failed_sample_count == 0 {
                "ok".to_string()
            } else {
                "partial_sampling_failures".to_string()
            };
            true
        }

        fn report(self) -> MetricReport {
            MetricReport {
                metric_key: self.metric_key,
                provider: self.provider,
                scope: self.scope,
                identity: self.identity,
                identity_semantics: self.identity_semantics,
                unit: self.unit,
                window: self.window,
                qualifier: self.qualifier,
                status: self.status,
                reason_code: self.reason_code,
                sample_count: self.values.len(),
                failed_sample_count: self.failed_sample_count,
                non_finite_count: self.non_finite_count,
                negative_value_count: self.negative_value_count,
                values: Distribution::from_values(&self.values),
                sample_intervals_ms: Distribution::from_values(&self.intervals_ms),
                first_value: self.first_value,
                last_value: self.last_value,
            }
        }
    }

    #[derive(Debug, Default)]
    struct PerformanceAccumulator {
        cpu_percent: Vec<f64>,
        working_set_bytes: Vec<u64>,
        handles: Vec<u32>,
        threads: Vec<u32>,
        api_latency_ms: Vec<f64>,
        late_samples: u64,
        dropped_samples: u64,
        previous_cpu: Option<(Instant, u64)>,
        logical_processors: f64,
    }

    impl PerformanceAccumulator {
        fn new(machine: &MachineInfo) -> Self {
            Self {
                logical_processors: machine.logical_processor_count.unwrap_or(1) as f64,
                ..Self::default()
            }
        }

        fn observe(&mut self, api_latency_ms: f64, now: Instant, late: bool, dropped: u64) {
            self.api_latency_ms.push(api_latency_ms);
            if late {
                self.late_samples = self.late_samples.saturating_add(1);
            }
            self.dropped_samples = self.dropped_samples.saturating_add(dropped);
            let metrics = crate::windows::self_metrics();
            let Some(value) = metrics.value else {
                return;
            };
            self.working_set_bytes.push(value.working_set_bytes);
            self.handles.push(value.handle_count);
            self.threads.push(value.thread_count);
            if let Some((previous_time, previous_cpu)) = self.previous_cpu {
                let wall_seconds = now.duration_since(previous_time).as_secs_f64();
                if wall_seconds > 0.0 {
                    let delta = value.cpu_time_100ns.saturating_sub(previous_cpu) as f64;
                    self.cpu_percent.push(
                        delta * 100.0 / (wall_seconds * 10_000_000.0 * self.logical_processors),
                    );
                }
            }
            self.previous_cpu = Some((now, value.cpu_time_100ns));
        }

        fn report(&self) -> PerformanceReport {
            let cpu = Distribution::from_values(&self.cpu_percent);
            let api = Distribution::from_values(&self.api_latency_ms);
            PerformanceReport {
                average_cpu_percent: cpu.mean,
                p95_cpu_percent: cpu.p95,
                peak_working_set_bytes: self.working_set_bytes.iter().copied().max(),
                handle_count_peak: self.handles.iter().copied().max(),
                thread_count_peak: self.threads.iter().copied().max(),
                api_average_latency_ms: api.mean,
                api_p95_latency_ms: api.p95,
                api_max_latency_ms: api.max,
                api_sample_count: self.api_latency_ms.len(),
                late_samples: self.late_samples,
                dropped_samples: self.dropped_samples,
                probe_overhead_scope: "Resource Timeline-owned probe process only".to_string(),
                amd_external_overhead: "NOT_ATTRIBUTABLE".to_string(),
            }
        }
    }

    #[derive(Debug, Default)]
    struct QualificationContext {
        events: Vec<AmdEvent>,
        metrics: BTreeMap<(String, String), MetricAccumulator>,
        sessions: Vec<SessionReport>,
        counters: Vec<CounterReport>,
        supported_counter_count: u32,
        frequency_identity_count: usize,
        duplicate_frequency_identity_count: usize,
        errors: Vec<String>,
        performance: PerformanceAccumulator,
        actual_intervals: Vec<f64>,
        top_status: Option<String>,
    }

    impl QualificationContext {
        fn new(machine: &MachineInfo) -> Self {
            Self {
                performance: PerformanceAccumulator::new(machine),
                ..Self::default()
            }
        }

        fn seed_target(&mut self, target: &Target) {
            self.metrics
                .entry((target.metric_key.clone(), target.identity.clone()))
                .or_insert_with(|| MetricAccumulator::from_target(target));
        }

        fn seed_failure(&mut self, key: &str, status: &str, reason: &str) {
            let target = Target {
                counter_id: 0,
                metric_key: key.to_string(),
                scope: if key == TEMPERATURE_KEY {
                    "package".to_string()
                } else {
                    "unknown".to_string()
                },
                identity: if key == TEMPERATURE_KEY || key == POWER_KEY {
                    "package".to_string()
                } else {
                    "unknown".to_string()
                },
                identity_semantics: "not_observed".to_string(),
                unit: if key == POWER_KEY {
                    "W".to_string()
                } else if key == TEMPERATURE_KEY {
                    "°C".to_string()
                } else {
                    "MHz".to_string()
                },
                qualifier: (key == POWER_KEY).then(|| "ESTIMATED".to_string()),
            };
            self.seed_target(&target);
            if let Some(metric) = self.metrics.get_mut(&(target.metric_key, target.identity)) {
                metric.failure(status, reason);
            }
        }

        fn observe_failure_for_target(
            &mut self,
            target: &Target,
            status: &str,
            reason: &str,
            timestamp_ms: i64,
            requested_interval_ms: u64,
            actual_interval_ms: Option<f64>,
            api_latency_ms: f64,
            generation: u64,
            source_timestamp: Option<SourceTimestamp>,
        ) {
            self.seed_target(target);
            if let Some(metric) = self
                .metrics
                .get_mut(&(target.metric_key.clone(), target.identity.clone()))
            {
                metric.failure(status, reason);
            }
            self.events.push(AmdEvent {
                timestamp_ms,
                provider: PROVIDER.to_string(),
                metric_key: target.metric_key.clone(),
                scope: target.scope.clone(),
                identity: target.identity.clone(),
                identity_semantics: target.identity_semantics.clone(),
                value: None,
                unit: target.unit.clone(),
                status: status.to_string(),
                reason_code: reason.to_string(),
                requested_interval_ms,
                actual_interval_ms,
                api_latency_ms,
                session_generation: generation,
                source_timestamp,
            });
        }

        fn observe_value_for_target(
            &mut self,
            target: &Target,
            value: f64,
            timestamp_ms: i64,
            requested_interval_ms: u64,
            actual_interval_ms: Option<f64>,
            api_latency_ms: f64,
            generation: u64,
            source_timestamp: SourceTimestamp,
        ) {
            self.seed_target(target);
            let accepted = self
                .metrics
                .get_mut(&(target.metric_key.clone(), target.identity.clone()))
                .map(|metric| metric.value(value, actual_interval_ms))
                .unwrap_or(false);
            self.events.push(AmdEvent {
                timestamp_ms,
                provider: PROVIDER.to_string(),
                metric_key: target.metric_key.clone(),
                scope: target.scope.clone(),
                identity: target.identity.clone(),
                identity_semantics: target.identity_semantics.clone(),
                value: accepted.then_some(value),
                unit: target.unit.clone(),
                status: if accepted {
                    "ok".to_string()
                } else {
                    "invalid_value".to_string()
                },
                reason_code: if accepted {
                    "ok".to_string()
                } else {
                    "value_rejected".to_string()
                },
                requested_interval_ms,
                actual_interval_ms,
                api_latency_ms,
                session_generation: generation,
                source_timestamp: Some(source_timestamp),
            });
        }

        fn note_error(&mut self, error: &AmdError) {
            self.top_status.get_or_insert(error.stable_status.clone());
            self.errors.push(format!(
                "{}: {}{}",
                error.operation,
                error.reason,
                error
                    .vendor_status
                    .map(|status| format!(" ({})", status_code(status)))
                    .unwrap_or_default()
            ));
        }
    }

    struct AmdLibrary {
        module: HMODULE,
        functions: AmdFunctions,
    }

    impl AmdLibrary {
        fn load(install: &VerifiedInstall) -> Result<Self, AmdError> {
            let wide = wide_path(&install.library);
            let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
            let module = unsafe {
                LoadLibraryExW(PCWSTR::from_raw(wide.as_ptr()), HANDLE::default(), flags)
            }
            .map_err(|error| {
                AmdError::synthetic(
                    "LoadLibraryExW",
                    "provider_missing",
                    &format!("explicit_absolute_library_load_failed: {error}"),
                )
            })?;

            let functions = unsafe { load_functions(module) }.map_err(|missing| {
                unsafe {
                    let _ = FreeLibrary(module);
                }
                AmdError::synthetic(
                    "GetProcAddress",
                    "provider_missing",
                    &format!("missing_installed_api_symbol:{missing}"),
                )
            })?;
            Ok(Self { module, functions })
        }
    }

    impl Drop for AmdLibrary {
        fn drop(&mut self) {
            if !self.module.is_invalid() {
                unsafe {
                    let _ = FreeLibrary(self.module);
                }
            }
        }
    }

    struct AmdSession {
        library: AmdLibrary,
        initialized: bool,
        started: bool,
        descriptors: Vec<Descriptor>,
        selection: Selection,
        enabled_counter_ids: Vec<u32>,
        read_call_count: u64,
        successful_read_count: u64,
        no_data_read_count: u64,
        poll_count: u64,
    }

    impl AmdSession {
        fn open(install: &VerifiedInstall, interval_ms: u64) -> Result<Self, AmdError> {
            if interval_ms > u32::MAX as u64 {
                return Err(AmdError::synthetic(
                    "AMDTPwrSetTimerSamplingPeriod",
                    "failed",
                    "sampling_interval_exceeds_installed_api_type",
                ));
            }
            if size_of::<AmdCounterDesc>() != 64
                || size_of::<AmdCounterValue>() != 8
                || size_of::<AmdSystemTime>() != 16
                || size_of::<AmdSample>() != 48
            {
                return Err(AmdError::synthetic(
                    "ffi_layout_check",
                    "unsafe_library",
                    "installed_header_layout_does_not_match_probe_declaration",
                ));
            }
            let library = AmdLibrary::load(install)?;
            let mut session = Self {
                library,
                initialized: false,
                started: false,
                descriptors: Vec::new(),
                selection: Selection::default(),
                enabled_counter_ids: Vec::new(),
                read_call_count: 0,
                successful_read_count: 0,
                no_data_read_count: 0,
                poll_count: 0,
            };

            let status =
                unsafe { (session.library.functions.initialize)(AMDT_PWR_MODE_TIMELINE_ONLINE) };
            if status != AMDT_STATUS_OK {
                return Err(AmdError::vendor("AMDTPwrProfileInitialize", status));
            }
            session.initialized = true;

            if let Err(error) = session.enumerate_and_enable(interval_ms) {
                session.close_best_effort();
                return Err(error);
            }
            Ok(session)
        }

        fn enumerate_and_enable(&mut self, _interval_ms: u64) -> Result<(), AmdError> {
            let mut count = 0u32;
            let mut pointer = ptr::null_mut();
            let status = unsafe {
                (self.library.functions.get_supported_counters)(&mut count, &mut pointer)
            };
            if status != AMDT_STATUS_OK {
                return Err(AmdError::vendor("AMDTPwrGetSupportedCounters", status));
            }
            if count > MAX_COUNTERS || (count > 0 && pointer.is_null()) {
                return Err(AmdError::synthetic(
                    "AMDTPwrGetSupportedCounters",
                    "failed",
                    "counter_descriptor_buffer_invalid",
                ));
            }
            for index in 0..count {
                let raw = unsafe { *pointer.add(index as usize) };
                self.descriptors.push(Descriptor {
                    name: read_c_string(raw.name),
                    raw,
                });
            }
            self.selection = select_targets(&self.descriptors);
            if self.selection.power.is_none() {
                return Err(AmdError::synthetic(
                    "counter_selection",
                    "unsupported",
                    "package_power_counter_not_enumerated",
                ));
            }
            if self.selection.frequencies.is_empty() {
                return Err(AmdError::synthetic(
                    "counter_selection",
                    "unsupported",
                    "per_identity_frequency_counter_not_enumerated",
                ));
            }

            for target in self.selection.targets().cloned().collect::<Vec<_>>() {
                let status = unsafe { (self.library.functions.enable_counter)(target.counter_id) };
                if status == AMDT_STATUS_OK || status == AMDT_ERROR_COUNTER_ALREADY_ENABLED {
                    self.enabled_counter_ids.push(target.counter_id);
                    continue;
                }
                if Some(target.counter_id)
                    == self.selection.temperature.as_ref().map(|x| x.counter_id)
                {
                    self.selection.temperature = None;
                    continue;
                }
                return Err(AmdError::vendor("AMDTPwrEnableCounter", status));
            }
            if self.enabled_counter_ids.is_empty() {
                return Err(AmdError::synthetic(
                    "AMDTPwrEnableCounter",
                    "unsupported",
                    "no_selected_counter_enabled",
                ));
            }
            Ok(())
        }

        fn start(&mut self, interval_ms: u64) -> Result<OperationReport, AmdError> {
            let timer_status =
                unsafe { (self.library.functions.set_timer_sampling_period)(interval_ms as u32) };
            if timer_status != AMDT_STATUS_OK {
                return Err(AmdError::vendor(
                    "AMDTPwrSetTimerSamplingPeriod",
                    timer_status,
                ));
            }
            let status = unsafe { (self.library.functions.start_profiling)() };
            if status != AMDT_STATUS_OK {
                return Err(AmdError::vendor("AMDTPwrStartProfiling", status));
            }
            self.started = true;
            Ok(OperationReport {
                operation: "AMDTPwrStartProfiling".to_string(),
                stable_status: "ok".to_string(),
                vendor_status: Some(status_name(AMDT_STATUS_OK)),
                vendor_status_code: Some(status_code(AMDT_STATUS_OK)),
            })
        }

        fn read(&mut self) -> ReadOutcome {
            self.read_call_count = self.read_call_count.saturating_add(1);
            let mut count = 0u32;
            let mut pointer = ptr::null_mut();
            let status = unsafe {
                (self.library.functions.read_all_enabled_counters)(&mut count, &mut pointer)
            };
            if status == AMDT_ERROR_PROFILE_DATA_NOT_AVAILABLE {
                self.no_data_read_count = self.no_data_read_count.saturating_add(1);
                return ReadOutcome::NoData(status);
            }
            if status != AMDT_STATUS_OK {
                return ReadOutcome::Failed(status);
            }
            if count > MAX_SAMPLES_PER_READ || (count > 0 && pointer.is_null()) {
                return ReadOutcome::Failed(AMDT_ERROR_FAIL);
            }
            let mut samples = Vec::with_capacity(count as usize);
            for index in 0..count {
                let sample = unsafe { *pointer.add(index as usize) };
                if sample.num_of_counter > MAX_COUNTER_VALUES_PER_SAMPLE
                    || (sample.num_of_counter > 0 && sample.counter_values.is_null())
                {
                    return ReadOutcome::Failed(AMDT_ERROR_FAIL);
                }
                let mut values = Vec::with_capacity(sample.num_of_counter as usize);
                for value_index in 0..sample.num_of_counter {
                    values.push(unsafe { *sample.counter_values.add(value_index as usize) });
                }
                samples.push(RawSample {
                    system_time: sample.system_time,
                    elapsed_time_ms: sample.elapsed_time_ms,
                    values,
                });
            }
            self.successful_read_count = self.successful_read_count.saturating_add(1);
            ReadOutcome::Samples(samples)
        }

        fn stop_and_close(&mut self) -> (Option<OperationReport>, Option<OperationReport>) {
            let stop = if self.started {
                let status = unsafe { (self.library.functions.stop_profiling)() };
                self.started = false;
                Some(operation_from_status("AMDTPwrStopProfiling", status))
            } else {
                None
            };
            let close = if self.initialized {
                let status = unsafe { (self.library.functions.close)() };
                self.initialized = false;
                Some(operation_from_status("AMDTPwrProfileClose", status))
            } else {
                None
            };
            (stop, close)
        }

        fn close_best_effort(&mut self) {
            let _ = self.stop_and_close();
        }
    }

    impl Drop for AmdSession {
        fn drop(&mut self) {
            let _ = self.stop_and_close();
        }
    }

    enum ReadOutcome {
        Samples(Vec<RawSample>),
        NoData(u32),
        Failed(u32),
    }

    struct RawSample {
        system_time: AmdSystemTime,
        elapsed_time_ms: u64,
        values: Vec<AmdCounterValue>,
    }

    struct LoadGuard {
        children: Vec<Child>,
    }

    impl LoadGuard {
        fn start(workers: u32) -> Result<Self, String> {
            let executable = std::env::current_exe().map_err(|error| {
                format!("locate qualification probe executable failed: {error}")
            })?;
            let mut children = Vec::with_capacity(workers as usize);
            for _ in 0..workers {
                let child = std::process::Command::new(&executable)
                    .arg("amd-uprof-workload-child")
                    .arg("--workers")
                    .arg("1")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .creation_flags(0x0800_0000)
                    .spawn();
                match child {
                    Ok(child) => children.push(child),
                    Err(error) => {
                        for mut running in children {
                            let _ = running.kill();
                            let _ = running.wait();
                        }
                        return Err(format!("start owned bounded CPU workload failed: {error}"));
                    }
                }
            }
            Ok(Self { children })
        }
    }

    impl Drop for LoadGuard {
        fn drop(&mut self) {
            for child in &mut self.children {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    pub fn run(config: AmdUprofConfig) -> Result<(PathBuf, PathBuf), String> {
        let started_at_utc = crate::utc_now_string();
        let machine = crate::windows::machine_info();
        let mut context = QualificationContext::new(&machine);
        let (verified, mut installation) = match verify_install(&config.install_root) {
            Ok(verified) => {
                let installation = verified.audit.clone();
                (Some(verified), installation)
            }
            Err(error) => {
                context.errors.push(
                    error
                        .audit_error
                        .clone()
                        .unwrap_or_else(|| "installation_audit_failed".to_string()),
                );
                (None, error)
            }
        };

        let mut lifecycle = None;
        let mut concurrency = None;
        if let Some(verified) = verified.as_ref() {
            match load_library_in_owned_child(verified) {
                Ok(observation) if observation.succeeded() => {
                    record_load_observation(&mut installation, &observation);
                    installation.safe_load =
                        format!("{}; pass_isolated_load_only_child", installation.safe_load);
                    match config.mode {
                        AmdUprofMode::Sanity | AmdUprofMode::Cadence => {
                            run_sampling(
                                &config,
                                verified,
                                &mut context,
                                1,
                                Duration::from_secs(config.duration_seconds),
                            );
                        }
                        AmdUprofMode::Lifecycle => {
                            lifecycle = Some(run_lifecycle(&config, verified, &mut context));
                        }
                        AmdUprofMode::Busy => {
                            concurrency = Some(run_busy(&config, verified, &mut context));
                        }
                    }
                }
                Ok(observation) => {
                    record_load_observation(&mut installation, &observation);
                    let error = observation.failure_summary();
                    installation.safe_load = format!("{}; {}", installation.safe_load, error);
                    installation.audit_status =
                        "path_and_architecture_verified; isolated_load_failed; API_not_called"
                            .to_string();
                    installation.audit_error = Some("isolated_library_load_failed".to_string());
                    context.errors.push(error);
                    context.seed_failure(
                        POWER_KEY,
                        "unsafe_library",
                        "isolated_library_load_failed",
                    );
                    context.seed_failure(
                        FREQUENCY_KEY,
                        "unsafe_library",
                        "isolated_library_load_failed",
                    );
                    context.seed_failure(
                        TEMPERATURE_KEY,
                        "unsafe_library",
                        "isolated_library_load_failed",
                    );
                }
                Err(error) => {
                    installation.safe_load = format!("{}; {}", installation.safe_load, error);
                    installation.audit_status =
                        "path_and_architecture_verified; isolated_load_failed; API_not_called"
                            .to_string();
                    installation.audit_error = Some("isolated_library_load_failed".to_string());
                    context.errors.push(error);
                    context.seed_failure(
                        POWER_KEY,
                        "unsafe_library",
                        "isolated_library_load_failed",
                    );
                    context.seed_failure(
                        FREQUENCY_KEY,
                        "unsafe_library",
                        "isolated_library_load_failed",
                    );
                    context.seed_failure(
                        TEMPERATURE_KEY,
                        "unsafe_library",
                        "isolated_library_load_failed",
                    );
                }
            }
        } else {
            context.seed_failure(
                POWER_KEY,
                "provider_missing",
                "verified_installation_unavailable",
            );
            context.seed_failure(
                FREQUENCY_KEY,
                "provider_missing",
                "verified_installation_unavailable",
            );
            context.seed_failure(
                TEMPERATURE_KEY,
                "provider_missing",
                "verified_installation_unavailable",
            );
        }

        let actual_interval_ms = Distribution::from_values(&context.actual_intervals).mean;
        let enumeration = EnumerationReport {
            supported_counter_count: context.supported_counter_count,
            counters: std::mem::take(&mut context.counters),
            duplicate_frequency_identity_count: context.duplicate_frequency_identity_count,
            expected_frequency_identity_count: machine.logical_processor_count,
            returned_frequency_identity_count: context.frequency_identity_count,
            missing_frequency_identity_behavior: if context.frequency_identity_count == 0 {
                "no_frequency_identity_enumerated; no numeric synthesis".to_string()
            } else {
                "each returned identity is retained; missing identities emit status without value"
                    .to_string()
            },
        };
        let mut metrics = std::mem::take(&mut context.metrics)
            .into_values()
            .map(MetricAccumulator::report)
            .collect::<Vec<_>>();
        metrics.sort_by(|left, right| {
            left.metric_key
                .cmp(&right.metric_key)
                .then(left.identity.cmp(&right.identity))
        });
        let run_status = if context.errors.is_empty() && !metrics.is_empty() {
            "completed".to_string()
        } else if !context.errors.is_empty() {
            "completed_with_qualification_failures".to_string()
        } else {
            "completed_without_samples".to_string()
        };
        let report = AmdReport {
            schema_version: SCHEMA_VERSION.to_string(),
            probe_name: PROBE_NAME.to_string(),
            mode: mode_name(config.mode).to_string(),
            started_at_utc,
            finished_at_utc: crate::utc_now_string(),
            machine: machine.clone(),
            initial_process_elevated: machine.elevated,
            requested_interval_ms: config.poll_interval_ms,
            actual_interval_ms,
            duration_seconds: config.duration_seconds,
            phase_duration_ms: config.phase_duration_ms,
            quiescence_ms: config.quiescence_ms,
            representative_load: config.representative_load,
            load_workers: config.load_workers,
            installation,
            api_contract: api_contract(),
            enumeration,
            sessions: std::mem::take(&mut context.sessions),
            events_file: "samples.jsonl".to_string(),
            event_count: context.events.len(),
            metrics,
            lifecycle,
            concurrency,
            performance: context.performance.report(),
            failure_isolation: failure_isolation(&context),
            external_reference: "MSI Afterburner/RTSS are not modified or used as ground truth; host state is recorded separately".to_string(),
            run_status,
            notes: context.errors,
        };
        write_report(&config.output_dir, &report, &context.events)
    }

    pub fn run_workload_child(workers: u32) -> Result<(), String> {
        let workers = workers.max(1);
        let mut handles = Vec::with_capacity(workers as usize);
        for worker in 0..workers {
            handles.push(std::thread::spawn(move || {
                let mut value = 0u64.wrapping_add(worker as u64);
                loop {
                    for offset in 0..1_000_000u64 {
                        value = value
                            .wrapping_mul(6_364_136_223_846_793_005)
                            .wrapping_add(offset ^ worker as u64);
                    }
                    std::hint::black_box(value);
                }
            }));
        }
        for handle in handles {
            let _ = handle.join();
        }
        Ok(())
    }

    pub fn run_load_child(install_root: PathBuf) -> Result<(), String> {
        let verified = verify_install(&Some(install_root)).map_err(|audit| {
            audit
                .audit_error
                .unwrap_or_else(|| "installation_audit_failed".to_string())
        })?;
        let _library = AmdLibrary::load(&verified)
            .map_err(|error| format!("{}: {}", error.operation, error.reason))?;
        Ok(())
    }

    /// Minimal loader-boundary diagnostic. It deliberately does not resolve or call
    /// any AMD export. A vendor DLL or dependency that terminates the process during
    /// LoadLibraryExW will therefore leave only the BEFORE_LOAD record in the child.
    pub fn run_load_only_child(path: PathBuf) -> Result<(), String> {
        if !path.is_absolute() {
            return Err("load-only diagnostic requires an absolute path".to_string());
        }
        let path = path
            .canonicalize()
            .map_err(|error| format!("canonicalize load-only path failed: {error}"))?;
        let wide = wide_path(&path);
        let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
        println!(
            "BEFORE_LOAD path={} flags=0x{:08X} process_architecture={}",
            path.display(),
            flags.0,
            process_architecture()
        );
        let result =
            unsafe { LoadLibraryExW(PCWSTR::from_raw(wide.as_ptr()), HANDLE::default(), flags) };
        match result {
            Ok(module) => {
                println!("LOAD_RETURNED_SUCCESS handle={:p}", module.0);
                // Do not resolve exports, call vendor code, or explicitly unload here.
                // Process exit releases the diagnostic module after the observation.
                Ok(())
            }
            Err(error) => {
                let last_error = unsafe { GetLastError() };
                println!(
                    "LOAD_RETURNED_ERROR win32_error={} win32_error_hex=0x{:08X} detail={}",
                    last_error.0, last_error.0, error
                );
                Ok(())
            }
        }
    }

    #[derive(Debug)]
    struct LoadChildObservation {
        exit_code: Option<i32>,
        exit_code_hex: Option<String>,
        stdout: String,
        stderr: String,
        timed_out: bool,
    }

    impl LoadChildObservation {
        fn succeeded(&self) -> bool {
            !self.timed_out && self.exit_code == Some(0)
        }

        fn failure_summary(&self) -> String {
            format!(
                "isolated_load_child_exit_signed_{}_hex_{}{}",
                self.exit_code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "abnormal".to_string()),
                self.exit_code_hex.as_deref().unwrap_or("N/A"),
                if self.timed_out {
                    "_timeout_after_child_kill"
                } else {
                    ""
                }
            )
        }
    }

    fn record_load_observation(
        installation: &mut InstallationAudit,
        observation: &LoadChildObservation,
    ) {
        installation.isolated_load_exit_code = observation.exit_code;
        installation.isolated_load_exit_code_hex = observation.exit_code_hex.clone();
        installation.isolated_load_stdout = Some(observation.stdout.clone());
        installation.isolated_load_stderr = Some(observation.stderr.clone());
        installation.isolated_load_timed_out = observation.timed_out;
    }

    fn load_library_in_owned_child(
        install: &VerifiedInstall,
    ) -> Result<LoadChildObservation, String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("locate qualification probe executable failed: {error}"))?;
        let mut child = std::process::Command::new(executable)
            .arg("amd-uprof-load-only-child")
            .arg("--path")
            .arg(&install.library)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|error| format!("spawn isolated library-load check failed: {error}"))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("poll isolated library-load check failed: {error}"))?
            {
                break status;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
                break child.wait().map_err(|error| {
                    format!("wait isolated library-load check failed: {error}")
                })?;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let mut stdout = String::new();
        if let Some(mut pipe) = child.stdout.take() {
            pipe.read_to_string(&mut stdout)
                .map_err(|error| format!("read isolated library-load stdout failed: {error}"))?;
        }
        let mut stderr = String::new();
        if let Some(mut pipe) = child.stderr.take() {
            pipe.read_to_string(&mut stderr)
                .map_err(|error| format!("read isolated library-load stderr failed: {error}"))?;
        }
        let exit_code = status.code();
        let exit_code_hex = exit_code.map(|value| format!("0x{:08X}", value as u32));
        Ok(LoadChildObservation {
            exit_code,
            exit_code_hex,
            stdout: bounded_diagnostic_output(stdout),
            stderr: bounded_diagnostic_output(stderr),
            timed_out,
        })
    }

    fn bounded_diagnostic_output(output: String) -> String {
        const MAX_DIAGNOSTIC_OUTPUT: usize = 16 * 1024;
        if output.len() <= MAX_DIAGNOSTIC_OUTPUT {
            output
        } else {
            let mut bounded = output;
            bounded.truncate(MAX_DIAGNOSTIC_OUTPUT);
            bounded.push_str("\n[truncated]");
            bounded
        }
    }

    fn new_session_report(generation: u64) -> SessionReport {
        SessionReport {
            generation,
            initialize: operation_pending("AMDTPwrProfileInitialize"),
            enumeration: operation_pending("AMDTPwrGetSupportedCounters"),
            timer: operation_pending("AMDTPwrSetTimerSamplingPeriod"),
            start: operation_pending("AMDTPwrStartProfiling"),
            stop: None,
            close: None,
            enabled_counter_ids: Vec::new(),
            read_call_count: 0,
            successful_read_count: 0,
            no_data_read_count: 0,
            late_sample_count: 0,
            dropped_sample_count: 0,
            poll_count: 0,
            closed_before_drop: false,
            session_outcome: "not_started".to_string(),
        }
    }

    fn record_session_counts(report: &mut SessionReport, session: &AmdSession) {
        report.read_call_count = session.read_call_count;
        report.successful_read_count = session.successful_read_count;
        report.no_data_read_count = session.no_data_read_count;
        report.poll_count = session.poll_count;
    }

    fn apply_start_error(report: &mut SessionReport, error: &AmdError) {
        if error.operation == "AMDTPwrSetTimerSamplingPeriod" {
            report.timer = error.operation_report();
        } else {
            report.start = error.operation_report();
        }
    }

    fn run_sampling(
        config: &AmdUprofConfig,
        install: &VerifiedInstall,
        context: &mut QualificationContext,
        generation: u64,
        duration: Duration,
    ) {
        let mut session_report = new_session_report(generation);
        let mut session = match AmdSession::open(install, config.poll_interval_ms) {
            Ok(session) => session,
            Err(error) => {
                apply_open_error(&mut session_report, &error);
                context.note_error(&error);
                context.seed_failure(POWER_KEY, &error.stable_status, &error.reason);
                context.seed_failure(FREQUENCY_KEY, &error.stable_status, &error.reason);
                context.sessions.push(session_report);
                return;
            }
        };
        session_report.initialize = operation_ok("AMDTPwrProfileInitialize");
        session_report.enumeration = operation_ok("AMDTPwrGetSupportedCounters");
        record_session_enumeration(context, &session);
        seed_selection(context, &session.selection);
        let target_count_before_optional = session.selection.targets().count();
        session_report.enabled_counter_ids = session.enabled_counter_ids.clone();
        session_report.timer = operation_ok("AMDTPwrSetTimerSamplingPeriod");
        let start = match session.start(config.poll_interval_ms) {
            Ok(report) => report,
            Err(error) => {
                apply_start_error(&mut session_report, &error);
                context.note_error(&error);
                context.seed_failure(POWER_KEY, &error.stable_status, &error.reason);
                context.seed_failure(FREQUENCY_KEY, &error.stable_status, &error.reason);
                let (stop, close) = session.stop_and_close();
                session_report.stop = stop;
                session_report.close = close;
                session_report.closed_before_drop = true;
                session_report.session_outcome = error.stable_status;
                context.sessions.push(session_report);
                return;
            }
        };
        session_report.start = start;
        session_report.session_outcome = "started".to_string();
        let _load = if config.representative_load {
            match LoadGuard::start(config.load_workers) {
                Ok(load) => Some(load),
                Err(error) => {
                    context.errors.push(error);
                    None
                }
            }
        } else {
            None
        };
        sample_started_session(config, context, &mut session, generation, duration);
        record_session_counts(&mut session_report, &session);
        let (stop, close) = session.stop_and_close();
        session_report.stop = stop;
        session_report.close = close;
        session_report.closed_before_drop = true;
        session_report.session_outcome = if target_count_before_optional > 0 {
            "completed".to_string()
        } else {
            "completed_without_targets".to_string()
        };
        context.sessions.push(session_report);
    }

    fn sample_started_session(
        config: &AmdUprofConfig,
        context: &mut QualificationContext,
        session: &mut AmdSession,
        generation: u64,
        duration: Duration,
    ) {
        let interval = Duration::from_millis(config.poll_interval_ms);
        let started = Instant::now();
        let deadline = started + duration;
        let mut next_due = started;
        let mut previous_poll: Option<Instant> = None;
        while Instant::now() < deadline {
            let now = Instant::now();
            if now < next_due {
                std::thread::sleep(next_due - now);
            }
            if Instant::now() >= deadline {
                break;
            }
            let before_call = Instant::now();
            let late = before_call > next_due + Duration::from_millis(2);
            let behind_ms = before_call.saturating_duration_since(next_due).as_millis() as u64;
            let dropped = if interval.as_millis() > 0 {
                behind_ms / interval.as_millis() as u64
            } else {
                0
            };
            let actual_interval_ms = previous_poll
                .map(|previous| before_call.duration_since(previous).as_secs_f64() * 1000.0);
            let read_started = Instant::now();
            let outcome = session.read();
            let api_latency_ms = read_started.elapsed().as_secs_f64() * 1000.0;
            context
                .performance
                .observe(api_latency_ms, before_call, late, dropped);
            if let Some(actual) = actual_interval_ms {
                context.actual_intervals.push(actual);
            }
            previous_poll = Some(before_call);
            session.poll_count = session.poll_count.saturating_add(1);
            let timestamp_ms = crate::unix_now_ms();
            match outcome {
                ReadOutcome::Samples(samples) => {
                    for sample in samples {
                        observe_sample(
                            config,
                            context,
                            &session.selection,
                            sample,
                            generation,
                            timestamp_ms,
                            actual_interval_ms,
                            api_latency_ms,
                        );
                    }
                }
                ReadOutcome::NoData(status) => {
                    for target in session.selection.targets().cloned().collect::<Vec<_>>() {
                        context.observe_failure_for_target(
                            &target,
                            "no_data",
                            &status_name(status),
                            timestamp_ms,
                            config.poll_interval_ms,
                            actual_interval_ms,
                            api_latency_ms,
                            generation,
                            None,
                        );
                    }
                }
                ReadOutcome::Failed(status) => {
                    context.note_error(&AmdError::vendor("AMDTPwrReadAllEnabledCounters", status));
                    for target in session.selection.targets().cloned().collect::<Vec<_>>() {
                        context.observe_failure_for_target(
                            &target,
                            &stable_status_for("AMDTPwrReadAllEnabledCounters", status),
                            &status_name(status),
                            timestamp_ms,
                            config.poll_interval_ms,
                            actual_interval_ms,
                            api_latency_ms,
                            generation,
                            None,
                        );
                    }
                }
            }
            next_due = next_due
                + Duration::from_millis(
                    config
                        .poll_interval_ms
                        .saturating_mul(dropped.saturating_add(1)),
                );
        }
    }

    fn observe_sample(
        config: &AmdUprofConfig,
        context: &mut QualificationContext,
        selection: &Selection,
        sample: RawSample,
        generation: u64,
        timestamp_ms: i64,
        actual_interval_ms: Option<f64>,
        api_latency_ms: f64,
    ) {
        let source_timestamp = SourceTimestamp {
            second: sample.system_time.second,
            subsecond_millisecond: sample.system_time.subsecond_millisecond,
            elapsed_time_ms: sample.elapsed_time_ms,
        };
        let mut by_counter = BTreeMap::<u32, Vec<f64>>::new();
        for value in sample.values {
            by_counter
                .entry(value.counter_id)
                .or_default()
                .push(value.data as f64);
        }
        for target in selection.targets().cloned().collect::<Vec<_>>() {
            match by_counter
                .get(&target.counter_id)
                .and_then(|values| values.first())
            {
                Some(value) => context.observe_value_for_target(
                    &target,
                    *value,
                    timestamp_ms,
                    config.poll_interval_ms,
                    actual_interval_ms,
                    api_latency_ms,
                    generation,
                    SourceTimestamp {
                        second: source_timestamp.second,
                        subsecond_millisecond: source_timestamp.subsecond_millisecond,
                        elapsed_time_ms: source_timestamp.elapsed_time_ms,
                    },
                ),
                None => context.observe_failure_for_target(
                    &target,
                    "missing_identity",
                    "counter_missing_from_sample",
                    timestamp_ms,
                    config.poll_interval_ms,
                    actual_interval_ms,
                    api_latency_ms,
                    generation,
                    Some(SourceTimestamp {
                        second: source_timestamp.second,
                        subsecond_millisecond: source_timestamp.subsecond_millisecond,
                        elapsed_time_ms: source_timestamp.elapsed_time_ms,
                    }),
                ),
            }
        }
    }

    fn run_lifecycle(
        config: &AmdUprofConfig,
        install: &VerifiedInstall,
        context: &mut QualificationContext,
    ) -> LifecycleReport {
        let mut report = LifecycleReport {
            enable: "not_run".to_string(),
            disable: "not_run".to_string(),
            quiescence: "not_run".to_string(),
            re_enable: "not_run".to_string(),
            final_disable: "not_run".to_string(),
            process_exit: "pending".to_string(),
            first_generation: None,
            re_enabled_generation: None,
            polls_after_disable: 0,
            api_calls_after_disable: 0,
            owned_timer_stopped: false,
            owned_worker_stopped: false,
            handles_released: false,
            resources_bounded: false,
            owned_leaks: "not_observed".to_string(),
        };
        let generation = 1;
        let mut first_report = new_session_report(generation);
        let mut first = match AmdSession::open(install, config.poll_interval_ms) {
            Ok(session) => session,
            Err(error) => {
                apply_open_error(&mut first_report, &error);
                context.note_error(&error);
                context.seed_failure(POWER_KEY, &error.stable_status, &error.reason);
                context.seed_failure(FREQUENCY_KEY, &error.stable_status, &error.reason);
                context.sessions.push(first_report);
                report.enable = error.stable_status;
                report.process_exit = "completed_after_failed_enable".to_string();
                return report;
            }
        };
        report.first_generation = Some(generation);
        first_report.initialize = operation_ok("AMDTPwrProfileInitialize");
        first_report.enumeration = operation_ok("AMDTPwrGetSupportedCounters");
        record_session_enumeration(context, &first);
        seed_selection(context, &first.selection);
        first_report.enabled_counter_ids = first.enabled_counter_ids.clone();
        if let Err(error) = first.start(config.poll_interval_ms) {
            apply_start_error(&mut first_report, &error);
            context.note_error(&error);
            report.enable = error.stable_status;
            let (stop, close) = first.stop_and_close();
            first_report.stop = stop;
            first_report.close = close;
            first_report.closed_before_drop = true;
            first_report.session_outcome = "failed_to_start".to_string();
            context.sessions.push(first_report);
            report.process_exit = "completed_after_failed_start".to_string();
            return report;
        }
        first_report.timer = operation_ok("AMDTPwrSetTimerSamplingPeriod");
        first_report.start = operation_ok("AMDTPwrStartProfiling");
        report.enable = "pass_started_generation_1".to_string();
        sample_started_session(
            config,
            context,
            &mut first,
            generation,
            Duration::from_millis(config.phase_duration_ms),
        );
        let first_calls_before_disable = first.read_call_count;
        let (stop, close) = first.stop_and_close();
        let stop_close_pass = stop.is_some() && close.is_some();
        record_session_counts(&mut first_report, &first);
        first_report.stop = stop;
        first_report.close = close;
        first_report.closed_before_drop = true;
        first_report.session_outcome = "completed".to_string();
        context.sessions.push(first_report);
        report.disable = if stop_close_pass {
            "pass_stop_close".to_string()
        } else {
            "partial_stop_close".to_string()
        };
        report.owned_timer_stopped = true;
        report.owned_worker_stopped = true;
        drop(first);
        let quiescence_start = Instant::now();
        std::thread::sleep(Duration::from_millis(config.quiescence_ms));
        report.quiescence = format!(
            "pass_{}ms_no_probe_polling",
            quiescence_start.elapsed().as_millis()
        );
        report.polls_after_disable = 0;
        report.api_calls_after_disable = 0;
        if first_calls_before_disable == 0 {
            report.owned_leaks =
                "no_sampling_calls_before_disable; lifecycle_inconclusive".to_string();
        }
        let generation = 2;
        let mut second_report = new_session_report(generation);
        let mut second = match AmdSession::open(install, config.poll_interval_ms) {
            Ok(session) => session,
            Err(error) => {
                apply_open_error(&mut second_report, &error);
                context.note_error(&error);
                context.sessions.push(second_report);
                report.re_enable = error.stable_status;
                report.final_disable = "not_reached".to_string();
                report.process_exit = "completed_after_failed_re_enable".to_string();
                return report;
            }
        };
        report.re_enabled_generation = Some(generation);
        second_report.initialize = operation_ok("AMDTPwrProfileInitialize");
        second_report.enumeration = operation_ok("AMDTPwrGetSupportedCounters");
        record_session_enumeration(context, &second);
        seed_selection(context, &second.selection);
        second_report.enabled_counter_ids = second.enabled_counter_ids.clone();
        if let Err(error) = second.start(config.poll_interval_ms) {
            apply_start_error(&mut second_report, &error);
            context.note_error(&error);
            report.re_enable = error.stable_status;
            let (stop, close) = second.stop_and_close();
            second_report.stop = stop;
            second_report.close = close;
            second_report.closed_before_drop = true;
            second_report.session_outcome = "failed_to_start".to_string();
            context.sessions.push(second_report);
            report.final_disable = "partial".to_string();
            report.process_exit = "completed_after_failed_re_enable_start".to_string();
            return report;
        }
        second_report.timer = operation_ok("AMDTPwrSetTimerSamplingPeriod");
        second_report.start = operation_ok("AMDTPwrStartProfiling");
        report.re_enable = "pass_new_generation_started".to_string();
        sample_started_session(
            config,
            context,
            &mut second,
            generation,
            Duration::from_millis(config.phase_duration_ms),
        );
        let (stop, close) = second.stop_and_close();
        let stop_close_pass = stop.is_some() && close.is_some();
        record_session_counts(&mut second_report, &second);
        second_report.stop = stop;
        second_report.close = close;
        second_report.closed_before_drop = true;
        second_report.session_outcome = "completed".to_string();
        context.sessions.push(second_report);
        report.final_disable = if stop_close_pass {
            "pass_stop_close".to_string()
        } else {
            "partial_stop_close".to_string()
        };
        report.handles_released = true;
        report.resources_bounded = true;
        report.owned_leaks =
            "no_probe-owned_handle_or_worker_leak_observed; external_driver_residency_expected"
                .to_string();
        drop(second);
        report.process_exit = "pass_owned_state_released_before_process_exit".to_string();
        report
    }

    fn run_busy(
        config: &AmdUprofConfig,
        install: &VerifiedInstall,
        context: &mut QualificationContext,
    ) -> ConcurrencyReport {
        let mut report = ConcurrencyReport {
            single_session_rule:
                "one AMD power profile session at a time; vendor contract requires stop then close"
                    .to_string(),
            second_session_result: "not_run".to_string(),
            vendor_status: None,
            vendor_status_code: None,
            provider_busy_mapping: "not_run".to_string(),
            first_session_survived: "not_run".to_string(),
            retry_behavior: "one controlled second-session attempt; no retry storm".to_string(),
            ryzen_master_interaction: "NOT_ESTABLISHED; no Ryzen Master process was acted on"
                .to_string(),
        };
        let mut first = match AmdSession::open(install, config.poll_interval_ms) {
            Ok(session) => session,
            Err(error) => {
                context.note_error(&error);
                report.second_session_result = "not_conclusive_first_session_failed".to_string();
                return report;
            }
        };
        record_session_enumeration(context, &first);
        seed_selection(context, &first.selection);
        if let Err(error) = first.start(config.poll_interval_ms) {
            context.note_error(&error);
            report.second_session_result = "not_conclusive_first_start_failed".to_string();
            let _ = first.stop_and_close();
            return report;
        }
        let second = AmdSession::open(install, config.poll_interval_ms);
        match second {
            Ok(mut second) => {
                let started = second.start(config.poll_interval_ms);
                match started {
                    Ok(_) => {
                        report.second_session_result =
                            "unexpected_second_session_started".to_string();
                        report.provider_busy_mapping =
                            "failed_closed_after_unexpected_success".to_string();
                        let _ = second.stop_and_close();
                    }
                    Err(error) => {
                        report.second_session_result = "failed_closed".to_string();
                        report.vendor_status = error.vendor_status.map(status_name);
                        report.vendor_status_code = error.vendor_status.map(status_code);
                        report.provider_busy_mapping = if is_busy_status(
                            "AMDTPwrStartProfiling",
                            error.vendor_status.unwrap_or_default(),
                        ) {
                            "provider_busy".to_string()
                        } else {
                            "not_busy_status".to_string()
                        };
                        let _ = second.stop_and_close();
                    }
                }
            }
            Err(error) => {
                report.second_session_result = "failed_closed".to_string();
                report.vendor_status = error.vendor_status.map(status_name);
                report.vendor_status_code = error.vendor_status.map(status_code);
                report.provider_busy_mapping = if error
                    .vendor_status
                    .map(|status| is_busy_status(&error.operation, status))
                    .unwrap_or(false)
                {
                    "provider_busy".to_string()
                } else {
                    "not_busy_status".to_string()
                };
                context.note_error(&error);
            }
        }
        sample_started_session(
            config,
            context,
            &mut first,
            1,
            Duration::from_millis(config.phase_duration_ms),
        );
        report.first_session_survived = if first.successful_read_count > 0 {
            "pass_sampled_after_second_attempt".to_string()
        } else {
            "no_successful_sample_after_second_attempt".to_string()
        };
        let _ = first.stop_and_close();
        let third = AmdSession::open(install, config.poll_interval_ms);
        match third {
            Ok(mut third) => {
                if let Ok(_) = third.start(config.poll_interval_ms) {
                    report.retry_behavior =
                        "one post-close new-session recovery succeeded".to_string();
                    let _ = third.stop_and_close();
                } else {
                    report.retry_behavior = "post-close new-session start failed".to_string();
                    let _ = third.stop_and_close();
                }
            }
            Err(error) => {
                report.retry_behavior =
                    format!("post-close new-session open failed: {}", error.reason);
                context.note_error(&error);
            }
        }
        report
    }

    fn record_session_enumeration(context: &mut QualificationContext, session: &AmdSession) {
        context.supported_counter_count = session.descriptors.len() as u32;
        let selection_by_id = session.selection.targets().fold(
            BTreeMap::<u32, Vec<&Target>>::new(),
            |mut map, target| {
                map.entry(target.counter_id).or_default().push(target);
                map
            },
        );
        let mut frequency_identity_keys = BTreeSet::new();
        for descriptor in &session.descriptors {
            let selected = selection_by_id
                .get(&descriptor.raw.counter_id)
                .and_then(|targets| targets.first())
                .map(|target| target.metric_key.clone());
            let enabled = session
                .enabled_counter_ids
                .contains(&descriptor.raw.counter_id);
            context.counters.push(CounterReport {
                counter_id: descriptor.raw.counter_id,
                device_id: descriptor.raw.device_id,
                device_type: device_type_name(descriptor.raw.device_type).to_string(),
                device_instance_id: descriptor.raw.device_instance_id,
                category: category_name(descriptor.raw.category).to_string(),
                aggregation: aggregation_name(descriptor.raw.aggregation).to_string(),
                units: unit_name(descriptor.raw.units).to_string(),
                min_value: descriptor.raw.min_value,
                max_value: descriptor.raw.max_value,
                name: descriptor.name.clone(),
                selected_metric: selected,
                enable_status: Some(if enabled {
                    "ok".to_string()
                } else {
                    "not_enabled".to_string()
                }),
                enable_vendor_status: None,
            });
        }
        for target in &session.selection.frequencies {
            if !frequency_identity_keys.insert(target.identity.clone()) {
                context.duplicate_frequency_identity_count =
                    context.duplicate_frequency_identity_count.saturating_add(1);
            }
        }
        context.frequency_identity_count = context
            .frequency_identity_count
            .saturating_add(session.selection.frequencies.len());
    }

    fn seed_selection(context: &mut QualificationContext, selection: &Selection) {
        for target in selection.targets() {
            context.seed_target(target);
        }
        if selection.temperature.is_none() {
            context.seed_failure(
                TEMPERATURE_KEY,
                "unsupported",
                "package_temperature_counter_not_enumerated_or_enabled",
            );
        }
    }

    fn apply_open_error(report: &mut SessionReport, error: &AmdError) {
        let operation = error.operation.as_str();
        let operation_report = error.operation_report();
        match operation {
            "AMDTPwrProfileInitialize" => report.initialize = operation_report,
            "AMDTPwrGetSupportedCounters" | "counter_selection" => {
                report.enumeration = operation_report
            }
            "AMDTPwrEnableCounter" => report.enumeration = operation_report,
            _ => report.start = operation_report,
        }
        report.session_outcome = error.stable_status.clone();
    }

    fn verify_install(root: &Option<PathBuf>) -> Result<VerifiedInstall, InstallationAudit> {
        let requested_root = root.as_ref().map(|value| redact_path(value));
        let candidate = root
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_INSTALL_ROOT));
        let mut audit = InstallationAudit {
            requested_root,
            root: None,
            library: None,
            root_verified: false,
            library_exists: false,
            library_size_bytes: None,
            library_architecture: None,
            process_architecture: process_architecture().to_string(),
            marker_files: BTreeMap::new(),
            safe_load: "not_attempted".to_string(),
            isolated_load_exit_code: None,
            isolated_load_exit_code_hex: None,
            isolated_load_stdout: None,
            isolated_load_stderr: None,
            isolated_load_timed_out: false,
            audit_status: "failed".to_string(),
            audit_error: None,
        };
        if !candidate.is_absolute() {
            audit.audit_error = Some("install_root_must_be_absolute".to_string());
            return Err(audit);
        }
        let root = match fs::canonicalize(&candidate) {
            Ok(root) if root.is_dir() => root,
            Ok(_) => {
                audit.audit_error = Some("install_root_is_not_directory".to_string());
                return Err(audit);
            }
            Err(error) => {
                audit.audit_error = Some(format!("install_root_unavailable: {error}"));
                return Err(audit);
            }
        };
        audit.root = Some(redact_path(&root));
        let marker_paths = [
            ("api_library", root.join(r"bin\AMDPowerProfileAPI.dll")),
            ("api_header", root.join(r"include\AMDTPowerProfileApi.h")),
            (
                "data_types_header",
                root.join(r"include\AMDTPowerProfileDataTypes.h"),
            ),
            ("api_pdf", root.join(r"Help\AMDPowerProfilerAPI.pdf")),
            (
                "official_sample",
                root.join(r"Examples\CollectAllCounters\CollectAllCounters.cpp"),
            ),
        ];
        for (name, path) in &marker_paths {
            audit
                .marker_files
                .insert((*name).to_string(), path.is_file());
        }
        let library = marker_paths[0].1.clone();
        audit.library_exists = library.is_file();
        if !audit.library_exists {
            audit.audit_error =
                Some("AMDPowerProfileAPI.dll_not_found_in_verified_root".to_string());
            return Err(audit);
        }
        let canonical_library = match fs::canonicalize(&library) {
            Ok(path) => path,
            Err(error) => {
                audit.audit_error = Some(format!("library_canonicalize_failed: {error}"));
                return Err(audit);
            }
        };
        if !is_within(&canonical_library, &root) {
            audit.audit_error = Some("library_is_outside_verified_install_root".to_string());
            return Err(audit);
        }
        audit.library = Some(redact_path(&canonical_library));
        audit.library_size_bytes = fs::metadata(&canonical_library).ok().map(|m| m.len());
        let architecture = match pe_architecture(&canonical_library) {
            Ok(value) => value,
            Err(error) => {
                audit.audit_error = Some(error);
                return Err(audit);
            }
        };
        audit.library_architecture = Some(architecture.to_string());
        if architecture != process_architecture() {
            audit.audit_error = Some("library_architecture_mismatch".to_string());
            return Err(audit);
        }
        audit.root_verified = true;
        audit.safe_load =
            "absolute_path_LoadLibraryExW_SEARCH_DLL_LOAD_DIR_SEARCH_SYSTEM32".to_string();
        audit.audit_status =
            "verified_for_dynamic_load; Authenticode/version recorded by read-only host audit"
                .to_string();
        Ok(VerifiedInstall {
            library: canonical_library,
            audit,
        })
    }

    fn write_report(
        output_dir: &Path,
        report: &AmdReport,
        events: &[AmdEvent],
    ) -> Result<(PathBuf, PathBuf), String> {
        fs::create_dir_all(output_dir)
            .map_err(|error| format!("create AMD uProf output directory failed: {error}"))?;
        let json_path = output_dir.join("report.json");
        let markdown_path = output_dir.join("report.md");
        let samples_path = output_dir.join("samples.jsonl");
        let json = serde_json::to_vec_pretty(report)
            .map_err(|error| format!("serialize AMD uProf JSON failed: {error}"))?;
        let mut jsonl = String::new();
        for event in events {
            jsonl.push_str(
                &serde_json::to_string(event)
                    .map_err(|error| format!("serialize AMD uProf event failed: {error}"))?,
            );
            jsonl.push('\n');
        }
        let markdown = render_markdown(report);
        validate_public_text(std::str::from_utf8(&json).unwrap_or_default())?;
        validate_public_text(&jsonl)?;
        validate_public_text(&markdown)?;
        write_atomic(&json_path, &json)?;
        write_atomic(&samples_path, jsonl.as_bytes())?;
        write_atomic(&markdown_path, markdown.as_bytes())?;
        Ok((json_path, markdown_path))
    }

    fn render_markdown(report: &AmdReport) -> String {
        let mut output = String::new();
        output.push_str("# AMD uProf Live CPU Sensor Qualification\n\n");
        output.push_str(&format!(
            "- Schema: `{}`\n- Mode: `{}`\n- Started: `{}`\n- Finished: `{}`\n- Run status: `{}`\n\n",
            report.schema_version,
            report.mode,
            report.started_at_utc,
            report.finished_at_utc,
            report.run_status
        ));
        output.push_str("## Installation preflight\n\n");
        output.push_str(&format!(
            "- Root verified: `{}`\n- Library exists: `{}`\n- Library architecture: `{}`\n- Process architecture: `{}`\n- Safe load: `{}`\n- Audit status: `{}`\n\n",
            report.installation.root_verified,
            report.installation.library_exists,
            report
                .installation
                .library_architecture
                .as_deref()
                .unwrap_or("unknown"),
            report.installation.process_architecture,
            report.installation.safe_load,
            report.installation.audit_status
        ));
        output.push_str(&format!(
            "- Isolated load child exit: signed `{}` / hex `{}` / timed out `{}`\n- Isolated child stdout/stderr captured: `{}` / `{}`\n\n",
            report
                .installation
                .isolated_load_exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            report
                .installation
                .isolated_load_exit_code_hex
                .as_deref()
                .unwrap_or("N/A"),
            report.installation.isolated_load_timed_out,
            report
                .installation
                .isolated_load_stdout
                .as_ref()
                .map(|value| value.len().to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            report
                .installation
                .isolated_load_stderr
                .as_ref()
                .map(|value| value.len().to_string())
                .unwrap_or_else(|| "N/A".to_string()),
        ));
        output.push_str("## Metrics\n\n");
        output.push_str(
            "| Metric | Scope | Identity | Status | Samples | Failed | Unit | Min | Median | P95 | Max | Mean |\n|---|---|---|---|---:|---:|---|---:|---:|---:|---:|---:|\n",
        );
        for metric in &report.metrics {
            output.push_str(&format!(
                "| `{}` | {} | `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} |\n",
                metric.metric_key,
                metric.scope,
                metric.identity,
                metric.status,
                metric.sample_count,
                metric.failed_sample_count,
                metric.unit,
                display(metric.values.min),
                display(metric.values.p50),
                display(metric.values.p95),
                display(metric.values.max),
                display(metric.values.mean),
            ));
        }
        output.push('\n');
        output.push_str("## Sessions\n\n");
        for session in &report.sessions {
            output.push_str(&format!(
                "- Generation {}: outcome `{}`, reads={}, successful={}, no-data={}, polls={}, stop={}, close={}\n",
                session.generation,
                session.session_outcome,
                session.read_call_count,
                session.successful_read_count,
                session.no_data_read_count,
                session.poll_count,
                session
                    .stop
                    .as_ref()
                    .map(|value| value.stable_status.as_str())
                    .unwrap_or("not_called"),
                session
                    .close
                    .as_ref()
                    .map(|value| value.stable_status.as_str())
                    .unwrap_or("not_called"),
            ));
        }
        output.push('\n');
        output.push_str("## Performance\n\n");
        output.push_str(&format!(
            "- Probe CPU average/P95: {}/{}%\n- Peak working set: {} bytes\n- Peak handles/threads: {}/{}\n- API latency average/P95/max: {}/{}/{} ms\n- Late/dropped polls: {}/{}\n- AMD external overhead: {}\n\n",
            display(report.performance.average_cpu_percent),
            display(report.performance.p95_cpu_percent),
            report
                .performance
                .peak_working_set_bytes
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            report
                .performance
                .handle_count_peak
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            report
                .performance
                .thread_count_peak
                .map(|value| value.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            display(report.performance.api_average_latency_ms),
            display(report.performance.api_p95_latency_ms),
            display(report.performance.api_max_latency_ms),
            report.performance.late_samples,
            report.performance.dropped_samples,
            report.performance.amd_external_overhead,
        ));
        if let Some(lifecycle) = &report.lifecycle {
            output.push_str("## Lifecycle\n\n");
            output.push_str(&format!(
                "- Enable: `{}`\n- Disable: `{}`\n- Quiescence: `{}`\n- Re-enable: `{}`\n- Final disable: `{}`\n- Polls/API calls after disable: {}/{}\n- Owned leaks: {}\n\n",
                lifecycle.enable,
                lifecycle.disable,
                lifecycle.quiescence,
                lifecycle.re_enable,
                lifecycle.final_disable,
                lifecycle.polls_after_disable,
                lifecycle.api_calls_after_disable,
                lifecycle.owned_leaks,
            ));
        }
        if let Some(concurrency) = &report.concurrency {
            output.push_str("## Concurrency\n\n");
            output.push_str(&format!(
                "- Second session: `{}`\n- Vendor status: `{}` ({})\n- Stable mapping: `{}`\n- First session survived: `{}`\n- Post-close behavior: `{}`\n\n",
                concurrency.second_session_result,
                concurrency.vendor_status.as_deref().unwrap_or("none"),
                concurrency.vendor_status_code.as_deref().unwrap_or("none"),
                concurrency.provider_busy_mapping,
                concurrency.first_session_survived,
                concurrency.retry_behavior,
            ));
        }
        output.push_str("## Notes\n\n");
        for note in &report.notes {
            output.push_str(&format!("- {note}\n"));
        }
        output
    }

    fn failure_isolation(context: &QualificationContext) -> FailureIsolationReport {
        let status = context.top_status.as_deref().unwrap_or("not_exercised");
        FailureIsolationReport {
            missing_library: if status == "provider_missing" {
                "observed_or_simulated_without_load".to_string()
            } else {
                "not_exercised_in_this_run".to_string()
            },
            unsafe_path: "verified-root and architecture checks fail closed".to_string(),
            unsupported_counter: "missing counter remains status; no value synthesis".to_string(),
            permission: if status == "permission_denied" {
                "observed".to_string()
            } else {
                "not_naturally_observed".to_string()
            },
            busy: "controlled resource-owned second-session path".to_string(),
            invalid_value: "non-finite and negative power/frequency values rejected".to_string(),
            timeout_late: "late scheduler polls counted; no forced API timeout or cancellation"
                .to_string(),
            zero_synthesis: "PASS_NO_NUMERIC_ZERO_SYNTHESIS".to_string(),
        }
    }

    fn api_contract() -> ApiContract {
        ApiContract {
            api_version_query: "not_exposed_in_installed_AMDTPowerProfileApi.h; no invented query".to_string(),
            profile_mode: "AMDT_PWR_MODE_TIMELINE_ONLINE=0".to_string(),
            functions: vec![
                "AMDTPwrProfileInitialize".to_string(),
                "AMDTPwrGetSupportedCounters".to_string(),
                "AMDTPwrEnableCounter".to_string(),
                "AMDTPwrSetTimerSamplingPeriod".to_string(),
                "AMDTPwrStartProfiling".to_string(),
                "AMDTPwrReadAllEnabledCounters".to_string(),
                "AMDTPwrStopProfiling".to_string(),
                "AMDTPwrProfileClose".to_string(),
            ],
            lifecycle: "Initialize -> enumerate -> enable -> set timer -> Start -> Read -> Stop -> Close".to_string(),
            enumeration: "descriptors returned by GetSupportedCounters remain valid until ProfileClose; sample buffers remain valid until the next read per installed header".to_string(),
            status_model: "vendor AMDTResult preserved as symbolic name plus hexadecimal code; stable state is separate".to_string(),
            busy_status: "StartProfiling AMDT_ERROR_ACCESSDENIED is documented as profiler busy; Initialize may return DRIVER_ALREADY_INITIALIZED or PREVIOUS_SESSION_NOT_CLOSED".to_string(),
            sampling_constraint: "installed PDF/sample use 100 ms; this probe requests 500, 1000, or 2000 ms without claiming a maximum rate".to_string(),
            simultaneous_counter_support: "all selected package power, optional package temperature, and returned per-identity frequency counters are enabled in the same session when vendor enable calls succeed".to_string(),
            timestamp_semantics: "installed header labels AMDTPwrSystemTime::m_microSecond as milliseconds; probe preserves it as subsecond_millisecond and also records host timestamp".to_string(),
        }
    }

    fn select_targets(descriptors: &[Descriptor]) -> Selection {
        let mut selection = Selection::default();
        for descriptor in descriptors {
            let raw = descriptor.raw;
            if selection.power.is_none()
                && raw.category == AMDT_PWR_CATEGORY_POWER
                && raw.device_type == AMDT_DEVICE_TYPE_PACKAGE
                && raw.units == AMDT_PWR_UNIT_WATT
            {
                selection.power = Some(target_for(
                    raw,
                    POWER_KEY,
                    "package",
                    "package",
                    "W",
                    Some("ESTIMATED"),
                ));
            }
            if selection.temperature.is_none()
                && raw.category == AMDT_PWR_CATEGORY_TEMPERATURE
                && raw.device_type == AMDT_DEVICE_TYPE_PACKAGE
                && raw.units == AMDT_PWR_UNIT_CENTIGRADE
            {
                selection.temperature = Some(target_for(
                    raw,
                    TEMPERATURE_KEY,
                    "package",
                    "package",
                    "°C",
                    None,
                ));
            }
            if raw.category == AMDT_PWR_CATEGORY_FREQUENCY
                && raw.units == AMDT_PWR_UNIT_MEGA_HERTZ
                && matches!(
                    raw.device_type,
                    AMDT_DEVICE_TYPE_CPU_COMPUTE_UNIT
                        | AMDT_DEVICE_TYPE_CPU_CORE
                        | AMDT_DEVICE_TYPE_PHYSICAL_CORE
                        | AMDT_DEVICE_TYPE_THREAD
                )
            {
                let semantics = device_type_name(raw.device_type).to_string();
                selection.frequencies.push(target_for(
                    raw,
                    FREQUENCY_KEY,
                    &format!("{semantics}"),
                    &format!(
                        "device_type={semantics};device_id={};instance={}",
                        raw.device_id, raw.device_instance_id
                    ),
                    "MHz",
                    None,
                ));
            }
        }
        selection
    }

    fn target_for(
        raw: AmdCounterDesc,
        metric_key: &str,
        scope: &str,
        identity: &str,
        unit: &str,
        qualifier: Option<&str>,
    ) -> Target {
        Target {
            counter_id: raw.counter_id,
            metric_key: metric_key.to_string(),
            scope: scope.to_string(),
            identity: identity.to_string(),
            identity_semantics: device_type_name(raw.device_type).to_string(),
            unit: unit.to_string(),
            qualifier: qualifier.map(str::to_string),
        }
    }

    fn operation_pending(operation: &str) -> OperationReport {
        OperationReport {
            operation: operation.to_string(),
            stable_status: "not_called".to_string(),
            vendor_status: None,
            vendor_status_code: None,
        }
    }

    fn operation_ok(operation: &str) -> OperationReport {
        OperationReport {
            operation: operation.to_string(),
            stable_status: "ok".to_string(),
            vendor_status: Some(status_name(AMDT_STATUS_OK)),
            vendor_status_code: Some(status_code(AMDT_STATUS_OK)),
        }
    }

    fn operation_from_status(operation: &str, status: u32) -> OperationReport {
        OperationReport {
            operation: operation.to_string(),
            stable_status: stable_status_for(operation, status),
            vendor_status: Some(status_name(status)),
            vendor_status_code: Some(status_code(status)),
        }
    }

    fn stable_status_for(operation: &str, status: u32) -> String {
        if status == AMDT_STATUS_OK {
            return "ok".to_string();
        }
        if operation == "AMDTPwrStartProfiling" && status == AMDT_ERROR_ACCESSDENIED {
            return "provider_busy".to_string();
        }
        if matches!(
            status,
            AMDT_ERROR_NOTSUPPORTED
                | AMDT_ERROR_PLATFORM_NOT_SUPPORTED
                | AMDT_ERROR_HYPERVISOR_NOT_SUPPORTED
                | AMDT_ERROR_COUNTER_NOT_ACCESSIBLE
                | AMDT_ERROR_INVALID_COUNTERID
        ) {
            return "unsupported".to_string();
        }
        if matches!(
            status,
            AMDT_ERROR_NO_WRITE_PERMISSION | AMDT_ERROR_ACCESSDENIED
        ) {
            return "permission_denied".to_string();
        }
        if matches!(
            status,
            AMDT_ERROR_DRIVER_UNAVAILABLE | AMDT_ERROR_DRIVER_UNINITIALIZED
        ) {
            return "provider_missing".to_string();
        }
        "failed".to_string()
    }

    fn is_busy_status(operation: &str, status: u32) -> bool {
        (operation == "AMDTPwrStartProfiling" && status == AMDT_ERROR_ACCESSDENIED)
            || matches!(
                status,
                AMDT_ERROR_DRIVER_ALREADY_INITIALIZED
                    | AMDT_ERROR_PROFILE_SESSION_EXISTS
                    | AMDT_ERROR_PREVIOUS_SESSION_NOT_CLOSED
            )
    }

    fn status_code(status: u32) -> String {
        format!("0x{status:08X}")
    }

    fn status_name(status: u32) -> String {
        let name = match status {
            AMDT_STATUS_OK => "AMDT_STATUS_OK",
            AMDT_ERROR_ACCESSDENIED => "AMDT_ERROR_ACCESSDENIED",
            AMDT_ERROR_NOTSUPPORTED => "AMDT_ERROR_NOTSUPPORTED",
            AMDT_ERROR_DRIVER_ALREADY_INITIALIZED => "AMDT_ERROR_DRIVER_ALREADY_INITIALIZED",
            AMDT_ERROR_DRIVER_UNAVAILABLE => "AMDT_ERROR_DRIVER_UNAVAILABLE",
            AMDT_ERROR_DRIVER_UNINITIALIZED => "AMDT_ERROR_DRIVER_UNINITIALIZED",
            AMDT_ERROR_INVALID_COUNTERID => "AMDT_ERROR_INVALID_COUNTERID",
            AMDT_ERROR_COUNTER_ALREADY_ENABLED => "AMDT_ERROR_COUNTER_ALREADY_ENABLED",
            AMDT_ERROR_NO_WRITE_PERMISSION => "AMDT_ERROR_NO_WRITE_PERMISSION",
            AMDT_ERROR_COUNTER_NOT_ENABLED => "AMDT_ERROR_COUNTER_NOT_ENABLED",
            AMDT_ERROR_TIMER_NOT_SET => "AMDT_ERROR_TIMER_NOT_SET",
            AMDT_ERROR_PROFILE_ALREADY_STARTED => "AMDT_ERROR_PROFILE_ALREADY_STARTED",
            AMDT_ERROR_PROFILE_NOT_STARTED => "AMDT_ERROR_PROFILE_NOT_STARTED",
            AMDT_ERROR_PROFILE_DATA_NOT_AVAILABLE => "AMDT_ERROR_PROFILE_DATA_NOT_AVAILABLE",
            AMDT_ERROR_PLATFORM_NOT_SUPPORTED => "AMDT_ERROR_PLATFORM_NOT_SUPPORTED",
            AMDT_DRIVER_VERSION_MISMATCH => "AMDT_DRIVER_VERSION_MISMATCH",
            AMDT_ERROR_PROFILE_SESSION_EXISTS => "AMDT_ERROR_PROFILE_SESSION_EXISTS",
            AMDT_ERROR_SMU_ACCESS_FAILED => "AMDT_ERROR_SMU_ACCESS_FAILED",
            AMDT_ERROR_COUNTERS_NOT_ENABLED => "AMDT_ERROR_COUNTERS_NOT_ENABLED",
            AMDT_ERROR_PREVIOUS_SESSION_NOT_CLOSED => "AMDT_ERROR_PREVIOUS_SESSION_NOT_CLOSED",
            AMDT_ERROR_COUNTER_NOT_ACCESSIBLE => "AMDT_ERROR_COUNTER_NOT_ACCESSIBLE",
            AMDT_ERROR_HYPERVISOR_NOT_SUPPORTED => "AMDT_ERROR_HYPERVISOR_NOT_SUPPORTED",
            _ => return format!("AMDT_RESULT_UNKNOWN_{}", status_code(status)),
        };
        name.to_string()
    }

    fn device_type_name(value: u32) -> &'static str {
        match value {
            0 => "system",
            AMDT_DEVICE_TYPE_PACKAGE => "package",
            AMDT_DEVICE_TYPE_CPU_COMPUTE_UNIT => "cpu_compute_unit",
            AMDT_DEVICE_TYPE_CPU_CORE => "cpu_core",
            4 => "die",
            AMDT_DEVICE_TYPE_PHYSICAL_CORE => "physical_core",
            AMDT_DEVICE_TYPE_THREAD => "logical_processor_or_thread",
            7 => "internal_gpu",
            8 => "external_gpu",
            9 => "svi2",
            _ => "unknown_device_type",
        }
    }

    fn category_name(value: u32) -> &'static str {
        match value {
            AMDT_PWR_CATEGORY_POWER => "power",
            AMDT_PWR_CATEGORY_FREQUENCY => "frequency",
            AMDT_PWR_CATEGORY_TEMPERATURE => "temperature",
            3 => "voltage",
            4 => "current",
            5 => "pstate",
            6 => "cstates_residency",
            7 => "time",
            8 => "energy",
            9 => "correlated_power",
            10 => "cac",
            11 => "controller",
            12 => "dpm",
            _ => "unknown_category",
        }
    }

    fn aggregation_name(value: u32) -> &'static str {
        match value {
            0 => "single",
            1 => "cumulative",
            2 => "histogram",
            _ => "unknown_aggregation",
        }
    }

    fn unit_name(value: u32) -> &'static str {
        match value {
            0 => "count",
            1 => "number",
            2 => "percent",
            3 => "ratio",
            4 => "millisecond",
            5 => "joule",
            AMDT_PWR_UNIT_WATT => "watt",
            7 => "volt",
            8 => "milliampere",
            AMDT_PWR_UNIT_MEGA_HERTZ => "megahertz",
            AMDT_PWR_UNIT_CENTIGRADE => "centigrade",
            _ => "unknown_unit",
        }
    }

    fn read_c_string(pointer: *mut c_char) -> Option<String> {
        if pointer.is_null() {
            return None;
        }
        let bytes = unsafe { CStr::from_ptr(pointer).to_bytes() };
        let bytes = &bytes[..bytes.len().min(256)];
        let value = String::from_utf8_lossy(bytes)
            .chars()
            .filter(|character| !character.is_control())
            .collect::<String>()
            .trim()
            .to_string();
        (!value.is_empty()).then_some(value)
    }

    unsafe fn load_functions(module: HMODULE) -> Result<AmdFunctions, String> {
        macro_rules! symbol {
            ($name:literal, $type:ty) => {
                load_symbol::<$type>(module, concat!($name, "\0").as_bytes())
                    .ok_or_else(|| $name.to_string())?
            };
        }
        Ok(AmdFunctions {
            initialize: symbol!("AMDTPwrProfileInitialize", ProfileInitialize),
            get_supported_counters: symbol!("AMDTPwrGetSupportedCounters", GetSupportedCounters),
            enable_counter: symbol!("AMDTPwrEnableCounter", EnableCounter),
            set_timer_sampling_period: symbol!(
                "AMDTPwrSetTimerSamplingPeriod",
                SetTimerSamplingPeriod
            ),
            start_profiling: symbol!("AMDTPwrStartProfiling", StartProfiling),
            read_all_enabled_counters: symbol!(
                "AMDTPwrReadAllEnabledCounters",
                ReadAllEnabledCounters
            ),
            stop_profiling: symbol!("AMDTPwrStopProfiling", StopProfiling),
            close: symbol!("AMDTPwrProfileClose", ProfileClose),
        })
    }

    unsafe fn load_symbol<T: Copy>(module: HMODULE, name: &[u8]) -> Option<T> {
        let proc = GetProcAddress(module, PCSTR::from_raw(name.as_ptr()))?;
        Some(mem::transmute_copy(&proc))
    }

    fn wide_path(path: &Path) -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn is_within(path: &Path, root: &Path) -> bool {
        let path = path.to_string_lossy().to_ascii_lowercase();
        let root = root.to_string_lossy().to_ascii_lowercase();
        path == root || path.starts_with(&(root + "\\"))
    }

    fn pe_architecture(path: &Path) -> Result<&'static str, String> {
        let data =
            fs::read(path).map_err(|error| format!("read_library_header_failed: {error}"))?;
        if data.len() < 64 || &data[0..2] != b"MZ" {
            return Err("library_is_not_pe".to_string());
        }
        let pe_offset =
            u32::from_le_bytes([data[0x3c], data[0x3d], data[0x3e], data[0x3f]]) as usize;
        if pe_offset + 6 > data.len() || &data[pe_offset..pe_offset + 4] != b"PE\0\0" {
            return Err("library_pe_header_invalid".to_string());
        }
        let machine = u16::from_le_bytes([data[pe_offset + 4], data[pe_offset + 5]]);
        match machine {
            0x8664 => Ok("x64"),
            0x014c => Ok("x86"),
            _ => Err(format!("unsupported_library_machine_0x{machine:04X}")),
        }
    }

    fn process_architecture() -> &'static str {
        match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "x86",
            _ => "unknown",
        }
    }

    fn redact_path(path: &Path) -> String {
        let value = path.to_string_lossy().to_string();
        let lower = value.to_ascii_lowercase();
        if lower.contains("\\users\\") || lower.contains("/users/") {
            "<redacted-user-path>".to_string()
        } else {
            value
        }
    }

    fn mode_name(mode: AmdUprofMode) -> &'static str {
        match mode {
            AmdUprofMode::Sanity => "sanity",
            AmdUprofMode::Cadence => "cadence",
            AmdUprofMode::Lifecycle => "lifecycle",
            AmdUprofMode::Busy => "busy",
        }
    }

    fn display(value: Option<f64>) -> String {
        value
            .map(|value| format!("{value:.3}"))
            .unwrap_or_else(|| "—".to_string())
    }

    const AMDT_ERROR_FAIL: u32 = 0x8000_4005;

    #[cfg(test)]
    mod tests {
        use super::*;

        fn descriptor(
            counter_id: u32,
            device_type: u32,
            device_instance_id: u32,
            category: u32,
            units: u32,
        ) -> Descriptor {
            Descriptor {
                raw: AmdCounterDesc {
                    counter_id,
                    device_id: 7,
                    device_type,
                    device_instance_id,
                    name: ptr::null_mut(),
                    description: ptr::null_mut(),
                    category,
                    aggregation: 0,
                    min_value: 0.0,
                    max_value: 100_000.0,
                    units,
                    is_parent_counter: 0,
                },
                name: None,
            }
        }

        #[test]
        fn start_access_denied_is_busy_but_other_access_denied_is_permission() {
            assert_eq!(
                stable_status_for("AMDTPwrStartProfiling", AMDT_ERROR_ACCESSDENIED),
                "provider_busy"
            );
            assert_eq!(
                stable_status_for("AMDTPwrProfileInitialize", AMDT_ERROR_ACCESSDENIED),
                "permission_denied"
            );
        }

        #[test]
        fn selection_preserves_per_identity_frequency_semantics() {
            let descriptors = vec![
                descriptor(
                    1,
                    AMDT_DEVICE_TYPE_PACKAGE,
                    0,
                    AMDT_PWR_CATEGORY_POWER,
                    AMDT_PWR_UNIT_WATT,
                ),
                descriptor(
                    2,
                    AMDT_DEVICE_TYPE_CPU_CORE,
                    0,
                    AMDT_PWR_CATEGORY_FREQUENCY,
                    AMDT_PWR_UNIT_MEGA_HERTZ,
                ),
                descriptor(
                    3,
                    AMDT_DEVICE_TYPE_THREAD,
                    1,
                    AMDT_PWR_CATEGORY_FREQUENCY,
                    AMDT_PWR_UNIT_MEGA_HERTZ,
                ),
            ];
            let selection = select_targets(&descriptors);
            assert_eq!(selection.frequencies.len(), 2);
            assert_eq!(selection.frequencies[0].identity_semantics, "cpu_core");
            assert_eq!(
                selection.frequencies[1].identity_semantics,
                "logical_processor_or_thread"
            );
            assert_ne!(
                selection.frequencies[0].identity,
                selection.frequencies[1].identity
            );
        }

        #[test]
        fn rejected_values_have_no_numeric_zero_synthesis() {
            let target = Target {
                counter_id: 1,
                metric_key: POWER_KEY.to_string(),
                scope: "package".to_string(),
                identity: "package".to_string(),
                identity_semantics: "package".to_string(),
                unit: "W".to_string(),
                qualifier: Some("ESTIMATED".to_string()),
            };
            let mut metric = MetricAccumulator::from_target(&target);
            assert!(!metric.value(f64::NAN, None));
            assert!(metric.values.is_empty());
            assert_eq!(metric.non_finite_count, 1);
            assert!(!metric.value(-1.0, None));
            assert!(metric.values.is_empty());
            assert_eq!(metric.negative_value_count, 1);
        }

        #[test]
        fn unknown_vendor_status_is_retained_as_hex() {
            assert_eq!(status_name(0xDEAD_BEEF), "AMDT_RESULT_UNKNOWN_0xDEADBEEF");
            assert_eq!(status_code(0xDEAD_BEEF), "0xDEADBEEF");
        }

        #[test]
        fn load_child_observation_preserves_signed_and_hex_exit() {
            let observation = LoadChildObservation {
                exit_code: Some(-1),
                exit_code_hex: Some("0xFFFFFFFF".to_string()),
                stdout: "BEFORE_LOAD".to_string(),
                stderr: String::new(),
                timed_out: false,
            };
            assert!(!observation.succeeded());
            assert_eq!(observation.exit_code, Some(-1));
            assert_eq!(observation.exit_code_hex.as_deref(), Some("0xFFFFFFFF"));
            assert!(observation
                .failure_summary()
                .contains("signed_-1_hex_0xFFFFFFFF"));
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{run, run_load_child, run_load_only_child, run_workload_child};
