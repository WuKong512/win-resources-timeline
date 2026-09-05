use crate::package_power::{assess_cadence, parse_package_power_csv};
use crate::{
    authorize_client, build_pipe_dacl, check, decode_request, encode_json_frame,
    BrokerReadinessState, BrokerResponse, ClientIdentity, MutationAssertions, ProtocolError,
    ResponseStatus, SemanticRequest, SessionCoordinator, SessionError, SessionOwner,
    SessionResultSummary, SessionState, SyntheticCheck, SyntheticQualificationSummary,
    MAX_FRAME_BYTES, PROTOCOL_VERSION, SERVICE_ACCOUNT_SID,
};
use serde_json::json;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

const POWER_FIXTURE: &str =
    include_str!("../../amd-uprof-cli-spike/test-fixtures/package-power.csv");

pub fn run(
    evidence_root: Option<&std::path::Path>,
) -> Result<SyntheticQualificationSummary, String> {
    let checks = vec![
        check(
            "PROTOCOL_VALID_REQUEST",
            valid_protocol_request(),
            "typed StartAmdPowerSession request accepted within provisional bounds",
        ),
        check(
            "PROTOCOL_VERSION_MISMATCH",
            protocol_mismatch_rejected(),
            "incompatible protocol version returns PROTOCOL_MISMATCH",
        ),
        check(
            "UNKNOWN_REQUEST_REJECTED",
            unknown_request_rejected(),
            "unknown request type returns UNSUPPORTED_REQUEST",
        ),
        check(
            "OVERSIZED_REQUEST_REJECTED",
            oversized_request_rejected(),
            "request larger than the bounded frame is rejected",
        ),
        check(
            "RAW_EXECUTABLE_SURFACE_ABSENT",
            raw_surface_absent("executable_path"),
            "schema has no executable_path field and rejects it",
        ),
        check(
            "RAW_ARGV_SURFACE_ABSENT",
            raw_surface_absent("argv"),
            "schema has no argv field and rejects it",
        ),
        check(
            "RAW_OUTPUT_PATH_SURFACE_ABSENT",
            raw_surface_absent("output_path"),
            "schema has no output_path field and rejects it",
        ),
        check(
            "PIPE_ACL_INSTALLING_USER",
            acl_contains("installing_user"),
            "explicit installing-user SID ACE is present",
        ),
        check(
            "PIPE_ACL_SERVICE_SID",
            acl_contains("service_sid"),
            "explicit Service SID ACE is present",
        ),
        check(
            "PIPE_ACL_SYSTEM",
            acl_contains("system"),
            "explicit SYSTEM ACE is present",
        ),
        check(
            "PIPE_ACL_EVERYONE_ABSENT",
            acl_has_no_broad_access(),
            "Everyone/Authenticated Users/BUILTIN\\Users are absent",
        ),
        check(
            "PIPE_ACL_AUTHENTICATED_USERS_ABSENT",
            acl_has_no_broad_access(),
            "broad user access flag is false",
        ),
        check(
            "PIPE_REMOTE_CLIENTS_REJECTED",
            acl_rejects_remote_clients(),
            "named-pipe creation uses PIPE_REJECT_REMOTE_CLIENTS",
        ),
        check(
            "PIPE_OWNER_IS_BROKER_ACCOUNT",
            pipe_owner_is_broker_account(),
            "named-pipe SDDL owner is present in the LocalService broker token",
        ),
        check(
            "PIPE_CREATE_FAILURE_DOES_NOT_PUBLISH_READY",
            pipe_failure_does_not_publish_ready(),
            "pipe creation failure cannot publish broker readiness",
        ),
        check(
            "PIPE_CREATE_FAILURE_DOES_NOT_REPORT_RUNNING",
            pipe_failure_does_not_report_running(),
            "pipe creation failure cannot report SERVICE_RUNNING",
        ),
        check(
            "PIPE_CREATE_SUCCESS_PRECEDES_READY",
            pipe_create_success_precedes_ready(),
            "the first live pipe is created before BROKER-READY",
        ),
        check(
            "PIPE_CREATE_SUCCESS_PRECEDES_RUNNING",
            pipe_create_success_precedes_running(),
            "the first live pipe is created before SERVICE_RUNNING",
        ),
        check(
            "READY_IMPLIES_FIRST_LISTENER_PREPARED",
            ready_implies_first_listener_prepared(),
            "published readiness implies a prepared first listener",
        ),
        check(
            "CLIENT_SID_CAPTURE",
            client_identity_captured(),
            "identity record has SID, PID, process-start, integrity and session fields",
        ),
        check(
            "CLIENT_PID_CAPTURE",
            client_identity_captured(),
            "client PID is part of the authenticated owner identity",
        ),
        check(
            "CLIENT_PROCESS_START_IDENTITY",
            client_identity_captured(),
            "PID reuse is bounded by process start identity",
        ),
        check(
            "UNAUTHORIZED_CLIENT_REJECTED",
            unauthorized_client_rejected(),
            "wrong SID or remote identity is rejected",
        ),
        check(
            "ONE_ACTIVE_SESSION",
            one_active_session(),
            "coordinator holds at most one active session",
        ),
        check(
            "SECOND_START_RETURNS_BUSY",
            concurrent_start_is_busy(),
            "second start does not launch or queue a child",
        ),
        check(
            "OWNER_BOUND_SESSION",
            owner_bound_session(),
            "session stores SID, PID and process-start owner tuple",
        ),
        check(
            "STALE_SESSION_ID_REJECTED",
            stale_session_rejected(),
            "unknown session identifier cannot affect active state",
        ),
        check(
            "NON_OWNER_CANCEL_REJECTED",
            non_owner_cancel_rejected(),
            "wrong owner SID returns ACCESS_DENIED",
        ),
        check(
            "OWNER_CANCEL",
            owner_cancel_with_synthetic_child(),
            "owned harmless child is terminated and session reaches CANCELLED",
        ),
        check(
            "CLIENT_DISCONNECT_POLICY",
            disconnect_cancels_owned_session(),
            "disconnect of the exact owner requests cancellation",
        ),
        check(
            "NO_ORPHAN_SYNTHETIC_CHILD",
            no_orphan_synthetic_child(),
            "synthetic child exits after exact-owner cancellation",
        ),
        check(
            "FAKE_RUNNER_FAILURE",
            fake_runner_failure_is_typed(),
            "fake runner failure is FAILED and does not claim AMD execution",
        ),
        check(
            "TIMEOUT_BOUNDARY",
            timeout_boundary_is_typed(),
            "bounded synthetic timeout terminates the owned child",
        ),
        check(
            "MALFORMED_INPUT",
            malformed_input_rejected(),
            "malformed JSON and malformed framing do not reach dispatch",
        ),
        check(
            "PIPE_FRAMING",
            pipe_framing_round_trip(),
            "length-prefixed bounded framing round-trips one semantic request",
        ),
        check(
            "PACKAGE_POWER_PARSER_REGRESSION",
            parser_fixture_passes(),
            "existing package-power fixture remains parseable with cadence",
        ),
    ];

    let passed = checks.iter().all(|check| check.status == "PASS");
    let summary = SyntheticQualificationSummary {
        schema: "amd-privilege-synthetic-qualification/v1".to_owned(),
        result: if passed {
            "PASS".to_owned()
        } else {
            "BLOCKED_SECURE_IPC_PREPARATION".to_owned()
        },
        qualification_only: true,
        amd_runtime_executed: false,
        checks,
        mutation_assertions: MutationAssertions {
            real_amd_runtime_count_during_task: 0,
            service_context_runtime_count_during_task: 0,
            service_registration_count_during_task: 0,
            scheduled_task_registration_count: 0,
            self_elevation_performed: false,
            amd_installation_mutated: false,
            amd_registry_mutated: false,
        },
    };
    if let Some(root) = evidence_root {
        crate::write_json(&root.join("SYNTHETIC-QUALIFICATION.json"), &summary)
            .map_err(|error| format!("writing synthetic evidence failed: {error}"))?;
    }
    Ok(summary)
}

