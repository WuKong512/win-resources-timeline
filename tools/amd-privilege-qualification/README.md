# AMD privilege and secure IPC qualification

This is an independent, qualification-only Windows Service Broker artifact for
`AMD-PRIVILEGE-I2`. It is not the Resource Timeline collector, production
provider, installer, autostart path, or database writer.

The automated path is completely synthetic. `--synthetic` exercises the
versioned semantic protocol, bounded framing, explicit pipe-DACL policy,
client identity authorization, one-session arbitration, ownership,
cancellation, disconnect cleanup, timeout, malformed input, and the existing
header-driven package-power parser fixture. It launches only a harmless child
of the qualification binary and reports:

```text
AMD_RUNTIME_EXECUTED = false
SERVICE_REGISTRATION_COUNT = 0
SCHEDULED_TASK_REGISTRATION_COUNT = 0
SELF_ELEVATION_PERFORMED = false
AMD_INSTALLATION_MUTATED = false
AMD_REGISTRY_MUTATED = false
```

The future broker path is explicit and narrow:

```text
standard-user client
    -> scoped Windows named pipe with explicit DACL
    -> LocalService broker + Service SID
    -> broker-derived AMD installation path and fixed semantic capability
    -> broker-owned ProgramData output
    -> validated package-power parser
    -> typed response over the pipe
```

The request schema contains only:

```text
GetAmdProviderStatus
GetAmdCounterAvailability
StartAmdPowerSession { duration_ms, interval_ms }
GetAmdSessionStatus { session_id }
CancelAmdSession { session_id }
```

The broker rejects unknown fields, unknown request types, incompatible
protocol versions, oversized messages, invalid bounds, stale session IDs, and
non-owner cancellation. It never accepts an executable path, raw argv, shell
command, working directory, environment, registry path, or output path.

`GetAmdCounterAvailability` is a qualification-only, non-sampling capability.
The broker derives the validated AMD CLI and executes exactly `timechart --list`
with broker-owned stdout/stderr, a bounded timeout, and a kill-on-job-close job.
It never accepts a command or argv from the client and never starts a power
timechart. The prepared standard-user handoff is
`run-standard-user-amd-counter-discovery.ps1`; it is not run by automated tests.

## Synthetic qualification

From the repository root:

```powershell
pwsh -NoProfile -File .\tools\amd-privilege-qualification\test-qualification.ps1
```

This command does not register a service, request elevation, access the AMD
installation, or run AMD uProf.

## Historical I2 power-sampling qualification path

The following path is retained as historical I2 qualification infrastructure.
It consumed the one bounded real power-sampling gate and is not the I2B
counter-discovery handoff:

`run-standard-user-amd-privilege-client.ps1` = **I2 POWER-SAMPLING CLIENT**
**NOT THE I2B COUNTER-DISCOVERY HANDOFF**; **DO NOT RUN DURING I2B
DIFFERENTIAL**.

1. An Administrator x64 PowerShell runs `run-admin-amd-privilege-qualification.ps1`.
   It verifies the exact x64 release artifact and SHA-256, registers the fixed
   service as `NT AUTHORITY\LocalService`, enables `UNRESTRICTED` Service SID,
   preflights the registry-derived x64 AMD CLI with valid AMD Authenticode,
   creates the scoped config/output ACLs, and starts the broker. This preflight
   reads metadata only and does not start AMD uProf.
2. A normal, non-elevated x64 PowerShell runs
    `run-standard-user-amd-privilege-client.ps1`. It sends only the semantic
    status/start/status requests and holds one pipe connection for the bounded
    normal-completion run.
3. The Administrator runs `cleanup-admin-amd-privilege-qualification.ps1`.
   It stops and deletes only the fixed qualification service and records
   cleanup evidence. Qualification evidence is retained for audit; AMD
   binaries, drivers, registry installation state, and production data are not
   touched.

The real run is at most one bounded `LocalService + Service SID + Session 0`
AMD package-power session. Cancellation is qualified synthetically and is not
performed against a real AMD runtime.

## I2B human handoff: non-sampling counter discovery

This is the only authorized LocalService client sequence for the I2B
differential. It is **NON_SAMPLING**, sends only
`GetAmdCounterAvailability`, and lets the broker derive the fixed
`AMDuProfCLI.exe timechart --list` command. It does not request a power event,
duration, interval, or CSV session.

