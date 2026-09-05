//! Windows-only service, named-pipe, identity, and future real-run implementation.
//!
//! The code is reachable only through explicit `--broker` or `--client` commands.  The
//! automated `--synthetic` path never calls this module and therefore cannot register a service
//! or launch AMD uProf.

use crate::package_power::{assess_cadence, parse_package_power_csv};
use crate::{
    authorize_client, build_pipe_dacl, decode_request, encode_json_frame, request_id_from_json,
    response_for_protocol_error, response_for_session_error, BrokerConfig, BrokerResponse,
    ClientIdentity, ProviderStatus, ResponseStatus, SemanticRequest, SessionCoordinator,
    SessionResultSummary, SessionState, CLIENT_DISCONNECT_POLICY, FIXED_EVENT, OUTPUT_SUBDIRECTORY,
    QUALIFICATION_ONLY, SERVICE_ACCOUNT_SID, SERVICE_NAME, SERVICE_SID_ACCOUNT,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Write};
use std::mem::size_of;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use windows::core::{HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_MORE_DATA,
    ERROR_NOT_FOUND, ERROR_OPERATION_ABORTED, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, WAIT_FAILED,
    WAIT_OBJECT_0, WIN32_ERROR,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::WinTrust::{
    WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0,
    WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
    WTD_STATEACTION_IGNORE, WTD_UI_NONE,
};
use windows::Win32::Security::{
    GetTokenInformation, RevertToSelf, TokenElevation, TokenGroups, TokenIntegrityLevel,
    TokenSessionId, TokenUser, TOKEN_ELEVATION, TOKEN_GROUPS, TOKEN_INFORMATION_CLASS,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetFileVersionInfoSizeW, GetFileVersionInfoW, ReadFile, VerQueryValueW, WriteFile,
    FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, PIPE_ACCESS_DUPLEX, SECURITY_IMPERSONATION, SECURITY_SQOS_PRESENT,
    VS_FIXEDFILEINFO,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeClientProcessId,
    ImpersonateNamedPipeClient, PIPE_READMODE_MESSAGE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Services::{
    CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceConfig2W,
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
    SC_MANAGER_CONNECT, SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP,
    SERVICE_CONFIG_SERVICE_SID_INFO, SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP,
    SERVICE_QUERY_CONFIG, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STATUS,
    SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_HANDLE, SERVICE_STOPPED, SERVICE_STOP_PENDING,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};
use windows::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, GetCurrentProcessId, GetCurrentThread, GetProcessTimes,
    OpenProcess, OpenProcessToken, OpenThreadToken, SetEvent, WaitForMultipleObjects, INFINITE,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

const AMD_INSTALL_REGISTRY_KEY: &str = r"SOFTWARE\WOW6432Node\AMD\AMDProfiler";
const AMD_INSTALL_REGISTRY_VALUE: &str = "InstallationPath";
const AMD_CLI_NAME: &str = "AMDuProfCLI.exe";
const REQUIRED_SERVICE_SID_TYPE: &str = "UNRESTRICTED";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const PIPE_INSTANCE_COUNT: u32 = 8;
const PIPE_BUFFER_BYTES: u32 = 16 * 1024;
const SE_GROUP_ENABLED: u32 = 0x0000_0004;

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
static CONFIG: OnceLock<BrokerConfig> = OnceLock::new();
static STOP_EVENT: Mutex<Option<isize>> = Mutex::new(None);
static STATUS_HANDLE: Mutex<Option<isize>> = Mutex::new(None);
static SERVICE_ERROR_DETAILS: Mutex<Option<Value>> = Mutex::new(None);

fn remember_service_error_details(details: Value) {
    let mut current = SERVICE_ERROR_DETAILS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *current = Some(details);
}

fn take_service_error_details() -> Option<Value> {
    SERVICE_ERROR_DETAILS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
}

fn windows_error_details(context: &str, error: &windows::core::Error) -> Value {
    let hresult = error.code().0 as u32;
    let win32_error = crate::win32_code_from_hresult_contract(hresult);
    json!({
        "error_message": format!("{context}: {error}"),
        "hresult_hex": format!("0x{hresult:08X}"),
        "win32_error_code": win32_error,
        "win32_error_hex": win32_error.map(|value| format!("0x{value:08X}")),
    })
}

fn win32_error_details(context: &str, win32_error: u32) -> Value {
    let hresult = crate::hresult_from_win32_contract(win32_error);
    json!({
        "error_message": format!("{context} failed with Win32 error {win32_error}"),
        "hresult_hex": hresult.map(|value| format!("0x{value:08X}")),
        "win32_error_code": win32_error,
        "win32_error_hex": format!("0x{win32_error:08X}"),
    })
}

fn service_error_evidence(error: String) -> Value {
    let mut evidence = json!({
        "schema": "amd-privilege-service-error/v1",
        "qualification_only": QUALIFICATION_ONLY,
        "service_name": SERVICE_NAME,
        "error": error,
    });
    if let Some(details) = take_service_error_details() {
        if let (Some(target), Some(source)) = (evidence.as_object_mut(), details.as_object()) {
            for (key, value) in source {
                target.insert(key.clone(), value.clone());
            }
        }
    }
    evidence
}

fn record_windows_service_error(context: &str, error: &windows::core::Error) {
    remember_service_error_details(windows_error_details(context, error));
}

fn record_win32_service_error(context: &str, win32_error: u32) {
    remember_service_error_details(win32_error_details(context, win32_error));
}

fn error_is_win32(error: &windows::core::Error, win32: WIN32_ERROR) -> bool {
    error.code() == HRESULT::from_win32(win32.0)
}

fn win32_code_from_hresult(error: HRESULT) -> Option<u32> {
    crate::win32_code_from_hresult_contract(error.0 as u32)
}

#[derive(Debug, Clone, Serialize)]
struct ServiceContextEvidence {
    schema: String,
    qualification_only: bool,
    service_name: String,
    service_account: String,
    account_sid: String,
    service_sid: Option<String>,
    service_sid_present: bool,
    service_sid_type: String,
    process_id: u32,
    session_id: Option<u32>,
    process_architecture: String,
    integrity_sid: Option<String>,
    token_elevated: Option<bool>,
    context_valid: bool,
}

#[derive(Clone)]
struct BrokerState {
    config: BrokerConfig,
    coordinator: SessionCoordinator,
    output_root: PathBuf,
}

pub struct ClientOptions {
    pub operation: String,
    pub duration_ms: u32,
    pub interval_ms: u32,
}

pub fn run_service() -> Result<(), String> {
    let config = load_config()?;
    CONFIG
        .set(config)
        .map_err(|_| "broker configuration was initialized twice".to_owned())?;
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
    let Some(config) = CONFIG.get().cloned() else {
        return;
    };
    let root = PathBuf::from(&config.output_root);
    let _ = fs::create_dir_all(&root);
    let service_name = wide_null(SERVICE_NAME);
    let handler = unsafe {
        RegisterServiceCtrlHandlerExW(
            PCWSTR::from_raw(service_name.as_ptr()),
            Some(service_handler),
            None,
        )
    };
    let Ok(status_handle) = handler else {
        let _ = crate::write_json(
            &root.join("SERVICE-HARNESS-ERROR.json"),
            &service_error_evidence("RegisterServiceCtrlHandlerExW failed".to_owned()),
        );
        return;
    };

    set_status_handle(status_handle);
    let _ = set_service_status(status_handle, SERVICE_START_PENDING, 0, 1, 30_000);
    match service_entry(status_handle, &config) {
        Ok(()) => {
            let _ = set_service_status(status_handle, SERVICE_STOPPED, 0, 0, 0);
        }
        Err(error) => {
            let _ = crate::write_json(
                &root.join("SERVICE-HARNESS-ERROR.json"),
                &service_error_evidence(error),
            );
            let _ = set_service_status(status_handle, SERVICE_STOPPED, 1, 0, 0);
        }
    }
    clear_status_handle(status_handle);
}

unsafe extern "system" fn service_handler(
    control: u32,
    _event_type: u32,
    _event_data: *mut std::ffi::c_void,
    _context: *mut std::ffi::c_void,
) -> u32 {
    if matches!(control, SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN) {
        STOP_REQUESTED.store(true, Ordering::SeqCst);
        if let Some(raw) = STATUS_HANDLE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .copied()
        {
            let handle = SERVICE_STATUS_HANDLE(raw as *mut std::ffi::c_void);
            let _ = set_service_status(handle, SERVICE_STOP_PENDING, 0, 1, 30_000);
        }
        signal_stop_event();
    }
    0
}

fn set_status_handle(status_handle: SERVICE_STATUS_HANDLE) {
    let mut current = STATUS_HANDLE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *current = Some(status_handle.0 as isize);
}

fn clear_status_handle(status_handle: SERVICE_STATUS_HANDLE) {
    let mut current = STATUS_HANDLE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if current.is_some_and(|raw| raw == status_handle.0 as isize) {
        *current = None;
    }
}

fn install_stop_event(handle: HANDLE) {
    let mut current = STOP_EVENT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *current = Some(handle.0 as isize);
}

fn clear_stop_event(handle: HANDLE) {
    let mut current = STOP_EVENT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if current.is_some_and(|raw| raw == handle.0 as isize) {
        *current = None;
    }
}

fn signal_stop_event() {
    let raw = STOP_EVENT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ref()
        .copied();
    if let Some(raw) = raw {
        unsafe {
            let _ = SetEvent(HANDLE(raw as *mut _));
        }
    }
}

fn set_service_status(
    status_handle: SERVICE_STATUS_HANDLE,
    state: SERVICE_STATUS_CURRENT_STATE,
    exit_code: u32,
    checkpoint: u32,
    wait_hint: u32,
) -> Result<(), String> {
    let controls = if state == SERVICE_RUNNING || state == SERVICE_STOP_PENDING {
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
    } else {
        0
    };
    let status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: controls,
        dwWin32ExitCode: exit_code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: checkpoint,
        dwWaitHint: wait_hint,
    };
    unsafe { SetServiceStatus(status_handle, &status) }
        .map_err(|error| format!("SetServiceStatus failed: {error}"))
}

fn service_entry(
    status_handle: SERVICE_STATUS_HANDLE,
    config: &BrokerConfig,
) -> Result<(), String> {
    let context = collect_service_context(config)?;
    crate::write_json(
        Path::new(&config.output_root)
            .join("SERVICE-CONTEXT.json")
            .as_path(),
        &context,
    )
    .map_err(|error| format!("writing SERVICE-CONTEXT.json failed: {error}"))?;
    if !context.context_valid {
        return Err(
            "LocalService, Service SID, Session 0 and x64 context was not established".to_owned(),
        );
    }
    let service_sid = config
        .service_sid
        .as_deref()
        .ok_or_else(|| "service SID is missing from broker configuration".to_owned())?;
    let dacl = build_pipe_dacl(&config.pipe_name, &config.installing_user_sid, service_sid)?;
    crate::write_json(
        Path::new(&config.output_root)
            .join("PIPE_DACL.json")
            .as_path(),
        &dacl,
    )
    .map_err(|error| format!("writing PIPE_DACL.json failed: {error}"))?;
    crate::write_json(
        Path::new(&config.output_root)
            .join("SERVICE-SID.json")
            .as_path(),
        &json!({
            "schema": "amd-privilege-service-sid/v1",
            "service_name": SERVICE_NAME,
            "service_sid_account": SERVICE_SID_ACCOUNT,
            "service_sid": service_sid,
            "service_sid_present": context.service_sid_present,
            "service_sid_type": context.service_sid_type,
            "required_service_sid_type": REQUIRED_SERVICE_SID_TYPE
        }),
    )
    .map_err(|error| format!("writing SERVICE-SID.json failed: {error}"))?;
    let state = BrokerState {
        config: config.clone(),
        coordinator: SessionCoordinator::new(),
        output_root: PathBuf::from(&config.output_root),
    };
    let stop_event = StopEvent::new()?;
    install_stop_event(stop_event.raw());
    if STOP_REQUESTED.load(Ordering::Acquire) {
        signal_stop_event();
    }
    let mut readiness = crate::BrokerReadinessState::new();
    let first_pipe = create_pipe(&state.config)?;
    let first_accept = arm_pipe_accept(first_pipe)?;
    if STOP_REQUESTED.load(Ordering::Acquire) {
        return abandon_startup_accept(&state.output_root, first_accept, None);
    }
    readiness.mark_first_listener_created();
    if let Err(error) = readiness.mark_first_accept_armed(first_accept.readiness_state()) {
        return abandon_startup_accept(&state.output_root, first_accept, Some(error.to_owned()));
    }
    let readiness_result = (|| {
        set_listener_ready_hint(
            &state.output_root,
            &state.config,
            first_accept.readiness_state(),
        )?;
        readiness.publish_ready().map_err(str::to_owned)?;
        set_service_running_hint(&state.output_root, first_accept.readiness_state())?;
        readiness.report_running().map_err(str::to_owned)?;
        set_service_status(status_handle, SERVICE_RUNNING, 0, 0, 0)
    })();
    if let Err(error) = readiness_result {
        return abandon_startup_accept(&state.output_root, first_accept, Some(error));
    }
    broker_loop(&state, first_accept, stop_event.raw())
}

fn abandon_startup_accept(
    output_root: &Path,
    accept: ArmedPipeAccept,
    primary_failure: Option<String>,
) -> Result<(), String> {
    let cancellation_requested = accept.is_io_pending();
    let drain_result = accept.cancel_and_drain();
    let (terminal_state, drained, drain_error) = match &drain_result {
        Ok(state) => (pending_accept_terminal_state_name(*state), true, None),
        Err(error) => ("FAILED", false, Some(error.clone())),
    };
    let primary_label = primary_failure
        .as_deref()
        .unwrap_or("STOP_REQUESTED_BEFORE_READY");
    let _ = crate::write_json(
        &output_root.join("PENDING-ACCEPT-CLEANUP.json"),
        &json!({
            "schema": "amd-privilege-pending-accept-cleanup/v1",
            "qualification_only": QUALIFICATION_ONLY,
            "primary_startup_failure": primary_label,
            "pending_accept_cancellation_requested": cancellation_requested,
            "pending_accept_terminal_state": terminal_state,
            "pending_accept_drained": drained,
            "pending_accept_drain_error": drain_error,
        }),
    );

    match (primary_failure, drain_result) {
        (None, Ok(_)) => Ok(()),
        (None, Err(error)) => Err(format!(
            "stop requested before readiness, but pending accept drain failed: {error}"
        )),
        (Some(error), Ok(_)) => Err(error),
        (Some(error), Err(drain_error)) => Err(format!(
            "{error}; pending accept drain failed: {drain_error}"
        )),
    }
}

fn pending_accept_terminal_state_name(state: PendingAcceptTerminalState) -> &'static str {
    match state {
        PendingAcceptTerminalState::Connected => "CONNECTED",
        PendingAcceptTerminalState::Cancelled => "CANCELLED",
    }
}