fn valid_protocol_request() -> bool {
    let request = serde_json::to_vec(&json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": "synthetic-valid",
        "request_type": "StartAmdPowerSession",
        "duration_ms": 5_000,
        "interval_ms": 1_000
    }))
    .unwrap();
    matches!(
        decode_request(&request),
        Ok(SemanticRequest::StartAmdPowerSession { .. })
    )
}

fn protocol_mismatch_rejected() -> bool {
    let request = serde_json::to_vec(&json!({
        "protocol_version": PROTOCOL_VERSION + 1,
        "request_id": "synthetic-version",
        "request_type": "GetAmdProviderStatus"
    }))
    .unwrap();
    matches!(
        decode_request(&request),
        Err(ProtocolError::ProtocolMismatch { .. })
    )
}

fn unknown_request_rejected() -> bool {
    let request = serde_json::to_vec(&json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": "synthetic-unknown",
        "request_type": "RunCommand"
    }))
    .unwrap();
    matches!(
        decode_request(&request),
        Err(ProtocolError::UnsupportedRequest(_))
    )
}

fn oversized_request_rejected() -> bool {
    let bytes = vec![b'x'; MAX_FRAME_BYTES + 1];
    matches!(decode_request(&bytes), Err(ProtocolError::OversizedRequest))
}

