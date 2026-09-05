//! Qualification-only policy, protocol, and session ownership primitives.
//!
//! This crate is intentionally separate from the Resource Timeline production collector.  The
//! Windows binary can act as a future LocalService broker, but automated tests use only the
//! semantic protocol and harmless synthetic runners.  No test path discovers or launches AMD
//! uProf, registers a service, requests elevation, or writes production data.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub mod package_power;
pub mod synthetic;

#[cfg(windows)]
pub mod windows;

pub const QUALIFICATION_ONLY: bool = true;
pub const PROTOCOL_SCHEMA: &str = "amd-privilege-qualification/1";
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 8 * 1024;
pub const MAX_REQUEST_ID_BYTES: usize = 128;
pub const SERVICE_NAME: &str = "ResourceTimelineAmdPrivilegeQualification";
pub const SERVICE_SID_ACCOUNT: &str = "NT SERVICE\\ResourceTimelineAmdPrivilegeQualification";
pub const SERVICE_ACCOUNT: &str = "NT AUTHORITY\\LOCAL SERVICE";
pub const SERVICE_ACCOUNT_SID: &str = "S-1-5-19";
pub const SYSTEM_SID: &str = "S-1-5-18";
pub const PIPE_PREFIX: &str = r"\\.\pipe\ResourceTimeline-AmdPrivilegeQualification-";
pub const OUTPUT_SUBDIRECTORY: &str = "ResourceTimeline\\qualification\\amd-privilege";
pub const FIXED_EVENT: &str = "power";
pub const MIN_DURATION_MS: u32 = 5_000;
pub const MAX_DURATION_MS: u32 = 60_000;
pub const MIN_INTERVAL_MS: u32 = 1_000;
pub const MAX_INTERVAL_MS: u32 = 10_000;
pub const CLI_TIMEOUT_SAFETY_MS: u64 = 90_000;
pub const CLIENT_DISCONNECT_POLICY: &str = "CANCEL_OWNED_SESSION";

/// The only ConnectNamedPipe outcomes that establish a valid first accept contract.
///
/// `IoPending` is not an error for an overlapped accept: it means the operation is
/// armed and its OVERLAPPED storage must remain alive until completion or cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstAcceptState {
    Connected,
    PipeConnected,
    IoPending,
}

/// Lifecycle states used by the pending-accept ownership contract.
///
/// An `IoPending` accept owns kernel-visible OVERLAPPED storage.  A cancellation request is
/// only an intent to cancel; the resources become releasable after a terminal completion has
/// been observed.  The Windows implementation enforces this contract with
/// `GetOverlappedResult(..., true)` before releasing the owned pipe, event, or OVERLAPPED.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAcceptLifecycleState {
    Created,
    IoPending,
    CancelRequested,
    TerminalCompletionObserved,
    Connected,
    Failed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingAcceptCompletion {
    NotObserved,
    Normal,
    OperationAborted,
    Failed,
}

/// Models the only safe outcomes after a cancellation request.  In particular, a cancellation
/// request without a completion observation remains `CancelRequested` and is not releasable.
pub const fn pending_accept_state_after_cancel(
    completion: PendingAcceptCompletion,
) -> PendingAcceptLifecycleState {
    match completion {
        PendingAcceptCompletion::NotObserved => PendingAcceptLifecycleState::CancelRequested,
        PendingAcceptCompletion::Normal => PendingAcceptLifecycleState::Connected,
        PendingAcceptCompletion::OperationAborted => {
            PendingAcceptLifecycleState::TerminalCompletionObserved
        }
        PendingAcceptCompletion::Failed => PendingAcceptLifecycleState::Failed,
    }
}

/// Returns whether it is safe to release an accept's kernel-visible resources.
pub const fn pending_accept_release_is_safe(state: PendingAcceptLifecycleState) -> bool {
    matches!(
        state,
        PendingAcceptLifecycleState::TerminalCompletionObserved
            | PendingAcceptLifecycleState::Connected
            | PendingAcceptLifecycleState::Failed
    )
}

/// Returns whether cancellation has been followed by an authoritative terminal observation.
pub const fn pending_accept_cancel_is_drained(state: PendingAcceptLifecycleState) -> bool {
    matches!(
        state,
        PendingAcceptLifecycleState::TerminalCompletionObserved
            | PendingAcceptLifecycleState::Connected
            | PendingAcceptLifecycleState::Failed
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrokerReadinessState {
    listener_created: bool,
    first_accept_armed: bool,
    first_accept_state: Option<FirstAcceptState>,
    ready_published: bool,
    service_running: bool,
}

impl BrokerReadinessState {
    pub const fn new() -> Self {
        Self {
            listener_created: false,
            first_accept_armed: false,
            first_accept_state: None,
            ready_published: false,
            service_running: false,
        }
    }

    pub fn mark_first_listener_created(&mut self) {
        self.listener_created = true;
    }

    pub fn mark_first_accept_armed(&mut self, state: FirstAcceptState) -> Result<(), &'static str> {
        if !self.listener_created {
            return Err("cannot arm the first accept before a live listener exists");
        }
        if self.first_accept_armed {
            return Err("the first accept was already armed");
        }
        self.first_accept_armed = true;
        self.first_accept_state = Some(state);
        Ok(())
    }

    pub fn publish_ready(&mut self) -> Result<(), &'static str> {
        if !self.first_accept_armed {
            return Err("cannot publish broker ready before the first accept is armed");
        }
        self.ready_published = true;
        Ok(())
    }

    pub fn report_running(&mut self) -> Result<(), &'static str> {
        if !self.first_accept_armed {
            return Err("cannot report service running before the first accept is armed");
        }
        if !self.ready_published {
            return Err("cannot report service running before broker ready");
        }
        self.service_running = true;
        Ok(())
    }

    pub const fn listener_created(&self) -> bool {
        self.listener_created
    }

    pub const fn first_accept_armed(&self) -> bool {
        self.first_accept_armed
    }

    pub const fn first_accept_state(&self) -> Option<FirstAcceptState> {
        self.first_accept_state
    }

    pub const fn ready_published(&self) -> bool {
        self.ready_published
    }

    pub const fn service_running(&self) -> bool {
        self.service_running
    }
}

