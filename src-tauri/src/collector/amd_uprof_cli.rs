//! Spike-only boundary for the AMD uProf CLI provider.
//!
//! This module deliberately is not registered by `collector::manager`.  It owns the
//! external-process, output parsing, and availability semantics needed to qualify a
//! future `MetricProvider`, while the current `SystemSample`/provider merge path has no
//! CPU package-power value slot for this adapter.  Keeping the module unregistered prevents an
//! unqualified AMD dependency from changing the default collector.
#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime},
};
use thiserror::Error;

use super::provider::{ProviderCapabilitySpec, ProviderDescriptor, ProviderSchedule};
use crate::models::{
    CapabilitySupportStatus, MetricCategory, MetricRuntimeSupportStatus, ProviderErrorCode,
    ProviderMetricMetadata, RuntimeDeviceMetadata,
};

pub const AMD_UPROF_CLI_PROVIDER_ID: &str = "amd-uprof-cli";
pub const AMD_UPROF_CLI_NAME: &str = "AMDuProfCLI.exe";
pub const AMD_UPROF_INSTALL_REGISTRY_KEY: &str = r"SOFTWARE\WOW6432Node\AMD\AMDProfiler";
pub const AMD_UPROF_INSTALL_REGISTRY_VALUE: &str = "InstallationPath";
pub const AMD_UPROF_POWER_METRIC_KEY: &str = "cpu.package.power_w";

/// The first spike is intentionally a bounded file-producing session.  No streaming
/// contract is inferred from the short historical control.
pub const SPIKE_DEFAULT_DURATION: Duration = Duration::from_secs(10);
pub const SPIKE_DEFAULT_SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
pub const SPIKE_DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const CLI_MIN_SAMPLE_INTERVAL_MS: u128 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmdCliAvailability {
    Available,
    NotInstalled,
    UnsupportedVersion,
    PermissionRequired,
    DriverOrServiceUnavailable,
    CounterUnavailable,
    RuntimeFailed,
    ParseFailed,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmdCliSignatureStatus {
    Valid,
    Invalid,
    NotSigned,
    Unknown,
}

impl AmdCliSignatureStatus {
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmdCliArchitecture {
    X64,
    X86,
    Arm64,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmdCliArtifactMetadata {
    pub path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
    pub architecture: AmdCliArchitecture,
    pub file_version: Option<String>,
    pub signature_status: AmdCliSignatureStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmdCliDiscoveryResult {
    pub status: AmdCliAvailability,
    pub install_root: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub artifact: Option<AmdCliArtifactMetadata>,
    pub reason: Option<String>,
}

pub trait AmdCliInstallRootSource: Send + Sync {
    fn read_install_root(&self) -> io::Result<Option<PathBuf>>;
}

pub trait AmdCliArtifactInspector: Send + Sync {
    fn inspect(&self, path: &Path) -> io::Result<AmdCliArtifactMetadata>;
}

/// Reads the 32-bit uProf installation value observed during qualification.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsRegistryInstallRoot;

#[cfg(windows)]
impl AmdCliInstallRootSource for WindowsRegistryInstallRoot {
    fn read_install_root(&self) -> io::Result<Option<PathBuf>> {
        use std::mem::size_of;
        use windows::{
            core::PCWSTR,
            Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ},
        };

        let key = wide(AMD_UPROF_INSTALL_REGISTRY_KEY);
        let value = wide(AMD_UPROF_INSTALL_REGISTRY_VALUE);
        let mut buffer = vec![0_u16; 512];
        let mut bytes = (buffer.len() * size_of::<u16>()) as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                PCWSTR::from_raw(key.as_ptr()),
                PCWSTR::from_raw(value.as_ptr()),
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr() as *mut _),
                Some(&mut bytes),
            )
        };
        if matches!(status.0, 2 | 3) {
            return Ok(None);
        }
        if status.0 != 0 {
            return Err(io::Error::from_raw_os_error(status.0 as i32));
        }
        let length = (bytes as usize / size_of::<u16>()).min(buffer.len());
        let value = String::from_utf16(&buffer[..length])
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .trim_end_matches('\0')
            .trim()
            .to_owned();
        if value.is_empty() {
            return Ok(None);
        }
        Ok(Some(PathBuf::from(value)))
    }
}

#[cfg(not(windows))]
impl AmdCliInstallRootSource for WindowsRegistryInstallRoot {
    fn read_install_root(&self) -> io::Result<Option<PathBuf>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "AMD uProf registry discovery is Windows-only",
        ))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FilesystemAmdCliArtifactInspector;

impl AmdCliArtifactInspector for FilesystemAmdCliArtifactInspector {
    fn inspect(&self, path: &Path) -> io::Result<AmdCliArtifactMetadata> {
        let bytes = fs::read(path)?;
        let metadata = fs::metadata(path)?;
        let digest = Sha256::digest(&bytes);
        Ok(AmdCliArtifactMetadata {
            path: path.to_path_buf(),
            sha256: digest.iter().map(|byte| format!("{byte:02X}")).collect(),
            size_bytes: metadata.len(),
            architecture: detect_pe_architecture(&bytes)?,
            file_version: platform_file_version(path),
            signature_status: platform_signature_status(path),
        })
    }
}

pub struct AmdCliDiscovery {
    root_source: Box<dyn AmdCliInstallRootSource>,
    artifact_inspector: Box<dyn AmdCliArtifactInspector>,
    require_valid_signature: bool,
    accepted_major_version: Option<u64>,
}

impl AmdCliDiscovery {
    pub fn new(
        root_source: Box<dyn AmdCliInstallRootSource>,
        artifact_inspector: Box<dyn AmdCliArtifactInspector>,
    ) -> Self {
        Self {
            root_source,
            artifact_inspector,
            require_valid_signature: true,
            accepted_major_version: None,
        }
    }

    pub fn require_valid_signature(mut self, required: bool) -> Self {
        self.require_valid_signature = required;
        self
    }

    /// A version policy is deliberately explicit.  `None` records the installed version
    /// without pretending that the historical 5.3 build is a universal compatibility rule.
    pub fn accepted_major_version(mut self, major: Option<u64>) -> Self {
        self.accepted_major_version = major;
        self
    }

    pub fn discover(&self) -> AmdCliDiscoveryResult {
        let install_root = match self.root_source.read_install_root() {
            Ok(Some(root)) => root,
            Ok(None) => {
                return unavailable(
                    AmdCliAvailability::NotInstalled,
                    None,
                    None,
                    None,
                    "AMD uProf installation root was not found",
                )
            }
            Err(error) => {
                return unavailable(
                    AmdCliAvailability::RuntimeFailed,
                    None,
                    None,
                    None,
                    format!("AMD uProf installation discovery failed: {error}"),
                )
            }
        };
        if !install_root.is_absolute() {
            return unavailable(
                AmdCliAvailability::RuntimeFailed,
                Some(install_root),
                None,
                None,
                "AMD uProf installation root is not absolute",
            );
        }
        let cli_path = install_root.join("bin").join(AMD_UPROF_CLI_NAME);
        if !cli_path.is_file() {
            return unavailable(
                AmdCliAvailability::NotInstalled,
                Some(install_root),
                Some(cli_path),
                None,
                "AMDuProfCLI.exe was not found under the discovered installation root",
            );
        }
        let artifact = match self.artifact_inspector.inspect(&cli_path) {
            Ok(artifact) => artifact,
            Err(error) => {
                return unavailable(
                    AmdCliAvailability::RuntimeFailed,
                    Some(install_root),
                    Some(cli_path),
                    None,
                    format!("AMDuProfCLI.exe inspection failed: {error}"),
                )
            }
        };
        if artifact.architecture != AmdCliArchitecture::X64 {
            return unavailable(
                AmdCliAvailability::UnsupportedVersion,
                Some(install_root),
                Some(cli_path),
                Some(artifact),
                "AMDuProfCLI.exe is not an x64 PE image",
            );
        }
        if self.require_valid_signature && !artifact.signature_status.is_valid() {
            return unavailable(
                AmdCliAvailability::RuntimeFailed,
                Some(install_root),
                Some(cli_path),
                Some(artifact),
                "AMDuProfCLI.exe did not satisfy the required Authenticode policy",
            );
        }
        if let Some(expected_major) = self.accepted_major_version {
            let actual_major = artifact
                .file_version
                .as_deref()
                .and_then(|version| version.split('.').next())
                .and_then(|major| major.parse::<u64>().ok());
            if actual_major != Some(expected_major) {
                return unavailable(
                    AmdCliAvailability::UnsupportedVersion,
                    Some(install_root),
                    Some(cli_path),
                    Some(artifact),
                    format!("AMDuProfCLI.exe version does not satisfy major {expected_major}"),
                );
            }
        }
        AmdCliDiscoveryResult {
            status: AmdCliAvailability::Available,
            install_root: Some(install_root),
            cli_path: Some(cli_path),
            artifact: Some(artifact),
            reason: None,
        }
    }
}