fn return_after_accept_drain<T>(
    accept: ArmedPipeAccept,
    primary_error: String,
) -> Result<T, String> {
    match accept.cancel_and_drain() {
        Ok(_) => Err(primary_error),
        Err(drain_error) => Err(format!(
            "{primary_error}; pending accept drain failed: {drain_error}"
        )),
    }
}

fn set_service_running_hint(
    root: &Path,
    first_accept_state: crate::FirstAcceptState,
) -> Result<(), String> {
    crate::write_json(
        &root.join("BROKER-READY.json"),
        &json!({
            "schema": "amd-privilege-broker-ready/v1",
            "qualification_only": QUALIFICATION_ONLY,
            "service_name": SERVICE_NAME,
            "ipc": "WINDOWS_NAMED_PIPE",
            "semantic_protocol_only": true,
            "pipe_reject_remote_clients": true,
            "listener_created": true,
            "live_listener_handle": true,
            "first_accept_armed": true,
            "accept_mode": "FILE_FLAG_OVERLAPPED",
            "first_accept_state": first_accept_state_name(first_accept_state),
            "client_disconnect_policy": CLIENT_DISCONNECT_POLICY,
            "active_sessions": 1
        }),
    )
    .map_err(|error| format!("writing BROKER-READY.json failed: {error}"))
}

fn set_listener_ready_hint(
    root: &Path,
    config: &BrokerConfig,
    first_accept_state: crate::FirstAcceptState,
) -> Result<(), String> {
    crate::write_json(
        &root.join("PIPE-LISTENER-READY.json"),
        &json!({
            "schema": "amd-privilege-pipe-listener-ready/v1",
            "qualification_only": QUALIFICATION_ONLY,
            "pipe_name": config.pipe_name,
            "listener_created": true,
            "first_accept_armed": true,
            "accept_mode": "FILE_FLAG_OVERLAPPED",
            "first_accept_state": first_accept_state_name(first_accept_state),
            "pipe_reject_remote_clients": true,
            "semantic_protocol_only": true
        }),
    )
    .map_err(|error| format!("writing PIPE-LISTENER-READY.json failed: {error}"))
}

