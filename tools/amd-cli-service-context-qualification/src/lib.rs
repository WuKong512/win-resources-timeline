//! Qualification-only policy and evidence types for a real LocalSystem service run.
//!
//! This package is deliberately separate from the production collector.  The binary is a
//! one-shot SCM service used to qualify the vendor CLI's Session 0 behavior; it is not a
//! broker, installer, IPC endpoint, or production provider.

use serde::Serialize;
use std::path::{Component, Path, PathBuf};

pub const SERVICE_NAME: &str = "ResourceTimelineAmdQualification";
pub const QUALIFICATION_ONLY: bool = true;
pub const AMD_INSTALL_REGISTRY_KEY: &str = r"SOFTWARE\WOW6432Node\AMD\AMDProfiler";
pub const AMD_INSTALL_REGISTRY_VALUE: &str = "InstallationPath";
pub const AMD_CLI_NAME: &str = "AMDuProfCLI.exe";
pub const EXPECTED_AMD_CLI_SHA256: &str =
    "D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC";
pub const EXPECTED_AMD_CLI_VERSION: &str = "5.3.521.0";
pub const CLI_TIMEOUT_MS: u64 = 30_000;
pub const SERVICE_TIMEOUT_MS: u64 = 45_000;
pub const PROFILE_DURATION_SECONDS: u32 = 10;
pub const PROFILE_INTERVAL_MS: u32 = 1_000;
pub const FIXED_PROFILE_EVENT: &str = "power";
pub const EXPECTED_LOCAL_SYSTEM_SID: &str = "S-1-5-18";

/// Convert a signed Windows process exit value without overflowing on `-1`.
pub fn exit_code_hex(exit_code: i32) -> String {
    let unsigned = u32::from_ne_bytes(exit_code.to_ne_bytes());
    format!("0x{unsigned:08X}")
}

/// The only service argument accepted by this qualification binary.
///
/// The executable, AMD arguments, working directory, and environment are all derived by the
/// service itself.  The run root is accepted only as a single child directory of the controlled
/// ProgramData qualification base; it is not an arbitrary output path.
pub fn validate_service_arguments(args: &[String], allowed_base: &Path) -> Result<PathBuf, String> {
    if args.len() != 2 || args[0] != "--run-root" {
        return Err("only --run-root <controlled child directory> is accepted".to_owned());
    }
    if !allowed_base.is_absolute() {
        return Err("qualification base must be absolute".to_owned());
    }

    let run_root = PathBuf::from(&args[1]);
    if !run_root.is_absolute() {
        return Err("run root must be absolute".to_owned());
    }
    if run_root
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err("run root must not contain . or .. path components".to_owned());
    }
    if run_root.parent() != Some(allowed_base) {
        return Err("run root must be exactly one child of the controlled base".to_owned());
    }
    if run_root.file_name().is_none() {
        return Err("run root must have a run identifier".to_owned());
    }
    Ok(run_root)
}

