use amd_cli_service_context_probe::{
    cli_execution_state, exit_code_hex, fixed_cli_arguments, write_json, CliArtifactIdentity,
    CliLaunchEvidence, CliProcessResult, ServiceContextEvidence, ServiceQualificationResult,
    ServiceState, ServiceStatusJournal, AMD_CLI_NAME, AMD_INSTALL_REGISTRY_KEY,
    AMD_INSTALL_REGISTRY_VALUE, CLI_TIMEOUT_MS, EXPECTED_AMD_CLI_SHA256, EXPECTED_AMD_CLI_VERSION,
    EXPECTED_LOCAL_SYSTEM_SID, QUALIFICATION_ONLY, SERVICE_NAME,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TokenElevationType, TokenIntegrityLevel, TokenUser,
    TOKEN_ELEVATION, TOKEN_ELEVATION_TYPE, TOKEN_INFORMATION_CLASS, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
    SERVICE_ACCEPT_STOP, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
    SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED, SERVICE_STOP_PENDING,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken};

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
static RUN_ROOT: OnceLock<PathBuf> = OnceLock::new();

pub fn run(run_root: PathBuf) -> Result<(), String> {
    RUN_ROOT
        .set(run_root)
        .map_err(|_| "service run root was initialized twice".to_owned())?;
    STOP_REQUESTED.store(false, Ordering::SeqCst);

    let mut service_name = wide_null(SERVICE_NAME);
    let table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR::from_raw(service_name.as_mut_ptr()),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: PWSTR::null(),
            lpServiceProc: None,
        },
    ];

    unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) }
        .map_err(|error| format!("StartServiceCtrlDispatcherW failed: {error}"))
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut PWSTR) {
    let Some(run_root) = RUN_ROOT.get().cloned() else {
        return;
    };
    let _ = fs::create_dir_all(&run_root);
    let service_name = wide_null(SERVICE_NAME);
    let handler = unsafe {
        RegisterServiceCtrlHandlerExW(
            PCWSTR::from_raw(service_name.as_ptr()),
            Some(service_handler),
            None,
        )
    };
    let Ok(status_handle) = handler else {
        let _ = write_json(
            &run_root.join("SERVICE-HARNESS-ERROR.json"),
            &serde_json::json!({
                "service_name": SERVICE_NAME,
                "qualification_only": QUALIFICATION_ONLY,
                "error": "RegisterServiceCtrlHandlerExW failed"
            }),
        );
        return;
    };

    let mut journal = ServiceStatusJournal::new();
    let _ = set_and_persist_status(
        status_handle,
        &run_root,
        &mut journal,
        ServiceState::StartPending,
        0,
        "service callback entered",
    );

    let context = match collect_service_context() {
        Ok(context) => context,
        Err(error) => {
            let _ = write_json(
                &run_root.join("SERVICE-HARNESS-ERROR.json"),
                &serde_json::json!({
                    "service_name": SERVICE_NAME,
                    "qualification_only": QUALIFICATION_ONLY,
                    "error": error,
                }),
            );
            let _ = set_and_persist_status(
                status_handle,
                &run_root,
                &mut journal,
                ServiceState::Stopped,
                1,
                "service context collection failed; AMD CLI was not launched",
            );
            return;
        }
    };
    let context_valid = context.satisfies_local_system_session_zero();
    let _ = write_json(&run_root.join("SERVICE-CONTEXT.json"), &context);
    if !context_valid {
        let _ = write_json(
            &run_root.join("SERVICE-HARNESS-ERROR.json"),
            &serde_json::json!({
                "service_name": SERVICE_NAME,
                "qualification_only": QUALIFICATION_ONLY,
                "error": "required LocalSystem Session 0 x64 context was not established",
                "session_id": context.session_id,
                "account_sid": context.account_sid,
                "process_architecture": context.process_architecture,
            }),
        );
        let _ = set_and_persist_status(
            status_handle,
            &run_root,
            &mut journal,
            ServiceState::Stopped,
            1,
            "required service context was not established; AMD CLI was not launched",
        );
        return;
    }

    let _ = set_and_persist_status(
        status_handle,
        &run_root,
        &mut journal,
        ServiceState::Running,
        0,
        "LocalSystem Session 0 context verified",
    );

    let result = match discover_cli() {
        Ok((cli_path, artifact)) => {
            let _ = write_json(&run_root.join("CLI-ARTIFACT-IDENTITY.json"), &artifact);
            run_cli(&run_root, &cli_path)
        }
        Err(error) => Err(error),
    };

    let (exit_code, detail) = match result {
        Ok(cli_result) => {
            let success = cli_result.process_started
                && !cli_result.timeout
                && !cli_result.cancelled
                && cli_result.target_exit_signed == Some(0)
                && cli_result.capture_complete
                && !cli_result.harness_failed;
            let code = if success { 0 } else { 1 };
            let _ = write_json(
                &run_root.join("AMD-SERVICE-CLI-PROCESS-RESULT.json"),
                &cli_result,
            );
            let qualification = if success {
                "TARGET_COMPLETED"
            } else {
                "TARGET_FAILED"
            };
            let launch_evidence_path = run_root.join("AMD-CLI-LAUNCH.json");
            let launch_evidence_present = launch_evidence_path.is_file();
            let execution_state = cli_execution_state(launch_evidence_present, true);
            let summary = ServiceQualificationResult {
                schema: "cpu-sensor-amd-service-context/v1".to_owned(),
                service_name: SERVICE_NAME.to_owned(),
                qualification_only: QUALIFICATION_ONLY,
                service_context_valid: context_valid,
                cli_identity_validated_by_wrapper: true,
                amd_runtime_executed: launch_evidence_present,
                cli_execution_state: execution_state.to_owned(),
                cli_launch_evidence_path: launch_evidence_path.to_string_lossy().into_owned(),
                cli_process_result_path: run_root
                    .join("AMD-SERVICE-CLI-PROCESS-RESULT.json")
                    .to_string_lossy()
                    .into_owned(),
                qualification: qualification.to_owned(),
                created_at_utc_unix_ms: amd_cli_service_context_probe::unix_time_millis(),
            };
            let _ = write_json(&run_root.join("SERVICE-RUN-RESULT.json"), &summary);
            (code, qualification.to_owned())
        }
        Err(error) => {
            let launch_evidence_path = run_root.join("AMD-CLI-LAUNCH.json");
            let launch_evidence_present = launch_evidence_path.is_file();
            let execution_state = cli_execution_state(launch_evidence_present, false);
            let _ = write_json(
                &run_root.join("SERVICE-HARNESS-ERROR.json"),
                &serde_json::json!({
                    "service_name": SERVICE_NAME,
                    "qualification_only": QUALIFICATION_ONLY,
                    "error": error,
                    "amd_cli_launched": launch_evidence_present,
                    "amd_runtime_executed": launch_evidence_present,
                    "amd_cli_execution_state": execution_state,
                    "amd_cli_launch_evidence_path": launch_evidence_path,
                }),
            );
            let detail = if launch_evidence_present {
                "service failed after AMD CLI launch before a complete CLI result"
            } else {
                "service failed before AMD CLI launch"
            };
            (1, detail.to_owned())
        }
    };

    let _ = set_and_persist_status(
        status_handle,
        &run_root,
        &mut journal,
        ServiceState::StopPending,
        exit_code,
        "finalizing qualification evidence",
    );
    let _ = set_and_persist_status(
        status_handle,
        &run_root,
        &mut journal,
        ServiceState::Stopped,
        exit_code,
        &detail,
    );
}