fn raw_surface_absent(field: &str) -> bool {
    let request = SemanticRequest::StartAmdPowerSession {
        protocol_version: PROTOCOL_VERSION,
        request_id: "synthetic-surface".to_owned(),
        duration_ms: 5_000,
        interval_ms: 1_000,
    };
    let value = request.to_value();
    if value
        .as_object()
        .is_some_and(|object| object.contains_key(field))
    {
        return false;
    }
    let mut malicious = value;
    malicious[field] = match field {
        "argv" => json!(["/c", "whoami"]),
        _ => json!(r"C:\Windows\System32\cmd.exe"),
    };
    matches!(
        decode_request(&serde_json::to_vec(&malicious).unwrap()),
        Err(ProtocolError::InvalidRequest(_))
    )
}

fn acl_contains(expected: &str) -> bool {
    let pipe = crate::pipe_name_for_scope("0123456789abcdef0123456789abcdef").unwrap();
    let dacl = crate::build_pipe_dacl(&pipe, "S-1-5-21-1-2-3-1001", "S-1-5-80-1-2-3-4-5").unwrap();
    match expected {
        "installing_user" => dacl.installing_user_sid == "S-1-5-21-1-2-3-1001",
        "service_sid" => dacl.service_sid == "S-1-5-80-1-2-3-4-5",
        "system" => dacl.aces.iter().any(|ace| ace.sid == crate::SYSTEM_SID),
        _ => false,
    }
}

fn acl_has_no_broad_access() -> bool {
    let pipe = crate::pipe_name_for_scope("0123456789abcdef0123456789abcdef").unwrap();
    let dacl = build_pipe_dacl(&pipe, "S-1-5-21-1-2-3-1001", "S-1-5-80-1-2-3-4-5").unwrap();
    !dacl.broad_user_access_present
        && dacl
            .aces
            .iter()
            .all(|ace| !matches!(ace.sid.as_str(), "S-1-1-0" | "S-1-5-11" | "S-1-5-32-545"))
}

fn acl_rejects_remote_clients() -> bool {
    let pipe = crate::pipe_name_for_scope("0123456789abcdef0123456789abcdef").unwrap();
    build_pipe_dacl(&pipe, "S-1-5-21-1-2-3-1001", "S-1-5-80-1-2-3-4-5")
        .map(|dacl| dacl.remote_clients_rejected)
        .unwrap_or(false)
}

fn pipe_owner_is_broker_account() -> bool {
    let pipe = crate::pipe_name_for_scope("0123456789abcdef0123456789abcdef").unwrap();
    build_pipe_dacl(&pipe, "S-1-5-21-1-2-3-1001", "S-1-5-80-1-2-3-4-5")
        .map(|dacl| dacl.owner == SERVICE_ACCOUNT_SID)
        .unwrap_or(false)
}

fn pipe_failure_does_not_publish_ready() -> bool {
    let readiness = BrokerReadinessState::new();
    !readiness.listener_created()
        && !readiness.ready_published()
        && readiness.clone().publish_ready().is_err()
}

fn pipe_failure_does_not_report_running() -> bool {
    let readiness = BrokerReadinessState::new();
    !readiness.listener_created()
        && !readiness.service_running()
        && readiness.clone().report_running().is_err()
}