impl Default for AmdCliDiscovery {
    fn default() -> Self {
        Self::new(
            Box::new(WindowsRegistryInstallRoot),
            Box::new(FilesystemAmdCliArtifactInspector),
        )
    }
}

fn unavailable(
    status: AmdCliAvailability,
    install_root: Option<PathBuf>,
    cli_path: Option<PathBuf>,
    artifact: Option<AmdCliArtifactMetadata>,
    reason: impl Into<String>,
) -> AmdCliDiscoveryResult {
    AmdCliDiscoveryResult {
        status,
        install_root,
        cli_path,
        artifact,
        reason: Some(reason.into()),
    }
}

pub fn provider_error_code(status: AmdCliAvailability) -> Option<ProviderErrorCode> {
    match status {
        AmdCliAvailability::Available => None,
        AmdCliAvailability::NotInstalled => Some(ProviderErrorCode::ProviderMissing),
        AmdCliAvailability::UnsupportedVersion => Some(ProviderErrorCode::Unsupported),
        AmdCliAvailability::PermissionRequired => Some(ProviderErrorCode::PermissionDenied),
        AmdCliAvailability::DriverOrServiceUnavailable => Some(ProviderErrorCode::RuntimeFailed),
        AmdCliAvailability::CounterUnavailable => Some(ProviderErrorCode::Unsupported),
        AmdCliAvailability::RuntimeFailed => Some(ProviderErrorCode::RuntimeFailed),
        AmdCliAvailability::ParseFailed => Some(ProviderErrorCode::SampleFailed),
        AmdCliAvailability::Disabled => Some(ProviderErrorCode::UserDisabled),
    }
}

pub fn package_power_metric_metadata(status: MetricRuntimeSupportStatus) -> ProviderMetricMetadata {
    ProviderMetricMetadata {
        category: MetricCategory::Power,
        metric_key: AMD_UPROF_POWER_METRIC_KEY.to_string(),
        device: Some(RuntimeDeviceMetadata {
            stable_key: "cpu:package".to_string(),
            category: MetricCategory::Power,
            vendor: Some("AMD".to_string()),
            model: None,
            capacity_bytes: None,
        }),
        support_status: status,
    }
}

/// Descriptor for the future adapter's insertion point.  It is intentionally not passed
/// to `ProviderHost` until the output/value persistence contract is extended and the
/// runtime qualification gates are complete.
pub fn spike_provider_descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: AMD_UPROF_CLI_PROVIDER_ID.to_string(),
        display_name: "AMD uProf CLI (spike)".to_string(),
        schedule: ProviderSchedule::System,
        capabilities: vec![ProviderCapabilitySpec {
            category: MetricCategory::Power,
            support_status: CapabilitySupportStatus::Supported,
            reason_code: None,
        }],
    }
}

#[derive(Debug, Clone)]
pub struct AmdCliCommand {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: PathBuf,
    pub timeout: Duration,
}

impl AmdCliCommand {
    pub fn new(
        executable: impl Into<PathBuf>,
        current_dir: impl Into<PathBuf>,
        args: Vec<OsString>,
        timeout: Duration,
    ) -> Result<Self, AmdCliConfigError> {
        if timeout.is_zero() {
            return Err(AmdCliConfigError::ZeroTimeout);
        }
        Ok(Self {
            executable: executable.into(),
            args,
            current_dir: current_dir.into(),
            timeout,
        })
    }

    pub fn power_session(
        cli_path: impl Into<PathBuf>,
        current_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
        duration: Duration,
        sample_interval: Duration,
        timeout: Duration,
    ) -> Result<Self, AmdCliConfigError> {
        let duration_secs = duration.as_secs();
        let interval_ms = sample_interval.as_millis();
        if duration_secs == 0
            || interval_ms < CLI_MIN_SAMPLE_INTERVAL_MS
            || interval_ms > u64::MAX as u128
        {
            return Err(AmdCliConfigError::InvalidSessionWindow);
        }
        let interval_ms = interval_ms as u64;
        let args = [
            "timechart",
            "--event",
            "power",
            "--interval",
            &interval_ms.to_string(),
            "--duration",
            &duration_secs.to_string(),
            "--format",
            "csv",
            "--output-dir",
        ]
        .into_iter()
        .map(OsString::from)
        .chain(std::iter::once(output_dir.into().into_os_string()))
        .collect();
        Self::new(cli_path, current_dir, args, timeout)
    }
}

#[derive(Debug, Error)]
pub enum AmdCliConfigError {
    #[error("CLI timeout must be greater than zero")]
    ZeroTimeout,
    #[error(
        "CLI session duration must be greater than zero and sample interval must be at least 10 ms"
    )]
    InvalidSessionWindow,
}

#[derive(Debug, Clone)]
pub struct AmdCliProcessResult {
    pub process_started: bool,
    pub process_id: Option<u32>,
    pub started_at: SystemTime,
    pub finished_at: SystemTime,
    pub duration: Duration,
    pub exit_code: Option<i32>,
    pub exit_code_hex: Option<String>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub cleanup_performed: bool,
    /// Whether Windows attached the target to a transient job for owned-child
    /// cleanup on timeout/cancellation. `false` is not a target failure.
    pub process_tree_cleanup_available: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub capture_complete: bool,
}

