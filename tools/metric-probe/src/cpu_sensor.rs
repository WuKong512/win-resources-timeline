use crate::{
    cli::{CpuSensorConfig, CpuSensorLifecycleConfig},
    model::{
        BudgetComparison, Conclusion, DeferredItem, MachineInfo, MetricRecord, PrivacySummary,
        SelfMetricSummary, SelfResourceSummary, SupportStatus,
    },
    report::{validate_public_text, write_atomic},
    stats::Distribution,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const CPU_SENSOR_SCHEMA: &str = "cpu-sensor-spike/v1";
const CPU_SENSOR_LIFECYCLE_SCHEMA: &str = "cpu-sensor-spike-lifecycle/v1";

#[derive(Debug, Clone, Serialize)]
struct CpuSensorSource {
    source_key: String,
    observed_status: SupportStatus,
    reason_code: String,
    admin_required: bool,
    driver_required: bool,
    external_process: bool,
    semantic_scope: String,
    limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CpuMetricSummary {
    metric_key: String,
    provider: String,
    status: SupportStatus,
    reason_code: String,
    sample_count: usize,
    failed_sample_count: usize,
    unique_value_count: usize,
    repeated_sample_count: usize,
    repeat_ratio: Option<f64>,
    values: Distribution,
}

#[derive(Debug, Clone, Serialize)]
struct SourceRefreshSample {
    sample_timestamp_ms: i64,
    source_timestamp_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
struct CpuSensorConfiguration {
    duration_seconds: u64,
    poll_interval_ms: u64,
    reference_adapter: String,
}

#[derive(Debug, Clone, Serialize, Default)]
struct CpuSensorSampling {
    wall_duration_ms: u64,
    expected_samples: u64,
    executed_samples: u64,
    dropped_samples: u64,
    late_wakeups: u64,
    logical_source_poll_count: u64,
}

#[derive(Debug, Clone, Serialize)]
struct CpuSensorReport {
    schema_version: String,
    probe_name: String,
    started_at_utc: String,
    finished_at_utc: String,
    machine: MachineInfo,
    configuration: CpuSensorConfiguration,
    sources: Vec<CpuSensorSource>,
    metrics: Vec<MetricRecord>,
    metric_summaries: Vec<CpuMetricSummary>,
    reference_source_refresh: Vec<SourceRefreshSample>,
    sampling: CpuSensorSampling,
    self_resource_summary: SelfResourceSummary,
    privacy: PrivacySummary,
    conclusion: Conclusion,
    deferred: Vec<DeferredItem>,
    rerun_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CpuSensorLifecycleConfiguration {
    enabled_duration_ms: u64,
    disabled_duration_ms: u64,
    poll_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct CpuSensorResourceSnapshot {
    threads: u32,
    handles: u32,
    working_set_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
struct CpuSensorLifecyclePhase {
    phase: String,
    enabled: bool,
    duration_ms: u64,
    scheduler_tick_count: u64,
    successful_sample_count: u64,
    source_poll_count_delta: u64,
    no_source_polling_observed: bool,
    resource_start: Option<CpuSensorResourceSnapshot>,
    resource_end: Option<CpuSensorResourceSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
struct CpuSensorLifecycleResources {
    baseline: Option<CpuSensorResourceSnapshot>,
    final_snapshot: Option<CpuSensorResourceSnapshot>,
    peak_threads: Option<u32>,
    peak_handles: Option<u32>,
    peak_working_set_bytes: Option<u64>,
    thread_delta: Option<i64>,
    handle_delta: Option<i64>,
    working_set_delta_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct CpuSensorLifecycleReport {
    schema_version: String,
    probe_name: String,
    generated_at_utc: String,
    machine: MachineInfo,
    configuration: CpuSensorLifecycleConfiguration,
    sources: Vec<CpuSensorSource>,
    phases: Vec<CpuSensorLifecyclePhase>,
    resources: CpuSensorLifecycleResources,
    enable_disable_reenable: bool,
    cleanup_completed: bool,
    failure_isolation: String,
    sleep_resume: String,
}

#[cfg(windows)]
#[derive(Debug)]
struct CpuSensorSession {
    enabled: bool,
    pdh: Option<crate::windows::CpuPerformanceProvider>,
    afterburner: Option<crate::windows::AfterburnerSharedMemory>,
    pdh_status: (crate::windows::ReadStatus, String),
    afterburner_status: (crate::windows::ReadStatus, String),
    logical_source_poll_count: u64,
}

#[cfg(windows)]
struct CpuSensorReading {
    os_frequency: crate::windows::ReadResult<crate::windows::CpuFrequencyInfo>,
    pdh: Option<crate::windows::ReadResult<crate::windows::CpuPerformanceCounters>>,
    afterburner: Option<crate::windows::ReadResult<crate::windows::AfterburnerSnapshot>>,
    os_latency_ms: f64,
    pdh_latency_ms: f64,
    afterburner_latency_ms: f64,
}

#[cfg(windows)]
impl CpuSensorSession {
    fn new() -> Self {
        let mut session = Self {
            enabled: false,
            pdh: None,
            afterburner: None,
            pdh_status: (
                crate::windows::ReadStatus::ProviderMissing,
                "not_initialized".to_string(),
            ),
            afterburner_status: (
                crate::windows::ReadStatus::ProviderMissing,
                "not_initialized".to_string(),
            ),
            logical_source_poll_count: 0,
        };
        session.enable();
        session
    }

    fn enable(&mut self) {
        if self.enabled {
            return;
        }
        let pdh = crate::windows::CpuPerformanceProvider::new();
        let (pdh_provider, pdh_status) = match pdh {
            crate::windows::ReadResult {
                status,
                reason_code,
                value: Some(provider),
            } => (Some(provider), (status, reason_code)),
            crate::windows::ReadResult {
                status,
                reason_code,
                value: None,
            } => (None, (status, reason_code)),
        };
        let afterburner = crate::windows::AfterburnerSharedMemory::open();
        let (afterburner_provider, afterburner_status) = match afterburner {
            crate::windows::ReadResult {
                status,
                reason_code,
                value: Some(provider),
            } => (Some(provider), (status, reason_code)),
            crate::windows::ReadResult {
                status,
                reason_code,
                value: None,
            } => (None, (status, reason_code)),
        };
        self.pdh = pdh_provider;
        self.afterburner = afterburner_provider;
        self.pdh_status = pdh_status;
        self.afterburner_status = afterburner_status;
        self.enabled = true;
    }

    fn disable(&mut self) {
        self.enabled = false;
        self.afterburner = None;
        self.pdh = None;
    }

    fn is_stopped(&self) -> bool {
        !self.enabled && self.pdh.is_none() && self.afterburner.is_none()
    }

    fn logical_source_poll_count(&self) -> u64 {
        self.logical_source_poll_count
    }

    fn sample(&mut self) -> Option<CpuSensorReading> {
        if !self.enabled {
            return None;
        }
        self.logical_source_poll_count = self.logical_source_poll_count.saturating_add(1);

        let started = Instant::now();
        let os_frequency = crate::windows::cpu_frequency_info();
        let os_latency_ms = started.elapsed().as_secs_f64() * 1000.0;

        let (pdh, pdh_latency_ms) = if let Some(provider) = self.pdh.as_mut() {
            let started = Instant::now();
            let result = provider.sample();
            (Some(result), started.elapsed().as_secs_f64() * 1000.0)
        } else {
            (None, 0.0)
        };

        let (afterburner, afterburner_latency_ms) =
            if let Some(provider) = self.afterburner.as_ref() {
                let started = Instant::now();
                let result = provider.sample();
                (Some(result), started.elapsed().as_secs_f64() * 1000.0)
            } else {
                (None, 0.0)
            };

        Some(CpuSensorReading {
            os_frequency,
            pdh,
            afterburner,
            os_latency_ms,
            pdh_latency_ms,
            afterburner_latency_ms,
        })
    }
}

pub fn run(config: CpuSensorConfig) -> Result<(PathBuf, PathBuf), String> {
    #[cfg(windows)]
    {
        run_windows(config)
    }
    #[cfg(not(windows))]
    {
        let _ = config;
        Err("CPU sensor probe is only executable on Windows".to_string())
    }
}

pub fn run_lifecycle(config: CpuSensorLifecycleConfig) -> Result<(PathBuf, PathBuf), String> {
    #[cfg(windows)]
    {
        run_lifecycle_windows(config)
    }
    #[cfg(not(windows))]
    {
        let _ = config;
        Err("CPU sensor lifecycle probe is only executable on Windows".to_string())
    }
}

#[cfg(windows)]
fn run_windows(config: CpuSensorConfig) -> Result<(PathBuf, PathBuf), String> {
    let machine = crate::windows::machine_info();
    let started_at_utc = crate::utc_now_string();
    let started = Instant::now();
    let mut session = CpuSensorSession::new();
    let mut metrics = metric_definitions();
    let mut reference_source_refresh = Vec::new();
    let mut self_samples = SelfSamples::default();
    let mut previous_self: Option<(Instant, u64)> = None;
    let mut previous_poll = None;
    let interval = Duration::from_millis(config.poll_interval_ms.max(1));
    let duration = Duration::from_secs(config.duration_seconds);
    let mut next_poll = started;
    let mut sampling = CpuSensorSampling::default();

    while started.elapsed() < duration {
        let now = Instant::now();
        if now >= next_poll {
            let due = due_count(now, next_poll, interval);
            sampling.dropped_samples = sampling
                .dropped_samples
                .saturating_add(due.saturating_sub(1));
            sampling.executed_samples = sampling.executed_samples.saturating_add(1);
            if sampling.executed_samples > 1 && now > next_poll + Duration::from_millis(50) {
                sampling.late_wakeups = sampling.late_wakeups.saturating_add(1);
            }
            let interval_ms = previous_poll
                .map(|previous: Instant| now.duration_since(previous).as_millis() as u64);
            let timestamp_ms = crate::unix_now_ms();
            if let Some(reading) = session.sample() {
                record_reading(&mut metrics, &reading, timestamp_ms, interval_ms);
                if let Some(Some(reference)) = reading.afterburner.as_ref().map(|result| {
                    result
                        .value
                        .as_ref()
                        .map(|value| value.source_timestamp_seconds)
                }) {
                    reference_source_refresh.push(SourceRefreshSample {
                        sample_timestamp_ms: timestamp_ms,
                        source_timestamp_seconds: reference,
                    });
                }
            }
            sample_self(&mut self_samples, &mut previous_self, now, &machine);
            previous_poll = Some(now);
            next_poll += interval * due.max(1) as u32;
        }
        if next_poll > Instant::now() {
            thread::sleep((next_poll - Instant::now()).min(Duration::from_millis(100)));
        }
    }

    sampling.wall_duration_ms = started.elapsed().as_millis() as u64;
    sampling.expected_samples = expected_samples(config.duration_seconds, config.poll_interval_ms);
    sampling.logical_source_poll_count = session.logical_source_poll_count();
    session.disable();

    let mut metrics: Vec<_> = metrics.into_values().collect();
    for metric in &mut metrics {
        metric.finalize();
    }
    let metric_summaries = metrics.iter().map(metric_summary).collect::<Vec<_>>();
    let sources = source_descriptions(&session, &metrics);
    let average_cpu = self_samples
        .probe_cpu_share_percent
        .iter()
        .copied()
        .reduce(|left, right| left + right)
        .map(|sum| sum / self_samples.probe_cpu_share_percent.len() as f64);
    let steady_memory = self_samples
        .working_set_bytes
        .iter()
        .copied()
        .reduce(f64::max)
        .map(|value| value as u64);
    let elevated = machine.elevated.unwrap_or(false);
    let report = CpuSensorReport {
        schema_version: CPU_SENSOR_SCHEMA.to_string(),
        probe_name: "cpu-sensors".to_string(),
        started_at_utc,
        finished_at_utc: crate::utc_now_string(),
        machine,
        configuration: CpuSensorConfiguration {
            duration_seconds: config.duration_seconds,
            poll_interval_ms: config.poll_interval_ms,
            reference_adapter: "MSI Afterburner MAHM shared memory, read-only and optional"
                .to_string(),
        },
        sources,
        metrics,
        metric_summaries,
        reference_source_refresh,
        sampling,
        self_resource_summary: self_resource_summary(&self_samples),
        privacy: privacy_summary(),
        conclusion: Conclusion {
            scope: "Current machine experiment only; cross-hardware support is not implied"
                .to_string(),
            default_budget_comparison: BudgetComparison {
                average_probe_cpu_share_percent: average_cpu,
                probe_cpu_share_under_0_5_percent: average_cpu.map(|value| value < 0.5),
                steady_state_working_set_bytes: steady_memory,
                steady_state_memory_under_80_mb: steady_memory
                    .map(|value| value < 80 * 1024 * 1024),
                is_current_machine_experiment: true,
            },
            cross_hardware_status: "not validated".to_string(),
            permission_scope: if elevated {
                "elevated administrator process"
            } else {
                "non-administrator process"
            }
            .to_string(),
        },
        deferred: vec![
            DeferredItem {
                item: "CPU package temperature admission".to_string(),
                status: "deferred".to_string(),
                reason: "Windows built-ins do not establish package semantics; external reference is not a production dependency".to_string(),
            },
            DeferredItem {
                item: "CPU package power admission".to_string(),
                status: "deferred".to_string(),
                reason: "No redistributable, privilege-safe package power source was validated".to_string(),
            },
            DeferredItem {
                item: "CPU effective frequency admission".to_string(),
                status: "deferred".to_string(),
                reason: "OS current frequency and performance counters are not an effective-average frequency".to_string(),
            },
            DeferredItem {
                item: "Administrator comparison".to_string(),
                status: "pending".to_string(),
                reason: "The probe does not elevate itself; run the same command in a separate controlled administrator session".to_string(),
            },
            DeferredItem {
                item: "Sleep/resume".to_string(),
                status: "not_exercised".to_string(),
                reason: "The probe does not trigger system sleep".to_string(),
            },
        ],
        rerun_commands: vec![format!(
            "cargo run --manifest-path tools/metric-probe/Cargo.toml -- cpu-sensors --duration-seconds {} --poll-interval-ms {}",
            config.duration_seconds, config.poll_interval_ms
        )],
    };
    write_cpu_report(&config.output_dir, &report)
}

#[cfg(windows)]
fn run_lifecycle_windows(config: CpuSensorLifecycleConfig) -> Result<(PathBuf, PathBuf), String> {
    let machine = crate::windows::machine_info();
    let mut session = CpuSensorSession::new();
    let poll_interval = Duration::from_millis(500);
    let phases = vec![
        run_lifecycle_phase(
            &mut session,
            "enabled-1",
            true,
            config.enabled_duration_ms,
            poll_interval,
        ),
        {
            session.disable();
            run_lifecycle_phase(
                &mut session,
                "disabled-1",
                false,
                config.disabled_duration_ms,
                poll_interval,
            )
        },
        {
            session.enable();
            run_lifecycle_phase(
                &mut session,
                "re-enabled-1",
                true,
                config.enabled_duration_ms,
                poll_interval,
            )
        },
        {
            session.disable();
            run_lifecycle_phase(
                &mut session,
                "disabled-2",
                false,
                config.disabled_duration_ms,
                poll_interval,
            )
        },
    ];
    session.disable();
    let cleanup_completed = session.is_stopped();
    let disabled_no_polling = phases
        .iter()
        .filter(|phase| !phase.enabled)
        .all(|phase| phase.scheduler_tick_count > 0 && phase.no_source_polling_observed);
    let reenabled_sampled = phases
        .iter()
        .any(|phase| phase.phase == "re-enabled-1" && phase.successful_sample_count > 0);
    let resources = lifecycle_resources(&phases);
    let report = CpuSensorLifecycleReport {
        schema_version: CPU_SENSOR_LIFECYCLE_SCHEMA.to_string(),
        probe_name: "cpu-sensor-lifecycle".to_string(),
        generated_at_utc: crate::utc_now_string(),
        machine,
        configuration: CpuSensorLifecycleConfiguration {
            enabled_duration_ms: config.enabled_duration_ms,
            disabled_duration_ms: config.disabled_duration_ms,
            poll_interval_ms: poll_interval.as_millis() as u64,
        },
        sources: source_descriptions(&session, &[]),
        phases,
        resources,
        enable_disable_reenable: disabled_no_polling && reenabled_sampled,
        cleanup_completed,
        failure_isolation: "Source handles are owned by this optional session; a missing PDH/shared-memory source is represented as a per-source failure and does not stop the lifecycle harness".to_string(),
        sleep_resume: "not_exercised".to_string(),
    };
    write_lifecycle_report(&config.output_dir, &report)
}

#[cfg(windows)]
fn run_lifecycle_phase(
    session: &mut CpuSensorSession,
    phase: &str,
    enabled: bool,
    duration_ms: u64,
    poll_interval: Duration,
) -> CpuSensorLifecyclePhase {
    let started = Instant::now();
    let resource_start = resource_snapshot();
    let source_poll_start = session.logical_source_poll_count();
    let mut next_poll = started;
    let mut scheduler_tick_count = 0;
    let mut successful_sample_count = 0;
    while started.elapsed() < Duration::from_millis(duration_ms) {
        let now = Instant::now();
        if now >= next_poll {
            scheduler_tick_count += 1;
            if session.sample().is_some() {
                successful_sample_count += 1;
            }
            next_poll += poll_interval;
        }
        if next_poll > Instant::now() {
            thread::sleep((next_poll - Instant::now()).min(Duration::from_millis(100)));
        }
    }
    let source_poll_end = session.logical_source_poll_count();
    let source_poll_count_delta = source_poll_end.saturating_sub(source_poll_start);
    CpuSensorLifecyclePhase {
        phase: phase.to_string(),
        enabled,
        duration_ms: started.elapsed().as_millis() as u64,
        scheduler_tick_count,
        successful_sample_count,
        source_poll_count_delta,
        no_source_polling_observed: !enabled && source_poll_count_delta == 0,
        resource_start,
        resource_end: resource_snapshot(),
    }
}

#[cfg(windows)]
fn record_reading(
    metrics: &mut BTreeMap<String, MetricRecord>,
    reading: &CpuSensorReading,
    timestamp_ms: i64,
    interval_ms: Option<u64>,
) {
    match &reading.os_frequency.value {
        Some(value) => {
            record_optional_value(
                metrics.get_mut("cpu.os_reported_current_mhz").unwrap(),
                value.current_mhz,
                timestamp_ms,
                interval_ms,
                reading.os_latency_ms,
                "os_current_frequency_missing",
                "mhz",
            );
            record_optional_value(
                metrics.get_mut("cpu.os_reported_max_mhz").unwrap(),
                value.max_mhz,
                timestamp_ms,
                interval_ms,
                reading.os_latency_ms,
                "os_max_frequency_missing",
                "mhz",
            );
        }
        None => {
            for key in ["cpu.os_reported_current_mhz", "cpu.os_reported_max_mhz"] {
                record_read_failure(
                    metrics.get_mut(key).unwrap(),
                    reading.os_frequency.status,
                    reading.os_frequency.reason_code.clone(),
                    reading.os_latency_ms,
                );
            }
        }
    }

    let pdh_keys = [
        "cpu.processor_frequency_mhz",
        "cpu.processor_performance_percent",
        "cpu.processor_utility_percent",
        "cpu.percent_maximum_frequency",
    ];
    match reading.pdh.as_ref() {
        Some(result) => match result.value {
            Some(value) => {
                let values = [
                    value.processor_frequency_mhz,
                    value.processor_performance_percent,
                    value.processor_utility_percent,
                    value.percent_maximum_frequency,
                ];
                for (key, value) in pdh_keys.into_iter().zip(values) {
                    record_value(
                        metrics.get_mut(key).unwrap(),
                        timestamp_ms,
                        value,
                        interval_ms,
                        reading.pdh_latency_ms,
                    );
                }
            }
            None => {
                for key in pdh_keys {
                    record_read_failure(
                        metrics.get_mut(key).unwrap(),
                        result.status,
                        result.reason_code.clone(),
                        reading.pdh_latency_ms,
                    );
                }
            }
        },
        None => {
            for key in pdh_keys {
                record_read_failure(
                    metrics.get_mut(key).unwrap(),
                    crate::windows::ReadStatus::ProviderMissing,
                    "pdh_processor_information_not_initialized".to_string(),
                    0.0,
                );
            }
        }
    }

    let reference_keys = [
        "reference.cpu_temperature_celsius",
        "reference.cpu_power_watts",
        "reference.cpu_clock_mhz",
    ];
    match reading.afterburner.as_ref() {
        Some(result) => match result.value {
            Some(value) => {
                let values = [
                    value.cpu_temperature_celsius,
                    value.cpu_power_watts,
                    value.cpu_clock_mhz,
                ];
                for (key, value) in reference_keys.into_iter().zip(values) {
                    record_optional_value(
                        metrics.get_mut(key).unwrap(),
                        value,
                        timestamp_ms,
                        interval_ms,
                        reading.afterburner_latency_ms,
                        "afterburner_cpu_value_missing",
                        "reference",
                    );
                }
            }
            None => {
                for key in reference_keys {
                    record_read_failure(
                        metrics.get_mut(key).unwrap(),
                        result.status,
                        result.reason_code.clone(),
                        reading.afterburner_latency_ms,
                    );
                }
            }
        },
        None => {
            for key in reference_keys {
                record_read_failure(
                    metrics.get_mut(key).unwrap(),
                    crate::windows::ReadStatus::ProviderMissing,
                    "afterburner_shared_memory_not_initialized".to_string(),
                    0.0,
                );
            }
        }
    }
}

#[cfg(windows)]
fn record_optional_value(
    metric: &mut MetricRecord,
    value: Option<f64>,
    timestamp_ms: i64,
    interval_ms: Option<u64>,
    latency_ms: f64,
    missing_reason: &str,
    _semantic_unit: &str,
) {
    match value {
        Some(value) if value.is_finite() => {
            if metric.metric_key == "reference.cpu_temperature_celsius"
                && !(-20.0..=150.0).contains(&value)
            {
                record_read_failure(
                    metric,
                    crate::windows::ReadStatus::Failed,
                    "temperature_value_out_of_plausible_range".to_string(),
                    latency_ms,
                );
            } else if metric.metric_key == "reference.cpu_power_watts"
                && !(0.0..=1_000.0).contains(&value)
            {
                record_read_failure(
                    metric,
                    crate::windows::ReadStatus::Failed,
                    "power_value_out_of_plausible_range".to_string(),
                    latency_ms,
                );
            } else {
                record_value(metric, timestamp_ms, value, interval_ms, latency_ms);
            }
        }
        Some(_) => record_read_failure(
            metric,
            crate::windows::ReadStatus::Failed,
            "non_finite_sensor_value".to_string(),
            latency_ms,
        ),
        None => metric.record_skip(
            SupportStatus::Unsupported,
            missing_reason.to_string(),
            latency_ms,
        ),
    }
}

#[cfg(windows)]
fn record_value(
    metric: &mut MetricRecord,
    timestamp_ms: i64,
    value: f64,
    interval_ms: Option<u64>,
    latency_ms: f64,
) {
    if value.is_finite() {
        metric.record_success(
            timestamp_ms,
            value,
            interval_ms,
            interval_ms.unwrap_or(0),
            latency_ms,
        );
    } else {
        record_read_failure(
            metric,
            crate::windows::ReadStatus::Failed,
            "non_finite_sensor_value".to_string(),
            latency_ms,
        );
    }
}

#[cfg(windows)]
fn record_read_failure(
    metric: &mut MetricRecord,
    status: crate::windows::ReadStatus,
    reason_code: String,
    latency_ms: f64,
) {
    let status = map_status(status);
    if reason_code.contains("warmup") {
        metric.record_skip(status, reason_code, latency_ms);
    } else {
        metric.record_failure_with_status(status, reason_code, latency_ms);
    }
}

fn metric_definitions() -> BTreeMap<String, MetricRecord> {
    let definitions = [
        (
            "cpu:system",
            "cpu.os_reported_current_mhz",
            "nt-power-processor-information",
            "MHz",
            ">=0",
            "OS CurrentMhz: maximum specified clock multiplied by current processor throttle; not effective average frequency",
        ),
        (
            "cpu:system",
            "cpu.os_reported_max_mhz",
            "nt-power-processor-information",
            "MHz",
            ">=0",
            "OS MaxMhz: maximum specified clock frequency; policy metadata, not a live measured clock",
        ),
        (
            "cpu:system",
            "cpu.processor_frequency_mhz",
            "pdh-processor-information",
            "MHz",
            ">=0",
            "PDH Processor Frequency counter; OS-reported processor frequency, not package-level effective frequency",
        ),
        (
            "cpu:system",
            "cpu.processor_performance_percent",
            "pdh-processor-information",
            "percent",
            "provider-defined",
            "PDH Processor Performance counter; performance-state ratio, not a temperature or power sensor",
        ),
        (
            "cpu:system",
            "cpu.processor_utility_percent",
            "pdh-processor-information",
            "percent",
            "provider-defined",
            "PDH Processor Utility counter; work/utilization signal that accounts for performance state and turbo, not frequency",
        ),
        (
            "cpu:system",
            "cpu.percent_maximum_frequency",
            "pdh-processor-information",
            "percent",
            "provider-defined",
            "PDH percentage of maximum frequency; normalized policy/current-state signal",
        ),
        (
            "reference:afterburner",
            "reference.cpu_temperature_celsius",
            "afterburner-shared-memory",
            "celsius",
            "-20..150",
            "Reference-only value exposed by an already-running MSI Afterburner CPUHAL; not a Resource Timeline package descriptor",
        ),
        (
            "reference:afterburner",
            "reference.cpu_power_watts",
            "afterburner-shared-memory",
            "W",
            "0..1000",
            "Reference-only value exposed by an already-running MSI Afterburner CPUHAL; scope and sampling semantics remain external",
        ),
        (
            "reference:afterburner",
            "reference.cpu_clock_mhz",
            "afterburner-shared-memory",
            "MHz",
            ">=0",
            "Reference-only CPU clock value; not assumed to be effective average frequency",
        ),
    ];
    definitions
        .into_iter()
        .map(|(device, key, provider, unit, range, limitation)| {
            (
                key.to_string(),
                MetricRecord::new(
                    device,
                    key,
                    provider,
                    SupportStatus::Supported,
                    "ready",
                    unit,
                    if provider.starts_with("pdh") {
                        "Windows PDH English counters"
                    } else if provider.starts_with("nt-") {
                        "Windows CallNtPowerInformation"
                    } else {
                        "MSI Afterburner MAHM shared memory"
                    },
                    vec![limitation.to_string()],
                )
                .with_value_range(range),
            )
        })
        .collect()
}

#[cfg(windows)]
fn source_descriptions(
    session: &CpuSensorSession,
    metrics: &[MetricRecord],
) -> Vec<CpuSensorSource> {
    let (nt_status, nt_reason) = metric_status(metrics, "cpu.os_reported_current_mhz")
        .unwrap_or((SupportStatus::Supported, "sample_not_attempted".to_string()));
    let (pdh_status, pdh_reason) = metric_status(metrics, "cpu.processor_frequency_mhz")
        .unwrap_or_else(|| {
            (
                map_status(session.pdh_status.0),
                session.pdh_status.1.clone(),
            )
        });
    let (afterburner_status, afterburner_reason) =
        metric_status(metrics, "reference.cpu_temperature_celsius").unwrap_or_else(|| {
            (
                map_status(session.afterburner_status.0),
                session.afterburner_status.1.clone(),
            )
        });
    vec![
        CpuSensorSource {
            source_key: "windows-nt-power-processor-information".to_string(),
            observed_status: nt_status,
            reason_code: nt_reason,
            admin_required: false,
            driver_required: false,
            external_process: false,
            semantic_scope: "OS processor policy/current-state fields".to_string(),
            limitations: vec![
                "CurrentMhz is maximum specified clock multiplied by current throttle".to_string(),
                "Does not prove package temperature, package power, or effective average frequency".to_string(),
            ],
        },
        CpuSensorSource {
            source_key: "windows-pdh-processor-information".to_string(),
            observed_status: pdh_status,
            reason_code: pdh_reason,
            admin_required: false,
            driver_required: false,
            external_process: false,
            semantic_scope: "OS aggregate frequency/performance/utility counters".to_string(),
            limitations: vec![
                "Microsoft documents periodic counter collection; sub-second polling can repeat values".to_string(),
                "Counters are not CPU package hardware sensors".to_string(),
            ],
        },
        CpuSensorSource {
            source_key: "msi-afterburner-mahm-shared-memory-reference".to_string(),
            observed_status: afterburner_status,
            reason_code: afterburner_reason,
            admin_required: true,
            driver_required: true,
            external_process: true,
            semantic_scope: "Reference monitor's CPUHAL labels and values".to_string(),
            limitations: vec![
                "Read only when an existing Afterburner instance publishes the documented mapping".to_string(),
                "Never start the monitor, install its driver, or use it as a production dependency".to_string(),
            ],
        },
    ]
}

#[cfg(windows)]
fn metric_status(metrics: &[MetricRecord], key: &str) -> Option<(SupportStatus, String)> {
    metrics
        .iter()
        .find(|metric| metric.metric_key == key)
        .map(|metric| (metric.support_status, metric.reason_code.clone()))
}

fn metric_summary(metric: &MetricRecord) -> CpuMetricSummary {
    let mut unique = BTreeSet::new();
    for sample in &metric.samples {
        unique.insert(sample.value.to_bits());
    }
    let unique_value_count = unique.len();
    let repeated_sample_count = metric.sample_count.saturating_sub(unique_value_count);
    CpuMetricSummary {
        metric_key: metric.metric_key.clone(),
        provider: metric.provider.clone(),
        status: metric.support_status,
        reason_code: metric.reason_code.clone(),
        sample_count: metric.sample_count,
        failed_sample_count: metric.failed_sample_count,
        unique_value_count,
        repeated_sample_count,
        repeat_ratio: (metric.sample_count > 0)
            .then(|| repeated_sample_count as f64 / metric.sample_count as f64),
        values: Distribution::from_values(
            &metric
                .samples
                .iter()
                .map(|sample| sample.value)
                .collect::<Vec<_>>(),
        ),
    }
}

#[derive(Debug, Default)]
struct SelfSamples {
    cpu_time_100ns: Vec<f64>,
    working_set_bytes: Vec<f64>,
    thread_count: Vec<f64>,
    handle_count: Vec<f64>,
    probe_cpu_share_percent: Vec<f64>,
}

#[cfg(windows)]
fn sample_self(
    samples: &mut SelfSamples,
    previous: &mut Option<(Instant, u64)>,
    now: Instant,
    machine: &MachineInfo,
) {
    let Some(value) = crate::windows::self_metrics().value else {
        return;
    };
    samples.cpu_time_100ns.push(value.cpu_time_100ns as f64);
    samples
        .working_set_bytes
        .push(value.working_set_bytes as f64);
    samples.thread_count.push(value.thread_count as f64);
    samples.handle_count.push(value.handle_count as f64);
    if let Some((previous_time, previous_cpu)) = *previous {
        let wall_seconds = now.duration_since(previous_time).as_secs_f64();
        if wall_seconds > 0.0 {
            let delta_100ns = value.cpu_time_100ns.saturating_sub(previous_cpu) as f64;
            let logical_processors = machine.logical_processor_count.unwrap_or(1) as f64;
            samples
                .probe_cpu_share_percent
                .push(delta_100ns * 100.0 / (wall_seconds * 10_000_000.0 * logical_processors));
        }
    }
    *previous = Some((now, value.cpu_time_100ns));
}

fn self_resource_summary(samples: &SelfSamples) -> SelfResourceSummary {
    SelfResourceSummary {
        cpu_time_100ns: self_metric_summary("100ns", &samples.cpu_time_100ns),
        probe_cpu_share_percent: self_metric_summary("percent", &samples.probe_cpu_share_percent),
        working_set_bytes: self_metric_summary("bytes", &samples.working_set_bytes),
        thread_count: self_metric_summary("count", &samples.thread_count),
        handle_count: self_metric_summary("count", &samples.handle_count),
    }
}

fn self_metric_summary(unit: &str, values: &[f64]) -> Option<SelfMetricSummary> {
    if values.is_empty() {
        return None;
    }
    Some(SelfMetricSummary {
        unit: unit.to_string(),
        sample_count: values.len(),
        start: values.first().copied(),
        average: Some(values.iter().sum::<f64>() / values.len() as f64),
        peak: values.iter().copied().reduce(f64::max),
        end: values.last().copied(),
        delta: values
            .last()
            .copied()
            .zip(values.first().copied())
            .map(|(end, start)| end - start),
        values: Distribution::from_values(values),
    })
}

#[cfg(windows)]
fn resource_snapshot() -> Option<CpuSensorResourceSnapshot> {
    crate::windows::self_metrics()
        .value
        .map(|value| CpuSensorResourceSnapshot {
            threads: value.thread_count,
            handles: value.handle_count,
            working_set_bytes: value.working_set_bytes,
        })
}

#[cfg(windows)]
fn lifecycle_resources(phases: &[CpuSensorLifecyclePhase]) -> CpuSensorLifecycleResources {
    let snapshots = phases
        .iter()
        .flat_map(|phase| [phase.resource_start.as_ref(), phase.resource_end.as_ref()])
        .flatten()
        .collect::<Vec<_>>();
    let baseline = phases
        .first()
        .and_then(|phase| phase.resource_start.clone());
    let final_snapshot = phases.last().and_then(|phase| phase.resource_end.clone());
    CpuSensorLifecycleResources {
        baseline: baseline.clone(),
        final_snapshot: final_snapshot.clone(),
        peak_threads: snapshots.iter().map(|snapshot| snapshot.threads).max(),
        peak_handles: snapshots.iter().map(|snapshot| snapshot.handles).max(),
        peak_working_set_bytes: snapshots
            .iter()
            .map(|snapshot| snapshot.working_set_bytes)
            .max(),
        thread_delta: final_snapshot
            .as_ref()
            .zip(baseline.as_ref())
            .map(|(end, start)| end.threads as i64 - start.threads as i64),
        handle_delta: final_snapshot
            .as_ref()
            .zip(baseline.as_ref())
            .map(|(end, start)| end.handles as i64 - start.handles as i64),
        working_set_delta_bytes: final_snapshot
            .as_ref()
            .zip(baseline.as_ref())
            .map(|(end, start)| end.working_set_bytes as i64 - start.working_set_bytes as i64),
    }
}

fn write_cpu_report(
    output_dir: &Path,
    report: &CpuSensorReport,
) -> Result<(PathBuf, PathBuf), String> {
    let markdown = render_cpu_sensor_markdown(report);
    write_serialized_reports(output_dir, "report", report, &markdown)
}

fn write_lifecycle_report(
    output_dir: &Path,
    report: &CpuSensorLifecycleReport,
) -> Result<(PathBuf, PathBuf), String> {
    let markdown = render_lifecycle_markdown(report);
    write_serialized_reports(output_dir, "lifecycle", report, &markdown)
}

fn write_serialized_reports<T: Serialize>(
    output_dir: &Path,
    stem: &str,
    report: &T,
    markdown: &str,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("create output directory failed: {error}"))?;
    let json_path = output_dir.join(format!("{stem}.json"));
    let markdown_path = output_dir.join(format!("{stem}.md"));
    let json = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("serialize CPU sensor report failed: {error}"))?;
    validate_public_text(std::str::from_utf8(&json).unwrap_or_default())?;
    validate_public_text(markdown)?;
    write_atomic(&json_path, &json)?;
    write_atomic(&markdown_path, markdown.as_bytes())?;
    Ok((json_path, markdown_path))
}

fn render_cpu_sensor_markdown(report: &CpuSensorReport) -> String {
    let mut output = String::new();
    output.push_str("# CPU sensor feasibility probe\n\n");
    output.push_str(&format!("- Schema: `{}`\n", report.schema_version));
    output.push_str(&format!("- Started: `{}`\n", report.started_at_utc));
    output.push_str(&format!("- Finished: `{}`\n", report.finished_at_utc));
    output.push_str(&format!("- Scope: {}\n\n", report.conclusion.scope));
    output.push_str("## Machine\n\n");
    output.push_str(&format!(
        "- OS: {} {} build {}\n- CPU: {}\n- Logical processors: {}\n- Elevated: {}\n\n",
        report.machine.os_name,
        report
            .machine
            .os_display_version
            .as_deref()
            .unwrap_or("unknown"),
        report.machine.os_build.as_deref().unwrap_or("unknown"),
        report.machine.cpu_model.as_deref().unwrap_or("unknown"),
        report
            .machine
            .logical_processor_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        report
            .machine
            .elevated
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    ));
    output.push_str("## Configuration\n\n");
    output.push_str(&format!(
        "- Duration: {} seconds\n- Poll interval: {} ms\n- Reference adapter: {}\n\n",
        report.configuration.duration_seconds,
        report.configuration.poll_interval_ms,
        report.configuration.reference_adapter,
    ));
    output.push_str("## Sources\n\n| Source | Status | Reason | Admin | Driver | External process | Scope |\n|---|---|---|---:|---:|---:|---|\n");
    for source in &report.sources {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} | {} | {} |\n",
            source.source_key,
            status_name(source.observed_status),
            source.reason_code,
            source.admin_required,
            source.driver_required,
            source.external_process,
            source.semantic_scope,
        ));
    }
    output.push('\n');
    output.push_str("## Metrics\n\n| Metric | Provider | Status | Samples | Failed | Unique | Repeat ratio | Min | Mean | Max | Latest |\n|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for (metric, summary) in report.metrics.iter().zip(report.metric_summaries.iter()) {
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            metric.metric_key,
            metric.provider,
            status_name(summary.status),
            summary.sample_count,
            summary.failed_sample_count,
            summary.unique_value_count,
            display(summary.repeat_ratio),
            display(summary.values.min),
            display(summary.values.mean),
            display(summary.values.max),
            display(metric.latest_value),
        ));
    }
    output.push('\n');
    output.push_str("## Sampling and overhead\n\n");
    output.push_str(&format!(
        "- Wall duration: {} ms\n- Expected/executed/dropped: {}/{}/{}\n- Logical source polls: {}\n- Late wakeups: {}\n- Average probe CPU share: {}%\n- P95 probe CPU share: {}%\n- Peak working set: {} bytes\n- Peak handles: {}\n- Peak threads: {}\n\n",
        report.sampling.wall_duration_ms,
        report.sampling.expected_samples,
        report.sampling.executed_samples,
        report.sampling.dropped_samples,
        report.sampling.logical_source_poll_count,
        report.sampling.late_wakeups,
        display(report.self_resource_summary.probe_cpu_share_percent.as_ref().and_then(|summary| summary.average)),
        display(report.self_resource_summary.probe_cpu_share_percent.as_ref().and_then(|summary| summary.values.p95)),
        display(report.self_resource_summary.working_set_bytes.as_ref().and_then(|summary| summary.peak)),
        display(report.self_resource_summary.handle_count.as_ref().and_then(|summary| summary.peak)),
        display(report.self_resource_summary.thread_count.as_ref().and_then(|summary| summary.peak)),
    ));
    output.push_str("## Interpretation\n\n");
    output.push_str("The `reference.*` metrics are intentionally not production metric keys. An existing Afterburner mapping can be used for trend comparison only; the probe never starts it or installs its driver. The Windows built-ins are recorded with their native semantics and are not relabeled as CPU package temperature, package power, or effective average frequency.\n\n");
    output.push_str("## Deferred\n\n");
    for item in &report.deferred {
        output.push_str(&format!(
            "- **{}**: {} ({})\n",
            item.item, item.status, item.reason
        ));
    }
    output.push('\n');
    output.push_str("## Re-run\n\n");
    for command in &report.rerun_commands {
        output.push_str(&format!("```powershell\n{command}\n```\n\n"));
    }
    output
}

fn render_lifecycle_markdown(report: &CpuSensorLifecycleReport) -> String {
    let mut output = String::new();
    output.push_str("# CPU sensor lifecycle probe\n\n");
    output.push_str(&format!("- Schema: `{}`\n", report.schema_version));
    output.push_str(&format!(
        "- Machine CPU: {}\n",
        report.machine.cpu_model.as_deref().unwrap_or("unknown")
    ));
    output.push_str(&format!(
        "- Enable → disable → re-enable: {}\n",
        report.enable_disable_reenable
    ));
    output.push_str(&format!(
        "- Cleanup completed: {}\n",
        report.cleanup_completed
    ));
    output.push_str(&format!("- Sleep/resume: {}\n\n", report.sleep_resume));
    output.push_str("## Phases\n\n| Phase | Enabled | Scheduler ticks | Successful samples | Source poll delta | No source polling while disabled |\n|---|---:|---:|---:|---:|---:|\n");
    for phase in &report.phases {
        output.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} |\n",
            phase.phase,
            phase.enabled,
            phase.scheduler_tick_count,
            phase.successful_sample_count,
            phase.source_poll_count_delta,
            phase.no_source_polling_observed,
        ));
    }
    output.push('\n');
    output.push_str(&format!(
        "## Resources\n\n- Baseline handles: {}\n- Final handles: {}\n- Handle delta: {}\n- Baseline working set: {}\n- Final working set: {}\n- Working-set delta: {}\n\n",
        display(report.resources.baseline.as_ref().map(|value| value.handles as f64)),
        display(report.resources.final_snapshot.as_ref().map(|value| value.handles as f64)),
        display(report.resources.handle_delta.map(|value| value as f64)),
        display(report.resources.baseline.as_ref().map(|value| value.working_set_bytes as f64)),
        display(report.resources.final_snapshot.as_ref().map(|value| value.working_set_bytes as f64)),
        display(report.resources.working_set_delta_bytes.map(|value| value as f64)),
    ));
    output.push_str("## Isolation\n\n");
    output.push_str(&format!("{}\n\n", report.failure_isolation));
    output
}