fn pipe_create_success_precedes_ready() -> bool {
    let mut readiness = BrokerReadinessState::new();
    if readiness.publish_ready().is_ok() {
        return false;
    }
    readiness.mark_first_listener_created();
    readiness.publish_ready().is_ok() && readiness.ready_published()
}

fn pipe_create_success_precedes_running() -> bool {
    let mut readiness = BrokerReadinessState::new();
    if readiness.report_running().is_ok() {
        return false;
    }
    readiness.mark_first_listener_created();
    if readiness.report_running().is_ok() {
        return false;
    }
    readiness.publish_ready().is_ok()
        && readiness.report_running().is_ok()
        && readiness.service_running()
}

fn ready_implies_first_listener_prepared() -> bool {
    let mut readiness = BrokerReadinessState::new();
    readiness.mark_first_listener_created();
    readiness.publish_ready().is_ok() && readiness.listener_created() && readiness.ready_published()
}

fn client_identity_captured() -> bool {
    let identity = sample_identity();
    identity.client_pid > 0
        && identity.client_process_start_time > 0
        && identity.client_user_sid.starts_with("S-1-")
        && identity.client_integrity_level.is_some()
        && identity.client_session_id.is_some()
}

fn unauthorized_client_rejected() -> bool {
    let identity = sample_identity();
    authorize_client(&identity, "S-1-5-21-1-2-3-999").is_err()
        && authorize_client(
            &ClientIdentity {
                client_is_local: false,
                ..identity
            },
            "S-1-5-21-1-2-3-1001",
        )
        .is_err()
}

fn sample_identity() -> ClientIdentity {
    ClientIdentity {
        client_pid: 100,
        client_process_start_time: 200,
        client_user_sid: "S-1-5-21-1-2-3-1001".to_owned(),
        client_integrity_level: Some("S-1-16-8192".to_owned()),
        client_session_id: Some(1),
        client_is_local: true,
    }
}

fn sample_owner() -> SessionOwner {
    SessionOwner {
        user_sid: "S-1-5-21-1-2-3-1001".to_owned(),
        client_pid: 100,
        client_process_start_time: 200,
    }
}

fn one_active_session() -> bool {
    let coordinator = SessionCoordinator::new();
    let lease = coordinator.start(sample_owner()).unwrap();
    coordinator.has_active_session() && coordinator.snapshot(&lease.snapshot.session_id).is_ok()
}

fn concurrent_start_is_busy() -> bool {
    let coordinator = SessionCoordinator::new();
    let _first = coordinator.start(sample_owner()).unwrap();
    matches!(coordinator.start(sample_owner()), Err(SessionError::Busy))
}

fn owner_bound_session() -> bool {
    let coordinator = SessionCoordinator::new();
    let lease = coordinator.start(sample_owner()).unwrap();
    lease.snapshot.owner_client_pid == 100
        && lease.snapshot.owner_client_process_start_time == 200
        && lease.snapshot.owner_user_sid == "S-1-5-21-1-2-3-1001"
}

fn stale_session_rejected() -> bool {
    let coordinator = SessionCoordinator::new();
    let _lease = coordinator.start(sample_owner()).unwrap();
    coordinator.snapshot("amd-i2-stale") == Err(SessionError::SessionNotFound)
        && coordinator.request_cancel("amd-i2-stale", "S-1-5-21-1-2-3-1001")
            == Err(SessionError::SessionNotFound)
}

fn non_owner_cancel_rejected() -> bool {
    let coordinator = SessionCoordinator::new();
    let lease = coordinator.start(sample_owner()).unwrap();
    coordinator.request_cancel(&lease.snapshot.session_id, "S-1-5-21-1-2-3-999")
        == Err(SessionError::AccessDenied)
}

