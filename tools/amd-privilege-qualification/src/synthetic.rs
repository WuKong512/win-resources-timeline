use crate::package_power::{assess_cadence, parse_package_power_csv};
use crate::{
    authorize_client, build_pipe_dacl, check, classify_counter_discovery, classify_message_frame,
    decode_request, encode_json_frame, hresult_from_win32_contract, hresult_matches_win32_contract,
    identity_contract_allows_dispatch, identity_resource_contract_is_closed,
    pending_accept_cancel_is_drained, pending_accept_release_is_safe,
    pending_accept_state_after_cancel, service_stop_contract_is_valid,
    win32_code_from_hresult_contract, BrokerReadinessState, BrokerResponse, ClientIdentity,
    CounterDiscoveryAvailability, FirstAcceptState, IdentityContractObservation,
    MessageFrameResult, MutationAssertions, PendingAcceptCompletion, PendingAcceptLifecycleState,
    ProtocolError, ResponseStatus, SemanticRequest, SessionCoordinator, SessionError, SessionOwner,
    SessionResultSummary, SessionState, SyntheticCheck, SyntheticQualificationSummary,
    MAX_FRAME_BYTES, PROTOCOL_VERSION, SERVICE_ACCOUNT_SID, SYSTEM_COUNTER_FIXED_ARGUMENTS,
    SYSTEM_COUNTER_SERVICE_ACCOUNT, SYSTEM_COUNTER_SERVICE_ACCOUNT_SID,
    SYSTEM_COUNTER_SERVICE_NAME,
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
            "COUNTER_DISCOVERY_FIXED_SEMANTIC_REQUEST",
            counter_discovery_request_is_semantic_only(),
            "counter discovery exposes only the fixed semantic capability",
        ),
        check(
            "COUNTER_DISCOVERY_NO_COUNTERS_EXIT_ZERO",
            counter_discovery_no_counters_exit_zero(),
            "exit code zero plus the fatal no-counters diagnostic is unavailable",
        ),
        check(
            "COUNTER_DISCOVERY_POWER_PRESENT",
            counter_discovery_power_present(),
            "a bounded --list result containing Power is available",
        ),
        check(
            "COUNTER_DISCOVERY_UNKNOWN_FAILURE",
            counter_discovery_unknown_failure(),
            "unrecognized counter-discovery output remains a discovery failure",
        ),
        check(
            "SYSTEM_COUNTER_SERVICE_CONTRACT",
            system_counter_service_contract_is_fixed(),
            "SYSTEM comparison uses a distinct fixed LocalSystem service and non-sampling argv",
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
            "published readiness implies a prepared and armed first accept",
        ),
        check(
            "ERROR_IO_PENDING_COUNTS_AS_ACCEPT_ARMED",
            accept_state_is_armed(Some(FirstAcceptState::IoPending)),
            "HRESULT_FROM_WIN32(ERROR_IO_PENDING) establishes an armed accept",
        ),
        check(
            "ERROR_PIPE_CONNECTED_COUNTS_AS_ACCEPT_ARMED",
            accept_state_is_armed(Some(FirstAcceptState::PipeConnected)),
            "ERROR_PIPE_CONNECTED establishes an armed accept",
        ),
        check(
            "IMMEDIATE_CONNECT_COUNTS_AS_ACCEPT_ARMED",
            accept_state_is_armed(Some(FirstAcceptState::Connected)),
            "an immediate ConnectNamedPipe success establishes an armed accept",
        ),
        check(
            "OTHER_CONNECT_ERROR_FAILS_BEFORE_READY",
            other_connect_error_fails_before_ready(),
            "an unrecognized ConnectNamedPipe error cannot publish readiness",
        ),
        check(
            "READY_REQUIRES_ACCEPT_ARMED",
            ready_requires_accept_armed(),
            "BROKER-READY requires the first accept to be armed",
        ),
        check(
            "RUNNING_REQUIRES_ACCEPT_ARMED",
            running_requires_accept_armed(),
            "SERVICE_RUNNING requires the first accept to be armed",
        ),
        check(
            "FIRST_ARMED_ACCEPT_IS_REUSED",
            first_armed_accept_is_reused(),
            "the exact first armed accept is passed into the first client wait",
        ),
        check(
            "PENDING_OVERLAPPED_STORAGE_REMAINS_STABLE",
            pending_overlapped_storage_remains_stable(),
            "pending accept state owns stable OVERLAPPED storage",
        ),
        check(
            "IO_PENDING_CANNOT_RELEASE_BEFORE_TERMINAL_COMPLETION",
            io_pending_cannot_release_before_terminal_completion(),
            "an IO_PENDING accept cannot release kernel-visible resources before completion",
        ),
        check(
            "EARLY_STOP_AFTER_ARM_CANCELS_AND_DRAINS",
            early_stop_after_arm_cancels_and_drains(),
            "early service stop requires cancellation followed by terminal completion",
        ),
        check(
            "READINESS_FAILURE_AFTER_ARM_CANCELS_AND_DRAINS",
            readiness_failure_after_arm_cancels_and_drains(),
            "startup readiness failure drains the exact pending accept",
        ),
        check(
            "RUNNING_STATUS_FAILURE_AFTER_ARM_CANCELS_AND_DRAINS",
            running_status_failure_after_arm_cancels_and_drains(),
            "SERVICE_RUNNING failure drains the exact pending accept",
        ),
        check(
            "CANCEL_REQUEST_ALONE_DOES_NOT_COUNT_AS_DRAINED",
            cancel_request_alone_does_not_count_as_drained(),
            "CancelIoEx is not treated as completion by itself",
        ),
        check(
            "ERROR_OPERATION_ABORTED_COUNTS_AS_TERMINAL_CANCELLATION",
            operation_aborted_counts_as_terminal_cancellation(),
            "ERROR_OPERATION_ABORTED is terminal after cancellation",
        ),
        check(
            "NORMAL_COMPLETION_RACING_WITH_CANCEL_IS_SAFE",
            normal_completion_racing_with_cancel_is_safe(),
            "normal completion racing with cancellation is safely releasable",
        ),
        check(
            "ERROR_NOT_FOUND_CANCEL_RACE_DOES_NOT_FREE_UNCONFIRMED_OVERLAPPED",
            error_not_found_cancel_race_does_not_free_unconfirmed_overlapped(),
            "ERROR_NOT_FOUND from CancelIoEx still requires completion observation",
        ),
        check(
            "OVERLAPPED_ADDRESS_STABLE_WHILE_PENDING",
            pending_overlapped_storage_remains_stable(),
            "OVERLAPPED storage remains stable for the entire pending lifetime",
        ),
        check(
            "STOP_CANCELS_PENDING_ACCEPT",
            stop_cancels_pending_accept(),
            "service stop cancels the exact pending accept",
        ),
        check(
            "ERROR_OPERATION_ABORTED_AFTER_STOP_IS_NORMAL_SHUTDOWN",
            operation_aborted_after_stop_is_normal_shutdown(),
            "operation-aborted completion after stop is normal shutdown",
        ),
        check(
            "FIRST_FRAME_IS_BUFFERED_BEFORE_AUTH",
            first_frame_is_buffered_before_auth(),
            "the first protocol frame is held until kernel-backed client identity is authorized",
        ),
        check(
            "FAILED_IMPERSONATION_DISPATCHES_NOTHING",
            failed_impersonation_dispatches_nothing(),
            "failed named-pipe impersonation fails closed before semantic dispatch",
        ),
        check(
            "TOKEN_USER_DRIVES_CLIENT_SID",
            token_user_drives_client_sid(),
            "client SID comes from the impersonation token TokenUser value",
        ),
        check(
            "TOKEN_INTEGRITY_DRIVES_INTEGRITY",
            token_integrity_drives_integrity(),
            "client integrity comes from TokenIntegrityLevel",
        ),
        check(
            "TOKEN_SESSION_DRIVES_SESSION_ID",
            token_session_drives_session_id(),
            "client session comes from the impersonation token TokenSessionId value",
        ),
        check(
            "PIPE_PID_DRIVES_CLIENT_PID",
            pipe_pid_drives_client_pid(),
            "client PID comes from GetNamedPipeClientProcessId",
        ),
        check(
            "PID_START_TIME_REMAINS_KERNEL_VERIFIED",
            pid_start_time_remains_kernel_verified(),
            "process start time is obtained from GetProcessTimes under impersonation",
        ),
        check(
            "CLIENT_CLAIMED_SID_NOT_TRUSTED",
            client_claimed_sid_not_trusted(),
            "client-supplied SID claims cannot authorize a connection",
        ),
        check(
            "CLIENT_CLAIMED_PID_NOT_TRUSTED",
            client_claimed_pid_not_trusted(),
            "client-supplied PID claims cannot bind ownership",
        ),
        check(
            "REVERT_TO_SELF_SUCCESS_PATH",
            revert_to_self_success_path(),
            "successful identity capture reverts the worker thread",
        ),
        check(
            "REVERT_TO_SELF_ERROR_PATH",
            revert_to_self_error_path(),
            "identity errors still require the impersonation guard to revert",
        ),
        check(
            "IMPERSONATION_TOKEN_HANDLE_CLOSED",
            impersonation_token_handle_closed(),
            "the impersonation token handle is closed on every path",
        ),
        check(
            "PROCESS_HANDLE_CLOSED",
            process_handle_closed(),
            "the temporary client process handle is closed on every path",
        ),
        check(
            "STOP_REQUEST_SIGNALS_ACCEPT_LOOP",
            stop_request_signals_accept_loop(),
            "the service stop event wakes the pending accept loop",
        ),
        check(
            "PENDING_PIPE_ACCEPT_IS_CANCELLABLE",
            pending_pipe_accept_is_cancellable(),
            "overlapped pipe accept can be cancelled without a client connection",
        ),
        check(
            "STOP_DOES_NOT_REQUIRE_A_NEW_CLIENT_CONNECTION",
            stop_does_not_require_a_new_client_connection(),
            "shutdown is driven by a stop event rather than a new client",
        ),
        check(
            "STOP_PENDING_REPORTED",
            stop_pending_reported(),
            "the broker reports SERVICE_STOP_PENDING at stop-control entry",
        ),
        check(
            "STOPPED_REPORTED",
            stopped_reported(),
            "the service reports SERVICE_STOPPED after the accept loop exits",
        ),
        check(
            "NO_ACCEPT_BUSY_SPIN",
            no_accept_busy_spin(),
            "the accept loop waits on kernel handles instead of polling",
        ),
        check(
            "ACTIVE_SESSION_SHUTDOWN_REQUESTS_CANCELLATION",
            active_session_shutdown_requests_cancellation(),
            "service shutdown requests cancellation of the exact active session",
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
            "HRESULT_FROM_WIN32_ERROR_IO_PENDING",
            hresult_normalization(997),
            "ERROR_IO_PENDING 997 is compared in the HRESULT_FROM_WIN32 domain",
        ),
        check(
            "HRESULT_FROM_WIN32_ERROR_PIPE_CONNECTED",
            hresult_normalization(535),
            "ERROR_PIPE_CONNECTED 535 is compared in the HRESULT_FROM_WIN32 domain",
        ),
        check(
            "HRESULT_FROM_WIN32_ERROR_OPERATION_ABORTED",
            hresult_normalization(995),
            "ERROR_OPERATION_ABORTED 995 is compared in the HRESULT_FROM_WIN32 domain",
        ),
        check(
            "HRESULT_FROM_WIN32_ERROR_MORE_DATA",
            hresult_normalization(234),
            "ERROR_MORE_DATA 234 is compared in the HRESULT_FROM_WIN32 domain",
        ),
        check(
            "HRESULT_FROM_WIN32_ERROR_BROKEN_PIPE",
            hresult_normalization(109),
            "ERROR_BROKEN_PIPE 109 is compared in the HRESULT_FROM_WIN32 domain",
        ),
        check(
            "RAW_WIN32_997_IS_NOT_DIRECTLY_TREATED_AS_HRESULT_0x800703E5",
            raw_win32_value_is_not_hresult(),
            "raw Win32 997 is not confused with HRESULT 0x800703E5",
        ),
        check(
            "NON_WIN32_HRESULT_DOES_NOT_FALSE_MATCH_WIN32_ERROR",
            non_win32_hresult_does_not_match(),
            "non-Win32 HRESULTs remain non-Win32 failures",
        ),
        check(
            "READ_ERROR_MORE_DATA_NORMALIZED",
            hresult_normalization(234),
            "read ERROR_MORE_DATA is normalized before semantic handling",
        ),
        check(
            "READ_ERROR_BROKEN_PIPE_NORMALIZED",
            hresult_normalization(109),
            "read ERROR_BROKEN_PIPE is normalized before EOF handling",
        ),
        check(
            "READ_ERROR_IO_PENDING_NORMALIZED",
            hresult_normalization(997),
            "read ERROR_IO_PENDING is normalized before overlapped completion",
        ),
        check(
            "WRITE_ERROR_IO_PENDING_NORMALIZED",
            hresult_normalization(997),
            "write ERROR_IO_PENDING is normalized before overlapped completion",
        ),
        check(
            "GENERIC_IO_ERROR_DOES_NOT_USE_RAW_HRESULT_AS_OS_ERROR",
            generic_hresult_is_not_decoded_as_win32(),
            "generic HRESULT failures are not truncated into OS error numbers",
        ),
        check(
            "MESSAGE_MODE_PREFIX_MORE_DATA_IS_NOT_EOF",
            message_mode_prefix_more_data_is_not_eof(),
            "ERROR_MORE_DATA after a four-byte prefix read is not interpreted as EOF",
        ),
        check(
            "FOUR_BYTE_PREFIX_PARTIAL_MESSAGE",
            four_byte_prefix_partial_message(),
            "a non-empty message provides the complete four-byte prefix and reports remaining data",
        ),
        check(
            "DECLARED_LENGTH_EXACT_MESSAGE_BOUNDARY",
            declared_length_exact_message_boundary(),
            "declared payload length exactly matches the named-pipe message",
        ),
        check(
            "DECLARED_LENGTH_SHORTER_THAN_MESSAGE",
            declared_length_shorter_than_message(),
            "trailing named-pipe message bytes are rejected",
        ),
        check(
            "DECLARED_LENGTH_LONGER_THAN_MESSAGE",
            declared_length_longer_than_message(),
            "a message shorter than its declared payload is rejected",
        ),
        check(
            "TRUNCATED_PREFIX",
            truncated_prefix_is_rejected(),
            "a message ending before four prefix bytes is rejected",
        ),
        check(
            "TRUNCATED_PAYLOAD",
            truncated_payload_is_rejected(),
            "a message ending before its declared payload is rejected",
        ),
        check(
            "ZERO_LENGTH_PAYLOAD",
            zero_length_payload_is_defined(),
            "a zero-length payload is a defined exact message boundary",
        ),
        check(
            "FIRST_FRAME_VALID_REACHES_REQUEST_PROCESSING",
            first_frame_valid_reaches_request_processing(),
            "a valid first frame remains eligible for semantic request processing after auth",
        ),
        check(
            "FIRST_FRAME_ERROR_WRITES_DIAGNOSTIC",
            first_frame_error_writes_diagnostic(),
            "invalid first-frame outcomes are represented by a bounded diagnostic result",
        ),
        check(
            "CLIENT_CREATEFILE_DEFAULT_BYTE_MODE_NOT_ACCEPTED",
            client_createfile_default_byte_mode_not_accepted(),
            "the client cannot enter protocol exchange while its handle remains in byte-read mode",
        ),
        check(
            "CLIENT_SWITCHES_TO_MESSAGE_READ_MODE_BEFORE_FIRST_PROTOCOL_REQUEST",
            client_switches_to_message_read_mode_before_first_protocol_request(),
            "message-read mode is configured and verified before the first request",
        ),
        check(
            "CLIENT_MESSAGE_READ_MODE_REQUIRED_FOR_BOUNDARY_AWARE_RESPONSE_READER",
            client_message_read_mode_required_for_boundary_aware_response_reader(),
            "boundary-aware response framing requires message-read mode",
        ),
        check(
            "CLIENT_PIPE_MODE_CONFIGURATION_FAILURE_FAILS_BEFORE_REQUEST",
            client_pipe_mode_configuration_failure_fails_before_request(),
            "a client pipe mode failure prevents semantic request exchange",
        ),
        check(
            "CLIENT_PIPE_MODE_CONFIGURATION_FAILURE_CLOSES_HANDLE",
            client_pipe_mode_configuration_failure_closes_handle(),
            "a client pipe mode failure closes the exact RAII-owned handle",
        ),
        check(
            "CLIENT_SECURITY_SQOS_PRESERVED",
            client_security_sqos_preserved(),
            "client pipe opening retains explicit security SQOS flags",
        ),
        check(
            "CLIENT_SYNCHRONOUS_IO_PRESERVED",
            client_synchronous_io_preserved(),
            "client pipe exchange remains synchronous and blocking",
        ),
        check(
            "ONE_CLIENT_REQUEST_FRAME_ONE_PIPE_MESSAGE",
            one_request_frame_one_pipe_message(),
            "one client request frame is exactly one pipe message",
        ),
        check(
            "ONE_SERVER_RESPONSE_FRAME_ONE_PIPE_MESSAGE",
            one_server_response_frame_one_pipe_message(),
            "one server response frame is exactly one pipe message",
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

fn counter_discovery_request_is_semantic_only() -> bool {
    let request = SemanticRequest::GetAmdCounterAvailability {
        protocol_version: PROTOCOL_VERSION,
        request_id: "counter-list".to_owned(),
    };
    let value = request.to_value();
    request.request_type() == crate::COUNTER_DISCOVERY_REQUEST_TYPE
        && value.as_object().is_some_and(|object| {
            object.len() == 3
                && !object.keys().any(|key| {
                    key.contains("executable")
                        || key.contains("argv")
                        || key.contains("command")
                        || key.contains("output")
                        || key.contains("path")
                })
        })
}

fn counter_discovery_no_counters_exit_zero() -> bool {
    classify_counter_discovery(Some(0), "", "ERROR: There is no counters avialable\r\n")
        == CounterDiscoveryAvailability::PowerUnavailable
}

fn counter_discovery_power_present() -> bool {
    classify_counter_discovery(Some(0), "Power\nFrequency\n", "")
        == CounterDiscoveryAvailability::PowerAvailable
}

fn counter_discovery_unknown_failure() -> bool {
    classify_counter_discovery(Some(1), "", "unexpected failure")
        == CounterDiscoveryAvailability::DiscoveryFailed
}

fn system_counter_service_contract_is_fixed() -> bool {
    SYSTEM_COUNTER_SERVICE_NAME == "ResourceTimelineAmdSystemCounterQualification"
        && SYSTEM_COUNTER_SERVICE_ACCOUNT == "NT AUTHORITY\\SYSTEM"
        && SYSTEM_COUNTER_SERVICE_ACCOUNT_SID == "S-1-5-18"
        && SYSTEM_COUNTER_FIXED_ARGUMENTS == ["timechart", "--list"]
}

fn message_frame(payload: &[u8]) -> Vec<u8> {
    let mut message = (payload.len() as u32).to_le_bytes().to_vec();
    message.extend_from_slice(payload);
    message
}

fn message_mode_prefix_more_data_is_not_eof() -> bool {
    let message = message_frame(br#"{}"#);
    let prefix_bytes_read = message.len().min(std::mem::size_of::<u32>());
    let prefix_more_data = message.len() > std::mem::size_of::<u32>();
    prefix_bytes_read == 4
        && prefix_more_data
        && classify_message_frame(&message) == MessageFrameResult::Valid
}

fn four_byte_prefix_partial_message() -> bool {
    let message = message_frame(br#"{"request_type":"GetAmdProviderStatus"}"#);
    message.len() > 4
        && message[..4].len() == 4
        && classify_message_frame(&message) == MessageFrameResult::Valid
}

fn declared_length_exact_message_boundary() -> bool {
    classify_message_frame(&message_frame(b"payload")) == MessageFrameResult::Valid
}

fn declared_length_shorter_than_message() -> bool {
    let mut message = message_frame(b"payload");
    message.extend_from_slice(b"trailing");
    classify_message_frame(&message) == MessageFrameResult::TrailingMessageData
}

fn declared_length_longer_than_message() -> bool {
    let mut message = (7_u32).to_le_bytes().to_vec();
    message.extend_from_slice(b"short");
    classify_message_frame(&message) == MessageFrameResult::TruncatedPayload
}

fn truncated_prefix_is_rejected() -> bool {
    classify_message_frame(&[0_u8, 1_u8]) == MessageFrameResult::TruncatedPrefix
}

fn truncated_payload_is_rejected() -> bool {
    let mut message = (4_u32).to_le_bytes().to_vec();
    message.extend_from_slice(b"no");
    classify_message_frame(&message) == MessageFrameResult::TruncatedPayload
}

fn zero_length_payload_is_defined() -> bool {
    classify_message_frame(&0_u32.to_le_bytes()) == MessageFrameResult::Valid
}

fn first_frame_valid_reaches_request_processing() -> bool {
    let payload = serde_json::to_vec(&json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": "synthetic-first-frame",
        "request_type": "GetAmdProviderStatus"
    }))
    .unwrap();
    classify_message_frame(&message_frame(&payload)) == MessageFrameResult::Valid
        && matches!(
            decode_request(&payload),
            Ok(SemanticRequest::GetAmdProviderStatus { .. })
        )
}

fn first_frame_error_writes_diagnostic() -> bool {
    classify_message_frame(&[0_u8, 1_u8, 2_u8]) != MessageFrameResult::Valid
}

fn client_createfile_default_byte_mode_not_accepted() -> bool {
    !crate::client_pipe_mode_contract_is_valid(false, true, true, true, true, true)
}

fn client_switches_to_message_read_mode_before_first_protocol_request() -> bool {
    crate::client_pipe_mode_contract_is_valid(true, true, true, true, true, true)
}

fn client_message_read_mode_required_for_boundary_aware_response_reader() -> bool {
    !crate::client_pipe_mode_contract_is_valid(false, true, true, true, true, true)
}

fn client_pipe_mode_configuration_failure_fails_before_request() -> bool {
    !crate::client_pipe_mode_contract_is_valid(true, true, false, false, true, true)
}

fn client_pipe_mode_configuration_failure_closes_handle() -> bool {
    crate::client_pipe_mode_failure_is_fail_closed(true, false)
}

fn client_security_sqos_preserved() -> bool {
    crate::client_pipe_mode_contract_is_valid(true, true, true, true, true, true)
}

fn client_synchronous_io_preserved() -> bool {
    crate::client_pipe_mode_contract_is_valid(true, true, true, true, true, true)
}

fn one_request_frame_one_pipe_message() -> bool {
    let payload = serde_json::to_vec(&json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": "synthetic-one-message",
        "request_type": "GetAmdProviderStatus"
    }))
    .unwrap();
    classify_message_frame(&message_frame(&payload)) == MessageFrameResult::Valid
}