impl AmdCliProcessResult {
    pub fn target_failed(&self) -> bool {
        self.exit_code != Some(0) || self.timed_out || self.cancelled
    }

    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

#[derive(Debug, Error)]
pub enum AmdCliRunnerError {
    #[error("failed to spawn AMD CLI: {0}")]
    Spawn(#[source] io::Error),
    #[error("failed to poll AMD CLI: {0}")]
    Poll(#[source] io::Error),
    #[error("failed to terminate AMD CLI: {0}")]
    Terminate(#[source] io::Error),
    #[error("failed to capture AMD CLI output")]
    OutputCapture,
}

#[derive(Debug, Clone, Default)]
pub struct AmdCliCancellation(Arc<AtomicBool>);

impl AmdCliCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub trait AmdCliProcessRunner: Send + Sync {
    fn run(
        &self,
        command: &AmdCliCommand,
        cancellation: Option<&AmdCliCancellation>,
    ) -> Result<AmdCliProcessResult, AmdCliRunnerError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AmdCliRunner;

impl AmdCliProcessRunner for AmdCliRunner {
    fn run(
        &self,
        command: &AmdCliCommand,
        cancellation: Option<&AmdCliCancellation>,
    ) -> Result<AmdCliProcessResult, AmdCliRunnerError> {
        let started_at = SystemTime::now();
        let monotonic_start = std::time::Instant::now();
        let mut process = Command::new(&command.executable);
        process
            .args(&command.args)
            .current_dir(&command.current_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            process.creation_flags(0x0800_0000);
        }
        let mut child = process.spawn().map_err(AmdCliRunnerError::Spawn)?;
        let process_id = Some(child.id());
        #[cfg(windows)]
        let process_job = OwnedProcessJob::try_attach(child.id());
        #[cfg(not(windows))]
        let process_job: Option<()> = None;
        let stdout = match child.stdout.take() {
            Some(stdout) => stdout,
            None => {
                let _ = terminate_owned_process(&mut child, process_job.as_ref());
                return Err(AmdCliRunnerError::OutputCapture);
            }
        };
        let stderr = match child.stderr.take() {
            Some(stderr) => stderr,
            None => {
                let _ = terminate_owned_process(&mut child, process_job.as_ref());
                return Err(AmdCliRunnerError::OutputCapture);
            }
        };
        let stdout_reader = thread::spawn(move || read_pipe(stdout));
        let stderr_reader = thread::spawn(move || read_pipe(stderr));
        let mut timed_out = false;
        let mut cancelled = false;
        let status = loop {
            if cancellation.is_some_and(AmdCliCancellation::is_cancelled) {
                cancelled = true;
                terminate_owned_process(&mut child, process_job.as_ref())?;
                break child
                    .try_wait()
                    .map_err(AmdCliRunnerError::Poll)?
                    .or_else(|| child.wait().ok());
            }
            match child.try_wait().map_err(AmdCliRunnerError::Poll)? {
                Some(status) => break Some(status),
                None if monotonic_start.elapsed() >= command.timeout => {
                    timed_out = true;
                    terminate_owned_process(&mut child, process_job.as_ref())?;
                    break child
                        .try_wait()
                        .map_err(AmdCliRunnerError::Poll)?
                        .or_else(|| child.wait().ok());
                }
                None => thread::sleep(Duration::from_millis(5)),
            }
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| AmdCliRunnerError::OutputCapture)?
            .map_err(|_| AmdCliRunnerError::OutputCapture)?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| AmdCliRunnerError::OutputCapture)?
            .map_err(|_| AmdCliRunnerError::OutputCapture)?;
        let finished_at = SystemTime::now();
        let exit_code = status.and_then(|status| status.code());
        Ok(AmdCliProcessResult {
            process_started: true,
            process_id,
            started_at,
            finished_at,
            duration: monotonic_start.elapsed(),
            exit_code,
            exit_code_hex: exit_code.map(exit_code_hex),
            timed_out,
            cancelled,
            cleanup_performed: timed_out || cancelled,
            process_tree_cleanup_available: process_job.is_some(),
            stdout,
            stderr,
            capture_complete: true,
        })
    }
}

impl AmdCliRunner {
    pub fn run(
        &self,
        command: &AmdCliCommand,
        cancellation: Option<&AmdCliCancellation>,
    ) -> Result<AmdCliProcessResult, AmdCliRunnerError> {
        <Self as AmdCliProcessRunner>::run(self, command, cancellation)
    }
}

fn read_pipe<R: Read>(mut pipe: R) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn terminate_owned_child(child: &mut Child) -> Result<(), AmdCliRunnerError> {
    match child.kill() {
        Ok(()) => child
            .wait()
            .map(|_| ())
            .map_err(AmdCliRunnerError::Terminate),
        Err(error) if error.kind() == io::ErrorKind::NotFound => child
            .wait()
            .map(|_| ())
            .map_err(AmdCliRunnerError::Terminate),
        Err(error) => Err(AmdCliRunnerError::Terminate(error)),
    }
}

#[cfg(windows)]
fn terminate_owned_process(
    child: &mut Child,
    process_job: Option<&OwnedProcessJob>,
) -> Result<(), AmdCliRunnerError> {
    if let Some(process_job) = process_job {
        process_job
            .terminate()
            .map_err(AmdCliRunnerError::Terminate)?;
        return child
            .wait()
            .map(|_| ())
            .map_err(AmdCliRunnerError::Terminate);
    }
    terminate_owned_child(child)
}

#[cfg(not(windows))]
fn terminate_owned_process(
    child: &mut Child,
    _process_job: Option<&()>,
) -> Result<(), AmdCliRunnerError> {
    terminate_owned_child(child)
}

#[cfg(windows)]
struct OwnedProcessJob(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl OwnedProcessJob {
    fn try_attach(process_id: u32) -> Option<Self> {
        use windows::core::PCWSTR;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::{
            JobObjects::{AssignProcessToJobObject, CreateJobObjectW},
            Threading::{
                OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA,
                PROCESS_TERMINATE,
            },
        };

        let job = unsafe { CreateJobObjectW(None, PCWSTR::null()).ok()? };
        let process = match unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_QUOTA | PROCESS_TERMINATE,
                false,
                process_id,
            )
        } {
            Ok(process) => process,
            Err(_) => {
                let _ = unsafe { CloseHandle(job) };
                return None;
            }
        };
        let assigned = unsafe { AssignProcessToJobObject(job, process).is_ok() };
        let _ = unsafe { CloseHandle(process) };
        if assigned {
            Some(Self(job))
        } else {
            let _ = unsafe { CloseHandle(job) };
            None
        }
    }

    fn terminate(&self) -> io::Result<()> {
        use windows::Win32::System::JobObjects::TerminateJobObject;
        unsafe { TerminateJobObject(self.0, 0xFFFF_FFFF) }
            .map_err(|error| io::Error::from_raw_os_error(error.code().0))
    }
}

#[cfg(windows)]
impl Drop for OwnedProcessJob {
    fn drop(&mut self) {
        use windows::Win32::Foundation::CloseHandle;
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub fn exit_code_hex(exit_code: i32) -> String {
    format!("0x{:08X}", exit_code as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmdCliSessionState {
    Idle,
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmdCliSessionFailureKind {
    PermissionRequired,
    RuntimeFailed,
    ParseFailed,
    Timeout,
    Cancelled,
    CounterUnavailable,
}

#[derive(Debug, Clone)]
pub struct AmdCliSessionConfig {
    pub current_dir: PathBuf,
    pub duration: Duration,
    pub sample_interval: Duration,
    pub timeout: Duration,
    pub output_dir: PathBuf,
    pub output_csv: PathBuf,
}

impl AmdCliSessionConfig {
    pub fn new(
        current_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
        output_csv: impl Into<PathBuf>,
        duration: Duration,
        sample_interval: Duration,
        timeout: Duration,
    ) -> Result<Self, AmdCliConfigError> {
        if duration.is_zero() || sample_interval.is_zero() {
            return Err(AmdCliConfigError::InvalidSessionWindow);
        }
        if timeout.is_zero() {
            return Err(AmdCliConfigError::ZeroTimeout);
        }
        Ok(Self {
            current_dir: current_dir.into(),
            duration,
            sample_interval,
            timeout,
            output_dir: output_dir.into(),
            output_csv: output_csv.into(),
        })
    }

    pub fn spike_defaults(
        current_dir: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
        output_csv: impl Into<PathBuf>,
    ) -> Self {
        Self {
            current_dir: current_dir.into(),
            duration: SPIKE_DEFAULT_DURATION,
            sample_interval: SPIKE_DEFAULT_SAMPLE_INTERVAL,
            timeout: SPIKE_DEFAULT_TIMEOUT,
            output_dir: output_dir.into(),
            output_csv: output_csv.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum AmdCliSessionError {
    #[error("invalid AMD CLI session state transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: AmdCliSessionState,
        to: AmdCliSessionState,
    },
    #[error("AMD CLI configuration error: {0}")]
    Configuration(#[from] AmdCliConfigError),
    #[error("AMD CLI runner error: {0}")]
    Runner(#[from] AmdCliRunnerError),
    #[error("AMD CLI process failed with status {kind:?}, exit={exit_code:?}")]
    ProcessFailed {
        kind: AmdCliSessionFailureKind,
        exit_code: Option<i32>,
    },
    #[error("AMD CLI output could not be read: {0}")]
    OutputRead(#[source] io::Error),
    #[error("AMD CLI output parsing failed: {0}")]
    Parse(#[from] AmdCliParseError),
}

pub struct AmdCliSession {
    cli_path: PathBuf,
    config: AmdCliSessionConfig,
    runner: Box<dyn AmdCliProcessRunner>,
    state: AmdCliSessionState,
    state_history: Vec<AmdCliSessionState>,
}

impl AmdCliSession {
    pub fn new(cli_path: impl Into<PathBuf>, config: AmdCliSessionConfig) -> Self {
        Self::with_runner(cli_path, config, Box::new(AmdCliRunner))
    }

    pub fn with_runner(
        cli_path: impl Into<PathBuf>,
        config: AmdCliSessionConfig,
        runner: Box<dyn AmdCliProcessRunner>,
    ) -> Self {
        Self {
            cli_path: cli_path.into(),
            config,
            runner,
            state: AmdCliSessionState::Idle,
            state_history: vec![AmdCliSessionState::Idle],
        }
    }

    pub fn state(&self) -> AmdCliSessionState {
        self.state
    }

    pub fn state_history(&self) -> &[AmdCliSessionState] {
        &self.state_history
    }

    pub fn run(
        &mut self,
        cancellation: Option<&AmdCliCancellation>,
    ) -> Result<AmdCliSessionResult, AmdCliSessionError> {
        self.transition(AmdCliSessionState::Starting)?;
        let command = match AmdCliCommand::power_session(
            self.cli_path.clone(),
            self.config.current_dir.clone(),
            self.config.output_dir.clone(),
            self.config.duration,
            self.config.sample_interval,
            self.config.timeout,
        ) {
            Ok(command) => command,
            Err(error) => {
                self.transition(AmdCliSessionState::Failed)?;
                return Err(error.into());
            }
        };
        let process = match self.runner.run(&command, cancellation) {
            Ok(process) => process,
            Err(error) => {
                self.transition(AmdCliSessionState::Failed)?;
                return Err(error.into());
            }
        };
        let output_csv = self.config.output_csv.clone();
        self.consume_process(process, || {
            fs::read_to_string(&output_csv).map_err(AmdCliSessionError::OutputRead)
        })
    }

    pub fn consume_process<F>(
        &mut self,
        process: AmdCliProcessResult,
        read_output: F,
    ) -> Result<AmdCliSessionResult, AmdCliSessionError>
    where
        F: FnOnce() -> Result<String, AmdCliSessionError>,
    {
        if self.state == AmdCliSessionState::Idle {
            self.transition(AmdCliSessionState::Starting)?;
        }
        if process.cancelled {
            self.transition(AmdCliSessionState::Cancelled)?;
            return Err(AmdCliSessionError::ProcessFailed {
                kind: AmdCliSessionFailureKind::Cancelled,
                exit_code: process.exit_code,
            });
        }
        if !process.process_started {
            self.transition(AmdCliSessionState::Failed)?;
            return Err(AmdCliSessionError::ProcessFailed {
                kind: AmdCliSessionFailureKind::RuntimeFailed,
                exit_code: process.exit_code,
            });
        }
        if !process.capture_complete {
            self.transition(AmdCliSessionState::Failed)?;
            return Err(AmdCliSessionError::ProcessFailed {
                kind: AmdCliSessionFailureKind::RuntimeFailed,
                exit_code: process.exit_code,
            });
        }
        self.transition(AmdCliSessionState::Running)?;
        if process.timed_out {
            self.transition(AmdCliSessionState::Failed)?;
            return Err(AmdCliSessionError::ProcessFailed {
                kind: AmdCliSessionFailureKind::Timeout,
                exit_code: process.exit_code,
            });
        }
        if process.exit_code != Some(0) {
            let kind = if process.exit_code.map(|code| code as u32) == Some(0x8007_0005) {
                AmdCliSessionFailureKind::PermissionRequired
            } else {
                AmdCliSessionFailureKind::RuntimeFailed
            };
            self.transition(AmdCliSessionState::Failed)?;
            return Err(AmdCliSessionError::ProcessFailed {
                kind,
                exit_code: process.exit_code,
            });
        }
        let csv = match read_output() {
            Ok(csv) => csv,
            Err(error) => {
                self.transition(AmdCliSessionState::Failed)?;
                return Err(error);
            }
        };
        let parsed = match parse_cli_power_csv(&csv) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.transition(AmdCliSessionState::Failed)?;
                return Err(error.into());
            }
        };
        self.transition(AmdCliSessionState::Completed)?;
        Ok(AmdCliSessionResult { process, parsed })
    }

    fn transition(&mut self, next: AmdCliSessionState) -> Result<(), AmdCliSessionError> {
        let allowed = matches!(
            (self.state, next),
            (AmdCliSessionState::Idle, AmdCliSessionState::Starting)
                | (AmdCliSessionState::Starting, AmdCliSessionState::Running)
                | (AmdCliSessionState::Starting, AmdCliSessionState::Failed)
                | (AmdCliSessionState::Starting, AmdCliSessionState::Cancelled)
                | (AmdCliSessionState::Running, AmdCliSessionState::Completed)
                | (AmdCliSessionState::Running, AmdCliSessionState::Failed)
                | (AmdCliSessionState::Running, AmdCliSessionState::Cancelled)
        );
        if !allowed {
            return Err(AmdCliSessionError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        self.state_history.push(next);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AmdCliSessionResult {
    pub process: AmdCliProcessResult,
    pub parsed: AmdCliParsedOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmdCliTimestampSemantics {
    ClockTimeWithoutDate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmdCliTimestamp {
    pub raw: String,
    pub clock_millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AmdCliSessionMetadata {
    pub profile_start_time: Option<String>,
    pub sampling_interval_ms: Option<u64>,
    pub profile_duration_secs: Option<u64>,
    pub fields: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmdCliCounterDefinition {
    pub id: Option<String>,
    pub name: String,
    pub category: String,
    pub unit: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmdCliPowerSample {
    pub record_id: Option<u64>,
    pub timestamp: AmdCliTimestamp,
    pub counter_name: String,
    pub raw_value: String,
    pub original_unit: String,
    pub value_watts: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AmdCliParsedOutput {
    pub metadata: AmdCliSessionMetadata,
    pub counters: Vec<AmdCliCounterDefinition>,
    pub power_samples: Vec<AmdCliPowerSample>,
    pub timestamp_semantics: AmdCliTimestampSemantics,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum AmdCliParseError {
    #[error("CLI output is empty")]
    EmptyOutput,
    #[error("CLI output is missing the {0} section")]
    MissingSection(&'static str),
    #[error("malformed CSV at line {line}")]
    MalformedCsv { line: usize },
    #[error("malformed counter row at line {line}")]
    MalformedCounterRow { line: usize },
    #[error("malformed record row at line {line}")]
    MalformedRecordRow { line: usize },
    #[error("package-power counter is missing")]
    MissingPowerCounter,
    #[error("package-power counter unit is unsupported: {0}")]
    UnsupportedPowerUnit(String),
    #[error("package-power value is missing at line {line}")]
    MissingValue { line: usize },
    #[error("package-power value is malformed at line {line}: {value}")]
    MalformedNumber { line: usize, value: String },
    #[error("package-power value is not finite at line {line}")]
    NonFiniteValue { line: usize },
    #[error("package-power value is negative at line {line}")]
    NegativeValue { line: usize },
    #[error("timestamp is malformed at line {line}: {value}")]
    MalformedTimestamp { line: usize, value: String },
    #[error("timestamp is duplicated at line {line}: {value}")]
    DuplicateTimestamp { line: usize, value: String },
}

pub fn parse_cli_power_csv(input: &str) -> Result<AmdCliParsedOutput, AmdCliParseError> {
    if input.trim().is_empty() {
        return Err(AmdCliParseError::EmptyOutput);
    }
    let lines: Vec<_> = input.lines().collect();
    let counters_marker = find_line(&lines, "PROFILED COUNTERS")
        .ok_or(AmdCliParseError::MissingSection("PROFILED COUNTERS"))?;
    let records_marker = find_line(&lines, "PROFILE RECORDS")
        .ok_or(AmdCliParseError::MissingSection("PROFILE RECORDS"))?;
    if records_marker <= counters_marker {
        return Err(AmdCliParseError::MalformedCsv {
            line: records_marker + 1,
        });
    }

    let metadata = parse_metadata(&lines);
    let counter_header_line =
        next_nonempty_line(&lines, counters_marker + 1).ok_or(AmdCliParseError::MalformedCsv {
            line: counters_marker + 2,
        })?;
    let counter_headers =
        parse_csv_line(lines[counter_header_line]).map_err(|_| AmdCliParseError::MalformedCsv {
            line: counter_header_line + 1,
        })?;
    let counter_columns =
        CounterColumns::new(&counter_headers).ok_or(AmdCliParseError::MalformedCounterRow {
            line: counter_header_line + 1,
        })?;
    let mut counters = Vec::new();
    for (index, line) in lines[counter_header_line + 1..records_marker]
        .iter()
        .enumerate()
    {
        let line_number = counter_header_line + 2 + index;
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line)
            .map_err(|_| AmdCliParseError::MalformedCsv { line: line_number })?;
        if fields.len() != counter_headers.len() {
            return Err(AmdCliParseError::MalformedCounterRow { line: line_number });
        }
        counters.push(AmdCliCounterDefinition {
            id: nonempty(fields.get(counter_columns.id)),
            name: fields[counter_columns.name].trim().to_string(),
            category: fields[counter_columns.category].trim().to_string(),
            unit: fields[counter_columns.unit].trim().to_string(),
            description: fields[counter_columns.description].trim().to_string(),
        });
    }
    let record_header_line =
        next_nonempty_line(&lines, records_marker + 1).ok_or(AmdCliParseError::MalformedCsv {
            line: records_marker + 2,
        })?;
    let record_headers =
        parse_csv_line(lines[record_header_line]).map_err(|_| AmdCliParseError::MalformedCsv {
            line: record_header_line + 1,
        })?;
    let timestamp_index = record_headers
        .iter()
        .position(|header| normalize_field(header) == "timestamp")
        .ok_or(AmdCliParseError::MalformedRecordRow {
            line: record_header_line + 1,
        })?;
    let package_index = record_headers
        .iter()
        .position(|header| normalize_field(header) == "socket0-package-power")
        .or_else(|| {
            record_headers
                .iter()
                .position(|header| normalize_field(header).contains("package-power"))
        })
        .ok_or(AmdCliParseError::MissingPowerCounter)?;
    let package_name = record_headers[package_index].trim().to_string();
    let counter = counters
        .iter()
        .find(|counter| counter.name.eq_ignore_ascii_case(&package_name))
        .ok_or(AmdCliParseError::MissingPowerCounter)?;
    if !counter.unit.trim().eq_ignore_ascii_case("W") {
        return Err(AmdCliParseError::UnsupportedPowerUnit(counter.unit.clone()));
    }
    let record_id_index = record_headers
        .iter()
        .position(|header| normalize_field(header) == "record-id");
    let mut timestamps = BTreeSet::new();
    let mut power_samples = Vec::new();
    for (index, line) in lines[record_header_line + 1..].iter().enumerate() {
        let line_number = record_header_line + 2 + index;
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line)
            .map_err(|_| AmdCliParseError::MalformedCsv { line: line_number })?;
        if fields.len() != record_headers.len() {
            return Err(AmdCliParseError::MalformedRecordRow { line: line_number });
        }
        let raw_timestamp = fields[timestamp_index].trim().to_string();
        let timestamp = parse_timestamp(&raw_timestamp).ok_or_else(|| {
            AmdCliParseError::MalformedTimestamp {
                line: line_number,
                value: raw_timestamp.clone(),
            }
        })?;
        if !timestamps.insert(timestamp.clock_millis) {
            return Err(AmdCliParseError::DuplicateTimestamp {
                line: line_number,
                value: raw_timestamp,
            });
        }
        let raw_value = fields[package_index].trim().to_string();
        if is_missing_value(&raw_value) {
            return Err(AmdCliParseError::MissingValue { line: line_number });
        }
        let value_watts =
            raw_value
                .parse::<f64>()
                .map_err(|_| AmdCliParseError::MalformedNumber {
                    line: line_number,
                    value: raw_value.clone(),
                })?;
        if !value_watts.is_finite() {
            return Err(AmdCliParseError::NonFiniteValue { line: line_number });
        }
        if value_watts < 0.0 {
            return Err(AmdCliParseError::NegativeValue { line: line_number });
        }
        let record_id = record_id_index
            .and_then(|index| nonempty(fields.get(index)))
            .and_then(|value| value.parse::<u64>().ok());
        power_samples.push(AmdCliPowerSample {
            record_id,
            timestamp,
            counter_name: package_name.clone(),
            raw_value,
            original_unit: counter.unit.clone(),
            value_watts,
        });
    }
    if power_samples.is_empty() {
        return Err(AmdCliParseError::MissingSection("PROFILE RECORDS data"));
    }
    Ok(AmdCliParsedOutput {
        metadata,
        counters,
        power_samples,
        timestamp_semantics: AmdCliTimestampSemantics::ClockTimeWithoutDate,
    })
}

#[derive(Debug, Clone, Copy)]
struct CounterColumns {
    id: usize,
    name: usize,
    category: usize,
    unit: usize,
    description: usize,
}

impl CounterColumns {
    fn new(headers: &[String]) -> Option<Self> {
        Some(Self {
            id: column(headers, "counter-id")?,
            name: column(headers, "name")?,
            category: column(headers, "category")?,
            unit: column(headers, "unit")?,
            description: column(headers, "description")?,
        })
    }
}

fn column(headers: &[String], name: &str) -> Option<usize> {
    headers
        .iter()
        .position(|header| normalize_field(header) == name)
}

fn parse_metadata(lines: &[&str]) -> AmdCliSessionMetadata {
    let Some(details_marker) = find_line(lines, "PROFILE DETAILS") else {
        return AmdCliSessionMetadata::default();
    };
    let end = find_line(lines, "PROFILED COUNTERS").unwrap_or(lines.len());
    let mut metadata = AmdCliSessionMetadata::default();
    for line in &lines[details_marker + 1..end] {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(fields) = parse_csv_line(line) else {
            continue;
        };
        if fields.len() < 2 {
            continue;
        }
        let key = fields[0].trim().trim_end_matches(':').to_string();
        let value = fields[1].trim().to_string();
        match normalize_field(&key).as_str() {
            "sampling-interval" => {
                metadata.sampling_interval_ms = first_number(&value);
            }
            "profile-duration" => {
                metadata.profile_duration_secs = first_number(&value);
            }
            "profile-start-time" => metadata.profile_start_time = Some(value.clone()),
            _ => {}
        }
        metadata.fields.insert(key, value);
    }
    metadata
}

fn first_number(value: &str) -> Option<u64> {
    value.split_whitespace().find_map(|part| {
        part.trim_matches(|character: char| !character.is_ascii_digit())
            .parse()
            .ok()
    })
}

fn find_line(lines: &[&str], marker: &str) -> Option<usize> {
    lines.iter().position(|line| {
        line.trim()
            .trim_start_matches('\u{feff}')
            .eq_ignore_ascii_case(marker)
    })
}

fn next_nonempty_line(lines: &[&str], start: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, line)| !line.trim().is_empty())
        .map(|(index, _)| index)
}

fn normalize_field(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('\u{feff}')
        .to_ascii_lowercase()
        .replace(['_', ' '], "-")
}

fn nonempty(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_missing_value(value: &str) -> bool {
    value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "-" | "--" | "na" | "n/a" | "null" | "not available"
        )
}

fn parse_timestamp(value: &str) -> Option<AmdCliTimestamp> {
    let normalized = value.trim().replace('.', ":");
    let parts: Vec<_> = normalized.split(':').collect();
    if parts.len() != 4 {
        return None;
    }
    let hour = parts[0].parse::<u64>().ok()?;
    let minute = parts[1].parse::<u64>().ok()?;
    let second = parts[2].parse::<u64>().ok()?;
    let millis = parts[3].parse::<u64>().ok()?;
    if hour >= 24 || minute >= 60 || second >= 60 || millis >= 1_000 {
        return None;
    }
    Some(AmdCliTimestamp {
        raw: value.trim().to_string(),
        clock_millis: (((hour * 60) + minute) * 60 + second) * 1_000 + millis,
    })
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, ()> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '"' if quoted && characters.peek() == Some(&'"') => {
                field.push('"');
                characters.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(std::mem::take(&mut field));
            }
            _ => field.push(character),
        }
    }
    if quoted {
        return Err(());
    }
    fields.push(field);
    Ok(fields)
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn detect_pe_architecture(bytes: &[u8]) -> io::Result<AmdCliArchitecture> {
    if bytes.len() < 0x40 || &bytes[..2] != b"MZ" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "artifact is not a PE image",
        ));
    }
    let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    if pe_offset.checked_add(6).is_none()
        || bytes.len() < pe_offset + 6
        || &bytes[pe_offset..pe_offset + 4] != b"PE\0\0"
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "artifact has an invalid PE header",
        ));
    }
    let machine = u16::from_le_bytes(bytes[pe_offset + 4..pe_offset + 6].try_into().unwrap());
    Ok(match machine {
        0x8664 => AmdCliArchitecture::X64,
        0x014c => AmdCliArchitecture::X86,
        0xAA64 => AmdCliArchitecture::Arm64,
        _ => AmdCliArchitecture::Unknown,
    })
}

#[cfg(windows)]
fn platform_file_version(path: &Path) -> Option<String> {
    use std::mem::size_of;
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW, VS_FIXEDFILEINFO,
        },
    };
    let path = path.to_str()?;
    let path = wide(path);
    let mut handle = 0_u32;
    let size =
        unsafe { GetFileVersionInfoSizeW(PCWSTR::from_raw(path.as_ptr()), Some(&mut handle)) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0_u8; size as usize];
    unsafe {
        GetFileVersionInfoW(
            PCWSTR::from_raw(path.as_ptr()),
            0,
            size,
            data.as_mut_ptr() as *mut _,
        )
        .ok()?;
    }
    let root = wide("\\");
    let mut value = std::ptr::null_mut();
    let mut length = 0_u32;
    let ok = unsafe {
        VerQueryValueW(
            data.as_ptr() as *const _,
            PCWSTR::from_raw(root.as_ptr()),
            &mut value,
            &mut length,
        )
        .as_bool()
    };
    if !ok || value.is_null() || length < size_of::<VS_FIXEDFILEINFO>() as u32 {
        return None;
    }
    let info = unsafe { *(value as *const VS_FIXEDFILEINFO) };
    if info.dwSignature != 0xFEEF04BD {
        return None;
    }
    Some(format!(
        "{}.{}.{}.{}",
        info.dwFileVersionMS >> 16,
        info.dwFileVersionMS & 0xFFFF,
        info.dwFileVersionLS >> 16,
        info.dwFileVersionLS & 0xFFFF
    ))
}

#[cfg(not(windows))]
fn platform_file_version(_path: &Path) -> Option<String> {
    None
}

#[cfg(windows)]
fn platform_signature_status(path: &Path) -> AmdCliSignatureStatus {
    use std::{ffi::c_void, mem::size_of};
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{HWND, TRUST_E_NOSIGNATURE},
            Security::WinTrust::{
                WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
                WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
                WTD_STATEACTION_IGNORE, WTD_UI_NONE,
            },
        },
    };
    let Some(path) = path.to_str() else {
        return AmdCliSignatureStatus::Unknown;
    };
    let path = wide(path);
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR::from_raw(path.as_ptr()),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_IGNORE,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let result = unsafe {
        WinVerifyTrust(
            HWND(std::ptr::null_mut()),
            &mut action,
            &mut data as *mut WINTRUST_DATA as *mut c_void,
        )
    };
    if result == 0 {
        AmdCliSignatureStatus::Valid
    } else if result as u32 == TRUST_E_NOSIGNATURE.0 as u32 {
        AmdCliSignatureStatus::NotSigned
    } else {
        AmdCliSignatureStatus::Invalid
    }
}

#[cfg(not(windows))]
fn platform_signature_status(_path: &Path) -> AmdCliSignatureStatus {
    AmdCliSignatureStatus::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        fs::{self, File},
        sync::Mutex,
    };

    const VALID_CSV: &str =
        include_str!("../../../tools/amd-uprof-cli-spike/test-fixtures/package-power.csv");

    fn successful_process() -> AmdCliProcessResult {
        AmdCliProcessResult {
            process_started: true,
            process_id: Some(7),
            started_at: SystemTime::UNIX_EPOCH,
            finished_at: SystemTime::UNIX_EPOCH,
            duration: Duration::from_millis(10),
            exit_code: Some(0),
            exit_code_hex: Some(exit_code_hex(0)),
            timed_out: false,
            cancelled: false,
            cleanup_performed: false,
            process_tree_cleanup_available: false,
            stdout: Vec::new(),
            stderr: Vec::new(),
            capture_complete: true,
        }
    }

    struct FakeRunner {
        result: Mutex<Option<AmdCliProcessResult>>,
    }

    impl FakeRunner {
        fn new(result: AmdCliProcessResult) -> Self {
            Self {
                result: Mutex::new(Some(result)),
            }
        }
    }

    impl AmdCliProcessRunner for FakeRunner {
        fn run(
            &self,
            _command: &AmdCliCommand,
            _cancellation: Option<&AmdCliCancellation>,
        ) -> Result<AmdCliProcessResult, AmdCliRunnerError> {
            Ok(self.result.lock().unwrap().take().unwrap())
        }
    }

    struct FakeRootSource(Option<PathBuf>);

    impl AmdCliInstallRootSource for FakeRootSource {
        fn read_install_root(&self) -> io::Result<Option<PathBuf>> {
            Ok(self.0.clone())
        }
    }

    struct FakeInspector(AmdCliArtifactMetadata);

    impl AmdCliArtifactInspector for FakeInspector {
        fn inspect(&self, _path: &Path) -> io::Result<AmdCliArtifactMetadata> {
            Ok(self.0.clone())
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        let path = env::temp_dir().join(format!(
            "resource-timeline-amd-cli-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("bin")).unwrap();
        path
    }

    fn fake_metadata(path: &Path) -> AmdCliArtifactMetadata {
        AmdCliArtifactMetadata {
            path: path.to_path_buf(),
            sha256: "A".repeat(64),
            size_bytes: 1,
            architecture: AmdCliArchitecture::X64,
            file_version: Some("5.3.521.0".into()),
            signature_status: AmdCliSignatureStatus::Valid,
        }
    }

    #[test]
    fn parses_power_series_by_header_and_preserves_timestamp_metadata() {
        let parsed = parse_cli_power_csv(VALID_CSV).unwrap();
        assert_eq!(parsed.power_samples.len(), 3);
        assert_eq!(parsed.power_samples[0].value_watts, 49.44);
        assert_eq!(parsed.power_samples[2].raw_value, "40.29");
        assert_eq!(parsed.metadata.sampling_interval_ms, Some(1000));
        assert_eq!(parsed.metadata.profile_duration_secs, Some(5));
        assert_eq!(
            parsed.timestamp_semantics,
            AmdCliTimestampSemantics::ClockTimeWithoutDate
        );
    }

    #[test]
    fn parser_accepts_reordered_record_columns() {
        let input = VALID_CSV.replace(
            "RecordId,Timestamp,socket0-package-power,core0-power",
            "core0-power,RecordId,socket0-package-power,Timestamp",
        );
        let input = input
            .replace("1,11:18:22:646,49.44,8.10", "8.10,1,49.44,11:18:22:646")
            .replace("2,11:18:23:646,42.26,7.90", "7.90,2,42.26,11:18:23:646")
            .replace("3,11:18:24:650,40.29,7.75", "7.75,3,40.29,11:18:24:650");
        let parsed = parse_cli_power_csv(&input).unwrap();
        assert_eq!(
            parsed
                .power_samples
                .iter()
                .map(|sample| sample.value_watts)
                .collect::<Vec<_>>(),
            vec![49.44, 42.26, 40.29]
        );
    }

    #[test]
    fn parser_rejects_missing_malformed_negative_and_locale_values() {
        let missing = VALID_CSV.replace("49.44", "N/A");
        assert!(matches!(
            parse_cli_power_csv(&missing),
            Err(AmdCliParseError::MissingValue { .. })
        ));
        let malformed = VALID_CSV.replace("49.44", "not-a-number");
        assert!(matches!(
            parse_cli_power_csv(&malformed),
            Err(AmdCliParseError::MalformedNumber { .. })
        ));
        let negative = VALID_CSV.replace("49.44", "-1");
        assert!(matches!(
            parse_cli_power_csv(&negative),
            Err(AmdCliParseError::NegativeValue { .. })
        ));
        let locale = VALID_CSV.replace("49.44", "49,44");
        assert!(parse_cli_power_csv(&locale).is_err());
    }

    #[test]
    fn parser_rejects_missing_unit_duplicate_timestamps_and_truncated_rows() {
        let no_power = VALID_CSV.replace("socket0-package-power", "socket0-total-energy");
        assert!(matches!(
            parse_cli_power_csv(&no_power),
            Err(AmdCliParseError::MissingPowerCounter)
        ));
        let bad_unit = VALID_CSV.replace(
            "48.,socket0-package-power,Power,W,",
            "48.,socket0-package-power,Power,kW,",
        );
        assert!(matches!(
            parse_cli_power_csv(&bad_unit),
            Err(AmdCliParseError::UnsupportedPowerUnit(_))
        ));
        let duplicate = VALID_CSV.replace("2,11:18:23:646", "2,11:18:22:646");
        assert!(matches!(
            parse_cli_power_csv(&duplicate),
            Err(AmdCliParseError::DuplicateTimestamp { .. })
        ));
        let truncated = VALID_CSV.replace("3,11:18:24:650,40.29,7.75", "3,11:18:24:650");
        assert!(matches!(
            parse_cli_power_csv(&truncated),
            Err(AmdCliParseError::MalformedRecordRow { .. })
        ));
    }

    #[test]
    fn parser_rejects_empty_output_without_inventing_a_sample() {
        assert_eq!(
            parse_cli_power_csv("\n\r\n").unwrap_err(),
            AmdCliParseError::EmptyOutput
        );
    }

    #[test]
    fn discovery_maps_installation_and_missing_cli_without_running_it() {
        let root = temp_root("discovery");
        let cli_path = root.join("bin").join(AMD_UPROF_CLI_NAME);
        File::create(&cli_path).unwrap();
        let discovery = AmdCliDiscovery::new(
            Box::new(FakeRootSource(Some(root.clone()))),
            Box::new(FakeInspector(fake_metadata(&cli_path))),
        )
        .accepted_major_version(Some(5));
        let result = discovery.discover();
        assert_eq!(result.status, AmdCliAvailability::Available);
        assert_eq!(
            result.artifact.unwrap().file_version.as_deref(),
            Some("5.3.521.0")
        );

        fs::remove_file(&cli_path).unwrap();
        let missing = AmdCliDiscovery::new(
            Box::new(FakeRootSource(Some(root.clone()))),
            Box::new(FakeInspector(fake_metadata(&cli_path))),
        )
        .discover();
        assert_eq!(missing.status, AmdCliAvailability::NotInstalled);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discovery_rejects_bad_architecture_or_signature_as_unavailable() {
        let root = temp_root("identity");
        let cli_path = root.join("bin").join(AMD_UPROF_CLI_NAME);
        File::create(&cli_path).unwrap();
        let mut metadata = fake_metadata(&cli_path);
        metadata.architecture = AmdCliArchitecture::X86;
        let result = AmdCliDiscovery::new(
            Box::new(FakeRootSource(Some(root.clone()))),
            Box::new(FakeInspector(metadata)),
        )
        .discover();
        assert_eq!(result.status, AmdCliAvailability::UnsupportedVersion);

        let mut metadata = fake_metadata(&cli_path);
        metadata.signature_status = AmdCliSignatureStatus::NotSigned;
        let result = AmdCliDiscovery::new(
            Box::new(FakeRootSource(Some(root.clone()))),
            Box::new(FakeInspector(metadata)),
        )
        .discover();
        assert_eq!(result.status, AmdCliAvailability::RuntimeFailed);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn availability_uses_existing_provider_error_vocabulary() {
        assert_eq!(provider_error_code(AmdCliAvailability::Available), None);
        assert_eq!(
            provider_error_code(AmdCliAvailability::NotInstalled),
            Some(ProviderErrorCode::ProviderMissing)
        );
        assert_eq!(
            provider_error_code(AmdCliAvailability::UnsupportedVersion),
            Some(ProviderErrorCode::Unsupported)
        );
        assert_eq!(
            provider_error_code(AmdCliAvailability::PermissionRequired),
            Some(ProviderErrorCode::PermissionDenied)
        );
        assert_eq!(
            provider_error_code(AmdCliAvailability::DriverOrServiceUnavailable),
            Some(ProviderErrorCode::RuntimeFailed)
        );
        assert_eq!(
            provider_error_code(AmdCliAvailability::CounterUnavailable),
            Some(ProviderErrorCode::Unsupported)
        );
        assert_eq!(
            provider_error_code(AmdCliAvailability::RuntimeFailed),
            Some(ProviderErrorCode::RuntimeFailed)
        );
        assert_eq!(
            provider_error_code(AmdCliAvailability::ParseFailed),
            Some(ProviderErrorCode::SampleFailed)
        );
        assert_eq!(
            provider_error_code(AmdCliAvailability::Disabled),
            Some(ProviderErrorCode::UserDisabled)
        );
    }

    #[test]
    fn discovery_rejects_a_non_absolute_install_root_without_running_a_binary() {
        let discovery = AmdCliDiscovery::new(
            Box::new(FakeRootSource(Some(PathBuf::from("relative-amd-root")))),
            Box::new(FakeInspector(fake_metadata(Path::new("unused.exe")))),
        );
        let result = discovery.discover();
        assert_eq!(result.status, AmdCliAvailability::RuntimeFailed);
        assert!(result
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("not absolute")));
    }

    #[test]
    fn command_builder_uses_direct_argument_vector_and_bounded_defaults() {
        let command = AmdCliCommand::power_session(
            r"C:\AMD uProf\bin\AMDuProfCLI.exe",
            r"C:\AMD uProf\bin",
            r"C:\Users\Public\resource-timeline\amd-session",
            Duration::from_secs(10),
            Duration::from_millis(1_000),
            Duration::from_secs(15),
        )
        .unwrap();
        assert_eq!(command.args[0], "timechart");
        assert!(command.args.iter().any(|arg| arg == "--format"));
        assert!(!command.args.iter().any(|arg| arg == "/c"));
        assert_eq!(command.timeout, Duration::from_secs(15));
        assert!(matches!(
            AmdCliCommand::power_session(
                "AMDuProfCLI.exe",
                ".",
                "out",
                Duration::from_secs(1),
                Duration::from_millis(9),
                Duration::from_secs(1),
            ),
            Err(AmdCliConfigError::InvalidSessionWindow)
        ));
    }

    #[test]
    fn session_tracks_success_lifecycle_and_maps_permission_failure() {
        let config = AmdCliSessionConfig::spike_defaults(".", "out", "out/timechart.csv");
        let mut session = AmdCliSession::with_runner(
            "AMDuProfCLI.exe",
            config,
            Box::new(FakeRunner::new(successful_process())),
        );
        let result = session
            .consume_process(successful_process(), || Ok(VALID_CSV.to_string()))
            .unwrap();
        assert_eq!(result.parsed.power_samples.len(), 3);
        assert_eq!(
            session.state_history(),
            &[
                AmdCliSessionState::Idle,
                AmdCliSessionState::Starting,
                AmdCliSessionState::Running,
                AmdCliSessionState::Completed,
            ]
        );

        let mut denied = AmdCliProcessResult {
            exit_code: Some(-2147024891),
            exit_code_hex: Some(exit_code_hex(-2147024891)),
            ..successful_process()
        };
        denied.stdout = b"permission".to_vec();
        let mut session = AmdCliSession::with_runner(
            "AMDuProfCLI.exe",
            AmdCliSessionConfig::spike_defaults(".", "out", "out/timechart.csv"),
            Box::new(FakeRunner::new(denied.clone())),
        );
        assert!(matches!(
            session.consume_process(denied, || Ok(VALID_CSV.to_string())),
            Err(AmdCliSessionError::ProcessFailed {
                kind: AmdCliSessionFailureKind::PermissionRequired,
                ..
            })
        ));
        assert_eq!(session.state(), AmdCliSessionState::Failed);
    }

    #[test]
    fn session_run_uses_explicit_current_dir_and_parses_owned_output() {
        let root = temp_root("session-run");
        let output_dir = root.join("session");
        let output_csv = output_dir.join("timechart.csv");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(&output_csv, VALID_CSV).unwrap();
        let config = AmdCliSessionConfig::new(
            root.join("bin"),
            &output_dir,
            &output_csv,
            Duration::from_secs(10),
            Duration::from_secs(1),
            Duration::from_secs(15),
        )
        .unwrap();
        let mut session = AmdCliSession::with_runner(
            "AMDuProfCLI.exe",
            config,
            Box::new(FakeRunner::new(successful_process())),
        );
        let result = session.run(None).unwrap();
        assert_eq!(result.parsed.power_samples.len(), 3);
        assert_eq!(session.state(), AmdCliSessionState::Completed);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn session_failure_is_returned_without_unwinding_caller_and_cancel_is_distinct() {
        let mut process = successful_process();
        process.timed_out = true;
        let mut session = AmdCliSession::with_runner(
            "AMDuProfCLI.exe",
            AmdCliSessionConfig::spike_defaults(".", "out", "out/timechart.csv"),
            Box::new(FakeRunner::new(process.clone())),
        );
        assert!(matches!(
            session.consume_process(process, || Ok(VALID_CSV.to_string())),
            Err(AmdCliSessionError::ProcessFailed {
                kind: AmdCliSessionFailureKind::Timeout,
                ..
            })
        ));
        assert_eq!(session.state(), AmdCliSessionState::Failed);
        assert_eq!(
            provider_error_code(AmdCliAvailability::ParseFailed),
            Some(ProviderErrorCode::SampleFailed)
        );

        let mut process = successful_process();
        process.cancelled = true;
        let mut session = AmdCliSession::with_runner(
            "AMDuProfCLI.exe",
            AmdCliSessionConfig::spike_defaults(".", "out", "out/timechart.csv"),
            Box::new(FakeRunner::new(process.clone())),
        );
        assert!(matches!(
            session.consume_process(process, || Ok(VALID_CSV.to_string())),
            Err(AmdCliSessionError::ProcessFailed {
                kind: AmdCliSessionFailureKind::Cancelled,
                ..
            })
        ));
        assert_eq!(session.state(), AmdCliSessionState::Cancelled);
    }

    #[cfg(windows)]
    fn powershell_command(script: &str, timeout: Duration) -> AmdCliCommand {
        let system_root =
            env::var_os("SystemRoot").unwrap_or_else(|| OsString::from(r"C:\Windows"));
        let executable = PathBuf::from(system_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        AmdCliCommand::new(
            executable,
            env::temp_dir(),
            vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                script.into(),
            ],
            timeout,
        )
        .unwrap()
    }

    #[cfg(windows)]
    #[test]
    fn runner_captures_empty_stderr_and_negative_exit_without_amd() {
        let command = powershell_command(
            "[Console]::Out.Write('synthetic-stdout'); exit -1",
            Duration::from_secs(2),
        );
        let result = AmdCliRunner.run(&command, None).unwrap();
        assert!(result.process_started);
        assert_eq!(result.exit_code, Some(-1));
        assert_eq!(result.exit_code_hex.as_deref(), Some("0xFFFFFFFF"));
        assert_eq!(result.stdout_text(), "synthetic-stdout");
        assert!(result.stderr.is_empty());
        assert!(result.capture_complete);
    }

    #[cfg(windows)]
    #[test]
    fn runner_timeout_terminates_only_owned_synthetic_process() {
        let command =
            powershell_command("Start-Sleep -Milliseconds 500", Duration::from_millis(40));
        let result = AmdCliRunner.run(&command, None).unwrap();
        assert!(result.timed_out);
        assert!(result.cleanup_performed);
        assert!(result.capture_complete);
    }

    #[cfg(windows)]
    #[test]
    fn runner_cancellation_is_distinct_from_timeout() {
        let command = powershell_command("Start-Sleep -Milliseconds 500", Duration::from_secs(2));
        let cancellation = AmdCliCancellation::default();
        let trigger = cancellation.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            trigger.cancel();
        });
        let result = AmdCliRunner.run(&command, Some(&cancellation)).unwrap();
        thread.join().unwrap();
        assert!(result.cancelled);
        assert!(!result.timed_out);
        assert!(result.cleanup_performed);
        assert!(result.capture_complete);
    }

    #[cfg(windows)]
    #[test]
    fn runner_supports_empty_arguments_and_paths_with_spaces() {
        let command = AmdCliCommand::new(
            PathBuf::from(env::var_os("SystemRoot").unwrap())
                .join("System32")
                .join("whoami.exe"),
            env::temp_dir().join("resource timeline amd cli test"),
            Vec::new(),
            Duration::from_secs(2),
        )
        .unwrap();
        fs::create_dir_all(&command.current_dir).unwrap();
        let result = AmdCliRunner.run(&command, None).unwrap();
        assert!(result.process_started);
        assert_eq!(result.exit_code, Some(0));
        assert!(!result.stdout.is_empty());
        let _ = fs::remove_dir_all(&command.current_dir);
    }

    #[test]
    fn exit_code_hex_preserves_signed_process_values() {
        assert_eq!(exit_code_hex(0), "0x00000000");
        assert_eq!(exit_code_hex(1), "0x00000001");
        assert_eq!(exit_code_hex(-1), "0xFFFFFFFF");
    }

    #[test]
    fn provider_metadata_keeps_package_power_outside_current_schema() {
        let metadata = package_power_metric_metadata(MetricRuntimeSupportStatus::Supported);
        assert_eq!(metadata.category, MetricCategory::Power);
        assert_eq!(metadata.metric_key, AMD_UPROF_POWER_METRIC_KEY);
        assert_eq!(spike_provider_descriptor().id, AMD_UPROF_CLI_PROVIDER_ID);
    }

    #[test]
    fn pe_architecture_parser_rejects_non_pe_input() {
        assert!(detect_pe_architecture(b"not an executable").is_err());
    }
}