impl Default for BrokerReadinessState {
    fn default() -> Self {
        Self::new()
    }
}

/// A narrowly checked HRESULT_FROM_WIN32 decoder used by the Windows implementation and
/// platform-independent synthetic tests.  Only the canonical failure form is decoded; arbitrary
/// HRESULTs are deliberately not truncated into an invented Win32 error.
pub const fn win32_code_from_hresult_contract(hresult: u32) -> Option<u32> {
    let win32_error = hresult & 0x0000_FFFF;
    if hresult & 0xFFFF_0000 == 0x8007_0000 && win32_error != 0 {
        Some(win32_error)
    } else {
        None
    }
}

pub const fn hresult_from_win32_contract(win32_error: u32) -> Option<u32> {
    if win32_error == 0 || win32_error > 0x0000_FFFF {
        None
    } else {
        Some(0x8007_0000 | win32_error)
    }
}

pub const fn hresult_matches_win32_contract(hresult: u32, win32_error: u32) -> bool {
    match hresult_from_win32_contract(win32_error) {
        Some(expected) => hresult == expected,
        None => false,
    }
}

/// Returns whether a buffered first request may be dispatched after the broker has
/// authenticated the connected named-pipe client.  The Windows implementation obtains
/// each value from the kernel; this pure contract keeps the security ordering testable
/// without registering a service or opening a real pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityContractObservation {
    pub first_frame_buffered: bool,
    pub impersonation_succeeded: bool,
    pub token_user_captured: bool,
    pub token_integrity_captured: bool,
    pub token_session_captured: bool,
    pub pipe_pid_captured: bool,
    pub process_start_time_kernel_verified: bool,
    pub impersonation_reverted: bool,
    pub client_claimed_identity_trusted: bool,
}

pub const fn identity_contract_allows_dispatch(observation: IdentityContractObservation) -> bool {
    observation.first_frame_buffered
        && observation.impersonation_succeeded
        && observation.token_user_captured
        && observation.token_integrity_captured
        && observation.token_session_captured
        && observation.pipe_pid_captured
        && observation.process_start_time_kernel_verified
        && observation.impersonation_reverted
        && !observation.client_claimed_identity_trusted
}

pub const fn identity_resource_contract_is_closed(
    impersonation_token_closed: bool,
    client_process_handle_closed: bool,
) -> bool {
    impersonation_token_closed && client_process_handle_closed
}

/// The accept loop must be woken by the service stop event, report STOP_PENDING before
/// waiting, and request cancellation of any exact owned active session.  `busy_spin` is
/// deliberately inverted: a stop contract is invalid if it depends on polling.
pub const fn service_stop_contract_is_valid(
    stop_requested: bool,
    accept_loop_signaled: bool,
    pending_pipe_accept_cancellable: bool,
    stop_pending_reported: bool,
    stopped_reported: bool,
    active_session_cancel_requested: bool,
    busy_spin: bool,
) -> bool {
    stop_requested
        && accept_loop_signaled
        && pending_pipe_accept_cancellable
        && stop_pending_reported
        && stopped_reported
        && active_session_cancel_requested
        && !busy_spin
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    OversizedRequest,
    MalformedInput(String),
    InvalidRequest(String),
    UnsupportedRequest(String),
    ProtocolMismatch { received: u32, expected: u32 },
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OversizedRequest => write!(formatter, "request exceeds {MAX_FRAME_BYTES} bytes"),
            Self::MalformedInput(error) => write!(formatter, "malformed JSON: {error}"),
            Self::InvalidRequest(error) => write!(formatter, "invalid request: {error}"),
            Self::UnsupportedRequest(request_type) => {
                write!(formatter, "unsupported request type: {request_type}")
            }
            Self::ProtocolMismatch { received, expected } => {
                write!(
                    formatter,
                    "protocol version {received} is incompatible with {expected}"
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    OversizedFrame,
    TruncatedFrame,
    TrailingBytes,
    InvalidJson(String),
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OversizedFrame => write!(formatter, "frame exceeds {MAX_FRAME_BYTES} bytes"),
            Self::TruncatedFrame => write!(formatter, "frame is truncated"),
            Self::TrailingBytes => write!(formatter, "frame contains trailing bytes"),
            Self::InvalidJson(error) => write!(formatter, "frame JSON is invalid: {error}"),
        }
    }
}

