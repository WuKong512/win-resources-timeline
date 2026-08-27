use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricCategory {
    Cpu,
    Gpu,
    Memory,
    Disk,
    Network,
    Power,
    Battery,
    Process,
}

impl MetricCategory {
    pub const ALL: [Self; 8] = [
        Self::Cpu,
        Self::Gpu,
        Self::Memory,
        Self::Disk,
        Self::Network,
        Self::Power,
        Self::Battery,
        Self::Process,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::Memory => "memory",
            Self::Disk => "disk",
            Self::Network => "network",
            Self::Power => "power",
            Self::Battery => "battery",
            Self::Process => "process",
        }
    }
}

pub const GPU_BOARD_POWER_SCOPE: &str = "gpu_board";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CapabilitySupportStatus {
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CapabilityState {
    SupportedEnabled,
    SupportedDisabled,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderLifecycleState {
    Stopped,
    Running,
    Paused,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderErrorCode {
    ProviderMissing,
    PermissionDenied,
    StartupFailed,
    ReconfigureFailed,
    SampleFailed,
    RuntimeFailed,
    StopFailed,
    Timeout,
    Unsupported,
    UserDisabled,
    CategoryDisabled,
    Paused,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderErrorSummary {
    pub code: ProviderErrorCode,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MetricCapabilityStatus {
    pub provider_id: String,
    pub category: MetricCategory,
    pub support_status: CapabilitySupportStatus,
    pub enabled: bool,
    pub can_toggle: bool,
    pub state: CapabilityState,
    pub reason_code: Option<ProviderErrorCode>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider_id: String,
    pub display_name: String,
    pub supported: bool,
    pub enabled: bool,
    pub lifecycle: ProviderLifecycleState,
    pub capabilities: Vec<MetricCapabilityStatus>,
    pub last_success_at_ms: Option<i64>,
    pub failure_count: u64,
    pub last_error: Option<ProviderErrorSummary>,
}

/// Runtime truth for a metric within the current collection session.
///
/// This deliberately uses the same vocabulary as `collection_session_metric`, while keeping
/// value presence independent from capability state: a legal numeric zero is still a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricRuntimeSupportStatus {
    Supported,
    Unsupported,
    PermissionDenied,
    ProviderMissing,
    ProbeFailed,
    Failed,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MetricCatalogDevice {
    pub stable_key: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub capacity_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MetricCatalogEntry {
    pub metric_key: String,
    pub category: MetricCategory,
    pub provider_id: String,
    pub device: Option<MetricCatalogDevice>,
    pub enabled: bool,
    pub support_status: MetricRuntimeSupportStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetricCatalogSnapshot {
    pub metrics: Vec<MetricCatalogEntry>,
    pub devices: Vec<MetricCatalogDevice>,
}

impl MetricRuntimeSupportStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::PermissionDenied => "permission_denied",
            Self::ProviderMissing => "provider_missing",
            Self::ProbeFailed => "probe_failed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDeviceMetadata {
    pub stable_key: String,
    pub category: MetricCategory,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub capacity_bytes: Option<i64>,
}

/// Per-metric runtime capability reported by a production provider. Providers only return this
/// data; the collector owns persistence through the existing writer boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderMetricMetadata {
    pub category: MetricCategory,
    pub metric_key: String,
    pub device: Option<RuntimeDeviceMetadata>,
    pub support_status: MetricRuntimeSupportStatus,
}

/// Metadata ready for `collection_session_metric` persistence after ProviderHost has reconciled
/// provider state with the desired CollectionPlan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionSessionMetricMetadata {
    pub provider_id: String,
    pub category: MetricCategory,
    pub metric_key: String,
    pub device: Option<RuntimeDeviceMetadata>,
    pub enabled: bool,
    pub support_status: MetricRuntimeSupportStatus,
    pub interval_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundApp {
    /// Stable logical-app key. The executable path is stored separately in app_executable.
    pub identity_key: String,
    pub process_name: String,
    pub exe_path: Option<String>,
    pub display_name: String,
    pub pid: Option<u32>,
    pub process_creation_time_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputerState {
    Active,
    Idle,
    Locked,
    Sleep,
    Disconnected,
    #[allow(dead_code)]
    Unknown,
}

impl ComputerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::Locked => "locked",
            Self::Sleep => "sleep",
            Self::Disconnected => "disconnected",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootIdentity {
    pub boot_id: String,
    pub boot_time_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityState {
    Active,
    #[allow(dead_code)]
    Idle,
}

impl ActivityState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppIdentity {
    pub id: i64,
    pub process_name: String,
    pub exe_path: Option<String>,
    pub display_name: String,
    pub publisher: Option<String>,
    pub is_hidden: bool,
    pub first_seen_at_ms: i64,
    pub last_seen_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundInterval {
    pub id: i64,
    pub app_id: i64,
    pub app_name: String,
    pub display_name: String,
    pub start_time_ms: i64,
    pub end_time_ms: i64,
    pub duration_ms: i64,
    pub activity_state: String,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageSummary {
    pub app_id: i64,
    pub app_name: String,
    pub display_name: String,
    pub foreground_total_ms: i64,
    pub active_usage_ms: i64,
    pub idle_foreground_ms: i64,
    pub active_seconds: i64,
    pub idle_seconds: i64,
    pub percentage: f64,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsageSummary {
    pub local_date: String,
    pub app_id: i64,
    pub app_name: String,
    pub display_name: String,
    pub foreground_total_ms: i64,
    pub active_usage_ms: i64,
    pub idle_foreground_ms: i64,
    pub launch_count: i64,
    pub processing_version: String,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerStateInterval {
    pub state: String,
    pub start_time_ms: i64,
    pub end_time_ms: i64,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    pub start_ms: i64,
    pub end_ms: i64,
    pub observed_until_ms: Option<i64>,
    pub coverage: f64,
    pub computer_active_seconds: i64,
    pub state_intervals: Vec<ComputerStateInterval>,
    pub apps: Vec<AppUsageSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuSample {
    /// Provider-assigned stable runtime identity; this is not a serial number or array index.
    pub device_key: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub capacity_bytes: Option<i64>,
    pub utilization_percent: Option<f64>,
    pub memory_controller_utilization_percent: Option<f64>,
    pub temperature_celsius: Option<f64>,
    pub power_watts: Option<f64>,
    pub graphics_clock_mhz: Option<f64>,
    pub memory_clock_mhz: Option<f64>,
    pub vram_used_bytes: Option<i64>,
    pub vram_total_bytes: Option<i64>,
    pub power_scope: Option<String>,
    pub quality_mask: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GpuSamplePoint {
    pub timestamp_ms: i64,
    pub sample_duration_ms: i64,
    pub gpu: GpuSample,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSample {
    pub timestamp_ms: i64,
    pub sample_duration_ms: i64,
    pub cpu_percent: Option<f64>,
    pub memory_percent: Option<f64>,
    pub memory_used_bytes: Option<i64>,
    pub memory_total_bytes: Option<i64>,
    pub disk_read_bytes_per_sec: Option<i64>,
    pub disk_write_bytes_per_sec: Option<i64>,
    #[serde(default)]
    pub gpus: Vec<GpuSample>,
    pub has_app_snapshot: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineGap {
    pub start_ms: i64,
    pub end_ms: i64,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTimeline {
    pub start_ms: i64,
    pub end_ms: i64,
    pub observed_ms: i64,
    pub coverage: f64,
    pub samples: Vec<SystemSample>,
    pub gaps: Vec<TimelineGap>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppResourceSample {
    pub app_key: String,
    pub process_name: String,
    pub exe_path: Option<String>,
    pub process_count: i64,
    pub cpu_percent: f64,
    pub memory_used_bytes: i64,
    pub io_read_bytes_per_sec: i64,
    pub io_write_bytes_per_sec: i64,
    /// Raw process identity. Logical-app aggregation remains a query/rollup concern.
    pub process_identity_key: Option<String>,
    pub pid: Option<u32>,
    pub process_creation_time_ms: Option<i64>,
    pub private_bytes: Option<i64>,
    pub cpu_time_delta_us: Option<i64>,
    pub gpu_percent: Option<f64>,
    pub vram_bytes: Option<i64>,
    pub network_bytes_per_sec: Option<i64>,
    pub selection_reason: i64,
    pub quality_mask: i64,
    /// These preserve SQL NULL semantics for raw process metrics while the legacy numeric fields
    /// keep the existing resource-page DTO compatible until PR-06 replaces it.
    pub measured_cpu_percent: Option<f64>,
    pub measured_working_set_bytes: Option<i64>,
    pub measured_read_bytes_per_sec: Option<i64>,
    pub measured_write_bytes_per_sec: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceApp {
    pub app_key: String,
    pub process_name: String,
    pub display_name: String,
    pub exe_path: Option<String>,
    pub last_sample_at_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppResourceHistoryPoint {
    pub timestamp_ms: i64,
    pub sample_duration_ms: i64,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<i64>,
    pub io_read_bytes_per_sec: Option<i64>,
    pub io_write_bytes_per_sec: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    pub system: SystemSample,
    pub apps: Vec<AppResourceSample>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayOverview {
    pub start_ms: i64,
    pub end_ms: i64,
    pub total_active_foreground_seconds: i64,
    pub total_idle_foreground_seconds: i64,
    pub computer_active_seconds: i64,
    pub hidden_active_foreground_seconds: i64,
    pub top_apps: Vec<AppUsageSummary>,
    pub cpu_sampled_peak: Option<f64>,
    pub memory_sampled_peak: Option<f64>,
    pub disk_read_sampled_peak: Option<i64>,
    pub disk_write_sampled_peak: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CollectorStatus {
    pub running: bool,
    pub paused: bool,
    pub started_at_ms: Option<i64>,
    pub last_heartbeat_at_ms: Option<i64>,
    pub last_foreground_sample_at_ms: Option<i64>,
    pub last_system_sample_at_ms: Option<i64>,
    pub dropped_system_samples: u64,
    pub usage_write_failures: u64,
    pub usage_write_retries: u64,
    pub last_usage_write_error: Option<String>,
    pub database_size_bytes: u64,
    pub database_path: String,
    pub provider_status: Vec<ProviderStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub main_bytes: u64,
    pub wal_bytes: u64,
    pub shm_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSettings {
    pub foreground_poll_interval_ms: u64,
    pub system_sample_interval_ms: u64,
    pub idle_threshold_seconds: u64,
    pub system_sample_retention_days: u64,
    #[serde(default = "default_enabled_categories")]
    pub enabled_categories: Vec<MetricCategory>,
    #[serde(default)]
    pub disabled_providers: Vec<String>,
}

fn default_enabled_categories() -> Vec<MetricCategory> {
    vec![
        MetricCategory::Cpu,
        MetricCategory::Memory,
        MetricCategory::Disk,
        MetricCategory::Process,
    ]
}

impl Default for CollectionSettings {
    fn default() -> Self {
        Self {
            foreground_poll_interval_ms: 1_000,
            system_sample_interval_ms: 5_000,
            idle_threshold_seconds: 300,
            system_sample_retention_days: 7,
            enabled_categories: default_enabled_categories(),
            disabled_providers: Vec::new(),
        }
    }
}

impl CollectionSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1_000..=10_000).contains(&self.foreground_poll_interval_ms) {
            return Err("foregroundPollIntervalMs must be between 1000 and 10000".into());
        }
        if !(5_000..=60_000).contains(&self.system_sample_interval_ms) {
            return Err("systemSampleIntervalMs must be between 5000 and 60000".into());
        }
        if !(60..=3_600).contains(&self.idle_threshold_seconds) {
            return Err("idleThresholdSeconds must be between 60 and 3600".into());
        }
        if !(1..=30).contains(&self.system_sample_retention_days) {
            return Err("systemSampleRetentionDays must be between 1 and 30".into());
        }
        if self
            .disabled_providers
            .iter()
            .any(|provider| provider.trim().is_empty())
        {
            return Err("disabledProviders must not contain empty provider ids".into());
        }
        Ok(())
    }
}

pub const DASHBOARD_CONFIG_VERSION: u32 = 1;
pub const DASHBOARD_MAX_CARDS: usize = 12;
pub const DASHBOARD_MAX_METRICS_PER_CARD: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardConfig {
    pub version: u32,
    pub cards: Vec<DashboardCardConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardCardConfig {
    pub id: String,
    pub metric_ids: Vec<String>,
    #[serde(default)]
    pub hidden_metric_ids: Vec<String>,
    pub order: u32,
    pub visible: bool,
}

impl DashboardConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != DASHBOARD_CONFIG_VERSION {
            return Err("dashboard config version is unsupported".into());
        }
        if self.cards.len() > DASHBOARD_MAX_CARDS {
            return Err("dashboard config contains too many cards".into());
        }
        let mut card_ids = HashSet::new();
        let mut orders = HashSet::new();
        for card in &self.cards {
            if card.id.trim().is_empty() || card.id.len() > 64 || !card_ids.insert(card.id.as_str())
            {
                return Err("dashboard card ids must be non-empty and unique".into());
            }
            if card.order as usize >= DASHBOARD_MAX_CARDS || !orders.insert(card.order) {
                return Err("dashboard card order must be bounded and unique".into());
            }
            if card.metric_ids.is_empty() || card.metric_ids.len() > DASHBOARD_MAX_METRICS_PER_CARD
            {
                return Err("dashboard card metric count is out of bounds".into());
            }
            let mut metric_ids = HashSet::new();
            let mut unit_family = None;
            for metric_id in &card.metric_ids {
                if metric_id.trim().is_empty()
                    || metric_id.len() > 256
                    || !metric_ids.insert(metric_id.as_str())
                {
                    return Err("dashboard metric ids must be non-empty and unique".into());
                }
                let family = dashboard_metric_family(metric_id)
                    .ok_or_else(|| format!("dashboard metric id is unsupported: {metric_id}"))?;
                if unit_family.is_some_and(|existing| existing != family) {
                    return Err("dashboard card metrics must use one compatible unit family".into());
                }
                unit_family = Some(family);
            }
            if card.hidden_metric_ids.len() > card.metric_ids.len() {
                return Err("dashboard hidden metric count is out of bounds".into());
            }
            let metric_ids: HashSet<&str> = card.metric_ids.iter().map(String::as_str).collect();
            let mut hidden_ids = HashSet::new();
            for metric_id in &card.hidden_metric_ids {
                if !hidden_ids.insert(metric_id.as_str())
                    || !metric_ids.contains(metric_id.as_str())
                {
                    return Err("dashboard hidden metrics must belong to their card".into());
                }
            }
        }
        Ok(())
    }
}

fn dashboard_metric_family(metric_id: &str) -> Option<&'static str> {
    match metric_id {
        "system.cpu.usage_pct" | "system.memory.usage_pct" => Some("percent"),
        "system.memory.used_bytes" => Some("bytes"),
        "system.disk.read_bps" | "system.disk.write_bps" => Some("throughput"),
        _ => {
            let (device_key, field) = metric_id.strip_prefix("gpu.")?.rsplit_once('.')?;
            if device_key.trim().is_empty() {
                return None;
            }
            match field {
                "utilization_pct" | "memory_controller_utilization_pct" => Some("percent"),
                "temperature_c" => Some("temperature"),
                "board_power_w" => Some("power"),
                "graphics_clock_mhz" | "memory_clock_mhz" => Some("frequency"),
                "vram_used_bytes" | "vram_total_bytes" => Some("bytes"),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod collection_settings_tests {
    use super::{
        CollectionSettings, DashboardCardConfig, DashboardConfig, DASHBOARD_CONFIG_VERSION,
    };

    #[test]
    fn validates_safe_low_overhead_ranges() {
        assert!(CollectionSettings::default().validate().is_ok());
        assert!(CollectionSettings {
            foreground_poll_interval_ms: 500,
            ..CollectionSettings::default()
        }
        .validate()
        .is_err());
        assert!(CollectionSettings {
            system_sample_interval_ms: 60_001,
            ..CollectionSettings::default()
        }
        .validate()
        .is_err());
    }

    fn dashboard(metric_ids: &[&str]) -> DashboardConfig {
        DashboardConfig {
            version: DASHBOARD_CONFIG_VERSION,
            cards: vec![DashboardCardConfig {
                id: "card-1".into(),
                metric_ids: metric_ids.iter().map(|value| (*value).into()).collect(),
                hidden_metric_ids: Vec::new(),
                order: 0,
                visible: true,
            }],
        }
    }

    #[test]
    fn dashboard_accepts_same_axis_metrics_and_rejects_incompatible_or_unknown_ids() {
        assert!(
            dashboard(&["system.cpu.usage_pct", "gpu.device-uuid.unknown"])
                .validate()
                .is_err()
        );
        assert!(
            dashboard(&["system.cpu.usage_pct", "gpu.device-uuid.utilization_pct"])
                .validate()
                .is_ok()
        );
        assert!(
            dashboard(&["system.cpu.usage_pct", "system.memory.used_bytes"])
                .validate()
                .is_err()
        );
        assert!(dashboard(&["system.not_real.value"]).validate().is_err());
    }

    #[test]
    fn dashboard_preserves_stable_gpu_identity_without_using_enumeration_index() {
        let config = dashboard(&["gpu.NVIDIA-UUID-123.temperature_c"]);
        assert!(config.validate().is_ok());
    }
}