unsafe extern "system" fn service_handler(
    control: u32,
    _event_type: u32,
    _event_data: *mut std::ffi::c_void,
    _context: *mut std::ffi::c_void,
) -> u32 {
    if control == SERVICE_CONTROL_STOP {
        STOP_REQUESTED.store(true, Ordering::SeqCst);
    }
    0
}

fn set_and_persist_status(
    status_handle: SERVICE_STATUS_HANDLE,
    run_root: &Path,
    journal: &mut ServiceStatusJournal,
    state: ServiceState,
    exit_code: u32,
    detail: &str,
) -> Result<(), String> {
    journal.record(state, exit_code, detail);
    write_json(&run_root.join("SERVICE-STATUS.json"), journal)
        .map_err(|error| format!("persisting service status failed: {error}"))?;
    let status = service_status(state, exit_code, journal.states.len() as u32);
    unsafe { SetServiceStatus(status_handle, &status) }
        .map_err(|error| format!("SetServiceStatus failed: {error}"))
}

fn service_status(state: ServiceState, exit_code: u32, checkpoint: u32) -> SERVICE_STATUS {
    let (current_state, controls, wait_hint) = match state {
        ServiceState::StartPending => (SERVICE_START_PENDING, 0, 10_000),
        ServiceState::Running => (SERVICE_RUNNING, SERVICE_ACCEPT_STOP, 0),
        ServiceState::StopPending => (SERVICE_STOP_PENDING, SERVICE_ACCEPT_STOP, 10_000),
        ServiceState::Stopped => (SERVICE_STOPPED, 0, 0),
    };
    let checkpoint = match state {
        ServiceState::StartPending | ServiceState::StopPending => checkpoint,
        ServiceState::Running | ServiceState::Stopped => 0,
    };
    SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: current_state,
        dwControlsAccepted: controls,
        dwWin32ExitCode: exit_code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: checkpoint,
        dwWaitHint: wait_hint,
    }
}