fn privacy_summary() -> PrivacySummary {
    PrivacySummary {
        sanitized: true,
        omitted_fields: vec![
            "account identity".to_string(),
            "host identity".to_string(),
            "hardware unique identifiers".to_string(),
            "network address identifiers".to_string(),
            "absolute paths".to_string(),
            "process identities and command lines".to_string(),
        ],
        process_detail_retention: "no process detail retained".to_string(),
    }
}

fn status_name(status: SupportStatus) -> &'static str {
    match status {
        SupportStatus::Supported => "supported",
        SupportStatus::Unsupported => "unsupported",
        SupportStatus::PermissionDenied => "permission_denied",
        SupportStatus::ProviderMissing => "provider_missing",
        SupportStatus::ProbeFailed => "probe_failed",
        SupportStatus::RuntimeFailed => "runtime_failed",
        SupportStatus::Disabled => "disabled",
    }
}

fn display(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "n/a".to_string())
}

#[cfg(windows)]
fn map_status(status: crate::windows::ReadStatus) -> SupportStatus {
    match status {
        crate::windows::ReadStatus::Value => SupportStatus::Supported,
        crate::windows::ReadStatus::Unsupported => SupportStatus::Unsupported,
        crate::windows::ReadStatus::PermissionDenied => SupportStatus::PermissionDenied,
        crate::windows::ReadStatus::ProviderMissing => SupportStatus::ProviderMissing,
        crate::windows::ReadStatus::Failed => SupportStatus::ProbeFailed,
        crate::windows::ReadStatus::RuntimeFailed => SupportStatus::RuntimeFailed,
    }
}