Run these commands manually, exactly once per stated shell, only after human
authorization. They are not executed by this repair or by synthetic tests:

```powershell
# Administrator x64 PowerShell — setup only; starts no AMD runtime itself
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-privilege-qualification.ps1'

# Normal, non-elevated, medium-integrity x64 PowerShell — I2B only
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-standard-user-amd-counter-discovery.ps1'

# Administrator x64 PowerShell — cleanup after the bounded handoff
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\cleanup-admin-amd-privilege-qualification.ps1'
```

The SYSTEM comparison is prepared as a separate qualification-only service
path below, but remains `NOT_EXECUTED / HUMAN_AUTHORIZATION_REQUIRED`. It does
not mutate the LocalService broker contract, switch the production account, or
authorize a SYSTEM fallback for production.

The fixed two-context plan is retained at
`counter-discovery-differential-plan.json`; its SYSTEM entry is prepared until
separately authorized.

## HISTORICAL / SUPERSEDED I2C human handoff: SYSTEM counter-discovery comparison

The LocalService differential side is complete real evidence at scope
`4b30b3d64b7e469cbce7c8080c84b7d4`: the fixed non-sampling
`AMDuProfCLI.exe timechart --list` operation reported
`POWER_UNAVAILABLE`. Do not rerun the LocalService side. The operator's
duplicate cleanup invocation overwrote only the single cleanup summary; the
discovery evidence and run validity remain preserved.

I2C prepared one isolated SYSTEM comparison. It used the distinct fixed
service `ResourceTimelineAmdSystemCounterQualification`, `LocalSystem`
(`S-1-5-18`), Session 0, x64, a Service SID, broker-derived AMD CLI
discovery, and the fixed arguments `timechart --list`. It has no named-pipe
client, no sampling request, no arbitrary command surface, and no production
integration. Setup and discovery are intentionally coupled: starting the
dedicated service performs the one fixed non-sampling discovery operation.
The one authorized comparison completed at scope
`091a72e1d38341ca9eca0877b1625082` and reported `POWER_AVAILABLE`. Do not run
the commands below again; they are retained as the historical execution shape
only:

```powershell
# HISTORICAL command shape — already consumed; do not rerun
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-system-counter-qualification.ps1'

# No standard-user client and no named-pipe IPC are used for the SYSTEM comparison.

# Historical cleanup command shape — already consumed; do not rerun
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\cleanup-admin-amd-system-counter-qualification.ps1'
```

The completed SYSTEM run remained isolated in its own evidence root and
produced
`SYSTEM-SERVICE-CONTEXT.json`, `CLI-ARTIFACT-IDENTITY.json`,
`AMD-COUNTER-DISCOVERY-LAUNCH.json`,
`AMD-COUNTER-DISCOVERY-RESULT.json`, bounded stdout/stderr, and unique
`SYSTEM-CLEANUP-RESULT-<timestamp>-<id>.json` cleanup evidence. The cleanup
wrapper never overwrote a previous cleanup attempt and never killed an
unrelated process. The SYSTEM result informs the privilege differential; it
does not select LocalSystem as the production account.

```text
SYSTEM_COUNTER_DISCOVERY = REAL_POWER_AVAILABLE
SYSTEM_SCOPE = 091a72e1d38341ca9eca0877b1625082
SYSTEM_CLEANUP = PASS
SYSTEM_SERVICE_REGISTRATION_FINAL = absent
SYSTEM_BROKER_PROCESS_FINAL = 0
SYSTEM_REAL_RUN_CONSUMED = true
```

## Frozen future-run artifact

The completed LocalService real run used the historical artifact below. Its
hash remains immutable in the evidence and is not rewritten by the I2C build:

```text
LOCAL_SERVICE_REAL_ARTIFACT_SHA256 = C9973BAAA01AF3C2673D8C70D8C7E626C577642505E6DFF7BA3C6026DEA63FB1
LOCAL_SERVICE_ARTIFACT_STATUS = historical / used by completed LocalService run
```

The dedicated SYSTEM comparison wrapper is pinned to the offline release
artifact used by the completed SYSTEM comparison. Changing that binary
requires rebuilding and recording a new hash before any future comparison:

