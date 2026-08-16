use serde::{Deserialize, Serialize};

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
pub struct SystemSample {
    pub timestamp_ms: i64,
    pub sample_duration_ms: i64,
    pub cpu_percent: Option<f64>,
    pub memory_percent: Option<f64>,
    pub memory_used_bytes: Option<i64>,
    pub memory_total_bytes: Option<i64>,
    pub disk_read_bytes_per_sec: Option<i64>,
    pub disk_write_bytes_per_sec: Option<i64>,
    pub has_app_snapshot: bool,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CollectionSettings {
    pub foreground_poll_interval_ms: u64,
    pub system_sample_interval_ms: u64,
    pub idle_threshold_seconds: u64,
    pub system_sample_retention_days: u64,
}

impl Default for CollectionSettings {
    fn default() -> Self {
        Self {
            foreground_poll_interval_ms: 1_000,
            system_sample_interval_ms: 5_000,
            idle_threshold_seconds: 300,
            system_sample_retention_days: 7,
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
        Ok(())
    }
}

#[cfg(test)]
mod collection_settings_tests {
    use super::CollectionSettings;

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
}