fn due_count(now: Instant, next: Instant, interval: Duration) -> u64 {
    if now < next {
        0
    } else {
        now.duration_since(next).as_millis() as u64 / interval.as_millis().max(1) as u64 + 1
    }
}

fn expected_samples(duration_seconds: u64, interval_ms: u64) -> u64 {
    duration_seconds
        .saturating_mul(1000)
        .saturating_sub(1)
        .checked_div(interval_ms.max(1))
        .unwrap_or(0)
        .saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::{expected_samples, metric_definitions, metric_summary};

    #[test]
    fn expected_samples_matches_poll_schedule() {
        assert_eq!(expected_samples(5, 500), 10);
        assert_eq!(expected_samples(5, 1_000), 5);
        assert_eq!(expected_samples(5, 2_500), 2);
        assert_eq!(expected_samples(5, 5_000), 1);
    }

    #[test]
    fn reference_metric_names_are_not_production_package_names() {
        let metrics = metric_definitions();
        assert!(metrics.contains_key("reference.cpu_temperature_celsius"));
        assert!(!metrics.contains_key("cpu.package_temperature_celsius"));
    }

    #[test]
    fn repeated_values_are_visible_in_summary() {
        let mut metric = metric_definitions()
            .remove("cpu.processor_frequency_mhz")
            .unwrap();
        metric.record_success(1, 100.0, None, 0, 0.1);
        metric.record_success(2, 100.0, Some(1), 1, 0.1);
        metric.record_success(3, 105.0, Some(1), 1, 0.1);
        let summary = metric_summary(&metric);
        assert_eq!(summary.unique_value_count, 2);
        assert_eq!(summary.repeated_sample_count, 1);
    }
}