```text
path = tools/amd-privilege-qualification/target/release/amd-privilege-qualification.exe
architecture = x64
build_mode = release / cargo --offline
sha256 = 9E5A012B0A95C84DD28CD607D99EF43C9BC4D700683F33890CDE6C2108794AC3
system_wrapper = run-admin-amd-system-counter-qualification.ps1
system_artifact_status = historical artifact used by completed SYSTEM comparison
broker_authenticode = NotSigned; exact SHA-256 is required by the SYSTEM wrapper
```

The existing LocalService wrappers remain pinned to the historical C9973...
hash and therefore fail closed against the SYSTEM-comparison binary; do not
use them to retry the consumed LocalService run. The older sampling wrapper
must not be substituted for either counter-discovery path:

`run-standard-user-amd-privilege-client.ps1` remains preserved as the
historical **I2 POWER-SAMPLING CLIENT**, not an active I2B command.

## I2E service-SID SeSystemProfile minimum-variable experiment

I2E prepared a paired control/treatment experiment against the real LocalService
counter-availability differential. Its contract keeps the
service account as NT AUTHORITY\LOCAL SERVICE (S-1-5-19), uses the fixed
qualification-only service ResourceTimelineAmdSystemProfileQualification, and
uses one derived Service SID for both phases. The Service SID type must be
UNRESTRICTED.

The control is LocalService plus that Service SID with
SeSystemProfilePrivilege absent. The treatment is the same service, account,
machine, Session 0, x64 artifact, AMD CLI, working directory, timeout, job
policy, classifier, and fixed command, with exactly one change:
SeSystemProfilePrivilege is assigned to the dedicated Service SID. The
experiment never assigns the right to the global LocalService account, never
adds Administrators membership, and never adds SeProfileSingleProcessPrivilege,
SeDebugPrivilege, SeTcbPrivilege, or another SYSTEM-only privilege.

The default administrator command is deliberately plan-only:

~~~powershell
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-i2e-service-profile-experiment.ps1'
~~~

It performs no service registration, LSA mutation, or AMD invocation. A
future human-authorized execution would require the explicit
ExecuteAuthorizedExperiment switch and would consume the paired control /
treatment gate, so it must not be run during ordinary documentation or
synthetic validation. The future fixed operation is non-sampling
AMDuProfCLI.exe timechart --list; no power event, duration, interval, or CSV
sampling is allowed.

The token-materialization gate requires LocalService, Session 0, x64, the
expected Service SID, no Administrators SID, SeSystemProfilePrivilege present
and enabled only in treatment, and no newly introduced
SeProfileSingleProcessPrivilege or SeDebugPrivilege. Before any future right
mutation, the direct Service SID rights are read and the experiment fails
closed if the exact right already exists. Rollback removes only the exact
right when the mutation journal proves this experiment added it; it never
uses an all-rights removal. Cleanup evidence is invocation-distinct and can
recover from service creation or any later failure point.

The LSA policy handle is operation-specific and least-privilege: read-only
enumeration uses `0x00000801`, first-time exact-right assignment uses
`0x00000810` (`POLICY_CREATE_ACCOUNT` plus name lookup), and exact-right
rollback uses only `0x00000800` (name lookup). The direct account baseline also
records whether the Service SID account object is `PRESENT`, `ABSENT`, or
`UNKNOWN`; this is diagnostic and does not replace the exact-right presence
gate.

The fixed artifact SHA is recorded in the I2E wrapper and is used unchanged
for both phases. I2E is prepared but not executed:

~~~text
FROZEN_EXPERIMENT_ARTIFACT_SHA256 = 871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9
I2E = CONTROL_REAL_COMPLETE_TREATMENT_PENDING
LOCAL_SERVICE_ACCOUNT_WIDE_RIGHT_MUTATION = FORBIDDEN
ADMINISTRATORS_MEMBERSHIP_MUTATION = FORBIDDEN
REAL_LSA_MUTATION_DURING_PREPARATION = 0
REAL_AMD_RUNTIME_DURING_PREPARATION = 0
NEXT_GATE = HUMAN_I2E_TREATMENT_ONLY_RESUME
PRODUCTION_ACCOUNT = UNRESOLVED
~~~

## Current I2E status: pre-control harness incident