fn discover_cli() -> Result<(PathBuf, CliArtifactIdentity), String> {
    let root = read_installation_root()?;
    if !root.is_absolute() {
        return Err("AMD installation root from registry is not absolute".to_owned());
    }
    let bin = root.join("bin");
    let cli_path = bin.join(AMD_CLI_NAME);
    let bytes =
        fs::read(&cli_path).map_err(|error| format!("reading AMDuProfCLI.exe failed: {error}"))?;
    let architecture = pe_architecture(&bytes)?;
    if architecture != "x64" {
        return Err("AMDuProfCLI.exe is not x64".to_owned());
    }
    let hash = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    if hash != EXPECTED_AMD_CLI_SHA256 {
        return Err(format!("AMDuProfCLI.exe SHA-256 mismatch: {hash}"));
    }
    let artifact = CliArtifactIdentity {
        path: cli_path.to_string_lossy().into_owned(),
        sha256: hash,
        expected_sha256: EXPECTED_AMD_CLI_SHA256.to_owned(),
        architecture,
        expected_version: EXPECTED_AMD_CLI_VERSION.to_owned(),
        signature_validation: "performed by Administrator wrapper before service start".to_owned(),
    };
    Ok((cli_path, artifact))
}

fn read_installation_root() -> Result<PathBuf, String> {
    let key = wide_null(AMD_INSTALL_REGISTRY_KEY);
    let value = wide_null(AMD_INSTALL_REGISTRY_VALUE);
    let mut buffer = vec![0_u16; 1024];
    let mut bytes = (buffer.len() * std::mem::size_of::<u16>()) as u32;
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
    if status.0 != 0 {
        return Err(format!(
            "RegGetValueW failed with Win32 status {}",
            status.0
        ));
    }
    let length = (bytes as usize / std::mem::size_of::<u16>()).min(buffer.len());
    let root = String::from_utf16(&buffer[..length])
        .map_err(|error| format!("InstallationPath is not UTF-16: {error}"))?
        .trim_end_matches('\0')
        .trim()
        .to_owned();
    if root.is_empty() {
        return Err("InstallationPath is empty".to_owned());
    }
    Ok(PathBuf::from(root))
}

