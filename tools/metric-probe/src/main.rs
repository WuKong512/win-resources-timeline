mod admission;
mod cli;
mod cpu_sensor;
mod model;
mod report;
mod stats;

#[cfg(windows)]
mod nvml;

#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
mod windows {
    use crate::model::{Capability, DeviceInfo, MachineInfo};

    pub fn machine_info() -> MachineInfo {
        MachineInfo {
            os_name: std::env::consts::OS.to_string(),
            os_display_version: None,
            os_build: None,
            architecture: std::env::consts::ARCH.to_string(),
            cpu_model: None,
            logical_processor_count: std::thread::available_parallelism()
                .ok()
                .map(|value| value.get() as u32),
            memory_total_bytes: None,
            elevated: None,
        }
    }

    pub fn inventory() -> (Vec<DeviceInfo>, Vec<Capability>) {
        (Vec::new(), Vec::new())
    }
}

use cli::{parse_args, Command, RunConfig};
use model::{
    BudgetComparison, Capability, Conclusion, DeferredItem, DeviceInfo, InventoryReport,
    MachineInfo, MetricRecord, PrivacySummary, ProbeReport, SamplingSummary, SelfMetricSummary,
    SelfResourceSummary, SupportStatus, TestConfiguration, SCHEMA_VERSION,
};
use report::{render_probe_markdown, write_inventory_json, write_probe_report};
use stats::Distribution;
use std::{
    collections::{BTreeMap, HashMap},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn main() {
    if let Err(error) = run_main() {
        eprintln!("metric-probe: {error}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    match parse_args(cli::args())? {
        Command::Inventory => run_inventory(),
        Command::Run(config) => run_probe(config),
        Command::Lifecycle(config) => {
            let (json_path, markdown_path) = admission::run_lifecycle(config)?;
            println!("JSON: {}", json_path.display());
            println!("Markdown: {}", markdown_path.display());
            Ok(())
        }
        Command::Scenarios(config) => {
            let (json_path, markdown_path) = admission::run_scenarios(config)?;
            println!("JSON: {}", json_path.display());
            println!("Markdown: {}", markdown_path.display());
            Ok(())
        }
        Command::CpuSensors(config) => {
            let (json_path, markdown_path) = cpu_sensor::run(config)?;
            println!("JSON: {}", json_path.display());
            println!("Markdown: {}", markdown_path.display());
            Ok(())
        }
        Command::CpuSensorLifecycle(config) => {
            let (json_path, markdown_path) = cpu_sensor::run_lifecycle(config)?;
            println!("JSON: {}", json_path.display());
            println!("Markdown: {}", markdown_path.display());
            Ok(())
        }
    }
}

fn run_inventory() -> Result<(), String> {
    let machine = windows::machine_info();
    let (devices, capabilities) = windows::inventory();
    let report = InventoryReport {
        schema_version: SCHEMA_VERSION.to_string(),
        probe_name: "metric-probe".to_string(),
        generated_at_utc: utc_now_string(),
        machine,
        devices,
        capabilities,
        privacy: privacy_summary(),
    };
    println!("{}", write_inventory_json(&report)?);
    Ok(())
}

fn run_probe(config: RunConfig) -> Result<(), String> {
    let session = ProbeSession::new(config.clone());
    let report = session.run()?;
    let (json_path, markdown_path) = write_probe_report(&config.output_dir, &report)?;
    println!("JSON: {}", json_path.display());
    println!("Markdown: {}", markdown_path.display());
    println!("Scope: {}", report.conclusion.scope);
    Ok(())
}

struct ProbeSession {
    config: RunConfig,
    machine: MachineInfo,
    devices: Vec<DeviceInfo>,
    capabilities: Vec<Capability>,
    metrics: BTreeMap<String, MetricRecord>,
    self_samples: SelfSamples,
    network_previous: HashMap<String, NetworkPrevious>,
    disk_provider: Option<windows::DiskProvider>,
    disk_init_status: Option<(windows::ReadStatus, String)>,
    gpu_provider: Option<windows::NvmlProvider>,
    gpu_init_status: Option<(windows::ReadStatus, String)>,
    started_at: String,
}

#[derive(Debug, Clone, Copy)]
struct NetworkPrevious {
    timestamp: Instant,
    in_octets: u64,
    out_octets: u64,
}

#[derive(Debug, Default)]
struct SelfSamples {
    cpu_time_100ns: Vec<f64>,
    working_set_bytes: Vec<f64>,
    thread_count: Vec<f64>,
    handle_count: Vec<f64>,
    probe_cpu_share_percent: Vec<f64>,
}

impl ProbeSession {
    fn new(config: RunConfig) -> Self {
        let machine = windows::machine_info();
        let (gpu_provider, gpu_init_status) = if config.gpu_probe {
            match windows::NvmlProvider::new() {
                windows::ReadResult {
                    status: windows::ReadStatus::Value,
                    value: Some(provider),
                    ..
                } => (Some(provider), None),
                result => (None, Some((result.status, result.reason_code))),
            }
        } else {
            (None, None)
        };
        let (mut devices, mut capabilities) = windows::inventory_with_options(
            config.disk_probe,
            config.network_probe,
            config.power_probe,
            config.process_probe,
        );
        if config.gpu_probe {
            windows::append_nvidia_inventory(
                &mut devices,
                &mut capabilities,
                gpu_provider.as_ref(),
                gpu_init_status.as_ref(),
            );
        }
        let (disk_provider, disk_init_status) = if config.disk_probe {
            match windows::DiskProvider::new() {
                windows::ReadResult {
                    status: windows::ReadStatus::Value,
                    value: Some(provider),
                    ..
                } => (Some(provider), None),
                result => (None, Some((result.status, result.reason_code))),
            }
        } else {
            (None, None)
        };
        let mut session = Self {
            config,
            machine,
            devices,
            capabilities,
            metrics: BTreeMap::new(),
            self_samples: SelfSamples::default(),
            network_previous: HashMap::new(),
            disk_provider,
            disk_init_status,
            gpu_provider,
            gpu_init_status,
            started_at: utc_now_string(),
        };
        session.initialize_metric_records();
        if let Some((status, reason_code)) = session.disk_init_status.take() {
            for key in ["disk.read_bytes_per_sec", "disk.write_bytes_per_sec"] {
                session.record_skip("disk:physical-total", key, status, reason_code.clone(), 0.0);
            }
        }
        session
    }

    fn initialize_metric_records(&mut self) {
        self.add_metric(
            "cpu:system",
            "cpu.usage_percent",
            "win32-system-times",
            "percent",
            "Windows GetSystemTimes; interval usage derived from kernel/user/idle deltas",
        );
        self.add_metric(
            "cpu:system",
            "cpu.frequency_current_mhz",
            "nt-power-processor-information",
            "MHz",
            "OS-exposed processor information; may be a policy/current-state hint rather than a continuously valid real-time frequency",
        );
        self.add_metric(
            "cpu:system",
            "cpu.frequency_max_mhz",
            "nt-power-processor-information",
            "MHz",
            "OS-exposed maximum/policy frequency information, not a measured instantaneous clock",
        );
        self.add_metric(
            "memory:physical",
            "memory.used_bytes",
            "global-memory-status-ex",
            "bytes",
            "Windows-reported total physical memory minus available physical memory",
        );
        self.add_metric(
            "memory:physical",
            "memory.available_bytes",
            "global-memory-status-ex",
            "bytes",
            "Windows-reported available physical memory",
        );
        self.add_metric(
            "memory:physical",
            "memory.usage_percent",
            "global-memory-status-ex",
            "percent",
            "Used physical memory divided by total physical memory",
        );
        self.add_metric(
            "system:uptime",
            "system.uptime_ms",
            "get-tick-count-64",
            "ms",
            "Monotonic milliseconds since Windows boot; not a UTC boot timestamp",
        );
        self.add_metric(
            "probe:current-process",
            "probe.cpu_time_100ns",
            "current-process-win32",
            "100ns",
            "This probe process CPU time only",
        );
        self.add_metric(
            "probe:current-process",
            "probe.working_set_bytes",
            "current-process-win32",
            "bytes",
            "This probe process working set only",
        );
        self.add_metric(
            "probe:current-process",
            "probe.thread_count",
            "current-process-win32",
            "count",
            "This probe process thread count",
        );
        self.add_metric(
            "probe:current-process",
            "probe.handle_count",
            "current-process-win32",
            "count",
            "This probe process handle count",
        );
        if self.config.disk_probe {
            self.add_metric(
                "disk:physical-total",
                "disk.read_bytes_per_sec",
                "pdh-physical-disk-total",
                "bytes_per_sec",
                "PDH PhysicalDisk(_Total); physical versus virtual classification is not independently guaranteed",
            );
            self.add_metric(
                "disk:physical-total",
                "disk.write_bytes_per_sec",
                "pdh-physical-disk-total",
                "bytes_per_sec",
                "PDH PhysicalDisk(_Total); physical versus virtual classification is not independently guaranteed",
            );
        }
        if self.config.power_probe {
            self.add_metric(
                "power:system",
                "power.ac_connected",
                "get-system-power-status",
                "boolean",
                "Windows ACLineStatus; unknown status is omitted rather than mapped to false",
            );
            self.add_metric(
                "power:system",
                "power.saver_active",
                "get-system-power-status",
                "boolean",
                "Windows SystemStatusFlag; not a complete power-plan or performance-mode taxonomy",
            );
        }
        if self.config.gpu_probe {
            let gpu_metric_definitions = gpu_metric_definitions();
            let device_keys = self
                .gpu_provider
                .as_ref()
                .map(windows::NvmlProvider::device_keys)
                .filter(|keys| !keys.is_empty())
                .unwrap_or_else(|| vec!["gpu:nvidia:provider".to_string()]);
            for device_key in &device_keys {
                for (metric_key, unit, value_range, power_scope, limitation) in
                    gpu_metric_definitions.iter().copied()
                {
                    self.add_gpu_metric(
                        device_key,
                        metric_key,
                        unit,
                        value_range,
                        power_scope,
                        limitation,
                    );
                }
            }
            if let Some(provider) = self.gpu_provider.as_ref() {
                let statuses = provider.device_statuses();
                if statuses.is_empty() {
                    for metric_key in gpu_metric_keys() {
                        self.record_skip(
                            "gpu:nvidia:provider",
                            metric_key,
                            windows::ReadStatus::Unsupported,
                            "no_compatible_nvidia_gpu".to_string(),
                            0.0,
                        );
                    }
                } else {
                    for (device_key, status, reason_code) in statuses {
                        if status != windows::ReadStatus::Value {
                            for metric_key in gpu_metric_keys() {
                                self.record_skip(
                                    &device_key,
                                    metric_key,
                                    status,
                                    reason_code.clone(),
                                    0.0,
                                );
                            }
                        }
                    }
                }
            } else if let Some((status, reason_code)) = self.gpu_init_status.clone() {
                for metric_key in gpu_metric_keys() {
                    self.record_skip(
                        "gpu:nvidia:provider",
                        metric_key,
                        status,
                        reason_code.clone(),
                        0.0,
                    );
                }
            }
        }
        if self.config.process_probe {
            for (metric_key, unit, limitation) in [
                ("process.enumerated_count", "count", "Enumerated PIDs; no process identity is retained"),
                ("process.accessible_count", "count", "PIDs opened with limited query access"),
                ("process.restricted_count", "count", "PIDs that could not be opened with limited query access"),
                ("process.enumeration_elapsed_ms", "ms", "Elapsed time for PID enumeration and limited-access checks"),
                ("process.detail_cpu_time_readable_count", "count", "Processes with readable CPU time, counted independently of memory and I/O"),
                ("process.detail_working_set_readable_count", "count", "Processes with readable working set in the in-memory feasibility pass"),
                ("process.detail_private_memory_readable_count", "count", "Processes with readable private memory in the in-memory feasibility pass"),
                ("process.detail_io_readable_count", "count", "Processes with readable process I/O counters, counted independently of CPU and memory"),
                ("process.detail_permission_denied_count", "count", "Processes with at least one denied process-detail child read; not a sum of denied API calls"),
                ("process.detail_probe_failed_count", "count", "Process detail reads failed for non-permission reasons"),
                ("process.detail_raced_count", "count", "Process detail reads affected by an exited process or transient PID race"),
            ] {
                self.add_metric("processes:system", metric_key, "psapi-process-status", unit, limitation);
            }
        }
    }

    fn add_metric(
        &mut self,
        device_key: &str,
        metric_key: &str,
        provider: &str,
        unit: &str,
        limitation: &str,
    ) {
        let key = metric_identity(device_key, metric_key);
        self.metrics.insert(
            key,
            MetricRecord::new(
                device_key,
                metric_key,
                provider,
                SupportStatus::Supported,
                "ready",
                unit,
                if provider.starts_with("pdh") {
                    windows::SOURCE_PDH
                } else if provider.starts_with("nvidia-nvml") {
                    windows::SOURCE_NVML
                } else {
                    windows::SOURCE_SYSTEM_INFO
                },
                vec![limitation.to_string()],
            ),
        );
    }

    fn add_gpu_metric(
        &mut self,
        device_key: &str,
        metric_key: &str,
        unit: &str,
        value_range: &str,
        power_scope: Option<&str>,
        limitation: &str,
    ) {
        let mut metric = MetricRecord::new(
            device_key,
            metric_key,
            "nvidia-nvml",
            SupportStatus::Supported,
            "ready",
            unit,
            windows::SOURCE_NVML,
            vec![limitation.to_string()],
        )
        .with_value_range(value_range);
        if let Some(power_scope) = power_scope {
            metric = metric.with_power_scope(power_scope);
        }
        self.metrics
            .insert(metric_identity(device_key, metric_key), metric);
    }

    fn run(mut self) -> Result<ProbeReport, String> {
        let started = Instant::now();
        let duration = Duration::from_secs(self.config.duration_seconds);
        let mut sampling = SamplingSummary::default();
        let mut next_core = started;
        let mut next_process = started;
        let mut first_core = true;
        let mut first_process = true;
        let mut last_cpu = None;
        let mut last_cpu_sample = None;
        let mut last_process_sample = None;

        while started.elapsed() < duration {
            let now = Instant::now();
            if now >= next_core {
                let due = due_count(
                    now,
                    next_core,
                    Duration::from_millis(self.config.core_interval_ms),
                );
                sampling.core_dropped_samples = sampling
                    .core_dropped_samples
                    .saturating_add(due.saturating_sub(1));
                sampling.core_executed_samples += 1;
                if !first_core && now > next_core + Duration::from_millis(50) {
                    sampling.late_wakeups += 1;
                }
                let current_cpu = self.sample_core(now, last_cpu, last_cpu_sample, &mut sampling);
                last_cpu = current_cpu.0;
                last_cpu_sample = current_cpu.1;
                first_core = false;
                next_core +=
                    Duration::from_millis(self.config.core_interval_ms) * due.max(1) as u32;
            }
            if self.config.process_probe && now >= next_process {
                let due = due_count(
                    now,
                    next_process,
                    Duration::from_millis(self.config.process_interval_ms),
                );
                sampling.process_dropped_samples = sampling
                    .process_dropped_samples
                    .saturating_add(due.saturating_sub(1));
                sampling.process_executed_samples += 1;
                if !first_process && now > next_process + Duration::from_millis(50) {
                    sampling.late_wakeups += 1;
                }
                self.sample_process(now, last_process_sample);
                last_process_sample = Some(now);
                first_process = false;
                next_process +=
                    Duration::from_millis(self.config.process_interval_ms) * due.max(1) as u32;
            }
            let next = if self.config.process_probe {
                next_core.min(next_process)
            } else {
                next_core
            };
            if next > Instant::now() {
                thread::sleep((next - Instant::now()).min(Duration::from_millis(100)));
            }
        }

        sampling.wall_duration_ms = started.elapsed().as_millis() as u64;
        if self.config.process_probe {
            sampling.process_expected_samples = expected_samples(
                self.config.duration_seconds,
                self.config.process_interval_ms,
            );
        }
        sampling.core_expected_samples =
            expected_samples(self.config.duration_seconds, self.config.core_interval_ms);
        let finished_at = utc_now_string();
        let mut metrics: Vec<_> = self.metrics.into_values().collect();
        for metric in &mut metrics {
            metric.finalize();
        }
        let self_resource_summary = self_resource_summary(&self.self_samples);
        let average_cpu = self
            .self_samples
            .probe_cpu_share_percent
            .iter()
            .copied()
            .reduce(|left, right| left + right)
            .map(|sum| sum / self.self_samples.probe_cpu_share_percent.len() as f64);
        let steady_memory = self
            .self_samples
            .working_set_bytes
            .iter()
            .copied()
            .reduce(|left, right| left.max(right))
            .map(|value| value as u64);
        let elevated = self.machine.elevated.unwrap_or(false);
        let mut deferred = deferred_items();
        deferred.push(DeferredItem {
            item: if elevated {
                "Non-administrator comparison"
            } else {
                "Administrator comparison"
            }
            .to_string(),
            status: "pending".to_string(),
            reason: "The other Windows integrity level was not run automatically".to_string(),
        });
        let report = ProbeReport {
            schema_version: SCHEMA_VERSION.to_string(),
            probe_name: "metric-probe".to_string(),
            started_at_utc: self.started_at,
            finished_at_utc: finished_at,
            machine: self.machine,
            configuration: TestConfiguration {
                duration_seconds: self.config.duration_seconds,
                core_interval_ms: self.config.core_interval_ms,
                process_interval_ms: self.config.process_interval_ms,
                process_probe: self.config.process_probe,
                disk_probe: self.config.disk_probe,
                network_probe: self.config.network_probe,
                power_probe: self.config.power_probe,
                gpu_probe: self.config.gpu_probe,
            },
            devices: self.devices,
            capabilities: self.capabilities,
            metrics,
            sampling,
            self_resource_summary,
            privacy: privacy_summary(),
            conclusion: Conclusion {
                scope: "Current machine experiment only; cross-hardware validation is not implied"
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
            deferred,
            rerun_commands: rerun_commands(&self.config),
        };
        let _ = render_probe_markdown(&report);
        Ok(report)
    }

    fn sample_core(
        &mut self,
        now: Instant,
        previous_cpu: Option<windows::CpuTimes>,
        previous_cpu_sample: Option<Instant>,
        sampling: &mut SamplingSummary,
    ) -> (Option<windows::CpuTimes>, Option<Instant>) {
        let timestamp_ms = unix_now_ms();
        let interval =
            previous_cpu_sample.map(|previous| now.duration_since(previous).as_millis() as u64);
        let cpu_started = Instant::now();
        let cpu_result = windows::cpu_times();
        let cpu_latency = elapsed_ms(cpu_started);
        if let Some(current) = cpu_result.value {
            if let Some(previous) = previous_cpu {
                let total_delta = current
                    .kernel_100ns
                    .saturating_sub(previous.kernel_100ns)
                    .saturating_add(current.user_100ns.saturating_sub(previous.user_100ns));
                let idle_delta = current.idle_100ns.saturating_sub(previous.idle_100ns);
                let usage = if total_delta == 0 {
                    None
                } else {
                    Some(
                        (total_delta.saturating_sub(idle_delta) as f64 * 100.0)
                            / total_delta as f64,
                    )
                };
                if let Some(value) = usage {
                    self.record_success(
                        "cpu:system",
                        "cpu.usage_percent",
                        timestamp_ms,
                        value,
                        interval,
                        previous_cpu_sample,
                        cpu_latency,
                    );
                }
            }
        } else {
            self.record_failure(
                "cpu:system",
                "cpu.usage_percent",
                cpu_result.status,
                cpu_result.reason_code,
                cpu_latency,
            );
        }

        let frequency_started = Instant::now();
        let frequency = windows::cpu_frequency_info();
        let frequency_latency = elapsed_ms(frequency_started);
        match frequency.value {
            Some(value) => {
                if let Some(current) = value.current_mhz {
                    self.record_success(
                        "cpu:system",
                        "cpu.frequency_current_mhz",
                        timestamp_ms,
                        current,
                        interval,
                        None,
                        frequency_latency,
                    );
                } else {
                    self.record_skip(
                        "cpu:system",
                        "cpu.frequency_current_mhz",
                        windows::ReadStatus::Unsupported,
                        "current_frequency_unavailable".to_string(),
                        frequency_latency,
                    );
                }
                if let Some(max) = value.max_mhz {
                    self.record_success(
                        "cpu:system",
                        "cpu.frequency_max_mhz",
                        timestamp_ms,
                        max,
                        interval,
                        None,
                        frequency_latency,
                    );
                } else {
                    self.record_skip(
                        "cpu:system",
                        "cpu.frequency_max_mhz",
                        windows::ReadStatus::Unsupported,
                        "maximum_frequency_unavailable".to_string(),
                        frequency_latency,
                    );
                }
            }
            None => {
                self.record_failure(
                    "cpu:system",
                    "cpu.frequency_current_mhz",
                    frequency.status,
                    frequency.reason_code.clone(),
                    frequency_latency,
                );
                self.record_failure(
                    "cpu:system",
                    "cpu.frequency_max_mhz",
                    frequency.status,
                    frequency.reason_code,
                    frequency_latency,
                );
            }
        }

        let memory_started = Instant::now();
        let memory = windows::memory_info();
        let memory_latency = elapsed_ms(memory_started);
        match memory.value {
            Some(value) => {
                self.record_success(
                    "memory:physical",
                    "memory.used_bytes",
                    timestamp_ms,
                    value.used_bytes as f64,
                    interval,
                    None,
                    memory_latency,
                );
                self.record_success(
                    "memory:physical",
                    "memory.available_bytes",
                    timestamp_ms,
                    value.available_bytes as f64,
                    interval,
                    None,
                    memory_latency,
                );
                self.record_success(
                    "memory:physical",
                    "memory.usage_percent",
                    timestamp_ms,
                    value.usage_percent,
                    interval,
                    None,
                    memory_latency,
                );
            }
            None => {
                for key in [
                    "memory.used_bytes",
                    "memory.available_bytes",
                    "memory.usage_percent",
                ] {
                    self.record_failure(
                        "memory:physical",
                        key,
                        memory.status,
                        memory.reason_code.clone(),
                        memory_latency,
                    );
                }
            }
        }

        let uptime_started = Instant::now();
        let uptime = windows::uptime_ms();
        self.record_success(
            "system:uptime",
            "system.uptime_ms",
            timestamp_ms,
            uptime as f64,
            interval,
            None,
            elapsed_ms(uptime_started),
        );
        self.sample_disk(timestamp_ms, interval);
        self.sample_network(timestamp_ms, now);
        self.sample_power(timestamp_ms, interval);
        self.sample_gpu(timestamp_ms, interval);
        self.sample_self(timestamp_ms, previous_cpu_sample, now, sampling);
        (cpu_result.value, Some(now))
    }

    fn sample_disk(&mut self, timestamp_ms: i64, interval: Option<u64>) {
        let Some(provider) = self.disk_provider.as_mut() else {
            return;
        };
        let started = Instant::now();
        let result = provider.sample();
        let latency = elapsed_ms(started);
        match result.value {
            Some(value) => {
                self.record_success(
                    "disk:physical-total",
                    "disk.read_bytes_per_sec",
                    timestamp_ms,
                    value.read_bytes_per_sec,
                    interval,
                    None,
                    latency,
                );
                self.record_success(
                    "disk:physical-total",
                    "disk.write_bytes_per_sec",
                    timestamp_ms,
                    value.write_bytes_per_sec,
                    interval,
                    None,
                    latency,
                );
            }
            None if result.reason_code == "pdh_warmup_sample" => {
                self.record_skip(
                    "disk:physical-total",
                    "disk.read_bytes_per_sec",
                    result.status,
                    result.reason_code.clone(),
                    latency,
                );
                self.record_skip(
                    "disk:physical-total",
                    "disk.write_bytes_per_sec",
                    result.status,
                    result.reason_code,
                    latency,
                );
            }
            None => {
                self.record_failure(
                    "disk:physical-total",
                    "disk.read_bytes_per_sec",
                    result.status,
                    result.reason_code.clone(),
                    latency,
                );
                self.record_failure(
                    "disk:physical-total",
                    "disk.write_bytes_per_sec",
                    result.status,
                    result.reason_code,
                    latency,
                );
            }
        }
    }

    fn sample_network(&mut self, timestamp_ms: i64, now: Instant) {
        if !self.config.network_probe {
            return;
        }
        let started = Instant::now();
        let result = windows::network_interfaces();
        let latency = elapsed_ms(started);
        match result.value {
            Some(interfaces) => {
                for interface in interfaces {
                    self.ensure_network_metrics(&interface);
                    let previous = self.network_previous.insert(
                        interface.device_key.clone(),
                        NetworkPrevious {
                            timestamp: now,
                            in_octets: interface.in_octets,
                            out_octets: interface.out_octets,
                        },
                    );
                    self.record_success(
                        &interface.device_key,
                        "network.receive_bytes_total",
                        timestamp_ms,
                        interface.in_octets as f64,
                        previous
                            .map(|value| now.duration_since(value.timestamp).as_millis() as u64),
                        None,
                        latency,
                    );
                    self.record_success(
                        &interface.device_key,
                        "network.transmit_bytes_total",
                        timestamp_ms,
                        interface.out_octets as f64,
                        previous
                            .map(|value| now.duration_since(value.timestamp).as_millis() as u64),
                        None,
                        latency,
                    );
                    if let Some(previous) = previous {
                        let elapsed = now.duration_since(previous.timestamp).as_secs_f64();
                        if elapsed > 0.0 {
                            let interval = Some(elapsed as u64 * 1000);
                            self.record_success(
                                &interface.device_key,
                                "network.receive_bytes_per_sec",
                                timestamp_ms,
                                interface.in_octets.saturating_sub(previous.in_octets) as f64
                                    / elapsed,
                                interval,
                                Some(previous.timestamp),
                                latency,
                            );
                            self.record_success(
                                &interface.device_key,
                                "network.transmit_bytes_per_sec",
                                timestamp_ms,
                                interface.out_octets.saturating_sub(previous.out_octets) as f64
                                    / elapsed,
                                interval,
                                None,
                                latency,
                            );
                        }
                    }
                }
            }
            None => {
                if self.network_previous.is_empty() {
                    self.capabilities.push(Capability {
                        device_key: "network:interfaces".to_string(),
                        category: "network".to_string(),
                        provider: "ip-helper-get-if-table2".to_string(),
                        support_status: map_status(result.status),
                        reason_code: result.reason_code,
                        details: BTreeMap::new(),
                        source: windows::SOURCE_SYSTEM_INFO.to_string(),
                        known_semantic_limitations: vec!["Cumulative interface counters are required to derive interval throughput".to_string()],
                    });
                }
            }
        }
    }

    fn ensure_network_metrics(&mut self, interface: &windows::NetworkInterfaceSnapshot) {
        let details = BTreeMap::from([
            (
                "classification".to_string(),
                interface.classification.clone(),
            ),
            (
                "interface_type".to_string(),
                interface.interface_type.to_string(),
            ),
        ]);
        if !self
            .devices
            .iter()
            .any(|device| device.device_key == interface.device_key)
        {
            self.devices.push(DeviceInfo {
                device_key: interface.device_key.clone(),
                category: interface.category.clone(),
                present: Some(true),
                classification: interface.classification.clone(),
                details,
            });
        }
        for (key, unit) in [
            ("network.receive_bytes_total", "bytes"),
            ("network.transmit_bytes_total", "bytes"),
            ("network.receive_bytes_per_sec", "bytes_per_sec"),
            ("network.transmit_bytes_per_sec", "bytes_per_sec"),
        ] {
            if !self
                .metrics
                .contains_key(&metric_identity(&interface.device_key, key))
            {
                self.add_metric(
                    &interface.device_key,
                    key,
                    "ip-helper-get-if-table2",
                    unit,
                    "Interface physicality may be unknown; counters are Windows cumulative octets",
                );
            }
        }
    }

    fn sample_power(&mut self, timestamp_ms: i64, interval: Option<u64>) {
        if !self.config.power_probe {
            return;
        }
        let started = Instant::now();
        let result = windows::power_info();
        let latency = elapsed_ms(started);
        match result.value {
            Some(value) => {
                if let Some(ac) = value.ac_line_status {
                    self.record_success(
                        "power:system",
                        "power.ac_connected",
                        timestamp_ms,
                        bool_value(ac),
                        interval,
                        None,
                        latency,
                    );
                } else {
                    self.record_skip(
                        "power:system",
                        "power.ac_connected",
                        windows::ReadStatus::Unsupported,
                        "ac_line_status_unknown".to_string(),
                        latency,
                    );
                }
                if let Some(saver) = value.saver_active {
                    self.record_success(
                        "power:system",
                        "power.saver_active",
                        timestamp_ms,
                        bool_value(saver),
                        interval,
                        None,
                        latency,
                    );
                } else {
                    self.record_skip(
                        "power:system",
                        "power.saver_active",
                        windows::ReadStatus::Unsupported,
                        "saver_status_unknown".to_string(),
                        latency,
                    );
                }
                if battery_metric_should_be_created(value.battery_present) {
                    self.ensure_battery_metric();
                    if let Some(percent) = value.battery_percent {
                        self.record_success(
                            "battery:system",
                            "battery.percent",
                            timestamp_ms,
                            percent as f64,
                            interval,
                            None,
                            latency,
                        );
                    } else {
                        self.record_skip(
                            "battery:system",
                            "battery.percent",
                            windows::ReadStatus::Unsupported,
                            "battery_percent_unavailable".to_string(),
                            latency,
                        );
                    }
                }
            }
            None => {
                for key in ["power.ac_connected", "power.saver_active"] {
                    self.record_failure(
                        "power:system",
                        key,
                        result.status,
                        result.reason_code.clone(),
                        latency,
                    );
                }
            }
        }
    }

    fn ensure_battery_metric(&mut self) {
        if !self
            .metrics
            .contains_key(&metric_identity("battery:system", "battery.percent"))
        {
            self.add_metric(
                "battery:system",
                "battery.percent",
                "get-system-power-status",
                "percent",
                "Created only when Windows reports a battery; absent battery does not create a value",
            );
        }
    }

    fn sample_process(&mut self, now: Instant, previous_sample: Option<Instant>) {
        let interval =
            previous_sample.map(|previous| now.duration_since(previous).as_millis() as u64);
        let access_started = Instant::now();
        let access = windows::process_access_summary();
        let access_latency = elapsed_ms(access_started);
        match access.value {
            Some(value) => {
                let timestamp_ms = unix_now_ms();
                self.record_success(
                    "processes:system",
                    "process.enumerated_count",
                    timestamp_ms,
                    value.enumerated as f64,
                    interval,
                    None,
                    access_latency,
                );
                self.record_success(
                    "processes:system",
                    "process.accessible_count",
                    timestamp_ms,
                    value.accessible as f64,
                    interval,
                    None,
                    access_latency,
                );
                self.record_success(
                    "processes:system",
                    "process.restricted_count",
                    timestamp_ms,
                    value.restricted as f64,
                    interval,
                    None,
                    access_latency,
                );
                self.record_success(
                    "processes:system",
                    "process.enumeration_elapsed_ms",
                    timestamp_ms,
                    value.elapsed.as_secs_f64() * 1000.0,
                    interval,
                    None,
                    access_latency,
                );
            }
            None => {
                for key in [
                    "process.enumerated_count",
                    "process.accessible_count",
                    "process.restricted_count",
                    "process.enumeration_elapsed_ms",
                ] {
                    self.record_failure(
                        "processes:system",
                        key,
                        access.status,
                        access.reason_code.clone(),
                        access_latency,
                    );
                }
            }
        }
        let detail_started = Instant::now();
        let details = windows::process_detail_summary();
        let detail_latency = elapsed_ms(detail_started);
        match details.value {
            Some(value) => {
                let timestamp_ms = unix_now_ms();
                for (key, count) in [
                    (
                        "process.detail_cpu_time_readable_count",
                        value.readable_cpu_time,
                    ),
                    (
                        "process.detail_working_set_readable_count",
                        value.readable_working_set,
                    ),
                    (
                        "process.detail_private_memory_readable_count",
                        value.readable_private_memory,
                    ),
                    ("process.detail_io_readable_count", value.readable_io),
                    (
                        "process.detail_permission_denied_count",
                        value.permission_denied,
                    ),
                    ("process.detail_probe_failed_count", value.probe_failed),
                    ("process.detail_raced_count", value.raced),
                ] {
                    self.record_success(
                        "processes:system",
                        key,
                        timestamp_ms,
                        count as f64,
                        interval,
                        None,
                        detail_latency,
                    );
                }
            }
            None => {
                for key in [
                    "process.detail_cpu_time_readable_count",
                    "process.detail_working_set_readable_count",
                    "process.detail_private_memory_readable_count",
                    "process.detail_io_readable_count",
                    "process.detail_permission_denied_count",
                    "process.detail_probe_failed_count",
                    "process.detail_raced_count",
                ] {
                    self.record_failure(
                        "processes:system",
                        key,
                        details.status,
                        details.reason_code.clone(),
                        detail_latency,
                    );
                }
            }
        }
    }

    fn sample_gpu(&mut self, timestamp_ms: i64, interval: Option<u64>) {
        if !self.config.gpu_probe {
            return;
        }
        let samples = self
            .gpu_provider
            .as_ref()
            .map(windows::NvmlProvider::sample_all)
            .unwrap_or_default();
        for sample in samples {
            self.record_gpu_read(
                &sample.device_key,
                "gpu.utilization_percent",
                sample.utilization_percent,
                timestamp_ms,
                interval,
            );
            self.record_gpu_read(
                &sample.device_key,
                "gpu.memory_controller_utilization_percent",
                sample.memory_controller_utilization_percent,
                timestamp_ms,
                interval,
            );
            self.record_gpu_read(
                &sample.device_key,
                "gpu.temperature_celsius",
                sample.temperature_celsius,
                timestamp_ms,
                interval,
            );
            self.record_gpu_read(
                &sample.device_key,
                "gpu.power_watts",
                sample.power_watts,
                timestamp_ms,
                interval,
            );
            self.record_gpu_read(
                &sample.device_key,
                "gpu.graphics_clock_mhz",
                sample.graphics_clock_mhz,
                timestamp_ms,
                interval,
            );
            self.record_gpu_read(
                &sample.device_key,
                "gpu.memory_clock_mhz",
                sample.memory_clock_mhz,
                timestamp_ms,
                interval,
            );
            self.record_gpu_read(
                &sample.device_key,
                "gpu.vram_used_bytes",
                sample.vram_used_bytes,
                timestamp_ms,
                interval,
            );
            self.record_gpu_read(
                &sample.device_key,
                "gpu.vram_total_bytes",
                sample.vram_total_bytes,
                timestamp_ms,
                interval,
            );
        }
    }

    fn record_gpu_read(
        &mut self,
        device_key: &str,
        metric_key: &str,
        timed: windows::TimedRead<f64>,
        timestamp_ms: i64,
        interval: Option<u64>,
    ) {
        let latency = timed.latency_ms;
        let result = timed.result;
        match result.value {
            Some(value) => self.record_success(
                device_key,
                metric_key,
                timestamp_ms,
                value,
                interval,
                None,
                latency,
            ),
            None => self.record_failure(
                device_key,
                metric_key,
                result.status,
                result.reason_code,
                latency,
            ),
        }
    }

    fn sample_self(
        &mut self,
        timestamp_ms: i64,
        previous_sample: Option<Instant>,
        now: Instant,
        _sampling: &mut SamplingSummary,
    ) {
        let started = Instant::now();
        let result = windows::self_metrics();
        let latency = elapsed_ms(started);
        if let Some(value) = result.value {
            let self_interval =
                previous_sample.map(|previous| now.duration_since(previous).as_millis() as u64);
            let previous_cpu_time = self.self_samples.cpu_time_100ns.last().copied();
            self.self_samples
                .cpu_time_100ns
                .push(value.cpu_time_100ns as f64);
            self.self_samples
                .working_set_bytes
                .push(value.working_set_bytes as f64);
            self.self_samples
                .thread_count
                .push(value.thread_count as f64);
            self.self_samples
                .handle_count
                .push(value.handle_count as f64);
            if let (Some(previous_cpu_time), Some(interval_ms)) =
                (previous_cpu_time, self_interval.filter(|value| *value > 0))
            {
                let delta_100ns = (value.cpu_time_100ns as f64 - previous_cpu_time).max(0.0);
                let logical_processors = self.machine.logical_processor_count.unwrap_or(1) as f64;
                let whole_machine_percent =
                    delta_100ns * 100.0 / (interval_ms as f64 * 10_000.0 * logical_processors);
                self.self_samples
                    .probe_cpu_share_percent
                    .push(whole_machine_percent);
            }
            self.record_success(
                "probe:current-process",
                "probe.cpu_time_100ns",
                timestamp_ms,
                value.cpu_time_100ns as f64,
                self_interval,
                previous_sample,
                latency,
            );
            self.record_success(
                "probe:current-process",
                "probe.working_set_bytes",
                timestamp_ms,
                value.working_set_bytes as f64,
                self_interval,
                previous_sample,
                latency,
            );
            self.record_success(
                "probe:current-process",
                "probe.thread_count",
                timestamp_ms,
                value.thread_count as f64,
                self_interval,
                previous_sample,
                latency,
            );
            self.record_success(
                "probe:current-process",
                "probe.handle_count",
                timestamp_ms,
                value.handle_count as f64,
                self_interval,
                previous_sample,
                latency,
            );
        } else {
            for key in [
                "probe.cpu_time_100ns",
                "probe.working_set_bytes",
                "probe.thread_count",
                "probe.handle_count",
            ] {
                self.record_failure(
                    "probe:current-process",
                    key,
                    result.status,
                    result.reason_code.clone(),
                    latency,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_success(
        &mut self,
        device_key: &str,
        metric_key: &str,
        timestamp_ms: i64,
        value: f64,
        interval_ms: Option<u64>,
        _previous_sample: Option<Instant>,
        latency_ms: f64,
    ) {
        if let Some(metric) = self
            .metrics
            .get_mut(&metric_identity(device_key, metric_key))
        {
            metric.record_success(
                timestamp_ms,
                value,
                interval_ms,
                interval_ms.unwrap_or(0),
                latency_ms,
            );
        }
    }

    fn record_failure(
        &mut self,
        device_key: &str,
        metric_key: &str,
        status: windows::ReadStatus,
        reason_code: String,
        latency_ms: f64,
    ) {
        if let Some(metric) = self
            .metrics
            .get_mut(&metric_identity(device_key, metric_key))
        {
            metric.record_failure_with_status(map_status(status), reason_code, latency_ms);
        }
    }

    fn record_skip(
        &mut self,
        device_key: &str,
        metric_key: &str,
        status: windows::ReadStatus,
        reason_code: String,
        latency_ms: f64,
    ) {
        if let Some(metric) = self
            .metrics
            .get_mut(&metric_identity(device_key, metric_key))
        {
            metric.record_skip(map_status(status), reason_code, latency_ms);
        }
    }
}

fn metric_identity(device_key: &str, metric_key: &str) -> String {
    format!("{device_key}\0{metric_key}")
}

fn gpu_metric_keys() -> [&'static str; 8] {
    [
        "gpu.utilization_percent",
        "gpu.memory_controller_utilization_percent",
        "gpu.temperature_celsius",
        "gpu.power_watts",
        "gpu.graphics_clock_mhz",
        "gpu.memory_clock_mhz",
        "gpu.vram_used_bytes",
        "gpu.vram_total_bytes",
    ]
}

fn gpu_metric_definitions() -> [(
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
    &'static str,
); 8] {
    [
        (
            "gpu.utilization_percent",
            "percent",
            "0..100",
            None,
            "NVML utilization over the driver's sampling window",
        ),
        (
            "gpu.memory_controller_utilization_percent",
            "percent",
            "0..100",
            None,
            "NVML memory-controller utilization over the driver's sampling window",
        ),
        (
            "gpu.temperature_celsius",
            "C",
            "driver-defined non-negative Celsius value",
            None,
            "NVML GPU temperature sensor; hotspot and memory temperatures are not sampled",
        ),
        (
            "gpu.power_watts",
            "W",
            "driver-defined non-negative board power",
            Some("gpu_board"),
            "NVML board power converted from milliwatts; not system or wall power",
        ),
        (
            "gpu.graphics_clock_mhz",
            "MHz",
            "driver-defined non-negative clock",
            None,
            "NVML graphics clock, not a guaranteed sustained frequency",
        ),
        (
            "gpu.memory_clock_mhz",
            "MHz",
            "driver-defined non-negative clock",
            None,
            "NVML memory clock, not a guaranteed sustained frequency",
        ),
        (
            "gpu.vram_used_bytes",
            "bytes",
            "0..gpu.vram_total_bytes",
            None,
            "NVML device memory used in bytes",
        ),
        (
            "gpu.vram_total_bytes",
            "bytes",
            "non-negative device memory capacity",
            None,
            "NVML device memory total in bytes",
        ),
    ]
}

fn map_status(status: windows::ReadStatus) -> SupportStatus {
    match status {
        windows::ReadStatus::Value => SupportStatus::Supported,
        windows::ReadStatus::Unsupported => SupportStatus::Unsupported,
        windows::ReadStatus::PermissionDenied => SupportStatus::PermissionDenied,
        windows::ReadStatus::ProviderMissing => SupportStatus::ProviderMissing,
        windows::ReadStatus::Failed => SupportStatus::ProbeFailed,
        windows::ReadStatus::RuntimeFailed => SupportStatus::RuntimeFailed,
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
    let duration_ms = duration_seconds.saturating_mul(1000);
    duration_ms
        .saturating_sub(1)
        .checked_div(interval_ms.max(1))
        .unwrap_or(0)
        .saturating_add(1)
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn bool_value(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

pub(crate) fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn utc_now_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let days = seconds / 86_400;
    let seconds_in_day = seconds % 86_400;
    let (year, month, day) = civil_date_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_in_day / 3_600,
        (seconds_in_day % 3_600) / 60,
        seconds_in_day % 60
    )
}

fn civil_date_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
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
            "process identity and invocation text".to_string(),
            "window caption text".to_string(),
            "security identifiers".to_string(),
            "database contents".to_string(),
        ],
        process_detail_retention: "in memory only; aggregate counts only in report".to_string(),
    }
}

fn deferred_items() -> Vec<DeferredItem> {
    [
        (
            "CPU temperature, power, and sensor frequency",
            "Outside Spike-01B scope",
        ),
        ("AMD and Intel GPU providers", "Outside Spike-01B scope"),
        (
            "Battery charge/discharge power, health, and cycles",
            "Windows baseline status only",
        ),
        ("SMART/NVMe temperature", "No storage health API is called"),
        (
            "Fan or pump telemetry",
            "No hardware sensor provider is called",
        ),
        (
            "Windows crash judgment",
            "No crash inference is implemented",
        ),
        (
            "Event Log event classification",
            "No Event Log subscription is implemented",
        ),
        (
            "Formal sleep/wake event subscription",
            "No event subscription is implemented",
        ),
        (
            "Process GPU/VRAM attribution",
            "No GPU process attribution is implemented",
        ),
        (
            "24-hour soak and storage-growth validation",
            "Deferred to release validation",
        ),
        (
            "Cross-hardware validation",
            "This result is limited to the current machine",
        ),
    ]
    .into_iter()
    .map(|(item, reason)| DeferredItem {
        item: item.to_string(),
        status: "deferred".to_string(),
        reason: reason.to_string(),
    })
    .collect()
}

fn rerun_commands(config: &RunConfig) -> Vec<String> {
    let mut command = format!(
        "cargo run --manifest-path tools/metric-probe/Cargo.toml -- run --duration-seconds {} --core-interval-ms {} --process-interval-ms {}",
        config.duration_seconds, config.core_interval_ms, config.process_interval_ms
    );
    if !config.process_probe {
        command.push_str(" --no-process-probe");
    }
    if !config.disk_probe {
        command.push_str(" --no-disk-probe");
    }
    if !config.network_probe {
        command.push_str(" --no-network-probe");
    }
    if !config.power_probe {
        command.push_str(" --no-power-probe");
    }
    if !config.gpu_probe {
        command.push_str(" --no-gpu-probe");
    }
    vec![
        command.clone(),
        format!(
            "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-Command','{}'",
            command.replace('\'', "''")
        ),
    ]
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
    let average = values.iter().sum::<f64>() / values.len() as f64;
    Some(SelfMetricSummary {
        unit: unit.to_string(),
        sample_count: values.len(),
        start: values.first().copied(),
        average: Some(average),
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

fn battery_metric_should_be_created(battery_present: bool) -> bool {
    battery_present
}

#[cfg(test)]
mod tests {
    use super::ProbeSession;
    use crate::cli::RunConfig;

    #[test]
    fn no_battery_does_not_create_battery_metric() {
        assert!(!super::battery_metric_should_be_created(false));
        assert!(super::battery_metric_should_be_created(true));
    }

    #[test]
    fn gpu_disabled_does_not_initialize_or_create_gpu_metrics() {
        let config = RunConfig {
            gpu_probe: false,
            process_probe: false,
            disk_probe: false,
            network_probe: false,
            power_probe: false,
            ..RunConfig::default()
        };
        let session = ProbeSession::new(config);
        assert!(session.gpu_provider.is_none());
        assert!(session.gpu_init_status.is_none());
        assert!(!session
            .metrics
            .values()
            .any(|metric| metric.metric_key.starts_with("gpu.")));
    }

    #[test]
    fn gpu_metric_keys_are_complete_and_stable() {
        assert_eq!(super::gpu_metric_keys().len(), 8);
        assert!(super::gpu_metric_keys().contains(&"gpu.power_watts"));
        assert!(super::gpu_metric_keys().contains(&"gpu.vram_total_bytes"));
    }

    #[test]
    fn gpu_metric_units_and_power_scope_are_explicit() {
        let definitions = super::gpu_metric_definitions();
        let power = definitions
            .iter()
            .find(|definition| definition.0 == "gpu.power_watts")
            .unwrap();
        assert_eq!(power.1, "W");
        assert_eq!(power.3, Some("gpu_board"));
        assert_eq!(
            definitions
                .iter()
                .find(|definition| definition.0 == "gpu.vram_used_bytes")
                .unwrap()
                .1,
            "bytes"
        );
        assert_eq!(
            definitions
                .iter()
                .find(|definition| definition.0 == "gpu.graphics_clock_mhz")
                .unwrap()
                .1,
            "MHz"
        );
    }
}