/// Requests are semantic capabilities.  There is deliberately no executable, argv, shell,
/// working-directory, environment, registry-path, or output-path field in this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticRequest {
    GetAmdProviderStatus {
        protocol_version: u32,
        request_id: String,
    },
    StartAmdPowerSession {
        protocol_version: u32,
        request_id: String,
        duration_ms: u32,
        interval_ms: u32,
    },
    GetAmdSessionStatus {
        protocol_version: u32,
        request_id: String,
        session_id: String,
    },
    CancelAmdSession {
        protocol_version: u32,
        request_id: String,
        session_id: String,
    },
}

impl SemanticRequest {
    pub fn request_type(&self) -> &'static str {
        match self {
            Self::GetAmdProviderStatus { .. } => "GetAmdProviderStatus",
            Self::StartAmdPowerSession { .. } => "StartAmdPowerSession",
            Self::GetAmdSessionStatus { .. } => "GetAmdSessionStatus",
            Self::CancelAmdSession { .. } => "CancelAmdSession",
        }
    }

    pub fn protocol_version(&self) -> u32 {
        match self {
            Self::GetAmdProviderStatus {
                protocol_version, ..
            }
            | Self::StartAmdPowerSession {
                protocol_version, ..
            }
            | Self::GetAmdSessionStatus {
                protocol_version, ..
            }
            | Self::CancelAmdSession {
                protocol_version, ..
            } => *protocol_version,
        }
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::GetAmdProviderStatus { request_id, .. }
            | Self::StartAmdPowerSession { request_id, .. }
            | Self::GetAmdSessionStatus { request_id, .. }
            | Self::CancelAmdSession { request_id, .. } => request_id,
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            Self::GetAmdProviderStatus {
                protocol_version,
                request_id,
            } => json!({
                "protocol_version": protocol_version,
                "request_id": request_id,
                "request_type": "GetAmdProviderStatus"
            }),
            Self::StartAmdPowerSession {
                protocol_version,
                request_id,
                duration_ms,
                interval_ms,
            } => json!({
                "protocol_version": protocol_version,
                "request_id": request_id,
                "request_type": "StartAmdPowerSession",
                "duration_ms": duration_ms,
                "interval_ms": interval_ms
            }),
            Self::GetAmdSessionStatus {
                protocol_version,
                request_id,
                session_id,
            } => json!({
                "protocol_version": protocol_version,
                "request_id": request_id,
                "request_type": "GetAmdSessionStatus",
                "session_id": session_id
            }),
            Self::CancelAmdSession {
                protocol_version,
                request_id,
                session_id,
            } => json!({
                "protocol_version": protocol_version,
                "request_id": request_id,
                "request_type": "CancelAmdSession",
                "session_id": session_id
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderStatusWire {
    request_type: String,
    protocol_version: u32,
    request_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartSessionWire {
    request_type: String,
    protocol_version: u32,
    request_id: String,
    duration_ms: u32,
    interval_ms: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionIdWire {
    request_type: String,
    protocol_version: u32,
    request_id: String,
    session_id: String,
}

pub fn decode_request(bytes: &[u8]) -> Result<SemanticRequest, ProtocolError> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::OversizedRequest);
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| ProtocolError::MalformedInput(error.to_string()))?;
    let object = value
        .as_object()
        .ok_or_else(|| ProtocolError::InvalidRequest("request must be a JSON object".to_owned()))?;
    let request_type = object
        .get("request_type")
        .and_then(Value::as_str)
        .ok_or_else(|| ProtocolError::InvalidRequest("request_type must be a string".to_owned()))?
        .to_owned();

    match request_type.as_str() {
        "GetAmdProviderStatus" => {
            let wire: ProviderStatusWire = serde_json::from_value(value.clone())
                .map_err(|error| ProtocolError::InvalidRequest(error.to_string()))?;
            ensure_request_type(&wire.request_type, &request_type)?;
            ensure_protocol(wire.protocol_version)?;
            ensure_request_id(&wire.request_id)?;
            Ok(SemanticRequest::GetAmdProviderStatus {
                protocol_version: wire.protocol_version,
                request_id: wire.request_id,
            })
        }
        "StartAmdPowerSession" => {
            let wire: StartSessionWire = serde_json::from_value(value.clone())
                .map_err(|error| ProtocolError::InvalidRequest(error.to_string()))?;
            ensure_request_type(&wire.request_type, &request_type)?;
            ensure_protocol(wire.protocol_version)?;
            ensure_request_id(&wire.request_id)?;
            validate_start_bounds(wire.duration_ms, wire.interval_ms)?;
            Ok(SemanticRequest::StartAmdPowerSession {
                protocol_version: wire.protocol_version,
                request_id: wire.request_id,
                duration_ms: wire.duration_ms,
                interval_ms: wire.interval_ms,
            })
        }
        "GetAmdSessionStatus" => {
            let wire: SessionIdWire = serde_json::from_value(value.clone())
                .map_err(|error| ProtocolError::InvalidRequest(error.to_string()))?;
            ensure_request_type(&wire.request_type, &request_type)?;
            ensure_protocol(wire.protocol_version)?;
            ensure_request_id(&wire.request_id)?;
            ensure_session_id(&wire.session_id)?;
            Ok(SemanticRequest::GetAmdSessionStatus {
                protocol_version: wire.protocol_version,
                request_id: wire.request_id,
                session_id: wire.session_id,
            })
        }
        "CancelAmdSession" => {
            let wire: SessionIdWire = serde_json::from_value(value.clone())
                .map_err(|error| ProtocolError::InvalidRequest(error.to_string()))?;
            ensure_request_type(&wire.request_type, &request_type)?;
            ensure_protocol(wire.protocol_version)?;
            ensure_request_id(&wire.request_id)?;
            ensure_session_id(&wire.session_id)?;
            Ok(SemanticRequest::CancelAmdSession {
                protocol_version: wire.protocol_version,
                request_id: wire.request_id,
                session_id: wire.session_id,
            })
        }
        other => Err(ProtocolError::UnsupportedRequest(other.to_owned())),
    }
}

fn ensure_request_type(actual: &str, expected: &str) -> Result<(), ProtocolError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ProtocolError::InvalidRequest(
            "request_type does not match the selected schema".to_owned(),
        ))
    }
}