fn broker_loop(
    state: &BrokerState,
    first_accept: ArmedPipeAccept,
    stop_event: HANDLE,
) -> Result<(), String> {
    let mut pending_accept = Some(first_accept);
    let mut first_accept_pending = true;
    while !STOP_REQUESTED.load(Ordering::Acquire) {
        let accept = match pending_accept.take() {
            Some(accept) => Ok(accept),
            None => create_pipe(&state.config).and_then(arm_pipe_accept),
        };
        let accept = match accept {
            Ok(accept) => accept,
            Err(error) => {
                if STOP_REQUESTED.load(Ordering::Acquire) {
                    break;
                }
                return Err(error);
            }
        };
        let Some(pipe) = wait_for_pipe_accept(accept, stop_event)? else {
            break;
        };
        if STOP_REQUESTED.load(Ordering::Acquire) {
            break;
        }
        if first_accept_pending {
            first_accept_pending = false;
            let _ = crate::write_json(
                &state.output_root.join("FIRST-ACCEPT-REUSED.json"),
                &json!({
                    "schema": "amd-privilege-first-accept-reused/v1",
                    "qualification_only": QUALIFICATION_ONLY,
                    "first_accept_reused_for_first_client": true,
                    "accept_mode": "FILE_FLAG_OVERLAPPED"
                }),
            );
        }
        let stream = NamedPipeStream::new(pipe);
        let state = state.clone();
        thread::spawn(move || handle_connection(stream, state));
    }
    state.coordinator.cancel_active_on_shutdown();
    if let Some(accept) = pending_accept.take() {
        accept.cancel_and_drain().map(|_| ()).map_err(|error| {
            format!("draining pending named-pipe accept during shutdown failed: {error}")
        })
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
struct PipeFrameDiagnostic {
    schema: &'static str,
    qualification_only: bool,
    frame_prefix_read_attempted: bool,
    prefix_bytes_read: usize,
    prefix_more_data: bool,
    declared_payload_length: Option<usize>,
    payload_bytes_read: usize,
    payload_more_data: bool,
    message_boundary_complete: bool,
    frame_result: &'static str,
    request_dispatch_allowed: bool,
}

struct PipeFrameRead {
    payload: Option<Vec<u8>>,
    error: Option<String>,
    diagnostic: PipeFrameDiagnostic,
}

fn initial_pipe_frame_diagnostic() -> PipeFrameDiagnostic {
    PipeFrameDiagnostic {
        schema: "amd-privilege-first-frame/v1",
        qualification_only: QUALIFICATION_ONLY,
        frame_prefix_read_attempted: true,
        prefix_bytes_read: 0,
        prefix_more_data: false,
        declared_payload_length: None,
        payload_bytes_read: 0,
        payload_more_data: false,
        message_boundary_complete: false,
        frame_result: "IO_ERROR",
        request_dispatch_allowed: false,
    }
}

fn pipe_frame_error(diagnostic: PipeFrameDiagnostic, message: impl Into<String>) -> PipeFrameRead {
    PipeFrameRead {
        payload: None,
        error: Some(message.into()),
        diagnostic,
    }
}

fn read_pipe_frame(stream: &mut NamedPipeStream) -> PipeFrameRead {
    let mut diagnostic = initial_pipe_frame_diagnostic();
    let mut prefix = [0_u8; 4];
    let prefix_chunk = match overlapped_read_chunk(stream.raw(), &mut prefix) {
        Ok(chunk) => chunk,
        Err(error) => {
            return pipe_frame_error(
                diagnostic,
                format!("pipe frame prefix read failed: {error}"),
            )
        }
    };
    diagnostic.prefix_bytes_read = prefix_chunk.bytes_read;
    diagnostic.prefix_more_data = prefix_chunk.more_data;

    if prefix_chunk.bytes_read == 0 {
        diagnostic.frame_result = "EOF_BEFORE_PREFIX";
        return PipeFrameRead {
            payload: None,
            error: None,
            diagnostic,
        };
    }
    if prefix_chunk.bytes_read != prefix.len() {
        diagnostic.frame_result = "TRUNCATED_PREFIX";
        return pipe_frame_error(
            diagnostic,
            format!(
                "pipe frame prefix was truncated: read {} of {} bytes",
                prefix_chunk.bytes_read,
                prefix.len()
            ),
        );
    }

    let declared_length = u32::from_le_bytes(prefix) as usize;
    diagnostic.declared_payload_length = Some(declared_length);
    if declared_length > crate::MAX_FRAME_BYTES {
        diagnostic.frame_result = "INVALID_LENGTH";
        return pipe_frame_error(
            diagnostic,
            format!("pipe frame exceeds maximum request size: {declared_length} bytes"),
        );
    }

    if declared_length == 0 {
        if prefix_chunk.more_data {
            diagnostic.frame_result = "TRAILING_MESSAGE_DATA";
            return pipe_frame_error(
                diagnostic,
                "zero-length frame has trailing named-pipe message data",
            );
        }
        diagnostic.frame_result = "VALID";
        diagnostic.message_boundary_complete = true;
        diagnostic.request_dispatch_allowed = true;
        return PipeFrameRead {
            payload: Some(Vec::new()),
            error: None,
            diagnostic,
        };
    }

    if !prefix_chunk.more_data {
        diagnostic.frame_result = "TRUNCATED_PAYLOAD";
        return pipe_frame_error(
            diagnostic,
            "pipe frame prefix message ended before the declared payload",
        );
    }

    let mut payload = vec![0_u8; declared_length];
    let payload_chunk = match overlapped_read_chunk(stream.raw(), &mut payload) {
        Ok(chunk) => chunk,
        Err(error) => {
            return pipe_frame_error(
                diagnostic,
                format!("pipe frame payload read failed: {error}"),
            )
        }
    };
    diagnostic.payload_bytes_read = payload_chunk.bytes_read;
    diagnostic.payload_more_data = payload_chunk.more_data;
    if payload_chunk.bytes_read != declared_length {
        diagnostic.frame_result = "TRUNCATED_PAYLOAD";
        return pipe_frame_error(
            diagnostic,
            format!(
                "pipe frame payload was truncated: read {} of {} bytes",
                payload_chunk.bytes_read, declared_length
            ),
        );
    }
    if payload_chunk.more_data {
        diagnostic.frame_result = "TRAILING_MESSAGE_DATA";
        return pipe_frame_error(
            diagnostic,
            "pipe frame message contains trailing data beyond its declared payload",
        );
    }

    diagnostic.frame_result = "VALID";
    diagnostic.message_boundary_complete = true;
    diagnostic.request_dispatch_allowed = true;
    PipeFrameRead {
        payload: Some(payload),
        error: None,
        diagnostic,
    }
}

fn write_first_frame_evidence(
    state: &BrokerState,
    identity: &ClientIdentity,
    frame: &PipeFrameRead,
    authorized: bool,
) {
    let mut diagnostic = frame.diagnostic.clone();
    diagnostic.request_dispatch_allowed = authorized
        && frame.error.is_none()
        && frame.payload.is_some()
        && diagnostic.frame_result == "VALID";
    let _ = crate::write_json(
        &state
            .output_root
            .join(format!("FIRST-FRAME-{}.json", identity.client_pid)),
        &json!({
            "schema": diagnostic.schema,
            "qualification_only": diagnostic.qualification_only,
            "client_pid": identity.client_pid,
            "frame_prefix_read_attempted": diagnostic.frame_prefix_read_attempted,
            "prefix_bytes_read": diagnostic.prefix_bytes_read,
            "prefix_more_data": diagnostic.prefix_more_data,
            "declared_payload_length": diagnostic.declared_payload_length,
            "payload_bytes_read": diagnostic.payload_bytes_read,
            "payload_more_data": diagnostic.payload_more_data,
            "message_boundary_complete": diagnostic.message_boundary_complete,
            "frame_result": diagnostic.frame_result,
            "request_dispatch_allowed": diagnostic.request_dispatch_allowed
        }),
    );
}

fn handle_connection(mut stream: NamedPipeStream, state: BrokerState) {
    // Named-pipe impersonation is only valid after a client message has been read.  Buffer the
    // first bounded frame and hold it outside the dispatch path until authentication completes.
    let first_frame = read_pipe_frame(&mut stream);
    let identity = match capture_client_identity(stream.raw()) {
        Ok(identity) => identity,
        Err(error) => {
            let response = BrokerResponse::new("identity-error", ResponseStatus::AccessDenied)
                .with_message(error);
            let _ = write_response(&mut stream, &response);
            return;
        }
    };
    let owner = identity.owner();
    let auth = authorize_client(&identity, &state.config.installing_user_sid);
    let _ = crate::write_json(
        &state
            .output_root
            .join(format!("CLIENT-AUTH-{}.json", identity.client_pid)),
        &json!({
            "schema": "amd-privilege-client-auth/v1",
            "request_id": null,
            "client_pid": identity.client_pid,
            "client_process_start_time": identity.client_process_start_time,
            "client_user_sid": identity.client_user_sid,
            "client_integrity_level": identity.client_integrity_level,
            "client_session_id": identity.client_session_id,
            "client_is_local": identity.client_is_local,
            "expected_user_sid": state.config.installing_user_sid,
            "pipe_reject_remote_clients": true,
            "identity_source": "NAMED_PIPE_CLIENT_IMPERSONATION_TOKEN",
            "client_pid_source": "GetNamedPipeClientProcessId",
            "client_process_start_time_source":
                "GetProcessTimes_under_impersonated_client_context",
            "client_user_sid_source": "TokenUser",
            "client_integrity_source": "TokenIntegrityLevel",
            "client_session_id_source": "TokenSessionId",
            "impersonation_reverted": true,
            "authorized": auth.is_ok()
        }),
    );
    write_first_frame_evidence(&state, &identity, &first_frame, auth.is_ok());
    if auth.is_err() {
        let _ = write_response(
            &mut stream,
            &BrokerResponse::new("authorization", ResponseStatus::AccessDenied)
                .with_message("connected client SID/locality is not authorized"),
        );
        return;
    }

    match first_frame {
        PipeFrameRead {
            payload: Some(payload),
            error: None,
            ..
        } => {
            if !process_request_frame(&mut stream, &state, &identity, &payload) {
                finish_authenticated_connection(&state, &identity, &owner);
                return;
            }
        }
        PipeFrameRead {
            payload: None,
            error: None,
            ..
        } => {
            finish_authenticated_connection(&state, &identity, &owner);
            return;
        }
        PipeFrameRead {
            payload: None,
            error: Some(error),
            ..
        } => {
            write_client_request_evidence(&state, &identity, None, None);
            let response = BrokerResponse::new("frame-error", ResponseStatus::InvalidRequest)
                .with_message(error);
            let _ = write_response(&mut stream, &response);
            finish_authenticated_connection(&state, &identity, &owner);
            return;
        }
        PipeFrameRead {
            payload: Some(_),
            error: Some(error),
            ..
        } => {
            write_client_request_evidence(&state, &identity, None, None);
            let response = BrokerResponse::new("frame-error", ResponseStatus::InvalidRequest)
                .with_message(error);
            let _ = write_response(&mut stream, &response);
            finish_authenticated_connection(&state, &identity, &owner);
            return;
        }
    }

    loop {
        let frame = read_pipe_frame(&mut stream);
        let payload = match frame {
            PipeFrameRead {
                payload: Some(payload),
                error: None,
                ..
            } => payload,
            PipeFrameRead {
                payload: None,
                error: None,
                ..
            } => break,
            PipeFrameRead {
                payload: None,
                error: Some(error),
                ..
            }
            | PipeFrameRead {
                payload: Some(_),
                error: Some(error),
                ..
            } => {
                let response = BrokerResponse::new("frame-error", ResponseStatus::InvalidRequest)
                    .with_message(error);
                let _ = write_response(&mut stream, &response);
                break;
            }
        };
        if !process_request_frame(&mut stream, &state, &identity, &payload) {
            break;
        }
    }
    finish_authenticated_connection(&state, &identity, &owner);
}

fn process_request_frame(
    stream: &mut NamedPipeStream,
    state: &BrokerState,
    identity: &ClientIdentity,
    payload: &[u8],
) -> bool {
    let request_id = request_id_from_json(payload);
    let decoded = decode_request(payload);
    write_client_request_evidence(
        state,
        identity,
        request_id.as_deref(),
        decoded.as_ref().ok(),
    );
    let response = match decoded {
        Ok(request) => dispatch_request(&request, identity, state),
        Err(error) => response_for_protocol_error(request_id, &error),
    };
    write_response(stream, &response).is_ok()
}

fn finish_authenticated_connection(
    state: &BrokerState,
    identity: &ClientIdentity,
    owner: &crate::SessionOwner,
) {
    if state.coordinator.disconnect(owner) {
        let _ = crate::write_json(
            &state
                .output_root
                .join(format!("CLIENT-DISCONNECT-{}.json", identity.client_pid)),
            &json!({
                "schema": "amd-privilege-client-disconnect/v1",
                "policy": CLIENT_DISCONNECT_POLICY,
                "client_pid": identity.client_pid,
                "client_process_start_time": identity.client_process_start_time,
                "session_cancellation_requested": true
            }),
        );
    }
}

fn write_client_request_evidence(
    state: &BrokerState,
    identity: &ClientIdentity,
    request_id: Option<&str>,
    request: Option<&SemanticRequest>,
) {
    let request_token = request_id
        .map(safe_request_file_component)
        .unwrap_or_else(|| "malformed".to_owned());
    let _ = crate::write_json(
        &state.output_root.join(format!(
            "CLIENT-REQUEST-{}-{request_token}.json",
            identity.client_pid
        )),
        &json!({
            "schema": "amd-privilege-client-request/v1",
            "request_id": request_id,
            "request_type": request.map(SemanticRequest::request_type),
            "protocol_version": request.map(SemanticRequest::protocol_version),
            "client_pid": identity.client_pid,
            "client_process_start_time": identity.client_process_start_time,
            "client_user_sid": identity.client_user_sid,
            "client_integrity_level": identity.client_integrity_level,
            "client_session_id": identity.client_session_id,
            "client_is_local": identity.client_is_local,
            "pipe_reject_remote_clients": true,
            "semantic_request_only": true
        }),
    );
}

fn safe_request_file_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .take(crate::MAX_REQUEST_ID_BYTES)
        .collect()
}

fn dispatch_request(
    request: &SemanticRequest,
    identity: &ClientIdentity,
    state: &BrokerState,
) -> BrokerResponse {
    match request {
        SemanticRequest::GetAmdProviderStatus { request_id, .. } => {
            let status = match discover_cli() {
                Ok((_, artifact)) => {
                    let _ = crate::write_json(
                        &state.output_root.join("CLI-ARTIFACT-IDENTITY.json"),
                        &artifact,
                    );
                    ProviderStatus {
                        available: true,
                        identity_valid: true,
                        reason: None,
                    }
                }
                Err(error) => ProviderStatus {
                    available: false,
                    identity_valid: false,
                    reason: Some(error),
                },
            };
            BrokerResponse::new(request_id, ResponseStatus::Ok).with_provider_status(status)
        }
        SemanticRequest::StartAmdPowerSession {
            request_id,
            duration_ms,
            interval_ms,
            ..
        } => start_session(request_id, *duration_ms, *interval_ms, identity, state),
        SemanticRequest::GetAmdSessionStatus {
            request_id,
            session_id,
            ..
        } => status_session(request_id, session_id, identity, state),
        SemanticRequest::CancelAmdSession {
            request_id,
            session_id,
            ..
        } => cancel_session(request_id, session_id, identity, state),
    }
}

fn start_session(
    request_id: &str,
    duration_ms: u32,
    interval_ms: u32,
    identity: &ClientIdentity,
    state: &BrokerState,
) -> BrokerResponse {
    let lease = match state.coordinator.start(identity.owner()) {
        Ok(lease) => lease,
        Err(error) => return response_for_session_error(request_id, &error),
    };
    let session_id = lease.snapshot.session_id.clone();
    let _ = crate::write_json(
        &state
            .output_root
            .join(format!("SESSION-OWNER-{session_id}.json")),
        &json!({
            "schema": "amd-privilege-session-owner/v1",
            "session_id": session_id,
            "owner_user_sid": identity.client_user_sid,
            "owner_client_pid": identity.client_pid,
            "owner_client_process_start_time": identity.client_process_start_time,
            "session_owner_match": true
        }),
    );
    let worker_state = state.clone();
    thread::spawn(move || run_real_session(worker_state, lease, duration_ms, interval_ms));
    BrokerResponse::new(request_id, ResponseStatus::Ok).with_session(
        state
            .coordinator
            .snapshot(&session_id)
            .unwrap_or_else(|_| unreachable!("new session must remain visible")),
    )
}

fn status_session(
    request_id: &str,
    session_id: &str,
    identity: &ClientIdentity,
    state: &BrokerState,
) -> BrokerResponse {
    let snapshot = match state.coordinator.snapshot(session_id) {
        Ok(snapshot) => snapshot,
        Err(error) => return response_for_session_error(request_id, &error),
    };
    if !owner_can_read(&snapshot, identity) {
        return BrokerResponse::new(request_id, ResponseStatus::AccessDenied)
            .with_message("session owner identity does not match");
    }
    BrokerResponse::new(request_id, ResponseStatus::Ok).with_session(snapshot)
}

fn cancel_session(
    request_id: &str,
    session_id: &str,
    identity: &ClientIdentity,
    state: &BrokerState,
) -> BrokerResponse {
    let snapshot = match state.coordinator.snapshot(session_id) {
        Ok(snapshot) => snapshot,
        Err(error) => return response_for_session_error(request_id, &error),
    };
    if !owner_exact(&snapshot, identity) {
        return BrokerResponse::new(request_id, ResponseStatus::AccessDenied)
            .with_message("only the exact authenticated owner may cancel");
    }
    match state
        .coordinator
        .request_cancel(session_id, &identity.client_user_sid)
    {
        Ok(snapshot) => BrokerResponse::new(request_id, ResponseStatus::Ok).with_session(snapshot),
        Err(error) => response_for_session_error(request_id, &error),
    }
}

fn owner_can_read(snapshot: &crate::SessionSnapshot, identity: &ClientIdentity) -> bool {
    snapshot
        .owner_user_sid
        .eq_ignore_ascii_case(&identity.client_user_sid)
}

fn owner_exact(snapshot: &crate::SessionSnapshot, identity: &ClientIdentity) -> bool {
    owner_can_read(snapshot, identity)
        && snapshot.owner_client_pid == identity.client_pid
        && snapshot.owner_client_process_start_time == identity.client_process_start_time
}

fn run_real_session(
    state: BrokerState,
    lease: crate::SessionLease,
    duration_ms: u32,
    interval_ms: u32,
) {
    let session_id = lease.snapshot.session_id.clone();
    let _ = state.coordinator.mark_running(&session_id);
    let (state_result, summary) = match execute_real_amd_session(
        &state,
        &session_id,
        &lease.cancellation,
        duration_ms,
        interval_ms,
    ) {
        Ok(summary) => (SessionState::Completed, summary),
        Err(RealSessionError::Cancelled(summary)) => (SessionState::Cancelled, summary),
        Err(RealSessionError::Failed(summary)) => (SessionState::Failed, summary),
    };
    let _ = crate::write_json(
        &state
            .output_root
            .join(format!("SESSION-RESULT-{session_id}.json")),
        &summary,
    );
    let _ = state.coordinator.finish(&session_id, state_result, summary);
}

enum RealSessionError {
    Cancelled(SessionResultSummary),
    Failed(SessionResultSummary),
}

fn empty_summary(classification: Option<&str>) -> SessionResultSummary {
    SessionResultSummary {
        amd_runtime_executed: false,
        cli_started_by_broker: false,
        cli_exit_code: None,
        package_power_sampling: "NOT_RUN".to_owned(),
        package_power_sample_count: 0,
        cadence_policy_result: "NOT_RUN".to_owned(),
        failure_classification: classification.map(ToOwned::to_owned),
        no_orphan_child: true,
    }
}

fn execute_real_amd_session(
    state: &BrokerState,
    session_id: &str,
    cancellation: &AtomicBool,
    duration_ms: u32,
    interval_ms: u32,
) -> Result<SessionResultSummary, RealSessionError> {
    let (cli_path, artifact) = match discover_cli() {
        Ok(value) => value,
        Err(error) => {
            let mut summary = empty_summary(Some("LOCAL_SERVICE_INSTALLATION_DISCOVERY_FAILED"));
            summary.failure_classification = Some(format!(
                "LOCAL_SERVICE_INSTALLATION_DISCOVERY_FAILED: {error}"
            ));
            return Err(RealSessionError::Failed(summary));
        }
    };
    let _ = crate::write_json(
        &state.output_root.join("CLI-ARTIFACT-IDENTITY.json"),
        &artifact,
    );
    let bin = cli_path.parent().unwrap_or_else(|| Path::new("."));
    let session_root = state.output_root.join("sessions").join(session_id);
    if let Err(error) = fs::create_dir_all(&session_root) {
        let mut summary = empty_summary(Some("OUTPUT_OR_COUNTER_FAILED"));
        summary.failure_classification = Some(format!("OUTPUT_OR_COUNTER_FAILED: {error}"));
        return Err(RealSessionError::Failed(summary));
    }
    let stdout_path = session_root.join("AMD-CLI.stdout.txt");
    let stderr_path = session_root.join("AMD-CLI.stderr.txt");
    let stdout = match File::create(&stdout_path) {
        Ok(file) => file,
        Err(error) => {
            let mut summary = empty_summary(Some("OUTPUT_OR_COUNTER_FAILED"));
            summary.failure_classification = Some(format!("OUTPUT_OR_COUNTER_FAILED: {error}"));
            return Err(RealSessionError::Failed(summary));
        }
    };
    let stderr = match File::create(&stderr_path) {
        Ok(file) => file,
        Err(error) => {
            let mut summary = empty_summary(Some("OUTPUT_OR_COUNTER_FAILED"));
            summary.failure_classification = Some(format!("OUTPUT_OR_COUNTER_FAILED: {error}"));
            return Err(RealSessionError::Failed(summary));
        }
    };
    let args = fixed_cli_arguments(duration_ms, interval_ms, &session_root);
    let mut command = Command::new(&cli_path);
    command
        .args(&args)
        .current_dir(bin)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .creation_flags(CREATE_NO_WINDOW);
    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let mut summary = empty_summary(Some("AMD_RUNTIME_FAILED"));
            summary.failure_classification = Some(format!("AMD_RUNTIME_FAILED: {error}"));
            return Err(RealSessionError::Failed(summary));
        }
    };
    let child_pid = child.id();
    let child_start = process_start_time(HANDLE(child.as_raw_handle())).unwrap_or(0);
    let mut owned = OwnedChild::new(child);
    if let Err(error) = owned.assign_job() {
        let no_orphan = owned.terminate_and_wait();
        let mut summary = empty_summary(Some("HARNESS_FAILED"));
        summary.amd_runtime_executed = true;
        summary.cli_started_by_broker = true;
        summary.no_orphan_child = no_orphan;
        summary.failure_classification = Some(format!("HARNESS_FAILED: {error}"));
        return Err(RealSessionError::Failed(summary));
    }
    let _ = crate::write_json(
        &state
            .output_root
            .join(format!("AMD-CLI-LAUNCH-{session_id}.json")),
        &json!({
            "schema": "amd-privilege-cli-launch/v1",
            "session_id": session_id,
            "amd_runtime_executed": true,
            "cli_started_by_broker": true,
            "target_pid": child_pid,
            "target_process_start_time": child_start,
            "executable_derived_by_broker": true,
            "event": FIXED_EVENT,
            "arguments": args,
            "working_directory": bin,
            "output_directory": session_root
        }),
    );
    let timeout = Duration::from_millis(u64::from(duration_ms) + crate::CLI_TIMEOUT_SAFETY_MS);
    let deadline = Instant::now() + timeout;
    loop {
        if cancellation.load(Ordering::Acquire) {
            let no_orphan = owned.terminate_and_wait();
            let mut summary = empty_summary(Some("CANCELLED"));
            summary.amd_runtime_executed = true;
            summary.cli_started_by_broker = true;
            summary.cli_exit_code = owned.exit_code();
            summary.no_orphan_child = no_orphan;
            return Err(RealSessionError::Cancelled(summary));
        }
        match owned.child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let no_orphan = owned.terminate_and_wait();
                let mut summary = empty_summary(Some("AMD_RUNTIME_FAILED"));
                summary.amd_runtime_executed = true;
                summary.cli_started_by_broker = true;
                summary.cli_exit_code = owned.exit_code();
                summary.no_orphan_child = no_orphan;
                summary.failure_classification =
                    Some("AMD_RUNTIME_FAILED: bounded timeout".to_owned());
                return Err(RealSessionError::Failed(summary));
            }
            Err(error) => {
                let no_orphan = owned.terminate_and_wait();
                let mut summary = empty_summary(Some("HARNESS_FAILED"));
                summary.amd_runtime_executed = true;
                summary.cli_started_by_broker = true;
                summary.no_orphan_child = no_orphan;
                summary.failure_classification =
                    Some(format!("HARNESS_FAILED: waiting for child failed: {error}"));
                return Err(RealSessionError::Failed(summary));
            }
        }
    }
    let exit_code = owned.exit_code();
    if exit_code != Some(0) {
        let mut summary = empty_summary(Some("AMD_RUNTIME_FAILED"));
        summary.amd_runtime_executed = true;
        summary.cli_started_by_broker = true;
        summary.cli_exit_code = exit_code;
        summary.no_orphan_child = owned.child.try_wait().ok().flatten().is_some();
        summary.failure_classification =
            Some(format!("AMD_RUNTIME_FAILED: exit code {exit_code:?}"));
        return Err(RealSessionError::Failed(summary));
    }
    let csv_path = match find_output_csv(&session_root) {
        Ok(path) => path,
        Err(error) => {
            let mut summary = empty_summary(Some("OUTPUT_OR_COUNTER_FAILED"));
            summary.amd_runtime_executed = true;
            summary.cli_started_by_broker = true;
            summary.cli_exit_code = exit_code;
            summary.no_orphan_child = true;
            summary.failure_classification = Some(format!("OUTPUT_OR_COUNTER_FAILED: {error}"));
            return Err(RealSessionError::Failed(summary));
        }
    };
    let csv = match fs::read_to_string(&csv_path) {
        Ok(csv) => csv,
        Err(error) => {
            let mut summary = empty_summary(Some("OUTPUT_OR_COUNTER_FAILED"));
            summary.amd_runtime_executed = true;
            summary.cli_started_by_broker = true;
            summary.cli_exit_code = exit_code;
            summary.failure_classification = Some(format!("OUTPUT_OR_COUNTER_FAILED: {error}"));
            return Err(RealSessionError::Failed(summary));
        }
    };
    let parsed = match parse_package_power_csv(&csv) {
        Ok(parsed) => parsed,
        Err(error) => {
            let mut summary = empty_summary(Some("OUTPUT_OR_COUNTER_FAILED"));
            summary.amd_runtime_executed = true;
            summary.cli_started_by_broker = true;
            summary.cli_exit_code = exit_code;
            summary.failure_classification = Some(format!("OUTPUT_OR_COUNTER_FAILED: {error}"));
            return Err(RealSessionError::Failed(summary));
        }
    };
    let cadence = assess_cadence(&parsed.samples, interval_ms);
    if cadence.status != "PASS" {
        let mut summary = empty_summary(Some("OUTPUT_OR_COUNTER_FAILED"));
        summary.amd_runtime_executed = true;
        summary.cli_started_by_broker = true;
        summary.cli_exit_code = exit_code;
        summary.package_power_sampling = "PASS".to_owned();
        summary.package_power_sample_count = parsed.samples.len();
        summary.cadence_policy_result = cadence.status.clone();
        summary.failure_classification =
            Some("OUTPUT_OR_COUNTER_FAILED: cadence policy failed".to_owned());
        return Err(RealSessionError::Failed(summary));
    }
    let summary = SessionResultSummary {
        amd_runtime_executed: true,
        cli_started_by_broker: true,
        cli_exit_code: exit_code,
        package_power_sampling: "PASS".to_owned(),
        package_power_sample_count: parsed.samples.len(),
        cadence_policy_result: cadence.status.clone(),
        failure_classification: None,
        no_orphan_child: true,
    };
    let _ = crate::write_json(
        &state
            .output_root
            .join(format!("PACKAGE-POWER-RESULT-{session_id}.json")),
        &json!({
            "schema": "amd-privilege-package-power-result/v1",
            "session_id": session_id,
            "package_power_sampling": summary.package_power_sampling,
            "sample_count": summary.package_power_sample_count,
            "cadence": cadence,
            "samples": parsed.samples
        }),
    );
    Ok(summary)
}