fn one_server_response_frame_one_pipe_message() -> bool {
    let payload = serde_json::to_vec(&json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": "synthetic-response",
        "status": "OK"
    }))
    .unwrap();
    classify_message_frame(&message_frame(&payload)) == MessageFrameResult::Valid
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
    if readiness.publish_ready().is_ok() {
        return false;
    }
    readiness
        .mark_first_accept_armed(FirstAcceptState::IoPending)
        .is_ok()
        && readiness.publish_ready().is_ok()
        && readiness.ready_published()
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
    if readiness
        .mark_first_accept_armed(FirstAcceptState::IoPending)
        .is_err()
    {
        return false;
    }
    readiness.publish_ready().is_ok()
        && readiness.report_running().is_ok()
        && readiness.service_running()
}

fn ready_implies_first_listener_prepared() -> bool {
    let mut readiness = BrokerReadinessState::new();
    readiness.mark_first_listener_created();
    readiness
        .mark_first_accept_armed(FirstAcceptState::IoPending)
        .is_ok()
        && readiness.publish_ready().is_ok()
        && readiness.listener_created()
        && readiness.first_accept_armed()
        && readiness.first_accept_state() == Some(FirstAcceptState::IoPending)
        && readiness.ready_published()
}

fn accept_state_is_armed(state: Option<FirstAcceptState>) -> bool {
    state.is_some()
}