fn ensure_protocol(version: u32) -> Result<(), ProtocolError> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(ProtocolError::ProtocolMismatch {
            received: version,
            expected: PROTOCOL_VERSION,
        })
    }
}

fn ensure_request_id(request_id: &str) -> Result<(), ProtocolError> {
    if request_id.is_empty()
        || request_id.len() > MAX_REQUEST_ID_BYTES
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ProtocolError::InvalidRequest(
            "request_id must be a bounded token".to_owned(),
        ));
    }
    Ok(())
}

pub fn ensure_session_id(session_id: &str) -> Result<(), ProtocolError> {
    if session_id.is_empty()
        || session_id.len() > MAX_REQUEST_ID_BYTES
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ProtocolError::InvalidRequest(
            "session_id must be a bounded token".to_owned(),
        ));
    }
    Ok(())
}

pub fn validate_start_bounds(duration_ms: u32, interval_ms: u32) -> Result<(), ProtocolError> {
    if !(MIN_DURATION_MS..=MAX_DURATION_MS).contains(&duration_ms)
        || !(MIN_INTERVAL_MS..=MAX_INTERVAL_MS).contains(&interval_ms)
        || duration_ms < interval_ms
        || !duration_ms.is_multiple_of(1_000)
    {
        return Err(ProtocolError::InvalidRequest(format!(
            "duration_ms must be {MIN_DURATION_MS}..{MAX_DURATION_MS} in whole seconds, interval_ms must be {MIN_INTERVAL_MS}..{MAX_INTERVAL_MS}, and duration_ms >= interval_ms"
        )));
    }
    Ok(())
}