fn run_cli(run_root: &Path, cli_path: &Path) -> Result<CliProcessResult, String> {
    let output_dir = run_root.join("timechart-output");
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("creating output directory failed: {error}"))?;
    let stdout_path = run_root.join("AMD-CLI.stdout.txt");
    let stderr_path = run_root.join("AMD-CLI.stderr.txt");
    let stdout_file = File::create(&stdout_path)
        .map_err(|error| format!("creating stdout file failed: {error}"))?;
    let stderr_file = File::create(&stderr_path)
        .map_err(|error| format!("creating stderr file failed: {error}"))?;
    let working_directory = cli_path
        .parent()
        .ok_or_else(|| "AMDuProfCLI.exe has no parent directory".to_owned())?;
    let arguments = fixed_cli_arguments(&output_dir);
    let started_at = amd_cli_service_context_probe::unix_time_millis();
    let stopwatch = Instant::now();

    let mut command = Command::new(cli_path);
    command
        .args(&arguments)
        .current_dir(working_directory)
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    let mut child = command
        .spawn()
        .map_err(|error| format!("AMDuProfCLI.exe spawn failed: {error}"))?;
    let target_pid = child.id();
    let launch_evidence = CliLaunchEvidence {
        schema: "cpu-sensor-amd-service-context/cli-launch/v1".to_owned(),
        process_started: true,
        target_pid,
        started_at_utc_unix_ms: started_at,
        executable: cli_path.to_string_lossy().into_owned(),
        arguments: arguments.clone(),
        working_directory: working_directory.to_string_lossy().into_owned(),
        output_directory: output_dir.to_string_lossy().into_owned(),
    };
    if let Err(error) = write_json(&run_root.join("AMD-CLI-LAUNCH.json"), &launch_evidence) {
        let _ = child.kill();
        let _ = wait_for_exit_until(&mut child, Instant::now() + Duration::from_secs(5));
        return Err(format!(
            "AMDuProfCLI.exe spawned with PID {target_pid}, but launch evidence persistence failed: {error}"
        ));
    }
    let job = OwnedJob::try_attach(&child);
    let job_assigned = job.is_some();

    let mut timeout = false;
    let mut cancelled = false;
    let mut cleanup_attempted = false;
    let mut cleanup_succeeded = false;
    let exit_status = loop {
        if STOP_REQUESTED.load(Ordering::SeqCst) {
            cancelled = true;
            cleanup_attempted = true;
            let (terminated, status) = terminate_owned(&mut child, job.as_ref());
            cleanup_succeeded = terminated;
            break status;
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("polling AMDuProfCLI.exe failed: {error}"))?
        {
            break Some(status);
        }
        if stopwatch.elapsed() >= Duration::from_millis(CLI_TIMEOUT_MS) {
            timeout = true;
            cleanup_attempted = true;
            let (terminated, status) = terminate_owned(&mut child, job.as_ref());
            cleanup_succeeded = terminated;
            break status;
        }
        thread::sleep(Duration::from_millis(50));
    };
    drop(job);

    let finished_at = amd_cli_service_context_probe::unix_time_millis();
    let target_exit_signed = exit_status.and_then(|status| status.code());
    let stdout_bytes = fs::metadata(&stdout_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let stderr_bytes = fs::metadata(&stderr_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let stdout_persisted = stdout_path.is_file();
    let stderr_persisted = stderr_path.is_file();
    let capture_complete = stdout_persisted && stderr_persisted;
    let target_process_failed =
        timeout || cancelled || target_exit_signed.is_none() || target_exit_signed != Some(0);
    Ok(CliProcessResult {
        executable: cli_path.to_string_lossy().into_owned(),
        arguments,
        working_directory: working_directory.to_string_lossy().into_owned(),
        output_directory: output_dir.to_string_lossy().into_owned(),
        process_started: true,
        target_pid: Some(target_pid),
        started_at_utc_unix_ms: started_at,
        finished_at_utc_unix_ms: finished_at,
        duration_ms: stopwatch.elapsed().as_millis(),
        timeout_ms: CLI_TIMEOUT_MS,
        timeout,
        cancelled,
        target_exit_hex: target_exit_signed.map(exit_code_hex),
        target_exit_signed,
        target_process_failed,
        stdout_path: stdout_path.to_string_lossy().into_owned(),
        stderr_path: stderr_path.to_string_lossy().into_owned(),
        stdout_bytes,
        stderr_bytes,
        stdout_persisted,
        stderr_persisted,
        capture_complete,
        job_assigned,
        cleanup_attempted,
        cleanup_succeeded: if cleanup_attempted {
            cleanup_succeeded
        } else {
            true
        },
        harness_failed: false,
        harness_error: None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminationTarget {
    Job,
    ExactChild,
}

fn termination_target(job_assigned: bool) -> TerminationTarget {
    if job_assigned {
        TerminationTarget::Job
    } else {
        TerminationTarget::ExactChild
    }
}

fn terminate_owned(child: &mut Child, job: Option<&OwnedJob>) -> (bool, Option<ExitStatus>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let target = termination_target(job.is_some());
    let termination_requested = match (target, job) {
        (TerminationTarget::Job, Some(job)) => unsafe { TerminateJobObject(job.0, 1) }.is_ok(),
        (TerminationTarget::ExactChild, _) => child.kill().is_ok(),
        (TerminationTarget::Job, None) => false,
    };

    if target == TerminationTarget::Job && !termination_requested {
        let _ = child.kill();
    }

    let status = wait_for_exit_until(child, deadline);
    (status.is_some(), status)
}

fn wait_for_exit_until(child: &mut Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(None) | Err(_) => return None,
        }
    }
}

struct OwnedJob(HANDLE);

impl OwnedJob {
    fn new() -> Result<Self, String> {
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|error| format!("CreateJobObjectW failed: {error}"))?;
        Ok(Self(handle))
    }

    fn assign(&self, child: &Child) -> Result<(), String> {
        let handle = HANDLE(child.as_raw_handle());
        unsafe { AssignProcessToJobObject(self.0, handle) }
            .map_err(|error| format!("AssignProcessToJobObject failed: {error}"))
    }

    fn try_attach(child: &Child) -> Option<Self> {
        let job = Self::new().ok()?;
        if job.assign(child).is_err() {
            return None;
        }
        Some(job)
    }
}

impl Drop for OwnedJob {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

fn collect_service_context() -> Result<ServiceContextEvidence, String> {
    let process_id = unsafe { GetCurrentProcessId() };
    let parent_process_id = find_parent_process_id(process_id);
    let session_id = process_id_to_session_id(process_id);
    let current_process = unsafe { GetCurrentProcess() };
    let token = open_process_token(current_process)?;
    let token_user = token_information(token, TokenUser)?;
    let user = unsafe { &*(token_user.as_ptr() as *const TOKEN_USER) };
    let account_sid = sid_to_string(user.User.Sid).unwrap_or_else(|| "UNKNOWN".to_owned());
    let account_is_local_system = account_sid.eq_ignore_ascii_case(EXPECTED_LOCAL_SYSTEM_SID);
    let account = if account_is_local_system {
        "NT AUTHORITY\\SYSTEM".to_owned()
    } else {
        account_sid.clone()
    };
    let integrity_sid = token_information(token, TokenIntegrityLevel)
        .ok()
        .and_then(|bytes| {
            let label = unsafe { &*(bytes.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
            sid_to_string(label.Label.Sid)
        });
    let token_elevated = token_information(token, TokenElevation)
        .ok()
        .map(|bytes| unsafe { (*(bytes.as_ptr() as *const TOKEN_ELEVATION)).TokenIsElevated != 0 });
    let token_elevation_type = token_information(token, TokenElevationType)
        .ok()
        .map(|bytes| {
            let value = unsafe { *(bytes.as_ptr() as *const TOKEN_ELEVATION_TYPE) };
            match value.0 {
                1 => "Default",
                2 => "Full",
                3 => "Limited",
                _ => "Unknown",
            }
            .to_owned()
        });
    let _ = unsafe { CloseHandle(token) };

    let current_directory = std::env::current_dir()
        .ok()
        .map(|path| path.to_string_lossy().into_owned());
    let environment_subset = std::env::vars_os()
        .filter_map(|(key, value)| {
            let key_string = key.to_string_lossy().into_owned();
            let allowed = matches!(
                key_string.as_str(),
                "PATH" | "TEMP" | "TMP" | "ProgramData" | "USERPROFILE"
            );
            allowed.then(|| (key_string, value.to_string_lossy().into_owned()))
        })
        .collect();

    Ok(ServiceContextEvidence {
        service_name: SERVICE_NAME.to_owned(),
        qualification_only: QUALIFICATION_ONLY,
        process_id,
        parent_process_id,
        account,
        account_sid,
        account_is_local_system,
        session_id,
        integrity_level: integrity_sid.clone(),
        integrity_sid,
        token_elevated,
        token_elevation_type,
        process_architecture: if std::mem::size_of::<usize>() == 8 {
            "x64".to_owned()
        } else {
            "non-x64".to_owned()
        },
        current_directory,
        environment_subset,
        service_started_at_utc_unix_ms: amd_cli_service_context_probe::unix_time_millis(),
    })
}

fn open_process_token(process: HANDLE) -> Result<HANDLE, String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
        .map_err(|error| format!("OpenProcessToken failed: {error}"))?;
    Ok(token)
}

fn token_information(
    token: HANDLE,
    information_class: TOKEN_INFORMATION_CLASS,
) -> Result<Vec<u8>, String> {
    let mut required = 0_u32;
    let _ = unsafe { GetTokenInformation(token, information_class, None, 0, &mut required) };
    if required == 0 {
        return Err("GetTokenInformation did not return a buffer size".to_owned());
    }
    let mut bytes = vec![0_u8; required as usize];
    unsafe {
        GetTokenInformation(
            token,
            information_class,
            Some(bytes.as_mut_ptr() as *mut _),
            required,
            &mut required,
        )
    }
    .map_err(|error| format!("GetTokenInformation failed: {error}"))?;
    Ok(bytes)
}

fn process_id_to_session_id(process_id: u32) -> Option<u32> {
    let mut session_id = 0_u32;
    unsafe { ProcessIdToSessionId(process_id, &mut session_id) }
        .ok()
        .map(|_| session_id)
}

fn find_parent_process_id(process_id: u32) -> Option<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut parent = None;
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok() {
        loop {
            if entry.th32ProcessID == process_id {
                parent = Some(entry.th32ParentProcessID);
                break;
            }
            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(snapshot) };
    parent
}

fn sid_to_string(sid: windows::Win32::Security::PSID) -> Option<String> {
    if sid.is_invalid() {
        return None;
    }
    let mut output = PWSTR::null();
    if unsafe { ConvertSidToStringSidW(sid, &mut output) }.is_err() {
        return None;
    }
    if output.is_null() {
        return None;
    }
    let mut length = 0_usize;
    unsafe {
        while *output.0.add(length) != 0 {
            length += 1;
        }
    }
    let value = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(output.0, length) });
    unsafe {
        let _ = windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(
            output.0 as *mut std::ffi::c_void,
        ));
    }
    Some(value)
}