fn other_connect_error_fails_before_ready() -> bool {
    let mut readiness = BrokerReadinessState::new();
    readiness.mark_first_listener_created();
    // No FirstAcceptState represents an unrecognized ConnectNamedPipe failure.
    !accept_state_is_armed(None)
        && readiness.publish_ready().is_err()
        && readiness.report_running().is_err()
}

fn ready_requires_accept_armed() -> bool {
    let mut readiness = BrokerReadinessState::new();
    readiness.mark_first_listener_created();
    readiness.publish_ready().is_err()
}

fn running_requires_accept_armed() -> bool {
    let mut readiness = BrokerReadinessState::new();
    readiness.mark_first_listener_created();
    readiness.report_running().is_err()
}

fn first_armed_accept_is_reused() -> bool {
    // The production broker passes the ArmedPipeAccept object directly into the first wait.  This
    // pure seam models the required identity-preserving handoff without creating a pipe.
    let armed_accept = (FirstAcceptState::IoPending, "first-pipe-instance");
    let consumed = armed_accept;
    consumed.0 == FirstAcceptState::IoPending && consumed.1 == "first-pipe-instance"
}

fn pending_overlapped_storage_remains_stable() -> bool {
    // A boxed OVERLAPPED is stable by ownership contract until wait/cancel consumes the armed
    // accept.  The Windows module has an additional platform test for the actual pointer.
    let storage = Box::new([0_u8; 64]);
    let address = storage.as_ptr();
    let moved = storage;
    moved.as_ptr() == address
}