The first human-authorized I2E invocation failed before Service creation:
`sc.exe create` returned `1057` because the SCM-facing account value was the
bare `LocalService` string. This did not execute the control or treatment, add
the right, or run AMD. The wrapper now uses the exact Windows SCM identity
`NT AUTHORITY\LocalService` and records explicit gate state in the mutable
`I2E-EXPERIMENT-CURRENT.json` pointer.

The failed attempt must first be closed by the human with the repaired cleanup
wrapper. Cleanup preserves an invocation-distinct `I2E-EXPERIMENT-FINAL-*.json`
inside the experiment evidence root, verifies that no exact right was added (or
that an experiment-added right was exactly rolled back), proves owned service
and process absence, and only then removes the top-level CURRENT pointer. Do
not delete the pointer or evidence manually, and do not rerun the experiment
before cleanup.

```text
I2E_FIRST_REAL_ATTEMPT = PRE_SERVICE_CREATE_FAILURE
SC_CREATE_EXIT = 1057
SCM_SERVICE_ACCOUNT = NT AUTHORITY\LocalService
CONTROL_EXECUTED = false
TREATMENT_EXECUTED = false
LSA_MUTATION = false
AMD_RUNTIME = false
PAIRED_EXPERIMENT_GATE = UNCONSUMED
HISTORICAL_NEXT_GATE_AT_FIRST_INCIDENT = HUMAN_I2E_FAILED_ATTEMPT_CLEANUP
HISTORICAL_AFTER_CLEANUP_NEXT_GATE = HUMAN_SERVICE_SID_SESYSTEMPROFILE_EXPERIMENT_EXECUTION
```

## Current I2E status: CONTROL complete, treatment-only resume prepared

The first corrected human I2E invocation reached and completed the real
LocalService CONTROL phase. The authoritative result is `POWER_UNAVAILABLE`;
the token gate passed, the fixed non-sampling `timechart --list` command ran,
and no orphan child remained. The orchestration then failed while stopping
the already-completed service because a local PowerShell `$pid` assignment
collided with the read-only, case-insensitive `$PID` automatic variable. The
service is currently `Stopped / PID0 / LocalService`, and no right mutation or
treatment execution occurred.

```text
I2E_SECOND_HUMAN_INVOCATION = CONTROL_REAL_EXECUTED_THEN_ORCHESTRATION_STOP_FAILURE
CONTROL_REAL_EXECUTION = REAL_COMPLETE
CONTROL_RESULT = POWER_UNAVAILABLE
CONTROL_TOKEN_GATE = PASS
CONTROL_NO_ORPHAN_CHILD = true
TREATMENT_REAL_EXECUTION = 0
LSA_MUTATION = 0
ROOT_CAUSE = POWERSHELL_AUTOMATIC_VARIABLE_PID_COLLISION
CURRENT_POINTER_STATE = STALE_AFTER_POST_CONTROL_STOP_FAILURE
CURRENT_SERVICE = STOPPED / PID0 / LocalService
PAIRED_GATE_CONSUMED = true
CONTROL_RECOVERY = PREPARED
CONTROL_RERUN = FORBIDDEN
TREATMENT_ONLY_RESUME = PREPARED
NEXT_REAL_AMD_OPERATION = TREATMENT_ONLY
NEXT_GATE = HUMAN_I2E_TREATMENT_ONLY_RESUME
```

Do not trust the stale CURRENT flags over the phase evidence and do not rerun
CONTROL. The future human gate is the explicit treatment-only wrapper
`resume-admin-amd-i2e-treatment.ps1`; it first validates the exact existing
experiment, writes `CONTROL-RECOVERY.json`, and reconciles the pointer. It
then requires the existing stopped service and absent right, applies exactly
one right to the same Service SID, enforces the treatment token gate before
AMD, and performs exact rollback plus cleanup. This repair performed no
recovery, cleanup, service start, LSA mutation, or AMD runtime.

The next human gate is an already elevated Administrator x64 PowerShell and
the treatment-only command below. It is intentionally not run as part of this
repair; it consumes the remaining treatment gate and must never be replaced by
the original control wrapper:

```powershell
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\resume-admin-amd-i2e-treatment.ps1' -ExecuteAuthorizedTreatmentOnly
```

### Treatment-resume evidence layout closure

The treatment-only resume reads the immutable CONTROL discovery evidence from
the actual phase root. It does not create or search a nested
`counter-discovery` directory:

```text
CONTROL_ROOT\AMD-COUNTER-DISCOVERY-RESULT.json
CONTROL_ROOT\AMD-COUNTER-DISCOVERY-LAUNCH.json
```

`TREATMENT_RESUME_CONTROL_DISCOVERY_PATH_DRIFT = CLOSED`. The resume fails
closed when either direct-root discovery file is absent, and synthetic tests
reject a nested-only layout. Historical CONTROL evidence is not moved or
rewritten; the next gate remains the treatment-only human invocation above.

### PR #22 treatment pre-run review closure

The two confirmed PR blockers are now closed offline. The real CONTROL phase
remains immutable authoritative evidence (`POWER_UNAVAILABLE`); it was not
rerun, and TREATMENT remains pending human authorization.

```text
PR_22_REVIEW = CLOSED_OFFLINE
BLOCKER_1 = TREATMENT_CURRENT_AMD_CLI_IDENTITY_NOT_REVALIDATED
BLOCKER_1_STATUS = CLOSED_OFFLINE
CONTROL_AMD_CLI_PREFLIGHT = PRESERVED
TREATMENT_AMD_CLI_REVALIDATION = PREPARED_BEFORE_LSA_MUTATION
AMD_CLI_PATH_MATCH_REQUIRED = true
AMD_CLI_SHA256_MATCH_REQUIRED = true
AMD_CLI_ARCHITECTURE_MATCH_REQUIRED = true
AMD_CLI_SIGNATURE_VALID_REQUIRED = true
AMD_CLI_SIGNER_MATCH_REQUIRED = true
AMD_IDENTITY_GATE_BEFORE_LSA_MUTATION = PASS
BLOCKER_2 = ROLLBACK_POLICY_VERIFICATION_CAN_PRECEDE_EFFECTIVE_TOKEN_TEARDOWN
BLOCKER_2_STATUS = CLOSED_OFFLINE
POLICY_ROLLBACK_VERIFIED_SEPARATE = PASS
EFFECTIVE_TOKEN_TEARDOWN_VERIFIED_SEPARATE = PASS
FULL_SECURITY_ROLLBACK = PASS
FULL_ROLLBACK_REQUIRES_SERVICE_PID0 = true
FULL_ROLLBACK_REQUIRES_OWNED_PROCESS_ABSENCE = true
FULL_ROLLBACK_REQUIRES_LSA_DUAL_READBACK = true
FAILED_STOP_DOES_NOT_CLAIM_FULL_ROLLBACK = PASS
CONTROL_RERUN = FORBIDDEN
CONTROL_REAL_EXECUTION = REAL_COMPLETE
CONTROL_RESULT = POWER_UNAVAILABLE
TREATMENT_REAL_EXECUTION = 0
REAL_LSA_MUTATION_DURING_REPAIR = 0
REAL_SERVICE_RUNTIME_DURING_REPAIR = 0
REAL_AMD_RUNTIME_DURING_REPAIR = 0
FROZEN_QUALIFICATION_ARTIFACT_SHA256 = 871CD20D228BD9510606DE640F516F62C2983B9F4A83C1AA807BA35329C778B9
ARTIFACT_CHANGED = false
NEXT_GATE = HUMAN_I2E_TREATMENT_ONLY_RESUME_REVIEW
```

Before any future mutation, the current registry-derived AMD CLI identity is
read again and compared with the immutable CONTROL preflight: path,
installation root, SHA-256, x64 architecture, valid signature, AMD signer,
signature subject, and issuer must match. A mismatch writes
`TREATMENT-AMD-CLI-PREFLIGHT.json` and stops before LSA mutation, service start,
or AMD execution.

Rollback is explicit and stop-first. `policy_rollback_verified` means the
exact Service SID right is absent in both LSA directions;
`effective_token_teardown_verified` means the service is absent or
`Stopped / PID0` and owned broker/AMD CLI processes are gone; only their
conjunction sets `full_rollback_verified` and the compatibility
`rollback_verified` field. Failed stop or policy verification retains the
CURRENT pointer and cannot finalize the experiment.

### PR #22 partial rollback retry closure