/// Fixed vendor command arguments.  No caller or service parameter can replace these values.
pub fn fixed_cli_arguments(output_dir: &Path) -> Vec<String> {
    vec![
        "timechart".to_owned(),
        "--event".to_owned(),
        FIXED_PROFILE_EVENT.to_owned(),
        "--interval".to_owned(),
        PROFILE_INTERVAL_MS.to_string(),
        "--duration".to_owned(),
        PROFILE_DURATION_SECONDS.to_string(),
        "--format".to_owned(),
        "csv".to_owned(),
        "--output-dir".to_owned(),
        output_dir.to_string_lossy().into_owned(),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceState {
    StartPending,
    Running,
    StopPending,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceStatusRecord {
    pub service_name: String,
    pub qualification_only: bool,
    pub state: ServiceState,
    pub recorded_at_utc_unix_ms: u128,
    pub win32_exit_code: u32,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceStatusJournal {
    pub service_name: String,
    pub qualification_only: bool,
    pub states: Vec<ServiceStatusRecord>,
}

impl ServiceStatusJournal {
    pub fn new() -> Self {
        Self {
            service_name: SERVICE_NAME.to_owned(),
            qualification_only: QUALIFICATION_ONLY,
            states: Vec::new(),
        }
    }

    pub fn record(&mut self, state: ServiceState, win32_exit_code: u32, detail: impl Into<String>) {
        self.states.push(ServiceStatusRecord {
            service_name: SERVICE_NAME.to_owned(),
            qualification_only: QUALIFICATION_ONLY,
            state,
            recorded_at_utc_unix_ms: unix_time_millis(),
            win32_exit_code,
            detail: detail.into(),
        });
    }

    pub fn final_state(&self) -> Option<ServiceState> {
        self.states.last().map(|record| record.state)
    }
}

impl Default for ServiceStatusJournal {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceContextEvidence {
    pub service_name: String,
    pub qualification_only: bool,
    pub process_id: u32,
    pub parent_process_id: Option<u32>,
    pub account: String,
    pub account_sid: String,
    pub account_is_local_system: bool,
    pub session_id: Option<u32>,
    pub integrity_sid: Option<String>,
    pub integrity_level: Option<String>,
    pub token_elevated: Option<bool>,
    pub token_elevation_type: Option<String>,
    pub process_architecture: String,
    pub current_directory: Option<String>,
    pub environment_subset: std::collections::BTreeMap<String, String>,
    pub service_started_at_utc_unix_ms: u128,
}

impl ServiceContextEvidence {
    pub fn satisfies_local_system_session_zero(&self) -> bool {
        self.qualification_only
            && self.service_name == SERVICE_NAME
            && self.account_is_local_system
            && self
                .account_sid
                .eq_ignore_ascii_case(EXPECTED_LOCAL_SYSTEM_SID)
            && self.session_id == Some(0)
            && self.integrity_sid.is_some()
            && self.integrity_level.is_some()
            && self.token_elevated == Some(true)
            && self.token_elevation_type.is_some()
            && self.process_architecture == "x64"
            && self.current_directory.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliArtifactIdentity {
    pub path: String,
    pub sha256: String,
    pub expected_sha256: String,
    pub architecture: String,
    pub expected_version: String,
    pub signature_validation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliProcessResult {
    pub executable: String,
    pub arguments: Vec<String>,
    pub working_directory: String,
    pub output_directory: String,
    pub process_started: bool,
    pub target_pid: Option<u32>,
    pub started_at_utc_unix_ms: u128,
    pub finished_at_utc_unix_ms: u128,
    pub duration_ms: u128,
    pub timeout_ms: u64,
    pub timeout: bool,
    pub cancelled: bool,
    pub target_exit_signed: Option<i32>,
    pub target_exit_hex: Option<String>,
    pub target_process_failed: bool,
    pub stdout_path: String,
    pub stderr_path: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub stdout_persisted: bool,
    pub stderr_persisted: bool,
    pub capture_complete: bool,
    pub job_assigned: bool,
    pub cleanup_attempted: bool,
    pub cleanup_succeeded: bool,
    pub harness_failed: bool,
    pub harness_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceQualificationResult {
    pub schema: String,
    pub service_name: String,
    pub qualification_only: bool,
    pub service_context_valid: bool,
    pub cli_identity_validated_by_wrapper: bool,
    pub cli_process_result_path: String,
    pub qualification: String,
    pub created_at_utc_unix_ms: u128,
}

pub fn unix_time_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(value)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(path, json)
}

pub fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn signed_exit_code_serialization_preserves_windows_bits() {
        assert_eq!(exit_code_hex(0), "0x00000000");
        assert_eq!(exit_code_hex(1), "0x00000001");
        assert_eq!(exit_code_hex(-1), "0xFFFFFFFF");
    }

    #[test]
    fn fixed_command_is_narrow_and_deterministic() {
        let args = fixed_cli_arguments(Path::new(r"C:\ProgramData\run\timechart-output"));
        assert_eq!(args[0], "timechart");
        assert_eq!(args[2], "power");
        assert_eq!(args[4], "1000");
        assert_eq!(args[6], "10");
        assert_eq!(args[8], "csv");
        assert!(!args.iter().any(|arg| arg == "--help" || arg == "--version"));
    }

    #[test]
    fn service_arguments_reject_arbitrary_command_surfaces() {
        let base = Path::new(r"C:\ProgramData\ResourceTimeline\qualification\amd-service-context");
        let valid = vec![
            "--run-root".to_owned(),
            r"C:\ProgramData\ResourceTimeline\qualification\amd-service-context\run-1".to_owned(),
        ];
        assert!(validate_service_arguments(&valid, base).is_ok());
        assert!(validate_service_arguments(&[], base).is_err());
        assert!(validate_service_arguments(
            &["--cli-path".to_owned(), "C:\\evil.exe".to_owned()],
            base
        )
        .is_err());
        assert!(validate_service_arguments(
            &[
                "--run-root".to_owned(),
                r"C:\ProgramData\other\run-1".to_owned(),
            ],
            base
        )
        .is_err());
    }

    #[test]
    fn service_status_journal_keeps_protocol_sequence() {
        let mut journal = ServiceStatusJournal::new();
        journal.record(ServiceState::StartPending, 0, "registered");
        journal.record(ServiceState::Running, 0, "context verified");
        journal.record(ServiceState::Stopped, 0, "completed");
        assert_eq!(journal.states.len(), 3);
        assert_eq!(journal.final_state(), Some(ServiceState::Stopped));
    }

    #[test]
    fn local_system_context_gate_requires_session_zero_and_x64() {
        let context = ServiceContextEvidence {
            service_name: SERVICE_NAME.to_owned(),
            qualification_only: true,
            process_id: 1,
            parent_process_id: Some(2),
            account: "NT AUTHORITY\\SYSTEM".to_owned(),
            account_sid: EXPECTED_LOCAL_SYSTEM_SID.to_owned(),
            account_is_local_system: true,
            session_id: Some(0),
            integrity_sid: Some("S-1-16-16384".to_owned()),
            integrity_level: Some("S-1-16-16384".to_owned()),
            token_elevated: Some(true),
            token_elevation_type: Some("Full".to_owned()),
            process_architecture: "x64".to_owned(),
            current_directory: Some(r"C:\ProgramData".to_owned()),
            environment_subset: Default::default(),
            service_started_at_utc_unix_ms: 1,
        };
        assert!(context.satisfies_local_system_session_zero());

        let mut incomplete = context.clone();
        incomplete.token_elevated = None;
        assert!(!incomplete.satisfies_local_system_session_zero());

        let mut incomplete = context;
        incomplete.current_directory = None;
        assert!(!incomplete.satisfies_local_system_session_zero());
    }

    #[test]
    fn qualification_result_serializes_for_wrapper_consumption() {
        let result = ServiceQualificationResult {
            schema: "cpu-sensor-amd-service-context/v1".to_owned(),
            service_name: SERVICE_NAME.to_owned(),
            qualification_only: true,
            service_context_valid: false,
            cli_identity_validated_by_wrapper: true,
            cli_process_result_path: "result.json".to_owned(),
            qualification: "SERVICE_HARNESS_FAILED".to_owned(),
            created_at_utc_unix_ms: 1,
        };
        let value = serde_json::to_value(result).expect("serializable result");
        assert_eq!(value["qualification"], "SERVICE_HARNESS_FAILED");
        assert_eq!(value["qualification_only"], true);
    }
}