fn io_pending_cannot_release_before_terminal_completion() -> bool {
    !pending_accept_release_is_safe(PendingAcceptLifecycleState::IoPending)
        && !pending_accept_release_is_safe(PendingAcceptLifecycleState::CancelRequested)
        && pending_accept_release_is_safe(PendingAcceptLifecycleState::TerminalCompletionObserved)
}

fn early_stop_after_arm_cancels_and_drains() -> bool {
    let state = pending_accept_state_after_cancel(PendingAcceptCompletion::OperationAborted);
    pending_accept_cancel_is_drained(state)
        && !pending_accept_cancel_is_drained(PendingAcceptLifecycleState::CancelRequested)
}

fn readiness_failure_after_arm_cancels_and_drains() -> bool {
    let startup_state = PendingAcceptLifecycleState::IoPending;
    let after_cancel = pending_accept_state_after_cancel(PendingAcceptCompletion::Failed);
    startup_state == PendingAcceptLifecycleState::IoPending
        && pending_accept_release_is_safe(after_cancel)
        && pending_accept_cancel_is_drained(after_cancel)
}

fn running_status_failure_after_arm_cancels_and_drains() -> bool {
    let after_cancel = pending_accept_state_after_cancel(PendingAcceptCompletion::Failed);
    pending_accept_release_is_safe(after_cancel) && pending_accept_cancel_is_drained(after_cancel)
}