pub fn encode_json_frame(value: &Value) -> Result<Vec<u8>, FrameError> {
    let payload =
        serde_json::to_vec(value).map_err(|error| FrameError::InvalidJson(error.to_string()))?;
    if payload.len() > MAX_FRAME_BYTES {
        return Err(FrameError::OversizedFrame);
    }
    let length = u32::try_from(payload.len()).map_err(|_| FrameError::OversizedFrame)?;
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn decode_json_frame(frame: &[u8]) -> Result<Value, FrameError> {
    if frame.len() < 4 {
        return Err(FrameError::TruncatedFrame);
    }
    let length = u32::from_le_bytes(frame[..4].try_into().expect("four-byte prefix")) as usize;
    if length > MAX_FRAME_BYTES {
        return Err(FrameError::OversizedFrame);
    }
    if frame.len() < 4 + length {
        return Err(FrameError::TruncatedFrame);
    }
    if frame.len() > 4 + length {
        return Err(FrameError::TrailingBytes);
    }
    serde_json::from_slice(&frame[4..]).map_err(|error| FrameError::InvalidJson(error.to_string()))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResponseStatus {
    Ok,
    InvalidRequest,
    UnsupportedRequest,
    ProtocolMismatch,
    Busy,
    AccessDenied,
    SessionNotFound,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderStatus {
    pub available: bool,
    pub identity_valid: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionState {
    Idle,
    Starting,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionResultSummary {
    pub amd_runtime_executed: bool,
    pub cli_started_by_broker: bool,
    pub cli_exit_code: Option<i32>,
    pub package_power_sampling: String,
    pub package_power_sample_count: usize,
    pub cadence_policy_result: String,
    pub failure_classification: Option<String>,
    pub no_orphan_child: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSnapshot {
    pub session_id: String,
    pub owner_user_sid: String,
    pub owner_client_pid: u32,
    pub owner_client_process_start_time: u64,
    pub created_at_utc_unix_ms: u128,
    pub state: SessionState,
    pub cancel_requested: bool,
    pub result: Option<SessionResultSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrokerResponse {
    pub protocol_version: u32,
    pub request_id: String,
    pub status: ResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_status: Option<ProviderStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionSnapshot>,
}

impl BrokerResponse {
    pub fn new(request_id: impl Into<String>, status: ResponseStatus) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            status,
            message: None,
            provider_status: None,
            session: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_session(mut self, session: SessionSnapshot) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_provider_status(mut self, provider_status: ProviderStatus) -> Self {
        self.provider_status = Some(provider_status);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientIdentity {
    pub client_pid: u32,
    pub client_process_start_time: u64,
    pub client_user_sid: String,
    pub client_integrity_level: Option<String>,
    pub client_session_id: Option<u32>,
    pub client_is_local: bool,
}

impl ClientIdentity {
    pub fn owner(&self) -> SessionOwner {
        SessionOwner {
            user_sid: self.client_user_sid.clone(),
            client_pid: self.client_pid,
            client_process_start_time: self.client_process_start_time,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOwner {
    pub user_sid: String,
    pub client_pid: u32,
    pub client_process_start_time: u64,
}

pub fn authorize_client(
    identity: &ClientIdentity,
    expected_user_sid: &str,
) -> Result<(), ProtocolError> {
    if !identity.client_is_local
        || !identity
            .client_user_sid
            .eq_ignore_ascii_case(expected_user_sid)
    {
        return Err(ProtocolError::InvalidRequest(
            "client identity is not authorized for this qualification endpoint".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    Busy,
    AccessDenied,
    SessionNotFound,
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => write!(formatter, "one AMD session is already active"),
            Self::AccessDenied => write!(formatter, "session owner authorization failed"),
            Self::SessionNotFound => write!(formatter, "session id is stale or unknown"),
        }
    }
}

#[derive(Clone)]
pub struct SessionLease {
    pub snapshot: SessionSnapshot,
    pub cancellation: Arc<AtomicBool>,
}

struct ActiveSession {
    snapshot: SessionSnapshot,
    cancellation: Arc<AtomicBool>,
}

struct SessionBook {
    active: Option<ActiveSession>,
    history: BTreeMap<String, SessionSnapshot>,
}

#[derive(Clone)]
pub struct SessionCoordinator {
    book: Arc<Mutex<SessionBook>>,
}

impl Default for SessionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionCoordinator {
    pub fn new() -> Self {
        Self {
            book: Arc::new(Mutex::new(SessionBook {
                active: None,
                history: BTreeMap::new(),
            })),
        }
    }

    pub fn start(&self, owner: SessionOwner) -> Result<SessionLease, SessionError> {
        let mut book = self
            .book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if book.active.is_some() {
            return Err(SessionError::Busy);
        }
        let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let session_id = format!(
            "amd-i2-{:x}-{:x}-{:x}",
            unix_time_millis(),
            std::process::id(),
            counter
        );
        let snapshot = SessionSnapshot {
            session_id,
            owner_user_sid: owner.user_sid,
            owner_client_pid: owner.client_pid,
            owner_client_process_start_time: owner.client_process_start_time,
            created_at_utc_unix_ms: unix_time_millis(),
            state: SessionState::Starting,
            cancel_requested: false,
            result: None,
        };
        let cancellation = Arc::new(AtomicBool::new(false));
        book.active = Some(ActiveSession {
            snapshot: snapshot.clone(),
            cancellation: cancellation.clone(),
        });
        Ok(SessionLease {
            snapshot,
            cancellation,
        })
    }

    pub fn mark_running(&self, session_id: &str) -> Result<SessionSnapshot, SessionError> {
        let mut book = self
            .book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = book.active.as_mut().ok_or(SessionError::SessionNotFound)?;
        if active.snapshot.session_id != session_id {
            return Err(SessionError::SessionNotFound);
        }
        active.snapshot.state = SessionState::Running;
        Ok(active.snapshot.clone())
    }

    pub fn finish(
        &self,
        session_id: &str,
        state: SessionState,
        result: SessionResultSummary,
    ) -> Result<SessionSnapshot, SessionError> {
        let mut book = self
            .book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut active = book.active.take().ok_or(SessionError::SessionNotFound)?;
        if active.snapshot.session_id != session_id {
            book.active = Some(active);
            return Err(SessionError::SessionNotFound);
        }
        active.snapshot.state = state;
        active.snapshot.result = Some(result);
        let snapshot = active.snapshot.clone();
        book.history.insert(session_id.to_owned(), snapshot.clone());
        while book.history.len() > 32 {
            let Some(oldest) = book.history.keys().next().cloned() else {
                break;
            };
            book.history.remove(&oldest);
        }
        Ok(snapshot)
    }

    pub fn snapshot(&self, session_id: &str) -> Result<SessionSnapshot, SessionError> {
        let book = self
            .book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(active) = book
            .active
            .as_ref()
            .filter(|active| active.snapshot.session_id == session_id)
        {
            return Ok(active.snapshot.clone());
        }
        book.history
            .get(session_id)
            .cloned()
            .ok_or(SessionError::SessionNotFound)
    }

    pub fn request_cancel(
        &self,
        session_id: &str,
        requester_sid: &str,
    ) -> Result<SessionSnapshot, SessionError> {
        let book = self
            .book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let active = book.active.as_ref().ok_or(SessionError::SessionNotFound)?;
        if active.snapshot.session_id != session_id {
            return Err(SessionError::SessionNotFound);
        }
        if !active
            .snapshot
            .owner_user_sid
            .eq_ignore_ascii_case(requester_sid)
        {
            return Err(SessionError::AccessDenied);
        }
        active.cancellation.store(true, Ordering::Release);
        let mut snapshot = active.snapshot.clone();
        snapshot.cancel_requested = true;
        Ok(snapshot)
    }

    pub fn disconnect(&self, owner: &SessionOwner) -> bool {
        let book = self
            .book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(active) = book.active.as_ref() else {
            return false;
        };
        let matches = active
            .snapshot
            .owner_user_sid
            .eq_ignore_ascii_case(&owner.user_sid)
            && active.snapshot.owner_client_pid == owner.client_pid
            && active.snapshot.owner_client_process_start_time == owner.client_process_start_time;
        if matches {
            active.cancellation.store(true, Ordering::Release);
        }
        matches
    }

    pub fn has_active_session(&self) -> bool {
        self.book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active
            .is_some()
    }

    pub fn cancel_active_on_shutdown(&self) {
        if let Some(active) = self
            .book
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .active
            .as_ref()
        {
            active.cancellation.store(true, Ordering::Release);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipeAce {
    pub principal: String,
    pub sid: String,
    pub access: String,
    pub rights: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipeDaclEvidence {
    pub schema: String,
    pub pipe_name: String,
    pub owner: String,
    pub aces: Vec<PipeAce>,
    pub installing_user_sid: String,
    pub service_sid: String,
    pub broad_user_access_present: bool,
    pub remote_clients_rejected: bool,
    pub sddl: String,
}

pub fn validate_scope(scope: &str) -> Result<(), String> {
    if scope.len() != 32 || !scope.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("scope must be exactly 32 hexadecimal characters".to_owned());
    }
    Ok(())
}

pub fn pipe_name_for_scope(scope: &str) -> Result<String, String> {
    validate_scope(scope)?;
    Ok(format!("{PIPE_PREFIX}{scope}"))
}

fn validate_sid(sid: &str) -> Result<(), String> {
    if sid.len() < 5
        || !sid
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'S' || byte == b'-')
        || !sid.starts_with("S-1-")
    {
        return Err(format!("invalid SID: {sid}"));
    }
    Ok(())
}

pub fn build_pipe_dacl(
    pipe_name: impl Into<String>,
    installing_user_sid: &str,
    service_sid: &str,
) -> Result<PipeDaclEvidence, String> {
    let pipe_name = pipe_name.into();
    if !pipe_name.starts_with(PIPE_PREFIX) {
        return Err("pipe name is outside the ResourceTimeline qualification namespace".to_owned());
    }
    validate_sid(installing_user_sid)?;
    validate_sid(service_sid)?;
    let aces = vec![
        PipeAce {
            principal: "installing_user".to_owned(),
            sid: installing_user_sid.to_owned(),
            access: "ALLOW".to_owned(),
            rights: "GENERIC_ALL".to_owned(),
        },
        PipeAce {
            principal: SERVICE_SID_ACCOUNT.to_owned(),
            sid: service_sid.to_owned(),
            access: "ALLOW".to_owned(),
            rights: "GENERIC_ALL".to_owned(),
        },
        PipeAce {
            principal: "SYSTEM".to_owned(),
            sid: SYSTEM_SID.to_owned(),
            access: "ALLOW".to_owned(),
            rights: "GENERIC_ALL".to_owned(),
        },
    ];
    let broad_user_access_present = aces.iter().any(|ace| {
        matches!(
            ace.sid.to_ascii_uppercase().as_str(),
            "S-1-1-0" | "S-1-5-11" | "S-1-5-32-545"
        ) || matches!(
            ace.principal.to_ascii_lowercase().as_str(),
            "everyone" | "authenticated users" | "builtin\\users" | "users"
        )
    });
    if broad_user_access_present {
        return Err("broad user access is forbidden".to_owned());
    }
    Ok(PipeDaclEvidence {
        schema: "amd-privilege-pipe-dacl/v1".to_owned(),
        pipe_name,
        // CreateNamedPipeW requires the owner to be a SID present in the broker token. The
        // installing user is a client principal, not a broker token principal.
        owner: SERVICE_ACCOUNT_SID.to_owned(),
        aces,
        installing_user_sid: installing_user_sid.to_owned(),
        service_sid: service_sid.to_owned(),
        broad_user_access_present,
        remote_clients_rejected: true,
        sddl: format!(
            "O:{SERVICE_ACCOUNT_SID}D:P(A;;GA;;;{installing_user_sid})(A;;GA;;;{service_sid})(A;;GA;;;{SYSTEM_SID})"
        ),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrokerConfig {
    pub schema: String,
    pub service_name: String,
    pub service_account: String,
    pub service_account_sid: String,
    pub service_sid: Option<String>,
    pub installing_user_sid: String,
    pub scope: String,
    pub pipe_name: String,
    pub output_root: String,
}

impl BrokerConfig {
    pub fn new(
        scope: &str,
        installing_user_sid: &str,
        output_root: impl Into<String>,
    ) -> Result<Self, String> {
        let pipe_name = pipe_name_for_scope(scope)?;
        validate_sid(installing_user_sid)?;
        Ok(Self {
            schema: "amd-privilege-broker-config/v1".to_owned(),
            service_name: SERVICE_NAME.to_owned(),
            service_account: SERVICE_ACCOUNT.to_owned(),
            service_account_sid: SERVICE_ACCOUNT_SID.to_owned(),
            service_sid: None,
            installing_user_sid: installing_user_sid.to_owned(),
            scope: scope.to_owned(),
            pipe_name,
            output_root: output_root.into(),
        })
    }

    pub fn with_service_sid(mut self, service_sid: impl Into<String>) -> Result<Self, String> {
        let service_sid = service_sid.into();
        validate_sid(&service_sid)?;
        self.service_sid = Some(service_sid);
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyntheticCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationAssertions {
    pub real_amd_runtime_count_during_task: u32,
    pub service_context_runtime_count_during_task: u32,
    pub service_registration_count_during_task: u32,
    pub scheduled_task_registration_count: u32,
    pub self_elevation_performed: bool,
    pub amd_installation_mutated: bool,
    pub amd_registry_mutated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyntheticQualificationSummary {
    pub schema: String,
    pub result: String,
    pub qualification_only: bool,
    pub amd_runtime_executed: bool,
    pub checks: Vec<SyntheticCheck>,
    pub mutation_assertions: MutationAssertions,
}

pub fn check(name: impl Into<String>, passed: bool, detail: impl Into<String>) -> SyntheticCheck {
    SyntheticCheck {
        name: name.into(),
        status: if passed { "PASS" } else { "FAIL" }.to_owned(),
        detail: detail.into(),
    }
}

pub fn unix_time_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub fn write_json<T: Serialize>(path: &std::path::Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(path, bytes)
}

pub fn request_id_from_json(bytes: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(bytes)
        .ok()?
        .get("request_id")?
        .as_str()
        .map(ToOwned::to_owned)
}

pub fn response_for_protocol_error(
    request_id: Option<String>,
    error: &ProtocolError,
) -> BrokerResponse {
    let request_id = request_id.unwrap_or_else(|| "protocol-error".to_owned());
    let (status, message) = match error {
        ProtocolError::OversizedRequest => (ResponseStatus::InvalidRequest, error.to_string()),
        ProtocolError::UnsupportedRequest(_) => {
            (ResponseStatus::UnsupportedRequest, error.to_string())
        }
        ProtocolError::ProtocolMismatch { .. } => {
            (ResponseStatus::ProtocolMismatch, error.to_string())
        }
        ProtocolError::MalformedInput(_) | ProtocolError::InvalidRequest(_) => {
            (ResponseStatus::InvalidRequest, error.to_string())
        }
    };
    BrokerResponse::new(request_id, status).with_message(message)
}

pub fn response_for_session_error(request_id: &str, error: &SessionError) -> BrokerResponse {
    let status = match error {
        SessionError::Busy => ResponseStatus::Busy,
        SessionError::AccessDenied => ResponseStatus::AccessDenied,
        SessionError::SessionNotFound => ResponseStatus::SessionNotFound,
    };
    BrokerResponse::new(request_id, status).with_message(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_start() -> Vec<u8> {
        serde_json::to_vec(&json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": "req-1",
            "request_type": "StartAmdPowerSession",
            "duration_ms": 5_000,
            "interval_ms": 1_000
        }))
        .unwrap()
    }

    #[test]
    fn valid_semantic_request_decodes() {
        let request = decode_request(&valid_start()).unwrap();
        assert_eq!(request.request_type(), "StartAmdPowerSession");
        assert_eq!(request.protocol_version(), PROTOCOL_VERSION);
    }

    #[test]
    fn protocol_version_mismatch_is_distinct() {
        let value = json!({
            "protocol_version": 99,
            "request_id": "req-1",
            "request_type": "GetAmdProviderStatus"
        });
        assert_eq!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(ProtocolError::ProtocolMismatch {
                received: 99,
                expected: PROTOCOL_VERSION
            })
        );
    }

    #[test]
    fn unknown_request_type_is_rejected() {
        let value = json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": "req-1",
            "request_type": "RunCommand"
        });
        assert_eq!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(ProtocolError::UnsupportedRequest("RunCommand".to_owned()))
        );
    }

    #[test]
    fn raw_executable_argv_and_output_path_are_not_schema_fields() {
        let value = SemanticRequest::StartAmdPowerSession {
            protocol_version: PROTOCOL_VERSION,
            request_id: "req-1".to_owned(),
            duration_ms: 5_000,
            interval_ms: 1_000,
        }
        .to_value();
        for forbidden in [
            "executable_path",
            "argv",
            "raw_command",
            "output_path",
            "working_directory",
        ] {
            assert!(!value.as_object().unwrap().contains_key(forbidden));
        }
        let malicious = json!({
            "protocol_version": PROTOCOL_VERSION,
            "request_id": "req-1",
            "request_type": "StartAmdPowerSession",
            "duration_ms": 5_000,
            "interval_ms": 1_000,
            "executable_path": "C:\\Windows\\System32\\cmd.exe"
        });
        assert!(matches!(
            decode_request(&serde_json::to_vec(&malicious).unwrap()),
            Err(ProtocolError::InvalidRequest(_))
        ));
    }

    #[test]
    fn bounds_reject_invalid_windows() {
        assert!(validate_start_bounds(5_000, 1_000).is_ok());
        assert!(validate_start_bounds(4_999, 1_000).is_err());
        assert!(validate_start_bounds(5_000, 10_001).is_err());
        assert!(validate_start_bounds(5_000, 6_000).is_err());
        assert!(validate_start_bounds(5_500, 1_000).is_err());
    }

    #[test]
    fn framing_rejects_oversized_truncated_and_trailing_data() {
        let frame = encode_json_frame(&json!({"ok": true})).unwrap();
        assert_eq!(decode_json_frame(&frame).unwrap()["ok"], true);
        assert_eq!(
            decode_json_frame(&frame[..3]),
            Err(FrameError::TruncatedFrame)
        );
        let mut trailing = frame.clone();
        trailing.push(0);
        assert_eq!(decode_json_frame(&trailing), Err(FrameError::TrailingBytes));
        let oversized = (MAX_FRAME_BYTES as u32 + 1).to_le_bytes().to_vec();
        assert_eq!(
            decode_json_frame(&oversized),
            Err(FrameError::OversizedFrame)
        );
    }

    #[test]
    fn acl_is_explicit_and_has_no_broad_user_allow() {
        let pipe = pipe_name_for_scope("0123456789abcdef0123456789abcdef").unwrap();
        let evidence =
            build_pipe_dacl(&pipe, "S-1-5-21-100-200-300-1001", "S-1-5-80-1-2-3-4-5").unwrap();
        assert_eq!(evidence.aces.len(), 3);
        assert!(!evidence.broad_user_access_present);
        assert!(evidence.remote_clients_rejected);
        assert_eq!(evidence.owner, SERVICE_ACCOUNT_SID);
        assert!(evidence
            .sddl
            .starts_with(&format!("O:{SERVICE_ACCOUNT_SID}D:P")));
        assert!(evidence.sddl.contains("S-1-5-80-1-2-3-4-5"));
        assert!(evidence.sddl.contains(SYSTEM_SID));
    }

    #[test]
    fn readiness_requires_live_listener_before_ready_and_running() {
        let mut readiness = BrokerReadinessState::new();
        assert!(readiness.publish_ready().is_err());
        assert!(readiness.report_running().is_err());
        assert!(!readiness.listener_created());
        assert!(!readiness.ready_published());
        assert!(!readiness.service_running());

        readiness.mark_first_listener_created();
        assert!(readiness.publish_ready().is_err());
        assert!(readiness.report_running().is_err());
        assert!(readiness
            .mark_first_accept_armed(FirstAcceptState::IoPending)
            .is_ok());
        assert!(readiness.publish_ready().is_ok());
        assert!(readiness.report_running().is_ok());
        assert!(readiness.listener_created());
        assert!(readiness.first_accept_armed());
        assert_eq!(
            readiness.first_accept_state(),
            Some(FirstAcceptState::IoPending)
        );
        assert!(readiness.ready_published());
        assert!(readiness.service_running());
    }

    #[test]
    fn pending_accept_requires_terminal_completion_before_release() {
        assert!(!pending_accept_release_is_safe(
            PendingAcceptLifecycleState::IoPending
        ));
        assert!(!pending_accept_release_is_safe(
            pending_accept_state_after_cancel(PendingAcceptCompletion::NotObserved)
        ));
        assert!(pending_accept_release_is_safe(
            pending_accept_state_after_cancel(PendingAcceptCompletion::OperationAborted)
        ));
        assert!(pending_accept_release_is_safe(
            pending_accept_state_after_cancel(PendingAcceptCompletion::Normal)
        ));
        assert!(pending_accept_cancel_is_drained(
            pending_accept_state_after_cancel(PendingAcceptCompletion::Failed)
        ));
    }

    #[test]
    fn client_authorization_requires_local_expected_sid() {
        let identity = ClientIdentity {
            client_pid: 42,
            client_process_start_time: 99,
            client_user_sid: "S-1-5-21-1-2-3-1001".to_owned(),
            client_integrity_level: Some("S-1-16-8192".to_owned()),
            client_session_id: Some(1),
            client_is_local: true,
        };
        assert!(authorize_client(&identity, "S-1-5-21-1-2-3-1001").is_ok());
        assert!(authorize_client(&identity, "S-1-5-21-1-2-3-1002").is_err());
        let mut remote = identity;
        remote.client_is_local = false;
        assert!(authorize_client(&remote, "S-1-5-21-1-2-3-1001").is_err());
    }

    #[test]
    fn session_arbitration_ownership_and_disconnect_are_bounded() {
        let coordinator = SessionCoordinator::new();
        let owner = SessionOwner {
            user_sid: "S-1-5-21-1-2-3-1001".to_owned(),
            client_pid: 10,
            client_process_start_time: 20,
        };
        let lease = coordinator.start(owner.clone()).unwrap();
        assert_eq!(lease.snapshot.state, SessionState::Starting);
        assert!(matches!(
            coordinator.start(owner.clone()),
            Err(SessionError::Busy)
        ));
        assert_eq!(
            coordinator
                .mark_running(&lease.snapshot.session_id)
                .unwrap()
                .state,
            SessionState::Running
        );
        assert_eq!(
            coordinator.request_cancel(&lease.snapshot.session_id, "S-1-5-21-1-2-3-999"),
            Err(SessionError::AccessDenied)
        );
        assert!(coordinator.disconnect(&owner));
        assert!(lease.cancellation.load(Ordering::Acquire));
        let result = SessionResultSummary {
            amd_runtime_executed: false,
            cli_started_by_broker: false,
            cli_exit_code: None,
            package_power_sampling: "NOT_RUN".to_owned(),
            package_power_sample_count: 0,
            cadence_policy_result: "NOT_RUN".to_owned(),
            failure_classification: Some("CANCELLED".to_owned()),
            no_orphan_child: true,
        };
        let finished = coordinator
            .finish(&lease.snapshot.session_id, SessionState::Cancelled, result)
            .unwrap();
        assert_eq!(finished.state, SessionState::Cancelled);
        assert_eq!(
            coordinator.start(owner).unwrap().snapshot.state,
            SessionState::Starting
        );
        assert_eq!(
            coordinator.snapshot("stale-session"),
            Err(SessionError::SessionNotFound)
        );
    }
}
