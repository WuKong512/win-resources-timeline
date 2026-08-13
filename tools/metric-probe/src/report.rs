use crate::model::{InventoryReport, MetricRecord, ProbeReport, SupportStatus};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn write_probe_report(
    output_dir: &Path,
    report: &ProbeReport,
) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("create output directory failed: {error}"))?;
    let json_path = output_dir.join("report.json");
    let markdown_path = output_dir.join("report.md");
    let json = serde_json::to_vec_pretty(report)
        .map_err(|error| format!("serialize JSON failed: {error}"))?;
    let markdown = render_probe_markdown(report);
    validate_public_text(std::str::from_utf8(&json).unwrap_or_default())?;
    validate_public_text(&markdown)?;
    write_atomic(&json_path, &json)?;
    write_atomic(&markdown_path, markdown.as_bytes())?;
    Ok((json_path, markdown_path))
}

pub fn write_inventory_json(report: &InventoryReport) -> Result<String, String> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("serialize inventory failed: {error}"))?;
    validate_public_text(&json)?;
    Ok(json)
}

pub fn render_probe_markdown(report: &ProbeReport) -> String {
    let mut output = String::new();
    output.push_str("# Spike-01A Windows Metric Probe\n\n");
    output.push_str(&format!("- Schema: `{}`\n", report.schema_version));
    output.push_str(&format!("- Started: `{}`\n", report.started_at_utc));
    output.push_str(&format!("- Finished: `{}`\n", report.finished_at_utc));
    output.push_str(&format!("- Scope: {}\n\n", report.conclusion.scope));

    output.push_str("## Machine\n\n");
    output.push_str(&format!("- OS: {}", report.machine.os_name));
    if let Some(version) = &report.machine.os_display_version {
        output.push_str(&format!(" {version}"));
    }
    if let Some(build) = &report.machine.os_build {
        output.push_str(&format!(" build {build}"));
    }
    output.push('\n');
    output.push_str(&format!(
        "- Architecture: {}\n",
        report.machine.architecture
    ));
    output.push_str(&format!(
        "- CPU: {}\n",
        report.machine.cpu_model.as_deref().unwrap_or("unknown")
    ));
    output.push_str(&format!(
        "- Logical processors: {}\n",
        report
            .machine
            .logical_processor_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ));
    output.push_str(&format!(
        "- Physical memory: {}\n",
        report
            .machine
            .memory_total_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "unknown".to_string())
    ));
    output.push_str(&format!(
        "- Elevated process: {}\n\n",
        report
            .machine
            .elevated
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ));

    output.push_str("## Configuration\n\n");
    output.push_str(&format!(
        "- Duration: {} seconds\n- Core interval: {} ms\n- Process interval: {} ms\n- Process probe: {}\n- Disk probe: {}\n- Network probe: {}\n- Power probe: {}\n\n",
        report.configuration.duration_seconds,
        report.configuration.core_interval_ms,
        report.configuration.process_interval_ms,
        report.configuration.process_probe,
        report.configuration.disk_probe,
        report.configuration.network_probe,
        report.configuration.power_probe
    ));

    output.push_str("## Devices\n\n| Device | Category | Present | Classification | Details |\n|---|---|---:|---|---|\n");
    for device in &report.devices {
        output.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            device.device_key,
            device.category,
            option_bool(device.present),
            device.classification,
            format_details(&device.details)
        ));
    }
    output.push('\n');

    output.push_str("## Capabilities\n\n| Device | Category | Provider | Status | Reason |\n|---|---|---|---|---|\n");
    for capability in &report.capabilities {
        output.push_str(&format!(
            "| `{}` | {} | `{}` | `{}` | {} |\n",
            capability.device_key,
            capability.category,
            capability.provider,
            status_name(capability.support_status),
            capability.reason_code
        ));
    }
    output.push('\n');

    output.push_str("## Metrics\n\n| Device | Metric | Provider | Status | Samples | Failed | Coverage | Interval P50 | Interval P95 | Interval Max | Call P50 | Call P95 | Call Max | Latest |\n|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for metric in &report.metrics {
        output.push_str(&format_metric_row(metric));
    }
    output.push('\n');

    output.push_str("Process detail permission-denied count is the number of processes with at least one denied child read; it is not the sum of denied API calls.\n\n");

    output.push_str("## Sampling\n\n");
    output.push_str(&format!(
        "- Wall duration: {} ms\n- Core expected/executed/dropped: {}/{}/{}\n- Process expected/executed/dropped: {}/{}/{}\n- Late wakeups: {}\n\n",
        report.sampling.wall_duration_ms,
        report.sampling.core_expected_samples,
        report.sampling.core_executed_samples,
        report.sampling.core_dropped_samples,
        report.sampling.process_expected_samples,
        report.sampling.process_executed_samples,
        report.sampling.process_dropped_samples,
        report.sampling.late_wakeups
    ));

    output.push_str("## Probe Resources\n\n");
    for (name, summary) in [
        ("CPU time", &report.self_resource_summary.cpu_time_100ns),
        (
            "Probe CPU share",
            &report.self_resource_summary.probe_cpu_share_percent,
        ),
        (
            "Working set",
            &report.self_resource_summary.working_set_bytes,
        ),
        ("Threads", &report.self_resource_summary.thread_count),
        ("Handles", &report.self_resource_summary.handle_count),
    ] {
        if let Some(summary) = summary {
            output.push_str(&format!(
                "- {} ({}): start={}, average={}, peak={}, end={}, delta={}\n",
                name,
                summary.unit,
                display_number(summary.start),
                display_number(summary.average),
                display_number(summary.peak),
                display_number(summary.end),
                display_number(summary.delta)
            ));
        }
    }
    output.push('\n');

    output.push_str("## Conclusion\n\n");
    output.push_str(&format!(
        "- Current-machine scope: {}\n",
        report.conclusion.scope
    ));
    output.push_str(&format!(
        "- Average probe CPU share of whole machine: {}\n- Probe CPU share under 0.5%: {}\n- Steady-state working set: {}\n- Steady-state memory under 80 MB: {}\n- Cross-hardware status: {}\n- Permission scope: {}\n\n",
        display_number(report.conclusion.default_budget_comparison.average_probe_cpu_share_percent),
        option_bool(report.conclusion.default_budget_comparison.probe_cpu_share_under_0_5_percent),
        report
            .conclusion
            .default_budget_comparison
            .steady_state_working_set_bytes
            .map(format_bytes)
            .unwrap_or_else(|| "unknown".to_string()),
        option_bool(report.conclusion.default_budget_comparison.steady_state_memory_under_80_mb),
        report.conclusion.cross_hardware_status,
        report.conclusion.permission_scope
    ));

    output.push_str("## Privacy\n\n");
    output.push_str(&format!(
        "- Sanitized: {}\n- Process detail retention: {}\n- Omitted fields: {}\n\n",
        report.privacy.sanitized,
        report.privacy.process_detail_retention,
        report.privacy.omitted_fields.join(", ")
    ));

    output.push_str("## Deferred\n\n");
    for item in &report.deferred {
        output.push_str(&format!(
            "- **{}**: {} ({})\n",
            item.item, item.status, item.reason
        ));
    }
    output.push('\n');

    output.push_str("## Re-run Commands\n\n");
    for command in &report.rerun_commands {
        output.push_str(&format!("```powershell\n{command}\n```\n\n"));
    }
    output
}