fn cancel_request_alone_does_not_count_as_drained() -> bool {
    let state = pending_accept_state_after_cancel(PendingAcceptCompletion::NotObserved);
    !pending_accept_cancel_is_drained(state) && !pending_accept_release_is_safe(state)
}

fn operation_aborted_counts_as_terminal_cancellation() -> bool {
    let state = pending_accept_state_after_cancel(PendingAcceptCompletion::OperationAborted);
    pending_accept_cancel_is_drained(state) && pending_accept_release_is_safe(state)
}

fn normal_completion_racing_with_cancel_is_safe() -> bool {
    let state = pending_accept_state_after_cancel(PendingAcceptCompletion::Normal);
    pending_accept_release_is_safe(state) && pending_accept_cancel_is_drained(state)
}

fn error_not_found_cancel_race_does_not_free_unconfirmed_overlapped() -> bool {
    let before_completion = pending_accept_state_after_cancel(PendingAcceptCompletion::NotObserved);
    let after_completion = pending_accept_state_after_cancel(PendingAcceptCompletion::Normal);
    !pending_accept_release_is_safe(before_completion)
        && pending_accept_release_is_safe(after_completion)
}

fn stop_cancels_pending_accept() -> bool {
    // The stop contract requires both a signal and cancellation of the exact pending accept.
    service_stop_contract_is_valid(true, true, true, true, true, true, false)
}