Cleanup retry semantics use `policy_rollback_verified` to decide whether an
LSA removal is still required. When that policy state is already verified,
cleanup performs fresh read-only direct-right and assignment readback,
records `policy_remove_skipped_reason = ALREADY_VERIFIED_REMOVED`, and issues
no duplicate `LsaRemoveAccountRights` call. Unexpected policy-state drift
fails closed. The full rollback compatibility field remains
`rollback_verified == full_rollback_verified`.

Active I2E AMD CLI process ownership is derived from the pinned
`AMD-CLI-PREFLIGHT.json` identity for the experiment. A missing or failed
preflight fails closed; no machine-specific fallback path is used.

```text
PR22_PARTIAL_ROLLBACK_RETRY = CLOSED_OFFLINE
POLICY_ROLLBACK_RETRY_SEMANTICS = policy_rollback_verified controls LSA re-removal
FULL_ROLLBACK_COMPATIBILITY_FIELD = rollback_verified == full_rollback_verified
ALREADY_REMOVED_RIGHT_RETRY = READ_ONLY_REVERIFY / NO_DUPLICATE_REMOVE
POLICY_STATE_DRIFT = FAIL_CLOSED
AMD_CLI_OWNERSHIP_PATH = PINNED_PREFLIGHT_DERIVED
HARD_CODED_AMD_CLI_PATH = REMOVED_FROM_ACTIVE_I2E_OWNERSHIP
```

### PR #22 I2E historical pointer schema compatibility closure

The latest authorized treatment-only attempt failed before LSA mutation when
an old JSON CURRENT pointer was deserialized as a `PSCustomObject` that lacked
new rollback fields. The resume path now updates historical pointers through a
canonical set-or-add helper, and persists the pre-mutation state before any
exact Service SID right can be added. Cleanup first re-reads both LSA
directions; an already-absent right is marked recovered without issuing a
duplicate removal, while state drift or unavailable readback fails closed.

```text
I2E_TREATMENT_ATTEMPT = PRE_MUTATION_ORCHESTRATION_FAILURE
ROOT_CAUSE = HISTORICAL_PSCUSTOMOBJECT_SCHEMA_EVOLUTION_UNSAFE_DIRECT_PROPERTY_ASSIGNMENT
CONTROL_RECOVERY = REAL_PERSISTED
CONTROL_RESULT = POWER_UNAVAILABLE
AMD_CLI_REVALIDATION = REAL_READ_ONLY_PASS
LSA_MUTATION = 0
SERVICE_START = 0
TREATMENT_RUNTIME = 0
AMD_RUNTIME = 0
SET_OR_ADD_PROPERTY_HELPER = PASS
HISTORICAL_POINTER_FIXTURE = PASS
POINTER_PERSIST_BEFORE_LSA_ADD = PASS
POST_REMOVE_PRE_POINTER_CRASH_RECOVERY = PASS
PRE_REMOVE_DUAL_READBACK = PASS
POLICY_STATE_DRIFT = FAIL_CLOSED
CONTROL_RERUN = FORBIDDEN
TREATMENT = PENDING_HUMAN_AUTHORIZATION
```

No service, LSA mutation, or AMD runtime was performed by this closure.

## I2D read-only minimum-capability forensics

> HISTORICAL / SUPERSEDED NEXT-GATE SNAPSHOT: I2D read-only evidence
> collection is complete. The active preparation is I2E above.

I2D compares the completed LocalService result with the completed SYSTEM
result. It does not execute AMD, open a device, register a service, or mutate
ACLs, privileges, policy, or production configuration. The repository-native
helper is read-only and emits JSON to the console; it reports protected
evidence as unavailable rather than attempting recovery:

```powershell
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\i2d-readonly-forensics.ps1'
```

The helper inventories fixed AMD service/file metadata and service DACLs,
opens the LSA policy with exactly
`POLICY_VIEW_LOCAL_INFORMATION | POLICY_LOOKUP_NAMES = 0x00000801`, records
raw NTSTATUS plus `LsaNtStatusToWinError`, cross-checks direct rights for
LocalService, SYSTEM, and Administrators with `LsaEnumerateAccountRights`,
and compares normalized token evidence when the two context JSON files are
readable. It never invokes
`AMDuProfCLI.exe`, `sc.exe create/start/stop/delete`, `sc.exe sdset`,
`Set-Acl`, or an LSA privilege-assignment API. Current classification is
`MINIMUM_REQUIRED_CAPABILITY = UNRESOLVED`; the next gate is
`HUMAN_ELEVATED_READ_ONLY_I2D_EVIDENCE_COLLECTION`.