fn fixed_cli_arguments(duration_ms: u32, interval_ms: u32, output_dir: &Path) -> Vec<String> {
    vec![
        "timechart".to_owned(),
        "--event".to_owned(),
        FIXED_EVENT.to_owned(),
        "--interval".to_owned(),
        interval_ms.to_string(),
        "--duration".to_owned(),
        (duration_ms / 1_000).to_string(),
        "--format".to_owned(),
        "csv".to_owned(),
        "--output-dir".to_owned(),
        output_dir.to_string_lossy().into_owned(),
    ]
}

fn find_output_csv(root: &Path) -> Result<PathBuf, String> {
    let preferred = root.join("timechart.csv");
    if preferred.is_file() {
        return Ok(preferred);
    }
    let mut candidates = fs::read_dir(root)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
        })
        .collect::<Vec<_>>();
    if candidates.len() == 1 {
        Ok(candidates.remove(0))
    } else {
        Err(format!(
            "expected one broker-owned CSV, found {}",
            candidates.len()
        ))
    }
}

#[derive(Debug, Serialize)]
struct CliArtifactIdentity {
    schema: String,
    path: String,
    sha256: String,
    architecture: String,
    file_version: Option<String>,
    signature_validation: String,
    identity_valid: bool,
}

fn discover_cli() -> Result<(PathBuf, CliArtifactIdentity), String> {
    let root = read_installation_root()?;
    if !root.is_absolute() {
        return Err("AMD installation root is not absolute".to_owned());
    }
    let bin = root.join("bin");
    let cli_path = bin.join(AMD_CLI_NAME);
    let bytes =
        fs::read(&cli_path).map_err(|error| format!("reading AMDuProfCLI.exe failed: {error}"))?;
    let architecture = pe_architecture(&bytes)?;
    let hash = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    let file_version = file_version(&cli_path);
    let identity = CliArtifactIdentity {
        schema: "amd-privilege-cli-identity/v1".to_owned(),
        path: cli_path.to_string_lossy().into_owned(),
        sha256: hash,
        architecture: architecture.clone(),
        file_version,
        signature_validation: verify_authenticode(&cli_path),
        identity_valid: architecture == "x64",
    };
    if !identity.identity_valid {
        return Err("AMDuProfCLI.exe is not x64".to_owned());
    }
    if !identity.signature_validation.starts_with("VALID:") {
        return Err(identity.signature_validation.clone());
    }
    Ok((cli_path, identity))
}