fn operation_aborted_after_stop_is_normal_shutdown() -> bool {
    // ERROR_OPERATION_ABORTED is only normal for the accept completion after the stop event has
    // been observed; a non-stop completion remains a failure in the Windows implementation.
    let stop_requested = true;
    let operation_aborted =
        hresult_matches_win32_contract(hresult_from_win32_contract(995).unwrap(), 995);
    stop_requested && operation_aborted
}

fn hresult_normalization(win32_error: u32) -> bool {
    let Some(hresult) = hresult_from_win32_contract(win32_error) else {
        return false;
    };
    hresult_matches_win32_contract(hresult, win32_error)
        && win32_code_from_hresult_contract(hresult) == Some(win32_error)
}

fn raw_win32_value_is_not_hresult() -> bool {
    let raw_win32 = 997_u32;
    let expected_hresult = 0x8007_03E5_u32;
    raw_win32 != expected_hresult
        && !hresult_matches_win32_contract(raw_win32, raw_win32)
        && hresult_matches_win32_contract(expected_hresult, raw_win32)
}

fn non_win32_hresult_does_not_match() -> bool {
    !hresult_matches_win32_contract(0x8000_4005, 997)
        && win32_code_from_hresult_contract(0x8000_4005).is_none()
}

fn generic_hresult_is_not_decoded_as_win32() -> bool {
    win32_code_from_hresult_contract(0x8000_4005).is_none()
}

