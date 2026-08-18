use crate::stats::Distribution;
use serde::Serialize;
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: &str = "spike-01b/v1";

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    Supported,
    Unsupported,
    PermissionDenied,
    ProviderMissing,
    ProbeFailed,
    RuntimeFailed,
    Disabled,
}

impl SupportStatus {
    pub fn is_successful(self) -> bool {
        matches!(self, Self::Supported)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Capability {
    pub device_key: String,
    pub category: String,
    pub provider: String,
    pub support_status: SupportStatus,
    pub reason_code: String,
    pub details: BTreeMap<String, String>,
    pub source: String,
    pub known_semantic_limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub device_key: String,
    pub category: String,
    pub present: Option<bool>,
    pub classification: String,
    pub details: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSample {
    pub timestamp_ms: i64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricRecord {
    pub device_key: String,
    pub metric_key: String,
    pub provider: String,
    pub support_status: SupportStatus,
    pub reason_code: String,
    pub unit: String,
    pub value_range: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_scope: Option<String>,
    pub sample_count: usize,
    pub failed_sample_count: usize,
    pub covered_duration_ms: u64,
    pub sampling_interval: Distribution,
    pub call_latency_ms: Distribution,
    pub source: String,
    pub known_semantic_limitations: Vec<String>,
    pub failure_reasons: BTreeMap<String, usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_value: Option<f64>,
    pub samples: Vec<MetricSample>,
    #[serde(skip)]
    pub interval_values: Vec<f64>,
    #[serde(skip)]
    pub latency_values: Vec<f64>,
}

impl MetricRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device_key: impl Into<String>,
        metric_key: impl Into<String>,
        provider: impl Into<String>,
        support_status: SupportStatus,
        reason_code: impl Into<String>,
        unit: impl Into<String>,
        source: impl Into<String>,
        known_semantic_limitations: Vec<String>,
    ) -> Self {
        Self {
            device_key: device_key.into(),
            metric_key: metric_key.into(),
            provider: provider.into(),
            support_status,
            reason_code: reason_code.into(),
            unit: unit.into(),
            value_range: "provider-defined".to_string(),
            power_scope: None,
            sample_count: 0,
            failed_sample_count: 0,
            covered_duration_ms: 0,
            sampling_interval: Distribution::from_values(&[]),
            call_latency_ms: Distribution::from_values(&[]),
            source: source.into(),
            known_semantic_limitations,
            failure_reasons: BTreeMap::new(),
            latest_value: None,
            samples: Vec::new(),
            interval_values: Vec::new(),
            latency_values: Vec::new(),
        }
    }

    pub fn with_value_range(mut self, value_range: impl Into<String>) -> Self {
        self.value_range = value_range.into();
        self
    }

    pub fn with_power_scope(mut self, power_scope: impl Into<String>) -> Self {
        self.power_scope = Some(power_scope.into());
        self
    }

    pub fn record_success(
        &mut self,
        timestamp_ms: i64,
        value: f64,
        interval_ms: Option<u64>,
        covered_duration_ms: u64,
        call_latency_ms: f64,
    ) {
        if matches!(self.support_status, SupportStatus::ProbeFailed) {
            self.support_status = SupportStatus::Supported;
            self.reason_code = "ok".to_string();
        }
        self.sample_count += 1;
        self.latest_value = Some(value);
        self.samples.push(MetricSample {
            timestamp_ms,
            value,
        });
        if let Some(interval_ms) = interval_ms {
            self.interval_values.push(interval_ms as f64);
        }
        self.covered_duration_ms = self.covered_duration_ms.saturating_add(covered_duration_ms);
        self.latency_values.push(call_latency_ms);
        self.sampling_interval = Distribution::from_values(&self.interval_values);
        self.call_latency_ms = Distribution::from_values(&self.latency_values);
    }

    pub fn record_failure_with_status(
        &mut self,
        status: SupportStatus,
        reason_code: String,
        call_latency_ms: f64,
    ) {
        self.failed_sample_count += 1;
        *self.failure_reasons.entry(reason_code.clone()).or_insert(0) += 1;
        self.latency_values.push(call_latency_ms);
        self.call_latency_ms = Distribution::from_values(&self.latency_values);
        if self.sample_count == 0 && self.support_status.is_successful() {
            self.support_status = status;
            self.reason_code = reason_code;
        }
    }

    pub fn record_skip(
        &mut self,
        status: SupportStatus,
        reason_code: String,
        call_latency_ms: f64,
    ) {
        self.latency_values.push(call_latency_ms);
        self.call_latency_ms = Distribution::from_values(&self.latency_values);
        if self.sample_count == 0 {
            self.support_status = status;
            self.reason_code = reason_code;
        }
    }

    pub fn finalize(&mut self) {
        if self.sample_count > 0 && self.failed_sample_count > 0 {
            self.reason_code = "partial_sampling_failures".to_string();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MachineInfo {
    pub os_name: String,
    pub os_display_version: Option<String>,
    pub os_build: Option<String>,
    pub architecture: String,
    pub cpu_model: Option<String>,
    pub logical_processor_count: Option<u32>,
    pub memory_total_bytes: Option<u64>,
    pub elevated: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestConfiguration {
    pub duration_seconds: u64,
    pub core_interval_ms: u64,
    pub process_interval_ms: u64,
    pub process_probe: bool,
    pub disk_probe: bool,
    pub network_probe: bool,
    pub power_probe: bool,
    pub gpu_probe: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelfResourceSummary {
    pub cpu_time_100ns: Option<SelfMetricSummary>,
    pub probe_cpu_share_percent: Option<SelfMetricSummary>,
    pub working_set_bytes: Option<SelfMetricSummary>,
    pub thread_count: Option<SelfMetricSummary>,
    pub handle_count: Option<SelfMetricSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelfMetricSummary {
    pub unit: String,
    pub sample_count: usize,
    pub start: Option<f64>,
    pub average: Option<f64>,
    pub peak: Option<f64>,
    pub end: Option<f64>,
    pub delta: Option<f64>,
    pub values: Distribution,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeferredItem {
    pub item: String,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SamplingSummary {
    pub wall_duration_ms: u64,
    pub core_expected_samples: u64,
    pub core_executed_samples: u64,
    pub core_dropped_samples: u64,
    pub process_expected_samples: u64,
    pub process_executed_samples: u64,
    pub process_dropped_samples: u64,
    pub late_wakeups: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProbeReport {
    pub schema_version: String,
    pub probe_name: String,
    pub started_at_utc: String,
    pub finished_at_utc: String,
    pub machine: MachineInfo,
    pub configuration: TestConfiguration,
    pub devices: Vec<DeviceInfo>,
    pub capabilities: Vec<Capability>,
    pub metrics: Vec<MetricRecord>,
    pub sampling: SamplingSummary,
    pub self_resource_summary: SelfResourceSummary,
    pub privacy: PrivacySummary,
    pub conclusion: Conclusion,
    pub deferred: Vec<DeferredItem>,
    pub rerun_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryReport {
    pub schema_version: String,
    pub probe_name: String,
    pub generated_at_utc: String,
    pub machine: MachineInfo,
    pub devices: Vec<DeviceInfo>,
    pub capabilities: Vec<Capability>,
    pub privacy: PrivacySummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct PrivacySummary {
    pub sanitized: bool,
    pub omitted_fields: Vec<String>,
    pub process_detail_retention: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conclusion {
    pub scope: String,
    pub default_budget_comparison: BudgetComparison,
    pub cross_hardware_status: String,
    pub permission_scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetComparison {
    pub average_probe_cpu_share_percent: Option<f64>,
    pub probe_cpu_share_under_0_5_percent: Option<bool>,
    pub steady_state_working_set_bytes: Option<u64>,
    pub steady_state_memory_under_80_mb: Option<bool>,
    pub is_current_machine_experiment: bool,
}

#[cfg(test)]
mod tests {
    use super::{MetricRecord, SupportStatus};

    #[test]
    fn failed_metric_does_not_serialize_a_value() {
        let mut metric = MetricRecord::new(
            "device:test",
            "metric:test",
            "provider:test",
            SupportStatus::Supported,
            "ready",
            "count",
            "test",
            Vec::new(),
        );
        metric.record_failure_with_status(
            SupportStatus::ProbeFailed,
            "sampling_failed".to_string(),
            1.0,
        );
        let json = serde_json::to_string(&metric).unwrap();
        assert!(!json.contains("latest_value"));
        assert!(!json.contains("\"value\""));
        assert_eq!(metric.support_status, SupportStatus::ProbeFailed);
    }

    #[test]
    fn non_success_statuses_do_not_serialize_zero_values() {
        for status in [
            SupportStatus::Unsupported,
            SupportStatus::PermissionDenied,
            SupportStatus::ProbeFailed,
        ] {
            let mut metric = MetricRecord::new(
                "device:test",
                "metric:test",
                "provider:test",
                SupportStatus::Supported,
                "ready",
                "count",
                "test",
                Vec::new(),
            );
            metric.record_failure_with_status(status, "not_readable".to_string(), 1.0);
            let json = serde_json::to_string(&metric).unwrap();
            assert_eq!(metric.support_status, status);
            assert_eq!(metric.sample_count, 0);
            assert_eq!(metric.latest_value, None);
            assert!(!json.contains("latest_value"));
        }
    }

    #[test]
    fn successful_metric_has_value_and_coverage() {
        let mut metric = MetricRecord::new(
            "device:test",
            "metric:test",
            "provider:test",
            SupportStatus::Supported,
            "ready",
            "count",
            "test",
            Vec::new(),
        );
        metric.record_success(10, 3.0, Some(20), 20, 2.0);
        metric.record_success(30, 4.0, Some(20), 20, 3.0);
        let json = serde_json::to_string(&metric).unwrap();
        assert!(json.contains("latest_value"));
        assert_eq!(metric.covered_duration_ms, 40);
        assert_eq!(metric.sampling_interval.p50, Some(20.0));
        assert_eq!(metric.sampling_interval.p95, Some(20.0));
    }

    #[test]
    fn one_metric_failure_does_not_affect_another_metric() {
        let mut successful = MetricRecord::new(
            "gpu:nvidia:index-0",
            "gpu.utilization_percent",
            "nvidia-nvml",
            SupportStatus::Supported,
            "ready",
            "percent",
            "NVIDIA NVML dynamic runtime",
            Vec::new(),
        );
        let mut failed = MetricRecord::new(
            "gpu:nvidia:index-0",
            "gpu.power_watts",
            "nvidia-nvml",
            SupportStatus::Supported,
            "ready",
            "W",
            "NVIDIA NVML dynamic runtime",
            Vec::new(),
        );
        successful.record_success(100, 42.0, Some(2_000), 2_000, 0.1);
        failed.record_failure_with_status(
            SupportStatus::Unsupported,
            "nvml_not_supported".to_string(),
            0.2,
        );
        assert_eq!(successful.support_status, SupportStatus::Supported);
        assert_eq!(successful.sample_count, 1);
        assert_eq!(failed.support_status, SupportStatus::Unsupported);
        assert_eq!(failed.sample_count, 0);
        assert_eq!(failed.latest_value, None);
    }
}