fn verify_authenticode(path: &Path) -> String {
    let Some(path) = path.to_str() else {
        return "INVALID: AMD CLI path is not valid UTF-16".to_owned();
    };
    let path = wide_null(path);
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
            windows::Win32::Foundation::HWND(std::ptr::null_mut()),
            &mut action,
            &mut data as *mut WINTRUST_DATA as *mut std::ffi::c_void,
        )
    };
    if result == 0 {
        "VALID: Authenticode verified with cache-only revocation policy".to_owned()
    } else {
        format!("INVALID: Authenticode verification returned 0x{result:08X}")
    }
}

fn read_installation_root() -> Result<PathBuf, String> {
    let key = wide_null(AMD_INSTALL_REGISTRY_KEY);
    let value = wide_null(AMD_INSTALL_REGISTRY_VALUE);
    let mut buffer = vec![0_u16; 1024];
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
    if status.0 != 0 {
        return Err(format!(
            "RegGetValueW failed with Win32 status {}",
            status.0
        ));
    }
    let length = (bytes as usize / size_of::<u16>()).min(buffer.len());
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

fn pe_architecture(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 0x40 || &bytes[..2] != b"MZ" {
        return Err("artifact is not a PE image".to_owned());
    }
    let offset =
        u32::from_le_bytes(bytes[0x3c..0x40].try_into().expect("four-byte PE offset")) as usize;
    if offset.checked_add(6).is_none()
        || bytes.len() < offset + 6
        || &bytes[offset..offset + 4] != b"PE\0\0"
    {
        return Err("artifact has an invalid PE header".to_owned());
    }
    Ok(
        match u16::from_le_bytes(
            bytes[offset + 4..offset + 6]
                .try_into()
                .expect("two-byte machine"),
        ) {
            0x8664 => "x64".to_owned(),
            0x014c => "x86".to_owned(),
            0xAA64 => "ARM64".to_owned(),
            _ => "UNKNOWN".to_owned(),
        },
    )
}

fn file_version(path: &Path) -> Option<String> {
    let path = wide_null(path.to_str()?);
    let mut handle = 0_u32;
    let size =
        unsafe { GetFileVersionInfoSizeW(PCWSTR::from_raw(path.as_ptr()), Some(&mut handle)) };
    if size == 0 {
        return None;
    }
    let mut bytes = vec![0_u8; size as usize];
    unsafe {
        GetFileVersionInfoW(
            PCWSTR::from_raw(path.as_ptr()),
            0,
            size,
            bytes.as_mut_ptr() as *mut _,
        )
        .ok()?;
    }
    let sub_block = wide_null("\\");
    let mut value = std::ptr::null_mut();
    let mut length = 0_u32;
    let ok = unsafe {
        VerQueryValueW(
            bytes.as_ptr() as *const _,
            PCWSTR::from_raw(sub_block.as_ptr()),
            &mut value,
            &mut length,
        )
        .as_bool()
    };
    if !ok {
        return None;
    }
    if value.is_null() || length < size_of::<VS_FIXEDFILEINFO>() as u32 {
        return None;
    }
    let info = unsafe { *(value as *const VS_FIXEDFILEINFO) };
    (info.dwSignature == 0xFEEF04BD).then(|| {
        format!(
            "{}.{}.{}.{}",
            info.dwFileVersionMS >> 16,
            info.dwFileVersionMS & 0xFFFF,
            info.dwFileVersionLS >> 16,
            info.dwFileVersionLS & 0xFFFF
        )
    })
}

struct OwnedChild {
    child: Child,
    job: Option<OwnedJob>,
}

impl OwnedChild {
    fn new(child: Child) -> Self {
        Self { child, job: None }
    }

    fn assign_job(&mut self) -> Result<(), String> {
        let job = OwnedJob::new()?;
        job.assign(&self.child)?;
        self.job = Some(job);
        Ok(())
    }

    fn terminate_and_wait(&mut self) -> bool {
        if let Some(job) = self.job.as_ref() {
            let _ = unsafe { TerminateJobObject(job.handle, 1) };
        } else {
            let _ = self.child.kill();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(_) => return false,
            }
        }
        false
    }

    fn exit_code(&mut self) -> Option<i32> {
        self.child
            .try_wait()
            .ok()
            .flatten()
            .and_then(|status| status.code())
    }
}

struct OwnedJob {
    handle: HANDLE,
}

impl OwnedJob {
    fn new() -> Result<Self, String> {
        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }
            .map_err(|error| format!("CreateJobObjectW failed: {error}"))?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Err(error) = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                std::ptr::addr_of!(limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(format!("SetInformationJobObject failed: {error}"));
        }
        Ok(Self { handle })
    }

    fn assign(&self, child: &Child) -> Result<(), String> {
        unsafe { AssignProcessToJobObject(self.handle, HANDLE(child.as_raw_handle())) }
            .map_err(|error| format!("AssignProcessToJobObject failed: {error}"))
    }
}

impl Drop for OwnedJob {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.handle) };
    }
}

struct SecurityDescriptor {
    pointer: windows::Win32::Security::PSECURITY_DESCRIPTOR,
}

impl SecurityDescriptor {
    fn from_sddl(sddl: &str) -> Result<Self, String> {
        let sddl = wide_null(sddl);
        let mut pointer = windows::Win32::Security::PSECURITY_DESCRIPTOR(std::ptr::null_mut());
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR::from_raw(sddl.as_ptr()),
                SDDL_REVISION_1,
                std::ptr::addr_of_mut!(pointer),
                None,
            )
        }
        .map_err(|error| {
            format!("ConvertStringSecurityDescriptorToSecurityDescriptorW failed: {error}")
        })?;
        Ok(Self { pointer })
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        if !self.pointer.0.is_null() {
            unsafe {
                let _ = windows::Win32::Foundation::LocalFree(HLOCAL(self.pointer.0));
            }
        }
    }
}

struct PipeHandle {
    raw: HANDLE,
}

// The handle is transferred exactly once to the dedicated connection worker.  No handle value is
// shared concurrently; ownership and closure remain in PipeHandle::drop on that worker.
unsafe impl Send for PipeHandle {}

impl Drop for PipeHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = DisconnectNamedPipe(self.raw);
            let _ = CloseHandle(self.raw);
        }
    }
}

struct NamedPipeStream {
    pipe: PipeHandle,
}

impl NamedPipeStream {
    fn new(pipe: PipeHandle) -> Self {
        Self { pipe }
    }

    fn raw(&self) -> HANDLE {
        self.pipe.raw
    }
}

struct OwnedHandle {
    raw: HANDLE,
}

impl OwnedHandle {
    fn new(raw: HANDLE, label: &str) -> Result<Self, String> {
        if raw.is_invalid() {
            return Err(format!("{label} returned an invalid handle"));
        }
        Ok(Self { raw })
    }

    fn raw(&self) -> HANDLE {
        self.raw
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.raw);
        }
    }
}

struct StopEvent {
    handle: OwnedHandle,
}

impl StopEvent {
    fn new() -> Result<Self, String> {
        let handle = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
            .map_err(|error| format!("CreateEventW for broker stop failed: {error}"))?;
        Ok(Self {
            handle: OwnedHandle::new(handle, "CreateEventW")?,
        })
    }

    fn raw(&self) -> HANDLE {
        self.handle.raw()
    }
}

impl Drop for StopEvent {
    fn drop(&mut self) {
        clear_stop_event(self.raw());
    }
}