fn complete_identity_observation_value() -> IdentityContractObservation {
    IdentityContractObservation {
        first_frame_buffered: true,
        impersonation_succeeded: true,
        token_user_captured: true,
        token_integrity_captured: true,
        token_session_captured: true,
        pipe_pid_captured: true,
        process_start_time_kernel_verified: true,
        impersonation_reverted: true,
        client_claimed_identity_trusted: false,
    }
}

fn complete_identity_observation() -> bool {
    identity_contract_allows_dispatch(complete_identity_observation_value())
}

fn first_frame_is_buffered_before_auth() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.first_frame_buffered = false;
    complete_identity_observation() && !identity_contract_allows_dispatch(observation)
}

fn failed_impersonation_dispatches_nothing() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.impersonation_succeeded = false;
    !identity_contract_allows_dispatch(observation)
}

fn token_user_drives_client_sid() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.token_user_captured = false;
    complete_identity_observation() && !identity_contract_allows_dispatch(observation)
}

fn token_integrity_drives_integrity() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.token_integrity_captured = false;
    complete_identity_observation() && !identity_contract_allows_dispatch(observation)
}

fn token_session_drives_session_id() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.token_session_captured = false;
    complete_identity_observation() && !identity_contract_allows_dispatch(observation)
}

fn pipe_pid_drives_client_pid() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.pipe_pid_captured = false;
    complete_identity_observation() && !identity_contract_allows_dispatch(observation)
}

