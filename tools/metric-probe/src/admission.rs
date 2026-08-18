use crate::{
    cli::{LifecycleConfig, ScenarioConfig},
    model::{MachineInfo, MetricRecord, SupportStatus},
    report::validate_public_text,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

const LIFECYCLE_SCHEMA: &str = "spike-01b-admission-lifecycle/v1";
const SCENARIO_SCHEMA: &str = "spike-01b-admission-scenarios/v1";

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleReport {
    pub schema_version: String,
    pub generated_at_utc: String,
    pub machine: MachineInfo,
    pub configuration: LifecycleConfiguration,
    pub phases: Vec<LifecyclePhase>,
    pub resources: ResourceSummary,
    pub final_cleanup_completed: bool,
    pub input_responsiveness_observation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleConfiguration {
    pub enabled_duration_ms: u64,
    pub disabled_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifecyclePhase {
    pub phase: String,
    pub started_at_utc: String,
    pub finished_at_utc: String,
    pub duration_ms: u64,
    pub initialization_status: SupportStatus,
    pub initialization_reason_code: String,
    pub shutdown_status: SupportStatus,
    pub shutdown_reason_code: String,
    pub sample_count: u64,
    pub failed_sample_count: u64,
    pub gpu_metric_call_count: u64,
    pub failed_gpu_metric_call_count: u64,
    pub nvml_calls_performed: bool,
    pub library_released: bool,
    pub session_resources_released: bool,
    pub resource_start: Option<ResourceSnapshot>,
    pub resource_end: Option<ResourceSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceSnapshot {
    pub threads: u32,
    pub handles: u32,
    pub working_set_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceSummary {
    pub baseline: Option<ResourceSnapshot>,
    pub peak_threads: Option<u32>,
    pub peak_handles: Option<u32>,
    pub peak_working_set_bytes: Option<u64>,
    pub final_snapshot: Option<ResourceSnapshot>,
    pub thread_delta: Option<i64>,
    pub handle_delta: Option<i64>,
    pub working_set_delta_bytes: Option<i64>,
    pub monotonic_thread_growth_observed: bool,
    pub monotonic_handle_growth_observed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioReport {
    pub schema_version: String,
    pub generated_at_utc: String,
    pub machine: MachineInfo,
    pub status_catalog: Vec<String>,
    pub scenarios: Vec<ScenarioResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioResult {
    pub scenario: String,
    pub injection: String,
    pub capability_status: SupportStatus,
    pub capability_reason_code: String,
    pub sample_count: u64,
    pub failed_sample_count: u64,
    pub gpu_metric_call_count: u64,
    pub failed_gpu_metric_call_count: u64,
    pub metrics: Vec<ScenarioMetric>,
    pub unsupported_metrics_have_no_numeric_zero: bool,
    pub one_metric_failure_did_not_disable_device: bool,
    pub transient_failure_recovered: bool,
    pub other_probe_categories_unaffected: bool,
    pub cleanup_completed: bool,
    pub library_released: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScenarioMetric {
    pub metric_key: String,
    pub support_status: SupportStatus,
    pub reason_code: String,
    pub sample_count: usize,
    pub failed_sample_count: usize,
    pub latest_value: Option<f64>,
    pub failure_reasons: BTreeMap<String, usize>,
}

pub fn run_lifecycle(config: LifecycleConfig) -> Result<(PathBuf, PathBuf), String> {
    #[cfg(windows)]
    {
        run_lifecycle_windows(config)
    }
    #[cfg(not(windows))]
    {
        let _ = config;
        Err("NVML admission lifecycle is only executable on Windows".to_string())
    }
}

pub fn run_scenarios(config: ScenarioConfig) -> Result<(PathBuf, PathBuf), String> {
    #[cfg(windows)]
    {
        run_scenarios_windows(config)
    }
    #[cfg(not(windows))]
    {
        let _ = config;
        Err("NVML injection scenarios are only executable on Windows".to_string())
    }
}

#[cfg(windows)]
fn run_lifecycle_windows(config: LifecycleConfig) -> Result<(PathBuf, PathBuf), String> {
    use crate::windows;

    let machine = windows::machine_info();
    let phases = vec![
        run_active_phase("enabled", config.enabled_duration_ms),
        run_disabled_phase(config.disabled_duration_ms),
        run_active_phase("re_enabled", config.enabled_duration_ms),
    ];
    let resources = summarize_resources(&phases);
    let final_cleanup_completed = phases
        .iter()
        .filter(|phase| phase.phase != "disabled")
        .all(|phase| phase.session_resources_released)
        && phases
            .iter()
            .find(|phase| phase.phase == "disabled")
            .map(|phase| !phase.nvml_calls_performed)
            .unwrap_or(false);
    let report = LifecycleReport {
        schema_version: LIFECYCLE_SCHEMA.to_string(),
        generated_at_utc: crate::utc_now_string(),
        machine,
        configuration: LifecycleConfiguration {
            enabled_duration_ms: config.enabled_duration_ms,
            disabled_duration_ms: config.disabled_duration_ms,
        },
        phases,
        resources,
        final_cleanup_completed,
        input_responsiveness_observation:
            "not instrumented by the probe; record visible input behavior separately and make no causal claim"
                .to_string(),
    };
    write_reports(
        &config.output_dir,
        "lifecycle",
        &report,
        render_lifecycle_markdown(&report),
    )
}

#[cfg(windows)]
fn run_active_phase(phase: &str, duration_ms: u64) -> LifecyclePhase {
    use crate::{
        nvml::NvmlProvider,
        windows::{ReadResult, ReadStatus},
    };

    let started_at_utc = crate::utc_now_string();
    let started = Instant::now();
    let resource_start = resource_snapshot();
    let result = NvmlProvider::new();
    let (
        initialization_status,
        initialization_reason_code,
        shutdown_status,
        shutdown_reason_code,
        sample_count,
        failed_sample_count,
        gpu_metric_call_count,
        failed_gpu_metric_call_count,
        library_released,
        session_resources_released,
    ) = match result {
        ReadResult {
            status: ReadStatus::Value,
            reason_code,
            value: Some(provider),
        } => {
            let sample_started = Instant::now();
            while sample_started.elapsed() < Duration::from_millis(duration_ms) {
                provider.sample_all();
                thread::sleep(Duration::from_millis(100));
            }
            let stop = provider.shutdown();
            let shutdown_status = map_read_status(stop.status);
            let library_released = stop.stats.library_release_count > 0;
            (
                SupportStatus::Supported,
                reason_code,
                shutdown_status,
                stop.reason_code,
                stop.stats.sample_count,
                stop.stats.failed_sample_count,
                stop.stats.gpu_metric_call_count,
                stop.stats.failed_gpu_metric_call_count,
                library_released,
                shutdown_status == SupportStatus::Supported
                    && library_released
                    && stop.stats.shutdown_successes > 0,
            )
        }
        ReadResult {
            status,
            reason_code,
            ..
        } => (
            map_read_status(status),
            reason_code,
            SupportStatus::Disabled,
            "not_initialized".to_string(),
            0,
            0,
            0,
            0,
            true,
            true,
        ),
    };

    let resource_end = resource_snapshot();
    LifecyclePhase {
        phase: phase.to_string(),
        started_at_utc,
        finished_at_utc: crate::utc_now_string(),
        duration_ms: started.elapsed().as_millis() as u64,
        initialization_status,
        initialization_reason_code,
        shutdown_status,
        shutdown_reason_code,
        sample_count,
        failed_sample_count,
        gpu_metric_call_count,
        failed_gpu_metric_call_count,
        nvml_calls_performed: gpu_metric_call_count > 0,
        library_released,
        session_resources_released,
        resource_start,
        resource_end,
    }
}

#[cfg(windows)]
fn run_disabled_phase(duration_ms: u64) -> LifecyclePhase {
    let started_at_utc = crate::utc_now_string();
    let started = Instant::now();
    let resource_start = resource_snapshot();
    thread::sleep(Duration::from_millis(duration_ms));
    LifecyclePhase {
        phase: "disabled".to_string(),
        started_at_utc,
        finished_at_utc: crate::utc_now_string(),
        duration_ms: started.elapsed().as_millis() as u64,
        initialization_status: SupportStatus::Disabled,
        initialization_reason_code: "gpu_probe_disabled".to_string(),
        shutdown_status: SupportStatus::Disabled,
        shutdown_reason_code: "no_nvml_session_created".to_string(),
        sample_count: 0,
        failed_sample_count: 0,
        gpu_metric_call_count: 0,
        failed_gpu_metric_call_count: 0,
        nvml_calls_performed: false,
        library_released: true,
        session_resources_released: true,
        resource_start,
        resource_end: resource_snapshot(),
    }
}

#[cfg(windows)]
fn run_scenarios_windows(config: ScenarioConfig) -> Result<(PathBuf, PathBuf), String> {
    use crate::{nvml::NvmlInjection, windows};

    let scenarios = [
        (
            "missing_dll",
            "missing_library",
            NvmlInjection::MissingLibrary,
        ),
        (
            "partial_unsupported",
            "selected_metric_not_supported",
            NvmlInjection::PartialUnsupported,
        ),
        (
            "transient_metric_failure",
            "power_timeout_then_success",
            NvmlInjection::TransientMetricFailure,
        ),
        (
            "provider_runtime_failure",
            "gpu_lost_during_initialization",
            NvmlInjection::ProviderRuntimeFailure,
        ),
    ];
    let report = ScenarioReport {
        schema_version: SCENARIO_SCHEMA.to_string(),
        generated_at_utc: crate::utc_now_string(),
        machine: windows::machine_info(),
        status_catalog: vec![
            "supported".to_string(),
            "unsupported".to_string(),
            "permission_denied".to_string(),
            "provider_missing".to_string(),
            "probe_failed".to_string(),
            "runtime_failed".to_string(),
            "disabled".to_string(),
        ],
        scenarios: scenarios
            .into_iter()
            .map(|(name, injection_name, injection)| {
                run_scenario(name, injection_name, injection, config.sample_count)
            })
            .collect(),
    };
    write_reports(
        &config.output_dir,
        "scenarios",
        &report,
        render_scenario_markdown(&report),
    )
}

#[cfg(windows)]
fn run_scenario(
    scenario: &str,
    injection_name: &str,
    injection: crate::nvml::NvmlInjection,
    sample_count: u64,
) -> ScenarioResult {
    use crate::{nvml::NvmlProvider, windows::ReadResult};

    let other_categories_before = other_probe_category_statuses();
    let result = NvmlProvider::new_injected(injection);
    let ReadResult {
        status,
        reason_code,
        value,
    } = result;
    let Some(provider) = value else {
        return ScenarioResult {
            scenario: scenario.to_string(),
            injection: injection_name.to_string(),
            capability_status: map_read_status(status),
            capability_reason_code: reason_code,
            sample_count: 0,
            failed_sample_count: 0,
            gpu_metric_call_count: 0,
            failed_gpu_metric_call_count: 0,
            metrics: Vec::new(),
            unsupported_metrics_have_no_numeric_zero: true,
            one_metric_failure_did_not_disable_device: true,
            transient_failure_recovered: false,
            other_probe_categories_unaffected: other_probe_categories_are_unaffected(
                other_categories_before,
                other_probe_category_statuses(),
            ),
            cleanup_completed: true,
            library_released: true,
        };
    };

    let mut metrics = metric_records();
    for index in 0..sample_count {
        for sample in provider.sample_all() {
            let timestamp_ms = crate::unix_now_ms() + index as i64;
            record_metric(
                metrics
                    .get_mut("gpu.utilization_percent")
                    .expect("metric exists"),
                sample.utilization_percent,
                timestamp_ms,
            );
            record_metric(
                metrics
                    .get_mut("gpu.memory_controller_utilization_percent")
                    .expect("metric exists"),
                sample.memory_controller_utilization_percent,
                timestamp_ms,
            );
            record_metric(
                metrics
                    .get_mut("gpu.temperature_celsius")
                    .expect("metric exists"),
                sample.temperature_celsius,
                timestamp_ms,
            );
            record_metric(
                metrics.get_mut("gpu.power_watts").expect("metric exists"),
                sample.power_watts,
                timestamp_ms,
            );
            record_metric(
                metrics
                    .get_mut("gpu.graphics_clock_mhz")
                    .expect("metric exists"),
                sample.graphics_clock_mhz,
                timestamp_ms,
            );
            record_metric(
                metrics
                    .get_mut("gpu.memory_clock_mhz")
                    .expect("metric exists"),
                sample.memory_clock_mhz,
                timestamp_ms,
            );
            record_metric(
                metrics
                    .get_mut("gpu.vram_used_bytes")
                    .expect("metric exists"),
                sample.vram_used_bytes,
                timestamp_ms,
            );
            record_metric(
                metrics
                    .get_mut("gpu.vram_total_bytes")
                    .expect("metric exists"),
                sample.vram_total_bytes,
                timestamp_ms,
            );
        }
    }
    let stop = provider.shutdown();
    let stats = stop.stats;
    let mut metrics: Vec<_> = metrics
        .into_values()
        .map(|mut metric| {
            metric.finalize();
            ScenarioMetric {
                metric_key: metric.metric_key,
                support_status: metric.support_status,
                reason_code: metric.reason_code,
                sample_count: metric.sample_count,
                failed_sample_count: metric.failed_sample_count,
                latest_value: metric.latest_value,
                failure_reasons: metric.failure_reasons,
            }
        })
        .collect();
    metrics.sort_by(|left, right| left.metric_key.cmp(&right.metric_key));
    let unsupported_metrics_have_no_numeric_zero = metrics
        .iter()
        .filter(|metric| metric.support_status == SupportStatus::Unsupported)
        .all(|metric| metric.latest_value.is_none());
    let one_metric_failure_did_not_disable_device = metrics
        .iter()
        .find(|metric| metric.metric_key == "gpu.utilization_percent")
        .map(|metric| metric.sample_count == sample_count as usize)
        .unwrap_or(false);
    let transient_failure_recovered = scenario == "transient_metric_failure"
        && metrics
            .iter()
            .find(|metric| metric.metric_key == "gpu.power_watts")
            .map(|metric| {
                metric.support_status == SupportStatus::Supported
                    && metric.sample_count == sample_count.saturating_sub(1) as usize
                    && metric.failed_sample_count == 1
                    && metric.failure_reasons.contains_key("nvml_timeout")
            })
            .unwrap_or(false);
    let other_probe_categories_unaffected = other_probe_categories_are_unaffected(
        other_categories_before,
        other_probe_category_statuses(),
    );
    ScenarioResult {
        scenario: scenario.to_string(),
        injection: injection_name.to_string(),
        capability_status: SupportStatus::Supported,
        capability_reason_code: "ok".to_string(),
        sample_count: stats.sample_count,
        failed_sample_count: stats.failed_sample_count,
        gpu_metric_call_count: stats.gpu_metric_call_count,
        failed_gpu_metric_call_count: stats.failed_gpu_metric_call_count,
        metrics,
        unsupported_metrics_have_no_numeric_zero,
        one_metric_failure_did_not_disable_device,
        transient_failure_recovered,
        other_probe_categories_unaffected,
        cleanup_completed: stop.status == crate::windows::ReadStatus::Value
            && stats.shutdown_successes == 1
            && stats.library_release_count == 1,
        library_released: stats.library_release_count == 1,
    }
}

#[cfg(windows)]
fn other_probe_category_statuses() -> (crate::windows::ReadStatus, crate::windows::ReadStatus) {
    (
        crate::windows::cpu_times().status,
        crate::windows::memory_info().status,
    )
}

#[cfg(windows)]
fn other_probe_categories_are_unaffected(
    before: (crate::windows::ReadStatus, crate::windows::ReadStatus),
    after: (crate::windows::ReadStatus, crate::windows::ReadStatus),
) -> bool {
    before == after
        && matches!(before.0, crate::windows::ReadStatus::Value)
        && matches!(before.1, crate::windows::ReadStatus::Value)
}

#[cfg(windows)]
fn metric_records() -> BTreeMap<String, MetricRecord> {
    [
        ("gpu.utilization_percent", "percent", "0..100", None),
        (
            "gpu.memory_controller_utilization_percent",
            "percent",
            "0..100",
            None,
        ),
        (
            "gpu.temperature_celsius",
            "C",
            "driver-defined non-negative Celsius value",
            None,
        ),
        (
            "gpu.power_watts",
            "W",
            "driver-defined non-negative board power",
            Some("gpu_board"),
        ),
        (
            "gpu.graphics_clock_mhz",
            "MHz",
            "driver-defined non-negative clock",
            None,
        ),
        (
            "gpu.memory_clock_mhz",
            "MHz",
            "driver-defined non-negative clock",
            None,
        ),
        (
            "gpu.vram_used_bytes",
            "bytes",
            "0..gpu.vram_total_bytes",
            None,
        ),
        (
            "gpu.vram_total_bytes",
            "bytes",
            "non-negative device memory capacity",
            None,
        ),
    ]
    .into_iter()
    .map(|(metric_key, unit, value_range, power_scope)| {
        let mut metric = MetricRecord::new(
            "gpu:nvidia:index-0",
            metric_key,
            "nvidia-nvml",
            SupportStatus::Supported,
            "ready",
            unit,
            "NVIDIA NVML injected dispatch",
            vec!["probe-only deterministic injection".to_string()],
        )
        .with_value_range(value_range);
        if let Some(power_scope) = power_scope {
            metric = metric.with_power_scope(power_scope);
        }
        (metric_key.to_string(), metric)
    })
    .collect()
}

#[cfg(windows)]
fn record_metric(metric: &mut MetricRecord, timed: crate::nvml::TimedRead<f64>, timestamp_ms: i64) {
    let latency = timed.latency_ms;
    match timed.result.value {
        Some(value) => metric.record_success(timestamp_ms, value, Some(100), 100, latency),
        None => metric.record_failure_with_status(
            map_read_status(timed.result.status),
            timed.result.reason_code,
            latency,
        ),
    }
}

#[cfg(windows)]
fn resource_snapshot() -> Option<ResourceSnapshot> {
    crate::windows::self_metrics()
        .value
        .map(|value| ResourceSnapshot {
            threads: value.thread_count,
            handles: value.handle_count,
            working_set_bytes: value.working_set_bytes,
        })
}

#[cfg(windows)]
fn map_read_status(status: crate::windows::ReadStatus) -> SupportStatus {
    match status {
        crate::windows::ReadStatus::Value => SupportStatus::Supported,
        crate::windows::ReadStatus::Unsupported => SupportStatus::Unsupported,
        crate::windows::ReadStatus::PermissionDenied => SupportStatus::PermissionDenied,
        crate::windows::ReadStatus::ProviderMissing => SupportStatus::ProviderMissing,
        crate::windows::ReadStatus::Failed => SupportStatus::ProbeFailed,
        crate::windows::ReadStatus::RuntimeFailed => SupportStatus::RuntimeFailed,
    }
}

#[cfg(windows)]
fn summarize_resources(phases: &[LifecyclePhase]) -> ResourceSummary {
    let snapshots = phases
        .iter()
        .flat_map(|phase| [phase.resource_start.clone(), phase.resource_end.clone()])
        .flatten()
        .collect::<Vec<_>>();
    let baseline = snapshots.first().cloned();
    let final_snapshot = snapshots.last().cloned();
    let peak_threads = snapshots.iter().map(|value| value.threads).max();
    let peak_handles = snapshots.iter().map(|value| value.handles).max();
    let peak_working_set_bytes = snapshots.iter().map(|value| value.working_set_bytes).max();
    let thread_delta = baseline
        .as_ref()
        .zip(final_snapshot.as_ref())
        .map(|(start, end)| end.threads as i64 - start.threads as i64);
    let handle_delta = baseline
        .as_ref()
        .zip(final_snapshot.as_ref())
        .map(|(start, end)| end.handles as i64 - start.handles as i64);
    let working_set_delta_bytes = baseline
        .as_ref()
        .zip(final_snapshot.as_ref())
        .map(|(start, end)| end.working_set_bytes as i64 - start.working_set_bytes as i64);
    ResourceSummary {
        baseline,
        peak_threads,
        peak_handles,
        peak_working_set_bytes,
        final_snapshot,
        thread_delta,
        handle_delta,
        working_set_delta_bytes,
        monotonic_thread_growth_observed: snapshots
            .windows(2)
            .all(|values| values[1].threads >= values[0].threads),
        monotonic_handle_growth_observed: snapshots
            .windows(2)
            .all(|values| values[1].handles >= values[0].handles),
    }
}

fn write_reports<T: Serialize>(
    output_dir: &Path,
    stem: &str,
    report: &T,
    markdown: String,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("create admission output directory failed: {error}"))?;
    let json = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("serialize admission JSON failed: {error}"))?;
    validate_public_text(std::str::from_utf8(&json).unwrap_or_default())?;
    validate_public_text(&markdown)?;
    let json_path = output_dir.join(format!("{stem}.json"));
    let markdown_path = output_dir.join(format!("{stem}.md"));
    fs::write(&json_path, json).map_err(|error| format!("write admission JSON failed: {error}"))?;
    fs::write(&markdown_path, markdown)
        .map_err(|error| format!("write admission Markdown failed: {error}"))?;
    Ok((json_path, markdown_path))
}

fn render_lifecycle_markdown(report: &LifecycleReport) -> String {
    let mut output = format!(
        "# Spike-01B NVML Lifecycle Evidence\n\n- Schema: `{}`\n- Generated: `{}`\n- Elevated: `{}`\n\n",
        report.schema_version,
        report.generated_at_utc,
        report.machine.elevated.unwrap_or(false)
    );
    output.push_str("## Phases\n\n| Phase | Init | Shutdown | Samples | Failed samples | GPU calls | Failed calls | Library released | Session released |\n|---|---|---|---:|---:|---:|---:|---:|---:|\n");
    for phase in &report.phases {
        output.push_str(&format!(
            "| `{}` | `{}` ({}) | `{}` ({}) | {} | {} | {} | {} | {} | {} |\n",
            phase.phase,
            status_name(phase.initialization_status),
            phase.initialization_reason_code,
            status_name(phase.shutdown_status),
            phase.shutdown_reason_code,
            phase.sample_count,
            phase.failed_sample_count,
            phase.gpu_metric_call_count,
            phase.failed_gpu_metric_call_count,
            phase.library_released,
            phase.session_resources_released
        ));
    }
    output.push_str(&format!(
        "\nFinal cleanup completed: `{}`\n\nInput responsiveness observation: {}\n",
        report.final_cleanup_completed, report.input_responsiveness_observation
    ));
    output
}

fn render_scenario_markdown(report: &ScenarioReport) -> String {
    let mut output = format!(
        "# Spike-01B NVML Deterministic Scenario Evidence\n\n- Schema: `{}`\n- Generated: `{}`\n- Status catalog: `{}`\n\n",
        report.schema_version,
        report.generated_at_utc,
        report.status_catalog.join(", ")
    );
    for scenario in &report.scenarios {
        output.push_str(&format!(
            "## {}\n\n- Injection: `{}`\n- Capability: `{}` (`{}`)\n- Samples: {}\n- Failed samples: {}\n- GPU calls: {}\n- Failed GPU calls: {}\n- Unsupported values omitted: `{}`\n- One metric failure isolated: `{}`\n- Transient recovery: `{}`\n- Other categories unaffected: `{}`\n- Cleanup completed: `{}`\n- Library released: `{}`\n\n",
            scenario.scenario,
            scenario.injection,
            status_name(scenario.capability_status),
            scenario.capability_reason_code,
            scenario.sample_count,
            scenario.failed_sample_count,
            scenario.gpu_metric_call_count,
            scenario.failed_gpu_metric_call_count,
            scenario.unsupported_metrics_have_no_numeric_zero,
            scenario.one_metric_failure_did_not_disable_device,
            scenario.transient_failure_recovered,
            scenario.other_probe_categories_unaffected,
            scenario.cleanup_completed,
            scenario.library_released
        ));
        output.push_str("| Metric | Status | Reason | Samples | Failed | Latest | Failure reasons |\n|---|---|---|---:|---:|---:|---|\n");
        for metric in &scenario.metrics {
            let reasons = metric
                .failure_reasons
                .iter()
                .map(|(reason, count)| format!("{reason}={count}"))
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | {} | {} | {} |\n",
                metric.metric_key,
                status_name(metric.support_status),
                metric.reason_code,
                metric.sample_count,
                metric.failed_sample_count,
                metric
                    .latest_value
                    .map(|value| format!("{value:.3}"))
                    .unwrap_or_else(|| "absent".to_string()),
                if reasons.is_empty() { "none" } else { &reasons }
            ));
        }
        output.push('\n');
    }
    output
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