fn create_io_event() -> Result<OwnedHandle, String> {
    let event = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
        .map_err(|error| format!("CreateEventW for overlapped I/O failed: {error}"))?;
    OwnedHandle::new(event, "CreateEventW")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingAcceptTerminalState {
    Connected,
    Cancelled,
}

struct ArmedPipeAccept {
    pipe: Option<PipeHandle>,
    connect_event: Option<OwnedHandle>,
    // This Box is intentional: Windows may dereference the OVERLAPPED after ConnectNamedPipe
    // returns ERROR_IO_PENDING.  Its address remains stable until wait/cancel observes terminal
    // completion.  The Option allows Drop to leak the exact resources rather than release memory
    // while Windows could still reference it if an exceptional drain failure occurs.
    overlapped: Option<Box<OVERLAPPED>>,
    state: crate::FirstAcceptState,
    pending_io_consumed: bool,
}

impl ArmedPipeAccept {
    fn readiness_state(&self) -> crate::FirstAcceptState {
        self.state
    }

    fn overlapped_ptr(&self) -> *const OVERLAPPED {
        self.overlapped
            .as_deref()
            .map_or(std::ptr::null(), |overlapped| {
                overlapped as *const OVERLAPPED
            })
    }

    fn pipe_raw(&self) -> HANDLE {
        self.pipe
            .as_ref()
            .expect("armed pipe accept must own a pipe until consumed")
            .raw
    }

    fn connect_event_raw(&self) -> HANDLE {
        self.connect_event
            .as_ref()
            .expect("armed pipe accept must own a connect event until consumed")
            .raw()
    }

    fn is_io_pending(&self) -> bool {
        matches!(self.state, crate::FirstAcceptState::IoPending) && !self.pending_io_consumed
    }

    fn mark_normal_completion(&mut self) {
        self.state = crate::FirstAcceptState::Connected;
        self.pending_io_consumed = true;
    }

    fn mark_cancelled_completion(&mut self) {
        self.pending_io_consumed = true;
    }

    fn into_pipe(mut self) -> Result<PipeHandle, String> {
        if self.is_io_pending() {
            return Err(
                "cannot release a pending named-pipe accept before terminal completion".to_owned(),
            );
        }
        self.pending_io_consumed = true;
        self.pipe
            .take()
            .ok_or_else(|| "named-pipe accept handle was already consumed".to_owned())
    }

    fn cancel_and_drain(mut self) -> Result<PendingAcceptTerminalState, String> {
        self.cancel_and_drain_in_place()
    }

    fn cancel_and_drain_in_place(&mut self) -> Result<PendingAcceptTerminalState, String> {
        if self.pending_io_consumed {
            return Ok(
                if matches!(self.state, crate::FirstAcceptState::IoPending) {
                    PendingAcceptTerminalState::Cancelled
                } else {
                    PendingAcceptTerminalState::Connected
                },
            );
        }
        if !matches!(self.state, crate::FirstAcceptState::IoPending) {
            self.pending_io_consumed = true;
            return Ok(PendingAcceptTerminalState::Connected);
        }

        let Some(pipe) = self.pipe.as_ref() else {
            return Err("pending named-pipe accept lost its pipe handle".to_owned());
        };
        let overlapped = self.overlapped_ptr();
        if overlapped.is_null() {
            return Err("pending named-pipe accept lost its OVERLAPPED storage".to_owned());
        }

        let cancel_error = unsafe { CancelIoEx(pipe.raw, Some(overlapped)) }.err();
        let unexpected_cancel_error = cancel_error
            .as_ref()
            .filter(|error| !error_is_win32(error, ERROR_NOT_FOUND))
            .map(ToString::to_string);
        if let Some(error) = cancel_error
            .as_ref()
            .filter(|error| !error_is_win32(error, ERROR_NOT_FOUND))
        {
            record_windows_service_error("CancelIoEx for pending named-pipe accept", error);
        }

        let mut transferred = 0_u32;
        let completion =
            unsafe { GetOverlappedResult(pipe.raw, overlapped, &mut transferred, true) };
        match completion {
            Ok(()) => {
                self.mark_normal_completion();
                if let Some(error) = unexpected_cancel_error {
                    return Err(format!(
                        "CancelIoEx failed after named-pipe accept completed normally: {error}"
                    ));
                }
                Ok(PendingAcceptTerminalState::Connected)
            }
            Err(error) if error_is_win32(&error, ERROR_OPERATION_ABORTED) => {
                self.mark_cancelled_completion();
                if let Some(cancel_error) = unexpected_cancel_error {
                    return Err(format!(
                        "CancelIoEx failed although pending accept completed as aborted: {cancel_error}"
                    ));
                }
                Ok(PendingAcceptTerminalState::Cancelled)
            }
            Err(error) => {
                self.mark_cancelled_completion();
                record_windows_service_error(
                    "GetOverlappedResult while draining pending named-pipe accept",
                    &error,
                );
                let completion_error = format!(
                    "GetOverlappedResult while draining pending named-pipe accept failed: {error}"
                );
                if let Some(cancel_error) = unexpected_cancel_error {
                    Err(format!("{cancel_error}; {completion_error}"))
                } else {
                    Err(completion_error)
                }
            }
        }
    }
}

impl Drop for ArmedPipeAccept {
    fn drop(&mut self) {
        if !self.is_io_pending() {
            return;
        }

        // Normal paths call cancel_and_drain explicitly.  Drop is deliberately non-blocking: it
        // requests cancellation and leaks the exact resources if a future path forgets to drain,
        // rather than introducing an unbounded destructor wait or freeing memory that Windows may
        // still reference.  The leak is a last-resort safety net, not a completion claim.
        let cancel_error = match (self.pipe.as_ref(), self.overlapped.as_ref()) {
            (Some(pipe), Some(overlapped)) => unsafe {
                CancelIoEx(pipe.raw, Some(overlapped.as_ref() as *const OVERLAPPED)).err()
            },
            _ => None,
        };
        remember_service_error_details(json!({
            "pending_accept_drop_cancellation_requested": true,
            "pending_accept_drop_drained": false,
            "pending_accept_drop_cancel_error": cancel_error.as_ref().map(ToString::to_string),
        }));
        if let Some(pipe) = self.pipe.take() {
            std::mem::forget(pipe);
        }
        if let Some(event) = self.connect_event.take() {
            std::mem::forget(event);
        }
        if let Some(overlapped) = self.overlapped.take() {
            std::mem::forget(overlapped);
        }
    }
}

fn first_accept_state_name(state: crate::FirstAcceptState) -> &'static str {
    match state {
        crate::FirstAcceptState::Connected => "CONNECTED",
        crate::FirstAcceptState::PipeConnected => "PIPE_CONNECTED",
        crate::FirstAcceptState::IoPending => "IO_PENDING",
    }
}

fn arm_pipe_accept(pipe: PipeHandle) -> Result<ArmedPipeAccept, String> {
    let connect_event = create_io_event()?;
    let mut overlapped = Box::new(unsafe { std::mem::zeroed::<OVERLAPPED>() });
    overlapped.hEvent = connect_event.raw();
    let connect_result =
        unsafe { ConnectNamedPipe(pipe.raw, Some(std::ptr::addr_of_mut!(*overlapped))) };
    let state = match connect_result {
        Ok(()) => crate::FirstAcceptState::Connected,
        Err(error) if error_is_win32(&error, ERROR_PIPE_CONNECTED) => {
            crate::FirstAcceptState::PipeConnected
        }
        Err(error) if error_is_win32(&error, ERROR_IO_PENDING) => {
            crate::FirstAcceptState::IoPending
        }
        Err(error) => {
            record_windows_service_error("ConnectNamedPipe", &error);
            return Err(format!("ConnectNamedPipe failed: {error}"));
        }
    };
    Ok(ArmedPipeAccept {
        pipe: Some(pipe),
        connect_event: Some(connect_event),
        overlapped: Some(overlapped),
        state,
        pending_io_consumed: false,
    })
}

fn wait_for_pipe_accept(
    mut accept: ArmedPipeAccept,
    stop_event: HANDLE,
) -> Result<Option<PipeHandle>, String> {
    if !matches!(accept.state, crate::FirstAcceptState::IoPending) {
        if STOP_REQUESTED.load(Ordering::Acquire) {
            return match accept.cancel_and_drain() {
                Ok(_) => Ok(None),
                Err(error) => Err(error),
            };
        }
        return accept.into_pipe().map(Some);
    }

    let handles = [accept.connect_event_raw(), stop_event];
    let wait_result = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
    match wait_result.0 {
        value if value == WAIT_OBJECT_0.0 => {
            let mut transferred = 0_u32;
            match unsafe {
                GetOverlappedResult(
                    accept.pipe_raw(),
                    accept.overlapped_ptr(),
                    &mut transferred,
                    false,
                )
            } {
                Ok(()) => {
                    accept.mark_normal_completion();
                    if STOP_REQUESTED.load(Ordering::Acquire) {
                        Ok(None)
                    } else {
                        accept.into_pipe().map(Some)
                    }
                }
                Err(error)
                    if STOP_REQUESTED.load(Ordering::Acquire)
                        && error_is_win32(&error, ERROR_OPERATION_ABORTED) =>
                {
                    accept.mark_cancelled_completion();
                    Ok(None)
                }
                Err(error) => {
                    record_windows_service_error(
                        "GetOverlappedResult for named-pipe connect",
                        &error,
                    );
                    return_after_accept_drain(
                        accept,
                        format!("GetOverlappedResult for named-pipe connect failed: {error}"),
                    )
                }
            }
        }
        value if value == WAIT_OBJECT_0.0 + 1 => match accept.cancel_and_drain() {
            Ok(_) => Ok(None),
            Err(error) => Err(error),
        },
        value if value == WAIT_FAILED.0 => {
            let win32_error = unsafe { GetLastError().0 };
            record_win32_service_error("WaitForMultipleObjects", win32_error);
            return_after_accept_drain(
                accept,
                format!("WaitForMultipleObjects failed: {win32_error}"),
            )
        }
        value => return_after_accept_drain(
            accept,
            format!("unexpected WaitForMultipleObjects result: {value}"),
        ),
    }
}

struct PipeReadChunk {
    bytes_read: usize,
    more_data: bool,
}

fn overlapped_read_chunk(pipe: HANDLE, buffer: &mut [u8]) -> io::Result<PipeReadChunk> {
    if buffer.is_empty() {
        return Ok(PipeReadChunk {
            bytes_read: 0,
            more_data: false,
        });
    }
    let event = create_io_event().map_err(io_other)?;
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event.raw();
    let mut transferred = 0_u32;
    let result = unsafe {
        ReadFile(
            pipe,
            Some(buffer),
            None,
            Some(std::ptr::addr_of_mut!(overlapped)),
        )
    };
    let returned_more_data = match result {
        Ok(()) => false,
        Err(error) if error_is_win32(&error, ERROR_IO_PENDING) => false,
        Err(error) if error_is_win32(&error, ERROR_MORE_DATA) => true,
        Err(error) if error_is_win32(&error, ERROR_BROKEN_PIPE) => {
            return Ok(PipeReadChunk {
                bytes_read: 0,
                more_data: false,
            });
        }
        Err(error) => return Err(io_win32(error)),
    };

    match unsafe {
        GetOverlappedResult(
            pipe,
            std::ptr::addr_of!(overlapped),
            std::ptr::addr_of_mut!(transferred),
            true,
        )
    } {
        Ok(()) => Ok(PipeReadChunk {
            bytes_read: transferred as usize,
            more_data: returned_more_data,
        }),
        Err(error) if error_is_win32(&error, ERROR_MORE_DATA) => Ok(PipeReadChunk {
            bytes_read: transferred as usize,
            more_data: true,
        }),
        Err(error) if error_is_win32(&error, ERROR_BROKEN_PIPE) => Ok(PipeReadChunk {
            bytes_read: 0,
            more_data: false,
        }),
        Err(error) => Err(io_win32(error)),
    }
}

fn overlapped_write(pipe: HANDLE, buffer: &[u8]) -> io::Result<usize> {
    if buffer.is_empty() {
        return Ok(0);
    }
    let event = create_io_event().map_err(io_other)?;
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event.raw();
    let mut transferred = 0_u32;
    let result = unsafe {
        WriteFile(
            pipe,
            Some(buffer),
            None,
            Some(std::ptr::addr_of_mut!(overlapped)),
        )
    };
    match result {
        Ok(()) => {}
        Err(error) if error_is_win32(&error, ERROR_IO_PENDING) => {}
        Err(error) => return Err(io_win32(error)),
    }
    unsafe {
        GetOverlappedResult(
            pipe,
            std::ptr::addr_of!(overlapped),
            std::ptr::addr_of_mut!(transferred),
            true,
        )
        .map(|_| transferred as usize)
        .map_err(io_win32)
    }
}