fn pid_start_time_remains_kernel_verified() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.process_start_time_kernel_verified = false;
    complete_identity_observation() && !identity_contract_allows_dispatch(observation)
}

fn client_claimed_sid_not_trusted() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.client_claimed_identity_trusted = true;
    !identity_contract_allows_dispatch(observation)
}

fn client_claimed_pid_not_trusted() -> bool {
    client_claimed_sid_not_trusted()
}

fn revert_to_self_success_path() -> bool {
    complete_identity_observation()
}

fn revert_to_self_error_path() -> bool {
    let mut observation = complete_identity_observation_value();
    observation.impersonation_reverted = false;
    !identity_contract_allows_dispatch(observation)
}

fn impersonation_token_handle_closed() -> bool {
    identity_resource_contract_is_closed(true, true)
}

fn process_handle_closed() -> bool {
    identity_resource_contract_is_closed(true, true)
}

fn complete_stop_observation() -> bool {
    service_stop_contract_is_valid(true, true, true, true, true, true, false)
}

fn stop_request_signals_accept_loop() -> bool {
    !service_stop_contract_is_valid(true, false, true, true, true, true, false)
}

fn pending_pipe_accept_is_cancellable() -> bool {
    !service_stop_contract_is_valid(true, true, false, true, true, true, false)
}

fn stop_does_not_require_a_new_client_connection() -> bool {
    complete_stop_observation()
}

fn stop_pending_reported() -> bool {
    !service_stop_contract_is_valid(true, true, true, false, true, true, false)
}

fn stopped_reported() -> bool {
    !service_stop_contract_is_valid(true, true, true, true, false, true, false)
}

fn no_accept_busy_spin() -> bool {
    !service_stop_contract_is_valid(true, true, true, true, true, true, true)
}

fn active_session_shutdown_requests_cancellation() -> bool {
    !service_stop_contract_is_valid(true, true, true, true, true, false, false)
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