## AMD-PRIVILEGE-I2E real closure / I2F self-enable preparation

The real I2E treatment answered the Windows token-materialization question once.
The dedicated Service SID right assignment passed dual LSA verification and
materialized `SeSystemProfilePrivilege` in the LocalService service token, but
the privilege was `PRESENT + DISABLED`. The in-service token gate therefore
failed before AMD; this is not a counter-backend result and I2E must not be
rerun.

```text
I2E_RESULT = PASS_WITH_NEGATIVE_TOKEN_ENABLEMENT_RESULT
I2E_CONTROL_RESULT = POWER_UNAVAILABLE
I2E_TOKEN_PRIVILEGE = PRESENT_DISABLED
I2E_AMD_RUNTIME = 0
I2E_COUNTER_DISCOVERY = NOT_EXECUTED
I2E_FULL_ROLLBACK = REAL_PASS
I2E_SERVICE_REMOVED = true
I2E_RESIDUAL_RIGHT = ABSENT
I2E_RERUN = FORBIDDEN
```

I2F is the next human-authorized qualification gate. It uses the same
LocalService security model with a fresh dedicated unrestricted Service SID.
The only intentional runtime variable is native
`AdjustTokenPrivileges(SeSystemProfilePrivilege)` from `DISABLED` to
`ENABLED` in the qualification service's own process token. The service must
capture `I2F-TOKEN-BEFORE-ENABLE.json`,
`I2F-ADJUST-TOKEN-PRIVILEGES.json`, `I2F-TOKEN-AFTER-ENABLE.json`, and
`I2F-TOKEN-ENABLE-DELTA.json`; `ERROR_NOT_ALL_ASSIGNED` or any unexplained
delta fails closed before AMD.

The fixed operation is non-sampling:

```text
I2F_COMMAND = timechart --list
I2F_SAMPLING = false
I2F_SERVICE = ResourceTimelineAmdSystemProfileEnableQualification
I2F_ACCOUNT = NT AUTHORITY\LocalService
I2F_ACCOUNT_SID = S-1-5-19
I2F_ARTIFACT_SHA256 = F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D
I2F_ARTIFACT_ARCHITECTURE = x64
I2F_STATUS = PREPARED / NOT_EXECUTED
NEXT_GATE = HUMAN_I2F_SELF_ENABLE_QUALIFICATION_REVIEW

## I2F PRE-RUN ROLLBACK SAFETY REVIEW CLOSURE

I2F_ROLLBACK_STOP_FIRST = PASS_STATIC
I2F_PROCESS_EVIDENCE_UNKNOWN_NOT_ZERO = PASS
I2F_POLICY_REMOVE_AFTER_TOKEN_TEARDOWN_ONLY = PASS_STATIC
I2F_ROLLBACK_PARTIAL_FAILURE_EVIDENCE = PASS
I2F_STANDALONE_CLEANUP_IDEMPOTENT = PASS_STATIC
I2F_RUST_PRE_ENABLE_TO_AMD_ORDER = PASS
I2F_HUMAN_RUNTIME = NOT_EXECUTED
I2F_REAL_LSA_MUTATION = 0
I2F_REAL_SERVICE_RUNTIME = 0
I2F_REAL_TOKEN_ADJUSTMENT = 0
I2F_REAL_AMD_RUNTIME = 0

The I2F cleanup path stops the qualification service first, verifies Stopped/PID0
and exact pinned owned-process absence, then performs pre-remove dual LSA
readback. A failed stop or failed process enumeration leaves policy rollback
closed and records nullable process counts; unknown is never represented as zero.
The rollback evidence writer accepts partial/error-only state so cleanup failures
remain durable. Service registration removal is allowed only after effective
token teardown and exact policy rollback have both been verified.

I2F_ARTIFACT_SHA256 = F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D
I2F_ARTIFACT_CHANGED = false
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
NEXT_GATE = HUMAN_I2F_SELF_ENABLE_QUALIFICATION_REVIEW
```

## PR22 I2E Resume wrapper-global restoration closure