fn io_other(error: String) -> io::Error {
    io::Error::other(error)
}

fn io_win32(error: windows::core::Error) -> io::Error {
    if let Some(win32_error) = win32_code_from_hresult(error.code()) {
        io::Error::from_raw_os_error(win32_error as i32)
    } else {
        io::Error::other(format!("Windows HRESULT {}: {error}", error.code()))
    }
}

fn create_pipe(config: &BrokerConfig) -> Result<PipeHandle, String> {
    let service_sid = config
        .service_sid
        .as_deref()
        .ok_or_else(|| "service SID is missing".to_owned())?;
    let dacl = build_pipe_dacl(&config.pipe_name, &config.installing_user_sid, service_sid)?;
    let descriptor = SecurityDescriptor::from_sddl(&dacl.sddl)?;
    let attributes = windows::Win32::Security::SECURITY_ATTRIBUTES {
        nLength: size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.pointer.0,
        bInheritHandle: false.into(),
    };
    let name = wide_null(&config.pipe_name);
    let mode = PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR::from_raw(name.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
            mode,
            PIPE_INSTANCE_COUNT,
            PIPE_BUFFER_BYTES,
            PIPE_BUFFER_BYTES,
            1_000,
            Some(&attributes),
        )
    };
    if handle.is_invalid() {
        let win32_error = unsafe { GetLastError().0 };
        record_win32_service_error("CreateNamedPipeW", win32_error);
        return Err(format!("CreateNamedPipeW failed: {win32_error}"));
    }
    Ok(PipeHandle { raw: handle })
}

fn synchronous_read_pipe_chunk(pipe: HANDLE, buffer: &mut [u8]) -> io::Result<PipeReadChunk> {
    if buffer.is_empty() {
        return Ok(PipeReadChunk {
            bytes_read: 0,
            more_data: false,
        });
    }
    let mut transferred = 0_u32;
    let result = unsafe {
        ReadFile(
            pipe,
            Some(buffer),
            Some(std::ptr::addr_of_mut!(transferred)),
            None,
        )
    };
    match result {
        Ok(()) => Ok(PipeReadChunk {
            bytes_read: transferred as usize,
            more_data: false,
        }),
        Err(error) if error_is_win32(&error, ERROR_MORE_DATA) => Ok(PipeReadChunk {
            bytes_read: transferred as usize,
            more_data: true,
        }),
        Err(error) if error_is_win32(&error, ERROR_BROKEN_PIPE) => Ok(PipeReadChunk {
            bytes_read: 0,
            more_data: false,
        }),
        Err(error) => Err(io_win32(error)),
    }
}

fn read_client_pipe_frame(reader: &mut File) -> Result<Option<Vec<u8>>, String> {
    let mut prefix = [0_u8; 4];
    let prefix_chunk = synchronous_read_pipe_chunk(HANDLE(reader.as_raw_handle()), &mut prefix)
        .map_err(|error| format!("pipe frame prefix read failed: {error}"))?;
    if prefix_chunk.bytes_read == 0 {
        return Ok(None);
    }
    if prefix_chunk.bytes_read != prefix.len() {
        return Err(format!(
            "pipe frame prefix was truncated: read {} of {} bytes",
            prefix_chunk.bytes_read,
            prefix.len()
        ));
    }
    let length = u32::from_le_bytes(prefix) as usize;
    if length > crate::MAX_FRAME_BYTES {
        return Err("pipe frame exceeds maximum request size".to_owned());
    }
    if length == 0 {
        if prefix_chunk.more_data {
            return Err("zero-length response has trailing named-pipe message data".to_owned());
        }
        return Ok(Some(Vec::new()));
    }
    if !prefix_chunk.more_data {
        return Err("pipe frame prefix message ended before the declared payload".to_owned());
    }
    let mut payload = vec![0_u8; length];
    let payload_chunk = synchronous_read_pipe_chunk(HANDLE(reader.as_raw_handle()), &mut payload)
        .map_err(|error| format!("pipe frame payload read failed: {error}"))?;
    if payload_chunk.bytes_read != length {
        return Err(format!(
            "pipe frame payload was truncated: read {} of {} bytes",
            payload_chunk.bytes_read, length
        ));
    }
    if payload_chunk.more_data {
        return Err("pipe frame message contains trailing data".to_owned());
    }
    Ok(Some(payload))
}

fn write_one_message(writer: &mut impl Write, frame: &[u8]) -> io::Result<()> {
    if frame.len() > std::mem::size_of::<u32>() + crate::MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "pipe message exceeds the bounded frame size",
        ));
    }
    let written = writer.write(frame)?;
    if written != frame.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            format!(
                "single pipe-message write was short: wrote {written} of {} bytes",
                frame.len()
            ),
        ));
    }
    writer.flush()
}

fn write_pipe_message(stream: &mut NamedPipeStream, frame: &[u8]) -> Result<(), String> {
    if frame.len() > std::mem::size_of::<u32>() + crate::MAX_FRAME_BYTES {
        return Err("pipe message exceeds the bounded frame size".to_owned());
    }
    let written = overlapped_write(stream.raw(), frame).map_err(|error| error.to_string())?;
    if written != frame.len() {
        return Err(format!(
            "single pipe-message write was short: wrote {written} of {} bytes",
            frame.len()
        ));
    }
    Ok(())
}

fn write_response(writer: &mut NamedPipeStream, response: &BrokerResponse) -> Result<(), String> {
    let value = serde_json::to_value(response).map_err(|error| error.to_string())?;
    let frame = encode_json_frame(&value).map_err(|error| error.to_string())?;
    write_pipe_message(writer, &frame)
}

pub fn run_client(options: ClientOptions) -> Result<(), String> {
    let config = load_config()?;
    let mut stream = open_client_pipe(&config.pipe_name)?;
    let status_request = SemanticRequest::GetAmdProviderStatus {
        protocol_version: crate::PROTOCOL_VERSION,
        request_id: next_request_id("status"),
    };
    let status = send_request(&mut stream, &status_request)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&status).map_err(|error| error.to_string())?
    );
    if options.operation == "get-status" {
        return Ok(());
    }
    let start_request = SemanticRequest::StartAmdPowerSession {
        protocol_version: crate::PROTOCOL_VERSION,
        request_id: next_request_id("start"),
        duration_ms: options.duration_ms,
        interval_ms: options.interval_ms,
    };
    let start = send_request(&mut stream, &start_request)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&start).map_err(|error| error.to_string())?
    );
    if start.status != ResponseStatus::Ok {
        return Err(format!("start request returned {:?}", start.status));
    }
    let session_id = start
        .session
        .as_ref()
        .map(|session| session.session_id.clone())
        .ok_or_else(|| "start response did not contain a session id".to_owned())?;
    let deadline = Instant::now() + Duration::from_millis(u64::from(options.duration_ms) + 120_000);
    loop {
        if Instant::now() >= deadline {
            return Err("bounded client wait expired".to_owned());
        }
        thread::sleep(Duration::from_millis(250));
        let request = SemanticRequest::GetAmdSessionStatus {
            protocol_version: crate::PROTOCOL_VERSION,
            request_id: next_request_id("session"),
            session_id: session_id.clone(),
        };
        let response = send_request(&mut stream, &request)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&response).map_err(|error| error.to_string())?
        );
        if response.session.as_ref().is_some_and(|session| {
            matches!(
                session.state,
                SessionState::Completed | SessionState::Failed | SessionState::Cancelled
            )
        }) {
            return Ok(());
        }
    }
}

fn open_client_pipe(pipe_name: &str) -> Result<File, String> {
    let name = wide_null(pipe_name);
    let desired_access = FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0;
    let share_mode = FILE_SHARE_READ | FILE_SHARE_WRITE;
    let flags = SECURITY_SQOS_PRESENT | SECURITY_IMPERSONATION;
    let handle = unsafe {
        CreateFileW(
            PCWSTR::from_raw(name.as_ptr()),
            desired_access,
            share_mode,
            None,
            OPEN_EXISTING,
            flags,
            None,
        )
    }
    .map_err(|error| format!("opening qualification pipe failed: {error}"))?;
    Ok(unsafe { File::from_raw_handle(handle.0) })
}

fn send_request(stream: &mut File, request: &SemanticRequest) -> Result<BrokerResponse, String> {
    let frame = encode_json_frame(&request.to_value()).map_err(|error| error.to_string())?;
    write_one_message(stream, &frame).map_err(|error| error.to_string())?;
    let response =
        read_client_pipe_frame(stream)?.ok_or_else(|| "broker closed the pipe".to_owned())?;
    serde_json::from_slice(&response)
        .map_err(|error| format!("broker response is invalid: {error}"))
}

fn collect_service_context(config: &BrokerConfig) -> Result<ServiceContextEvidence, String> {
    let current = unsafe { GetCurrentProcess() };
    let token = open_process_token(current)?;
    let user_bytes = token_information(token.raw(), TokenUser)?;
    let user = unsafe { &*(user_bytes.as_ptr() as *const TOKEN_USER) };
    let account_sid = sid_to_string(user.User.Sid)?;
    let service_sid = config.service_sid.clone();
    let service_sid_present = service_sid
        .as_deref()
        .is_some_and(|sid| token_contains_sid(token.raw(), TokenGroups, sid).unwrap_or(false));
    let service_sid_type = query_service_sid_type().unwrap_or_else(|_| "UNKNOWN".to_owned());
    let integrity_sid = token_information(token.raw(), TokenIntegrityLevel)
        .ok()
        .and_then(|bytes| {
            let label = unsafe { &*(bytes.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
            sid_to_string(label.Label.Sid).ok()
        });
    let token_elevated = token_information(token.raw(), TokenElevation)
        .ok()
        .map(|bytes| unsafe { (*(bytes.as_ptr() as *const TOKEN_ELEVATION)).TokenIsElevated != 0 });
    let session_id = process_session_id(unsafe { GetCurrentProcessId() });
    let process_architecture = if size_of::<usize>() == 8 {
        "x64"
    } else {
        "non-x64"
    };
    let context_valid = account_sid.eq_ignore_ascii_case(SERVICE_ACCOUNT_SID)
        && service_sid_present
        && session_id == Some(0)
        && process_architecture == "x64"
        && service_sid_type == REQUIRED_SERVICE_SID_TYPE;
    Ok(ServiceContextEvidence {
        schema: "amd-privilege-service-context/v1".to_owned(),
        qualification_only: QUALIFICATION_ONLY,
        service_name: SERVICE_NAME.to_owned(),
        service_account: crate::SERVICE_ACCOUNT.to_owned(),
        account_sid,
        service_sid,
        service_sid_present,
        service_sid_type,
        process_id: unsafe { GetCurrentProcessId() },
        session_id,
        process_architecture: process_architecture.to_owned(),
        integrity_sid,
        token_elevated,
        context_valid,
    })
}

fn capture_client_identity(pipe: HANDLE) -> Result<ClientIdentity, String> {
    let mut pid = 0_u32;
    unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) }
        .map_err(|error| format!("GetNamedPipeClientProcessId failed: {error}"))?;

    // The broker is intentionally LocalService and must not inspect a standard-user process
    // using its own primary token.  The authenticated pipe client supplies the security context
    // for identity inspection, while the kernel-reported pipe PID remains the binding anchor.
    let mut impersonation = ImpersonationGuard::new(pipe)?;
    let token = open_thread_token()?;
    let user_bytes = token_information(token.raw(), TokenUser)?;
    let user = unsafe { &*(user_bytes.as_ptr() as *const TOKEN_USER) };
    let client_user_sid = sid_to_string(user.User.Sid)?;
    let integrity_bytes = token_information(token.raw(), TokenIntegrityLevel)?;
    let label = unsafe { &*(integrity_bytes.as_ptr() as *const TOKEN_MANDATORY_LABEL) };
    let integrity = sid_to_string(label.Label.Sid)?;
    let session_bytes = token_information(token.raw(), TokenSessionId)?;
    if session_bytes.len() < size_of::<u32>() {
        return Err("TokenSessionId returned an undersized buffer".to_owned());
    }
    let client_session_id = Some(u32::from_ne_bytes(
        session_bytes[..size_of::<u32>()]
            .try_into()
            .map_err(|_| "TokenSessionId conversion failed".to_owned())?,
    ));

    // OpenProcess/GetProcessTimes is deliberately performed while impersonating the client.  It
    // preserves the exact PID + process-start-time binding without granting LocalService extra
    // privileges such as SeDebugPrivilege.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }
        .map_err(|error| format!("OpenProcess for client under impersonation failed: {error}"))?;
    let process = OwnedHandle::new(process, "OpenProcess for client")?;
    let start = process_start_time(process.raw())
        .ok_or_else(|| "GetProcessTimes for client under impersonation failed".to_owned())?;
    impersonation.revert()?;

    // The pipe is created with PIPE_REJECT_REMOTE_CLIENTS. A client PID returned by
    // GetNamedPipeClientProcessId on this endpoint is therefore a local client.
    Ok(ClientIdentity {
        client_pid: pid,
        client_process_start_time: start,
        client_user_sid,
        client_integrity_level: Some(integrity),
        client_session_id,
        client_is_local: true,
    })
}