pub fn validate_public_text(text: &str) -> Result<(), String> {
    let lower = text.to_ascii_lowercase();
    let forbidden = [
        "username",
        "computer_name",
        "serial_number",
        "mac_address",
        "ip_address",
        "command_line",
        "window_title",
        "executable_path",
        "process_name",
        "user_directory",
        "windows\\users\\",
        "c:\\users\\",
        "\\users\\",
        "s-1-5-",
    ];
    if let Some(marker) = forbidden.iter().find(|marker| lower.contains(**marker)) {
        return Err(format!("privacy scan failed on marker '{marker}'"));
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "output path has no parent".to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("report"),
        process::id(),
        nonce
    ));
    let result = (|| {
        let mut file = File::create(&temp_path)
            .map_err(|error| format!("create temporary report failed: {error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("write temporary report failed: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync temporary report failed: {error}"))?;
        replace_file(&temp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> Result<(), String> {
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING},
    };
    let source = wide_path(source)?;
    let target = wide_path(target)?;
    unsafe {
        MoveFileExW(
            PCWSTR::from_raw(source.as_ptr()),
            PCWSTR::from_raw(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING,
        )
    }
    .map_err(|error| format!("atomic report replace failed: {error}"))
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> Result<(), String> {
    fs::rename(source, target).map_err(|error| format!("atomic report replace failed: {error}"))
}

#[cfg(windows)]
fn wide_path(path: &Path) -> Result<Vec<u16>, String> {
    use std::os::windows::ffi::OsStrExt;
    Ok(path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect())
}

fn status_name(status: SupportStatus) -> &'static str {
    match status {
        SupportStatus::Supported => "supported",
        SupportStatus::Unsupported => "unsupported",
        SupportStatus::PermissionDenied => "permission_denied",
        SupportStatus::ProviderMissing => "provider_missing",
        SupportStatus::ProbeFailed => "probe_failed",
    }
}

fn format_metric_row(metric: &MetricRecord) -> String {
    format!(
        "| `{}` | `{}` | `{}` | `{}` | {} | {} | {} ms | {} | {} | {} | {} | {} | {} | {} |\n",
        metric.device_key,
        metric.metric_key,
        metric.provider,
        status_name(metric.support_status),
        metric.sample_count,
        metric.failed_sample_count,
        metric.covered_duration_ms,
        display_number(metric.sampling_interval.p50),
        display_number(metric.sampling_interval.p95),
        display_number(metric.sampling_interval.max),
        display_number(metric.call_latency_ms.p50),
        display_number(metric.call_latency_ms.p95),
        display_number(metric.call_latency_ms.max),
        display_number(metric.latest_value)
    )
}

fn format_details(details: &std::collections::BTreeMap<String, String>) -> String {
    details
        .iter()
        .map(|(key, value)| format!("{}={}", key, value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = value as f64;
    let mut index = 0;
    while value >= 1024.0 && index < UNITS.len() - 1 {
        value /= 1024.0;
        index += 1;
    }
    format!("{value:.2} {}", UNITS[index])
}

fn option_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn display_number(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:.3}"))
        .unwrap_or_else(|| "n/a".to_string())
}

#[cfg(test)]
mod tests {
    use super::{render_probe_markdown, validate_public_text, write_atomic};
    use crate::model::{
        BudgetComparison, Conclusion, DeferredItem, DeviceInfo, InventoryReport, MachineInfo,
        MetricRecord, PrivacySummary, ProbeReport, SamplingSummary, SelfResourceSummary,
        SupportStatus, TestConfiguration,
    };
    use crate::stats::Distribution;
    use std::{collections::BTreeMap, fs, path::PathBuf};

    fn fixture() -> ProbeReport {
        ProbeReport {
            schema_version: "spike-01a/v1".to_string(),
            probe_name: "metric-probe".to_string(),
            started_at_utc: "2026-08-13T00:00:00Z".to_string(),
            finished_at_utc: "2026-08-13T00:00:01Z".to_string(),
            machine: MachineInfo {
                os_name: "Windows".to_string(),
                os_display_version: Some("11".to_string()),
                os_build: Some("1".to_string()),
                architecture: "x64".to_string(),
                cpu_model: Some("Test CPU".to_string()),
                logical_processor_count: Some(8),
                memory_total_bytes: Some(1024),
                elevated: Some(false),
            },
            configuration: TestConfiguration {
                duration_seconds: 1,
                core_interval_ms: 100,
                process_interval_ms: 200,
                process_probe: true,
                disk_probe: true,
                network_probe: true,
                power_probe: true,
            },
            devices: vec![DeviceInfo {
                device_key: "device:test".to_string(),
                category: "test".to_string(),
                present: Some(true),
                classification: "test".to_string(),
                details: BTreeMap::new(),
            }],
            capabilities: Vec::new(),
            metrics: vec![MetricRecord::new(
                "device:test",
                "metric:test",
                "provider:test",
                SupportStatus::Unsupported,
                "not_available",
                "count",
                "test",
                Vec::new(),
            )],
            sampling: SamplingSummary {
                wall_duration_ms: 1000,
                ..Default::default()
            },
            self_resource_summary: SelfResourceSummary {
                cpu_time_100ns: None,
                probe_cpu_share_percent: None,
                working_set_bytes: None,
                thread_count: None,
                handle_count: None,
            },
            privacy: PrivacySummary {
                sanitized: true,
                omitted_fields: vec!["user data".to_string()],
                process_detail_retention: "memory only".to_string(),
            },
            conclusion: Conclusion {
                scope: "current machine".to_string(),
                default_budget_comparison: BudgetComparison {
                    average_probe_cpu_share_percent: None,
                    probe_cpu_share_under_0_5_percent: None,
                    steady_state_working_set_bytes: None,
                    steady_state_memory_under_80_mb: None,
                    is_current_machine_experiment: true,
                },
                cross_hardware_status: "not validated".to_string(),
                permission_scope: "non-admin".to_string(),
            },
            deferred: vec![DeferredItem {
                item: "GPU usage".to_string(),
                status: "deferred".to_string(),
                reason: "not implemented".to_string(),
            }],
            rerun_commands: vec![
                "cargo run --manifest-path tools/metric-probe/Cargo.toml -- run".to_string(),
            ],
        }
    }

    #[test]
    fn markdown_contains_the_same_metric_identity() {
        let report = fixture();
        let markdown = render_probe_markdown(&report);
        assert!(markdown.contains("metric:test"));
        assert!(markdown.contains("unsupported"));
    }

    #[test]
    fn privacy_scan_rejects_identifying_fields() {
        assert!(validate_public_text("safe output").is_ok());
        assert!(validate_public_text("command_line").is_err());
        assert!(validate_public_text("C:\\Users\\someone\\file.exe").is_err());
        assert!(validate_public_text("Windows SID is intentionally omitted").is_ok());
        assert!(validate_public_text("S-1-5-18").is_err());
    }

    #[test]
    fn atomic_output_replaces_existing_files() {
        let root = std::env::temp_dir().join(format!("metric-probe-report-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("report.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn json_and_markdown_keep_metric_identity_and_status_consistent() {
        let report = fixture();
        let json = serde_json::to_value(&report).unwrap();
        let metric = &json["metrics"][0];
        let status = metric["support_status"].as_str().unwrap();
        let markdown = render_probe_markdown(&report);
        assert!(markdown.contains("device:test"));
        assert!(markdown.contains("metric:test"));
        assert!(markdown.contains(status));
    }

    #[allow(dead_code)]
    fn _keep_imports(_: InventoryReport, _: Distribution, _: PathBuf) {}
}