The shared-library extraction exposed a deterministic StrictMode regression in
the treatment-resume executable because its experiment-specific state was no
longer inherited from the executable I2E wrapper. The Resume wrapper now owns
its service/account/Service-SID identity, frozen artifact, qualification root,
config path, and CURRENT-pointer path explicitly. It still loads only the
side-effect-free `i2e-runtime-library.ps1` and the Resume contract.

```text
I2E_RESUME_WRAPPER_GLOBALS_EXPLICIT = PASS
I2E_RESUME_DOTSOURCE_EXECUTABLE_I2E_WRAPPER = FORBIDDEN / ABSENT
SHARED_RUNTIME_LIBRARY_WRAPPER_GLOBALS = ABSENT
I2E_RESUME_PLAN_ONLY_REAL_ENTRYPOINT = PASS
I2E_RESUME_PLAN_ONLY_OUTPUT = I2E_TREATMENT_RESUME_PLAN_ONLY=true
I2E_RESUME_PLAN_ONLY_MACHINE_STATE = UNCHANGED
I2E_RESUME_AUTHORIZED_FLAG_PRESERVATION = PASS
SHARED_LIBRARY_CALLER_GLOBAL_AUDIT = PASS
PREVIOUS_TEST_GAP = I2E_RESUME_WAS_PARSE_AND_CONTRACT_TESTED_BUT_NOT_REAL_ENTRYPOINT_EXECUTED
I2E_RESUME_REAL_RUNTIME = 0
I2E_RERUN = FORBIDDEN
I2F_SCOPE_ISOLATION = PRESERVED
I2F_GATE_CONSUMED = false
I2F_ARTIFACT_CHANGED = false
NEXT_GATE = HUMAN_I2F_SELF_ENABLE_QUALIFICATION_REVIEW
```

The behavioral test executes the actual Resume wrapper in a child PowerShell
process, validates the canonical service and artifact identity in its plan, and
proves the machine snapshot is unchanged. A guarded authorized-entry sentinel
proves the treatment-only authorization switch survives helper loading without
performing any service, LSA, token, or AMD operation.

The future wrapper is plan-only unless explicitly authorized:

```powershell
Set-Location 'F:\File\codex\codex-worktrees\ac74\resource-timeline'
& '.\tools\amd-privilege-qualification\run-admin-amd-i2f-service-profile-experiment.ps1'
```

No I2F service, LSA mutation, token adjustment, AMD process, or sampling was
performed while preparing this path. I2E evidence and its historical artifact
(`871CD20D228BD9510606DE640F516F62C2983B9`) remain immutable.

The offline I2E closure-contract fixture is
`i2e-token-materialization-final.example.json`. It is explicitly marked as an
example and is not historical ProgramData evidence.

## PR22 I2F entrypoint scope isolation closure

The first human I2F invocation was a confirmed no-op before experiment entry.
The cause was structural PowerShell scope pollution: the I2F setup and cleanup
wrappers had dot-sourced the executable I2E wrapper with `-LibraryOnly`, whose
parameter binder overwrote the I2F caller's authorization and library switches.

The wrappers now load reusable helpers directly from
`i2e-runtime-library.ps1`, which has no executable parameters or entrypoint
side effects. Real child-process checks cover both plan-only entrypoints, and
offline authorized-entry sentinels prove that the authorization switches reach
the pre-mutation boundary without creating a service, changing LSA policy,
adjusting a token, or launching AMD uProf.

```text
I2F_HUMAN_INVOCATION_1 = CONFIRMED_NO_OP
I2F_GATE_CONSUMED = false
I2F_ROOT_CREATED = false
I2F_SERVICE_CREATED = false
I2F_LSA_MUTATION = 0
I2F_TOKEN_ADJUSTMENT = 0
I2F_AMD_RUNTIME = 0
I2F_DOTSOURCE_EXECUTABLE_I2E_WRAPPER = REMOVED
SHARED_RUNTIME_LIBRARY = PASS
I2F_PLAN_ONLY_REAL_ENTRYPOINT = PASS
I2F_CLEANUP_PLAN_ONLY_REAL_ENTRYPOINT = PASS
I2F_ROLLBACK_SAFETY = PRESERVED
I2F_RUST_SELF_ENABLE_SEMANTICS = UNCHANGED
I2F_ARTIFACT_CHANGED = false
NEXT_GATE = HUMAN_I2F_SELF_ENABLE_QUALIFICATION_REVIEW
```