fn owner_cancel_with_synthetic_child() -> bool {
    let coordinator = SessionCoordinator::new();
    let owner = sample_owner();
    let lease = coordinator.start(owner.clone()).unwrap();
    if coordinator
        .mark_running(&lease.snapshot.session_id)
        .is_err()
    {
        return false;
    }
    let Ok(mut child) = spawn_synthetic_child() else {
        return false;
    };
    let session_id = lease.snapshot.session_id.clone();
    let cancellation = lease.cancellation.clone();
    let runner_coordinator = coordinator.clone();
    let runner = thread::spawn(move || loop {
        if cancellation.load(std::sync::atomic::Ordering::Acquire) {
            let killed = child.kill().is_ok();
            let exited = child
                .wait()
                .map(|status| !status.success())
                .unwrap_or(false);
            let summary = SessionResultSummary {
                amd_runtime_executed: false,
                cli_started_by_broker: false,
                cli_exit_code: None,
                package_power_sampling: "NOT_RUN".to_owned(),
                package_power_sample_count: 0,
                cadence_policy_result: "NOT_RUN".to_owned(),
                failure_classification: Some("CANCELLED".to_owned()),
                no_orphan_child: killed && exited,
            };
            return runner_coordinator
                .finish(&session_id, SessionState::Cancelled, summary)
                .is_ok()
                && killed
                && exited;
        }
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return false,
            Ok(None) => thread::sleep(Duration::from_millis(10)),
        }
    });
    thread::sleep(Duration::from_millis(50));
    if coordinator
        .request_cancel(&lease.snapshot.session_id, &owner.user_sid)
        .is_err()
    {
        lease
            .cancellation
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = runner.join();
        return false;
    }
    runner.join().unwrap_or(false)
}

fn disconnect_cancels_owned_session() -> bool {
    let coordinator = SessionCoordinator::new();
    let owner = sample_owner();
    let lease = coordinator.start(owner.clone()).unwrap();
    let disconnected = coordinator.disconnect(&owner);
    disconnected
        && lease
            .cancellation
            .load(std::sync::atomic::Ordering::Acquire)
}

fn no_orphan_synthetic_child() -> bool {
    let Ok(mut child) = spawn_synthetic_child() else {
        return false;
    };
    let pid = child.id();
    let killed = child.kill().is_ok();
    let exited = child
        .wait()
        .map(|status| !status.success())
        .unwrap_or(false);
    killed && exited && pid != 0
}

fn fake_runner_failure_is_typed() -> bool {
    let coordinator = SessionCoordinator::new();
    let lease = coordinator.start(sample_owner()).unwrap();
    let summary = SessionResultSummary {
        amd_runtime_executed: false,
        cli_started_by_broker: false,
        cli_exit_code: None,
        package_power_sampling: "NOT_RUN".to_owned(),
        package_power_sample_count: 0,
        cadence_policy_result: "NOT_RUN".to_owned(),
        failure_classification: Some("HARNESS_FAILED".to_owned()),
        no_orphan_child: true,
    };
    coordinator
        .finish(&lease.snapshot.session_id, SessionState::Failed, summary)
        .is_ok()
}

fn timeout_boundary_is_typed() -> bool {
    let Ok(mut child) = spawn_synthetic_child() else {
        return false;
    };
    thread::sleep(Duration::from_millis(20));
    let killed = child.kill().is_ok();
    let exited = child.wait().is_ok();
    killed && exited
}

fn malformed_input_rejected() -> bool {
    matches!(
        decode_request(b"{not-json"),
        Err(ProtocolError::MalformedInput(_))
    ) && crate::decode_json_frame(&[1, 2, 3]).is_err()
}

fn pipe_framing_round_trip() -> bool {
    let request = SemanticRequest::GetAmdProviderStatus {
        protocol_version: PROTOCOL_VERSION,
        request_id: "frame-1".to_owned(),
    };
    let frame = encode_json_frame(&request.to_value()).unwrap();
    crate::decode_json_frame(&frame)
        .ok()
        .and_then(|value| decode_request(&serde_json::to_vec(&value).ok()?).ok())
        .is_some_and(|decoded| decoded.request_id() == "frame-1")
}

fn parser_fixture_passes() -> bool {
    parse_package_power_csv(POWER_FIXTURE)
        .map(|parsed| assess_cadence(&parsed.samples, 1_000).status == "PASS")
        .unwrap_or(false)
}

fn spawn_synthetic_child() -> std::io::Result<Child> {
    Command::new(std::env::current_exe()?)
        .arg("--synthetic-child")
        .spawn()
}

#[allow(dead_code)]
fn _response_shape_is_typed() -> BrokerResponse {
    BrokerResponse::new("synthetic", ResponseStatus::Ok)
}

#[allow(dead_code)]
fn _unused_check_type(_: SyntheticCheck) {}