struct ImpersonationGuard {
    active: bool,
}

impl ImpersonationGuard {
    fn new(pipe: HANDLE) -> Result<Self, String> {
        unsafe { ImpersonateNamedPipeClient(pipe) }
            .map_err(|error| format!("ImpersonateNamedPipeClient failed: {error}"))?;
        Ok(Self { active: true })
    }

    fn revert(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        unsafe { RevertToSelf() }.map_err(|error| {
            format!("RevertToSelf failed after client identity capture: {error}")
        })?;
        self.active = false;
        Ok(())
    }
}

impl Drop for ImpersonationGuard {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                let _ = RevertToSelf();
            }
            self.active = false;
        }
    }
}

fn open_thread_token() -> Result<OwnedHandle, String> {
    let mut token = HANDLE::default();
    unsafe {
        OpenThreadToken(
            GetCurrentThread(),
            TOKEN_QUERY,
            false,
            std::ptr::addr_of_mut!(token),
        )
    }
    .map_err(|error| format!("OpenThreadToken for pipe client failed: {error}"))?;
    OwnedHandle::new(token, "OpenThreadToken")
}

fn open_process_token(process: HANDLE) -> Result<OwnedHandle, String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
        .map_err(|error| format!("OpenProcessToken failed: {error}"))?;
    OwnedHandle::new(token, "OpenProcessToken")
}

fn token_information(token: HANDLE, class: TOKEN_INFORMATION_CLASS) -> Result<Vec<u8>, String> {
    let mut required = 0_u32;
    let _ = unsafe { GetTokenInformation(token, class, None, 0, &mut required) };
    if required == 0 {
        return Err("GetTokenInformation did not return a buffer size".to_owned());
    }
    let mut bytes = vec![0_u8; required as usize];
    unsafe {
        GetTokenInformation(
            token,
            class,
            Some(bytes.as_mut_ptr() as *mut _),
            required,
            &mut required,
        )
    }
    .map_err(|error| format!("GetTokenInformation failed: {error}"))?;
    Ok(bytes)
}

fn token_contains_sid(
    token: HANDLE,
    class: TOKEN_INFORMATION_CLASS,
    expected_sid: &str,
) -> Result<bool, String> {
    let bytes = token_information(token, class)?;
    if class != TokenGroups {
        return Ok(false);
    }
    let groups = unsafe { &*(bytes.as_ptr() as *const TOKEN_GROUPS) };
    let base = groups.Groups.as_ptr();
    for index in 0..groups.GroupCount as usize {
        let group = unsafe { *base.add(index) };
        if sid_to_string(group.Sid)?.eq_ignore_ascii_case(expected_sid)
            && group.Attributes & SE_GROUP_ENABLED != 0
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn query_service_sid_type() -> Result<String, String> {
    let manager = unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT) }
        .map_err(|error| format!("OpenSCManagerW failed: {error}"))?;
    let service_name = wide_null(SERVICE_NAME);
    let service = unsafe {
        OpenServiceW(
            manager,
            PCWSTR::from_raw(service_name.as_ptr()),
            SERVICE_QUERY_CONFIG,
        )
    }
    .map_err(|error| {
        unsafe {
            let _ = CloseServiceHandle(manager);
        }
        format!("OpenServiceW failed: {error}")
    })?;
    let mut bytes = vec![0_u8; size_of::<windows::Win32::System::Services::SERVICE_SID_INFO>()];
    let mut required = 0_u32;
    let result = unsafe {
        QueryServiceConfig2W(
            service,
            SERVICE_CONFIG_SERVICE_SID_INFO,
            Some(&mut bytes),
            &mut required,
        )
    };
    let value = match result {
        Ok(()) => {
            let info = unsafe {
                *(bytes.as_ptr() as *const windows::Win32::System::Services::SERVICE_SID_INFO)
            };
            if info.dwServiceSidType
                == windows::Win32::System::Services::SERVICE_SID_TYPE_UNRESTRICTED
            {
                REQUIRED_SERVICE_SID_TYPE.to_owned()
            } else {
                format!("SERVICE_SID_TYPE_{}", info.dwServiceSidType)
            }
        }
        Err(error) => format!("UNKNOWN: {error}"),
    };
    unsafe {
        let _ = CloseServiceHandle(service);
        let _ = CloseServiceHandle(manager);
    }
    Ok(value)
}

fn sid_to_string(sid: windows::Win32::Security::PSID) -> Result<String, String> {
    let mut text = PWSTR::null();
    unsafe { windows::Win32::Security::Authorization::ConvertSidToStringSidW(sid, &mut text) }
        .map_err(|error| format!("ConvertSidToStringSidW failed: {error}"))?;
    let result = unsafe { text.to_string() }.map_err(|error| error.to_string());
    unsafe {
        let _ = windows::Win32::Foundation::LocalFree(HLOCAL(text.0 as *mut _));
    }
    result
}

fn process_session_id(pid: u32) -> Option<u32> {
    let mut session = 0_u32;
    unsafe { ProcessIdToSessionId(pid, &mut session) }
        .ok()
        .map(|_| session)
}

fn process_start_time(process: HANDLE) -> Option<u64> {
    let mut creation = windows::Win32::Foundation::FILETIME::default();
    let mut exit = windows::Win32::Foundation::FILETIME::default();
    let mut kernel = windows::Win32::Foundation::FILETIME::default();
    let mut user = windows::Win32::Foundation::FILETIME::default();
    unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) }
        .ok()
        .map(|_| (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
}

fn load_config() -> Result<BrokerConfig, String> {
    let path = config_path();
    let bytes =
        fs::read(&path).map_err(|error| format!("reading broker config failed: {error}"))?;
    let config: BrokerConfig = serde_json::from_slice(&bytes)
        .map_err(|error| format!("broker config is invalid: {error}"))?;
    let expected_output_root = program_data_root()
        .join(OUTPUT_SUBDIRECTORY)
        .join(&config.scope);
    let output_root_matches = Path::new(&config.output_root)
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected_output_root.to_string_lossy());
    if config.service_name != SERVICE_NAME
        || config.service_account_sid != SERVICE_ACCOUNT_SID
        || config.service_account != crate::SERVICE_ACCOUNT
        || config.pipe_name != crate::pipe_name_for_scope(&config.scope)?
        || !output_root_matches
    {
        return Err("broker config is outside the fixed qualification contract".to_owned());
    }
    Ok(config)
}

pub fn config_path() -> PathBuf {
    program_data_root()
        .join(OUTPUT_SUBDIRECTORY)
        .join("BROKER-CONFIG.json")
}

pub fn program_data_root() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
}

fn next_request_id(kind: &str) -> String {
    format!("client-{kind}-{}", crate::unix_time_millis())
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_cli_arguments_have_no_client_command_surface() {
        let args = fixed_cli_arguments(
            5_000,
            1_000,
            Path::new(r"C:\ProgramData\ResourceTimeline\qualification\amd-privilege\sessions\one"),
        );
        assert_eq!(args[0], "timechart");
        assert_eq!(args[2], FIXED_EVENT);
        assert_eq!(args[4], "1000");
        assert_eq!(args[6], "5");
        assert!(!args.iter().any(|arg| arg == "--help" || arg == "--version"));
    }

    #[test]
    fn service_name_and_scope_are_fixed() {
        assert_eq!(SERVICE_NAME, "ResourceTimelineAmdPrivilegeQualification");
        assert!(crate::pipe_name_for_scope("0123456789abcdef0123456789abcdef").is_ok());
        assert!(crate::pipe_name_for_scope("bad").is_err());
    }

    #[test]
    fn hresult_from_win32_states_are_compared_in_the_same_domain() {
        for (win32, expected_hresult) in [
            (ERROR_IO_PENDING, 0x8007_03E5_u32),
            (ERROR_PIPE_CONNECTED, 0x8007_0217_u32),
            (ERROR_OPERATION_ABORTED, 0x8007_03E3_u32),
            (ERROR_MORE_DATA, 0x8007_00EA_u32),
            (ERROR_BROKEN_PIPE, 0x8007_006D_u32),
        ] {
            let error = windows::core::Error::from_hresult(HRESULT(expected_hresult as i32));
            assert!(error_is_win32(&error, win32));
            assert_eq!(win32_code_from_hresult(error.code()), Some(win32.0));
        }
    }

    #[test]
    fn raw_win32_value_is_not_directly_treated_as_hresult() {
        let error = windows::core::Error::from_hresult(HRESULT(997));
        assert!(!error_is_win32(&error, ERROR_IO_PENDING));
        assert_eq!(win32_code_from_hresult(error.code()), None);
    }

    #[test]
    fn non_win32_hresult_is_not_decoded_as_a_win32_error() {
        let error = windows::core::Error::from_hresult(HRESULT(0x8000_4005_u32 as i32));
        assert_eq!(win32_code_from_hresult(error.code()), None);
        assert_eq!(io_win32(error).raw_os_error(), None);
    }

    #[test]
    fn pending_accept_overlapped_storage_has_stable_address() {
        let overlapped = Box::new(unsafe { std::mem::zeroed::<OVERLAPPED>() });
        let address = overlapped.as_ref() as *const OVERLAPPED;
        let moved = overlapped;
        assert_eq!(address, moved.as_ref() as *const OVERLAPPED);
    }
}