fn pe_architecture(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 0x40 {
        return Err("PE image is too small".to_owned());
    }
    let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    if pe_offset.checked_add(6).is_none() || bytes.get(pe_offset..pe_offset + 4) != Some(b"PE\0\0")
    {
        return Err("PE signature is missing".to_owned());
    }
    let machine = u16::from_le_bytes(bytes[pe_offset + 4..pe_offset + 6].try_into().unwrap());
    match machine {
        0x8664 => Ok("x64".to_owned()),
        0x014c => Ok("x86".to_owned()),
        0xaa64 => Ok("ARM64".to_owned()),
        _ => Ok(format!("UNKNOWN(0x{machine:04X})")),
    }
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scm_status_mapping_reports_expected_protocol_states() {
        let pending = service_status(ServiceState::StartPending, 0, 1);
        assert_eq!(pending.dwCurrentState, SERVICE_START_PENDING);
        assert_eq!(pending.dwControlsAccepted, 0);

        let running = service_status(ServiceState::Running, 0, 2);
        assert_eq!(running.dwCurrentState, SERVICE_RUNNING);
        assert_eq!(running.dwControlsAccepted, SERVICE_ACCEPT_STOP);

        let stopped = service_status(ServiceState::Stopped, 7, 3);
        assert_eq!(stopped.dwCurrentState, SERVICE_STOPPED);
        assert_eq!(stopped.dwWin32ExitCode, 7);
        assert_eq!(stopped.dwControlsAccepted, 0);
    }

    #[test]
    fn cancellation_status_keeps_stop_pending_distinct() {
        let status = service_status(ServiceState::StopPending, 0, 1);
        assert_eq!(status.dwCurrentState, SERVICE_STOP_PENDING);
        assert_eq!(status.dwControlsAccepted, SERVICE_ACCEPT_STOP);
    }

    #[test]
    fn job_assignment_failure_uses_exact_child_termination_fallback() {
        assert_eq!(
            termination_target(false),
            TerminationTarget::ExactChild,
            "an unassigned job must never be used for cleanup"
        );
        assert_eq!(termination_target(true), TerminationTarget::Job);
    }
}
