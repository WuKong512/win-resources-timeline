# AMD privilege and secure IPC qualification

This is an independent, qualification-only Windows Service Broker artifact for
`AMD-PRIVILEGE-I2`. It is not the Resource Timeline collector, production
provider, installer, autostart path, or database writer.

## CURRENT STATUS — AUTHORITATIVE

I2E paired, treatment-resume, and standalone cleanup real entrypoints are
retired and permanently fail closed. I2F experiment and cleanup real
entrypoints are also retired. I2G Attempt #1 and Attempt #2 remain immutable
historical harness failures; Attempt #3 completed the one authorized paired
qualification and its gate is consumed. No additional real authorization is
granted by this document.

```text
I2E_REAL_PAIRED_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2E_REAL_TREATMENT_RESUME_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2E_REAL_CLEANUP_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2E_HISTORICAL_EVIDENCE = IMMUTABLE
I2E_RERUN = FORBIDDEN
I2E_CLEANUP_RERUN = FORBIDDEN
I2F_REAL_EXECUTION_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2F_REAL_CLEANUP_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2F_RERUN = FORBIDDEN
I2F_CLEANUP_RERUN = FORBIDDEN
I2F_RESULT = PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT
I2_BROKER_SETUP_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2_POWER_SAMPLING_CLIENT_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2B_COUNTER_DISCOVERY_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2_LEGACY_CLEANUP_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2C_SYSTEM_COMPARISON_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2C_SYSTEM_CLEANUP_REAL_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2_LEGACY_RERUN = FORBIDDEN
I2B_RERUN = FORBIDDEN
I2C_RERUN = FORBIDDEN
AMD_QUALIFICATION_EXECUTABLE_ENTRYPOINT_AUDIT = PASS_NO_UNRETIRED_HISTORICAL_REAL_GATE
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_VARIABLE_SELECTION = PASS_READ_ONLY
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_SINGLE_VARIABLE_ISOLATABLE = true
I2G_SELECTION_CHANGED = false
BLOCKER = I2G_PAIRED_PHASE_CONFIGURATION_INVARIANT_CONTRADICTS_TREATMENT_MUTATION
BLOCKER_STATUS = CLOSED_OFFLINE
PREVIOUS_BLOCKER_1 = I2G_BASELINE_RECONSTRUCTION_CONTRACT_INCONSISTENT / CLOSED_OFFLINE
PREVIOUS_BLOCKER_2 = I2G_STAGED_TREATMENT_TOKEN_MISCLASSIFIED_AS_EXACT_I2F_BASELINE / CLOSED_OFFLINE
PREVIOUS_BLOCKER_3 = I2G_HISTORICAL_CONTROL_LEAVES_NON_TREATMENT_CONFOUNDERS_UNCONTROLLED / CLOSED_OFFLINE
I2G_BASELINE_RECONSTRUCTION_RIGHT = SeSystemProfilePrivilege
I2G_TREATMENT_VARIABLE = SeProfileSingleProcessPrivilege
I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT
HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY
HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false
TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1
HISTORICAL_I2F_PROFILE_SINGLE_STATE = ABSENT
I2G_CONTROL_PROFILE_SINGLE_STATE = ABSENT
I2G_CONTROL_SYSTEM_PROFILE_STATE = PRESENT + ENABLED
I2G_TREATMENT_PROFILE_SINGLE_STATE = PRESENT + ENABLED
I2G_TREATMENT_SYSTEM_PROFILE_STATE = PRESENT + ENABLED
I2G_PAIRED_CAUSAL_TREATMENT_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + ENABLED
I2G_TREATMENT_MATERIALIZATION_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + DISABLED
I2G_TREATMENT_ACTIVATION_DELTA = SeProfileSingleProcessPrivilege DISABLED -> ENABLED
NO_CODE_CHANGE_BETWEEN_PHASES = true
NO_NON_TREATMENT_CONFIGURATION_CHANGE_BETWEEN_PHASES = true
ALLOWED_TREATMENT_CONFIGURATION_DELTA = SeProfileSingleProcessPrivilege assignment to same Service SID only
CONTROL_TO_TREATMENT_POLICY_DELTA_COUNT = 1
PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1
PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
PLANNED_VALID_PAIR_COUNTER_DISCOVERY_RUNS = 2
MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1
MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2
ACTUAL_RUN_COUNT_EVIDENCE_SCHEMA = DEFINED
CONTROL_RETRY_ALLOWED = false
TREATMENT_RETRY_ALLOWED = false
POWER_SAMPLING_RUNS = 0
CONTROL_SERVICE_NAME_EQUALS_TREATMENT = true
CONTROL_SERVICE_SID_EQUALS_TREATMENT = true
CONTROL_HARNESS_SHA_EQUALS_TREATMENT = true
CONTROL_EXPECTED_RESULT = POWER_UNAVAILABLE
CONTROL_DRIFT_STOP_BEFORE_TREATMENT = true
CONTROL_TOKEN_TEARDOWN_BEFORE_TREATMENT_POLICY_MUTATION = true
I2G_HARNESS = IMPLEMENTED_OFFLINE
I2G_REAL_RUNTIME = ATTEMPT3_COMPLETE
I2G_REAL_QUALIFICATION = PASS_AMD_PRIVILEGE_I2G_REAL_QUALIFICATION
I2G_REAL_SCIENTIFIC_RESULT = PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT
I2G_CAUSAL_INTERPRETATION_VALID = true
I2G_CONTROL_RESULT = POWER_UNAVAILABLE
I2G_TREATMENT_RESULT = POWER_UNAVAILABLE
I2G_CONTROL_TOKEN_GATE = PASS
I2G_TREATMENT_TOKEN_GATE = PASS
I2G_PAIRED_CONFIG_DELTA = PASS
I2G_PAIRED_TOKEN_DELTA = PASS
I2G_CONTROL_RUNS = 1
I2G_TREATMENT_RUNS = 1
I2G_TOTAL_DISCOVERY_RUNS = 2
I2G_RETRY_OCCURRED = false
I2G_POWER_SAMPLING_RUNS = 0
I2G_ROLLBACK = PASS
I2G_FINAL_MACHINE_STATE = CLEAN
I2G_RECOVERY_REQUIRED = false
I2G_ATTEMPT3_AUTHORIZATION = CONSUMED
I2G_HUMAN_REAL_RUN_AUTHORIZATION = CONSUMED
I2G_HARNESS_IMPLEMENTATION_AUTHORIZED = true
I2G_REAL_RUNTIME_AUTHORIZED = false
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = DEFER
ATTEMPT1_AUTHORIZATION = CONSUMED
ATTEMPT2_AUTHORIZATION = CONSUMED
ATTEMPT3_AUTHORIZATION = CONSUMED
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
NEW_REAL_RUN_REQUIRED = false
PR24 = MERGED
I2G_CURRENT_STATE = COMPLETE / ATTEMPT3_AUTHORITATIVE
I2G_CAUSAL_INTERPRETATION_VALID = true
I2H_JUSTIFIED = NO
NEXT_GATE = HUMAN_REVIEW_SELECTED_POST_I2G_NEXT_TASK
NEXT_TASK = AMD-CLI-LIST-PATH-VALIDITY-Q1
POST_I2G_DECISION = docs/upgrade/amd-post-i2g-production-admission.md
README_CURRENT_STATE_RECONCILED=PASS
```

The selected I2G treatment remains one variable. Historical I2F is predecessor
evidence only, not the active causal control. Attempt #3 completed the paired
CONTROL -> TREATMENT design on one fresh service identity. CONTROL assigned
only `SeSystemProfilePrivilege` and returned `POWER_UNAVAILABLE`; treatment
added `SeProfileSingleProcessPrivilege` to the same Service SID and also
returned `POWER_UNAVAILABLE`. The valid conclusion is narrowly
`PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT`. The expected control,
treatment, invariant comparison, drift gate, and independent rollback are
specified in
[`docs/upgrade/amd-i2g-variable-selection.md`](../../docs/upgrade/amd-i2g-variable-selection.md).
The only allowed CONTROL-to-TREATMENT configuration delta is assignment of
`SeProfileSingleProcessPrivilege` to the same Service SID after CONTROL
teardown. Attempt #3 consumed one run in each phase with no retry and no
sampling. No further real run is required or authorized.

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
timechart. The historical standard-user handoff is
`run-standard-user-amd-counter-discovery.ps1`; its real wrapper is retired and
is not run by automated tests.

## Synthetic qualification

From the repository root:

```powershell
pwsh -NoProfile -File .\tools\amd-privilege-qualification\test-qualification.ps1
```

This command does not register a service, request elevation, access the AMD
installation, or run AMD uProf.

## HISTORICAL / CONSUMED / DO NOT RUN — I2 power-sampling qualification path

The following path is retained as historical I2 qualification infrastructure.
It consumed the one bounded real power-sampling gate and is not the I2B
counter-discovery handoff. Its real setup, client, and cleanup wrappers now
fail closed before machine access; do not run the historical commands below.

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

The historical run was at most one bounded `LocalService + Service SID + Session 0`
AMD package-power session. Cancellation is qualified synthetically and is not
performed against a real AMD runtime.

## HISTORICAL / CONSUMED / DO NOT RUN — I2B human handoff: non-sampling counter discovery

This is the historical LocalService client sequence for the I2B
differential. It is **NON_SAMPLING**, sends only
`GetAmdCounterAvailability`, and lets the broker derive the fixed
`AMDuProfCLI.exe timechart --list` command. It does not request a power event,
duration, interval, or CSV session.

The commands below are retained only as historical execution shape. Do not run
them: the setup, counter client, and cleanup wrappers now reject real
execution before configuration/evidence reads or machine access. They are not
executed by this repair or by synthetic tests:

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

## HISTORICAL / SUPERSEDED — I2E service-SID SeSystemProfile minimum-variable experiment

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

## HISTORICAL / SUPERSEDED — Current I2E status: pre-control harness incident

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

## HISTORICAL / SUPERSEDED — Current I2E status: CONTROL complete, treatment-only resume prepared

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

## HISTORICAL / SUPERSEDED — AMD-PRIVILEGE-I2E real closure / I2F self-enable preparation

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

At the time of this historical preparation snapshot, I2F was the next
human-authorized qualification gate. The later real I2F closure supersedes this
preparation-only status. It used the same
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

## HISTORICAL / SUPERSEDED — I2F PRE-RUN ROLLBACK SAFETY REVIEW CLOSURE

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

## HISTORICAL / SUPERSEDED — PR22 I2F cleanup entrypoint retirement closure

This is the current state and supersedes earlier I2F preparation and recovery
instructions. The authoritative I2F experiment completed full rollback, so
the standalone real cleanup entrypoint is retired. The historical I2F
evidence, including the authoritative rollback event, is immutable and no
cleanup is required for the completed scope. Future capability work requires a
fresh task, fresh harness, fresh rollback boundary, and separate human
authorization.

```text
I2F_AUTHORITATIVE_SCOPE = f68bf4d3d36547a0ba753cff489bb6eb
I2F_GATE_CONSUMED = true
I2F_RERUN = FORBIDDEN
I2F_CLEANUP_RERUN = FORBIDDEN
I2F_REAL_EXECUTION_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2F_REAL_CLEANUP_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2F_REAL_CLEANUP_ERROR = I2F_CLEANUP_RERUN_FORBIDDEN
I2F_HISTORICAL_EVIDENCE = IMMUTABLE
I2F_AUTHORITATIVE_ROLLBACK = REAL_PASS
I2F_CLEANUP_REQUIRED = false
I2F_CLEANUP_GUARD_PRECEDES_ADMIN = PASS
I2F_CLEANUP_GUARD_PRECEDES_ROOT_ENUMERATION = PASS
I2F_CLEANUP_GUARD_PRECEDES_STATE_MACHINE = PASS
I2F_CLEANUP_PLAN_ONLY = PASS
I2F_CLEANUP_LIBRARY_ONLY = PASS
I2F_CLEANUP_OFFLINE_AUTHORIZED_SENTINEL = PASS
I2F_HISTORICAL_EVIDENCE_CONTENT_UNCHANGED = PASS
I2F_RESULT = PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT
I2F_FULL_ROLLBACK = REAL_PASS
SE_SYSTEM_PROFILE_PRIVILEGE_ALONE_SUFFICIENT = false
SE_SYSTEM_PROFILE_PRIVILEGE_NECESSITY = UNRESOLVED
I2G_VARIABLE = UNRESOLVED
I2G_HARNESS = NOT_IMPLEMENTED
I2G_REAL_RUNTIME = 0
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = NOT_COMPLETE
NEXT_GATE = PR22_FINAL_MERGE_READINESS_REVIEW
```

The cleanup wrapper rejects every future real `-ExecuteAuthorizedCleanup`
invocation with `I2F_CLEANUP_RERUN_FORBIDDEN` before administrator checks,
historical scope enumeration, service/process access, LSA access, cleanup
state-machine execution, or rollback-evidence writes. Plan-only, `LibraryOnly`,
and the explicitly guarded offline cleanup sentinel remain available for
repository validation. The offline child-process regression confirms the
nonzero rejection, unchanged machine state, and unchanged historical evidence
observation without modifying the protected ProgramData scope.

## HISTORICAL / SUPERSEDED — PR22 I2E Resume wrapper-global restoration closure

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

## CURRENT STATE — I2G OFFLINE HARNESS IMPLEMENTED

The design-only handoff is superseded by the fixed I2G contract described in
[`docs/upgrade/amd-i2g-harness.md`](../../docs/upgrade/amd-i2g-harness.md).
The default execution surface remains synthetic/offline/fail-closed. A separate
one-shot real runner is task-local, exact-token gated, qualification-only, and
does not touch production code or data.

```text
I2G_HARNESS = IMPLEMENTED_OFFLINE
I2G_HARNESS_IMPLEMENTED = true
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT
HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY
HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false
I2G_GATE_CONSUMED = true
I2G_REAL_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
I2G_HUMAN_REAL_RUN_AUTHORIZATION = CONSUMED
I2G_REAL_RUNTIME = ATTEMPT3_COMPLETE
I2G_REAL_QUALIFICATION = PASS_AMD_PRIVILEGE_I2G_REAL_QUALIFICATION
I2G_REAL_SCIENTIFIC_RESULT = PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT
I2G_CAUSAL_INTERPRETATION_VALID = true
I2G_CONTROL_RESULT = POWER_UNAVAILABLE
I2G_TREATMENT_RESULT = POWER_UNAVAILABLE
I2G_CONTROL_TOKEN_GATE = PASS
I2G_TREATMENT_TOKEN_GATE = PASS
I2G_PAIRED_CONFIG_DELTA = PASS
I2G_PAIRED_TOKEN_DELTA = PASS
I2G_CONTROL_RUNS = 1
I2G_TREATMENT_RUNS = 1
I2G_TOTAL_DISCOVERY_RUNS = 2
I2G_RETRY_OCCURRED = false
I2G_POWER_SAMPLING_RUNS = 0
I2G_ROLLBACK = PASS
I2G_FINAL_MACHINE_STATE = CLEAN
I2G_RECOVERY_REQUIRED = false
I2G_ATTEMPT3_AUTHORIZATION = CONSUMED
I2G_OFFLINE_VALIDATION = PASS
I2G_HARNESS_ARTIFACT_ARCHITECTURE = x64
I2G_HARNESS_ARTIFACT_PATH = tools/amd-privilege-qualification/target/release/amd-privilege-qualification.exe
I2G_HARNESS_ARTIFACT_SHA256 = 2613129D179EA2A0496AD680E68E77A79FFFBB569D0802A11AC03346E162DD80
I2G_EXECUTION_SURFACE = SYNTHETIC_OFFLINE_FAIL_CLOSED
I2G_SHARED_EXECUTABLE_OFFLINE_ONLY = false
I2G_TASK_LOCAL_REAL_RUNNER = ONE_SHOT_EXACT_AUTHORIZATION_ONLY
I2G_TASK_LOCAL_REAL_RUN_STATUS = PASS_AMD_PRIVILEGE_I2G_REAL_QUALIFICATION
CONTROL_POLICY_RIGHTS = SeSystemProfilePrivilege only
TREATMENT_POLICY_DELTA = SeProfileSingleProcessPrivilege assignment to same Service SID only
FIXED_OPERATION = timechart --list
POWER_SAMPLING_RUNS = 0
MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1
MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2
CONTROL_RETRY_ALLOWED = false
TREATMENT_RETRY_ALLOWED = false
CONTROL_DRIFT_STOP_BEFORE_TREATMENT = true
TOKEN_TEARDOWN_BEFORE_TREATMENT_POLICY_MUTATION = true
NEW_REAL_RUN_REQUIRED = false
PR24 = MERGED
PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
NEXT_GATE = HUMAN_REVIEW_SELECTED_POST_I2G_NEXT_TASK
NEXT_TASK = AMD-CLI-LIST-PATH-VALIDITY-Q1
POST_I2G_DECISION = docs/upgrade/amd-post-i2g-production-admission.md
```

## HISTORICAL — AMD-I2G REAL ATTEMPT #1

Run `9ae1e7898f6b4a438f1acc41b76c2715` is immutable historical evidence. It
produced a valid CONTROL baseline (`POWER_UNAVAILABLE`, one discovery run),
then failed in harness setup while constructing the TREATMENT configuration:
Windows PowerShell attempted to call a missing clone method on an
`OrderedDictionary`. TREATMENT discovery did not run, rollback passed, the
machine was clean, and the authorization was consumed. This is a harness
runtime failure, not a scientific treatment rejection; no causal interpretation
was obtained. The repair uses an explicit configuration copy, preserves the
scientific-gate state separately from execution progress, and isolates the
reviewed real runner in a child `powershell.exe` process so the parent can
always restore its gate in `finally`. Any future real run requires new explicit
human authorization after review.

```text
ATTEMPT1_CONTROL_RUNS = 1
ATTEMPT1_TREATMENT_RUNS = 0
ATTEMPT1_CONTROL_RESULT = POWER_UNAVAILABLE
ATTEMPT1_TREATMENT_DISCOVERY = NOT_RUN
ATTEMPT1_HARNESS_RUNTIME_FAILURE = true
ATTEMPT1_FAILURE_CLASS = HARNESS_RUNTIME_ERROR
ATTEMPT1_CAUSAL_INTERPRETATION_VALID = false
ATTEMPT1_ROLLBACK = PASS
ATTEMPT1_FINAL_MACHINE_STATE = CLEAN
ATTEMPT1_AUTHORIZATION = CONSUMED
ATTEMPT1_EVIDENCE = IMMUTABLE
ATTEMPT1_NEW_REAL_RUN_AUTHORIZATION_REQUIRED = true
```

The reviewed real wrapper now returns deterministic child-runner status codes:
`0` means a complete paired qualification with a scientific result, `1` means
blocked/harness/runtime/cleanup failure, and `2` means an invalid or
non-causal scientific result. The wrapper launches the real runner in a
separate Windows PowerShell 5.1 process so its explicit exit cannot bypass the
parent launcher’s gate restoration.

The manual launcher adds the outer process boundary: manual launcher parent ->
Windows PowerShell 5.1 wrapper child -> Windows PowerShell 5.1 runner child.
Treatment service-phase completion is persisted from explicit lifecycle state
after the treatment service function returns successfully; it is never inferred
from discovery completion alone.

`run-admin-amd-i2g-qualification.ps1` is plan-only by default and has a
deterministic `-OfflineSynthetic` test seam. The synthetic surface validates
the exact release artifact path, x64 PE architecture, and SHA-256 before
launch; tampered and missing artifacts are rejected. Its unauthorized real
switch emits `I2G_REAL_EXECUTION_NOT_AUTHORIZED` before any machine access.
The exact task-local runner is fixed to the paired qualification contract. The cleanup
entrypoint remains permanently fail-closed with
`I2G_REAL_CLEANUP_NOT_AUTHORIZED`. `test-i2g-harness.ps1` covers the fixed
paired contract, atomic evidence inventory, result/cleanup separation,
all persisted-state recovery decisions, executable recovery/no-retry behavior,
the 20-point crash-window matrix, and synthetic fault matrix.

## AMD-I2G REAL ATTEMPT #3 — AUTHORITATIVE PAIRED RESULT

Attempt `d6d6c33003934dc5ad2b0b79307e5b2c` is the first complete valid paired
qualification. The raw evidence is immutable. Both fixed `timechart --list`
discoveries returned `POWER_UNAVAILABLE` under the same LocalService + Service
SID context; the only policy delta was `SeProfileSingleProcessPrivilege`.

```text
ATTEMPT3_RUN_ID = d6d6c33003934dc5ad2b0b79307e5b2c
ATTEMPT3_REAL_PRIVILEGED_RUN = YES
ATTEMPT3_REAL_CLEANUP_RUN = YES
ATTEMPT3_CONTROL_RUNS = 1
ATTEMPT3_TREATMENT_RUNS = 1
ATTEMPT3_TOTAL_DISCOVERY_RUNS = 2
ATTEMPT3_POWER_SAMPLING_RUNS = 0
ATTEMPT3_RETRY_OCCURRED = false
ATTEMPT3_CONTROL_RESULT = POWER_UNAVAILABLE
ATTEMPT3_TREATMENT_RESULT = POWER_UNAVAILABLE
ATTEMPT3_CONTROL_TOKEN_GATE = PASS
ATTEMPT3_TREATMENT_TOKEN_GATE = PASS
ATTEMPT3_PAIRED_CONFIG_DELTA = PASS
ATTEMPT3_PAIRED_TOKEN_DELTA = PASS
ATTEMPT3_TREATMENT_SERVICE_PHASE_STARTED = true
ATTEMPT3_TREATMENT_SERVICE_PHASE_COMPLETED = true
ATTEMPT3_TREATMENT_DISCOVERY_STARTED = true
ATTEMPT3_TREATMENT_DISCOVERY_COMPLETED = true
ATTEMPT3_FAILURE_CLASS = NONE
ATTEMPT3_SCIENTIFIC_RESULT = PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT
ATTEMPT3_CAUSAL_INTERPRETATION_VALID = true
ATTEMPT3_ROLLBACK = PASS
ATTEMPT3_FINAL_MACHINE_STATE = CLEAN
ATTEMPT3_RECOVERY_REQUIRED = false
ATTEMPT3_AUTHORIZATION = CONSUMED
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
NEW_REAL_RUN_REQUIRED = false
```

The conclusion is limited to the paired I2G context and does not generalize to
other accounts, privileges, AMD operations, or production behavior. Attempts
#1 and #2 remain immutable historical harness failures with no causal result;
no Attempt #4 is required or allowed.

The default I2G execution surface is synthetic/offline-only and fail-closed; the shared
`amd-privilege-qualification.exe` is not itself offline-only because it retains
historical non-I2G entrypoints (`--broker`, `--system-counter-service`,
`--service-profile-counter-service`, `--service-profile-enable-counter-service`,
and `--client`). This task-local runner does not authorize those entrypoints.

## HISTORICAL / SUPERSEDED — PR22 I2F consumed-gate runtime guard closure

This section supersedes the earlier I2F preparation-only status blocks in this
README; those blocks are historical snapshots from before the real I2F gate was
consumed.

The authoritative I2F real qualification consumed its one-time gate. The
historical wrapper is retained for plan-only, `LibraryOnly`, and the guarded
offline pre-mutation sentinel, but its real `-ExecuteAuthorizedExperiment`
path is permanently fail-closed before administrator checks, artifact
validation, scope creation, SCM access, LSA mutation, token adjustment, or AMD
execution.

```text
I2F_AUTHORITATIVE_SCOPE = f68bf4d3d36547a0ba753cff489bb6eb
I2F_GATE_CONSUMED = true
I2F_RERUN = FORBIDDEN
I2F_REAL_EXECUTION_ENTRYPOINT = PERMANENTLY_FAIL_CLOSED
I2F_REAL_RERUN_ERROR = I2F_RERUN_FORBIDDEN
I2F_REAL_RERUN_GUARD_BEFORE_ADMIN = PASS
I2F_REAL_RERUN_MACHINE_STATE = UNCHANGED
I2F_REAL_RERUN_NEW_SCOPE_CREATED = false
I2F_REAL_RERUN_SERVICE_MUTATION = 0
I2F_REAL_RERUN_LSA_MUTATION = 0
I2F_REAL_RERUN_TOKEN_ADJUSTMENT = 0
I2F_REAL_RERUN_AMD_RUNTIME = 0
I2F_PLAN_ONLY = PASS
I2F_LIBRARY_ONLY = PASS
I2F_OFFLINE_AUTHORIZED_SENTINEL = PASS
I2F_RESULT = PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT
SE_SYSTEM_PROFILE_PRIVILEGE_ALONE_SUFFICIENT = false
SE_SYSTEM_PROFILE_PRIVILEGE_NECESSITY = UNRESOLVED
I2G_VARIABLE = UNRESOLVED
I2G_HARNESS = NOT_IMPLEMENTED
I2G_REAL_RUNTIME = 0
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = NOT_COMPLETE
NEXT_GATE = PR22_FINAL_CLOSURE_REVIEW
```

The real child-process rerun guard proves that a forbidden authorized
invocation exits nonzero with `I2F_RERUN_FORBIDDEN` and the authoritative
scope, without creating a new scope or changing service, evidence-root, or
owned-process state. The counter-discovery evidence repair remains intact:
spawned counter discovery is explicit while power sampling remains false.
Any future capability experiment requires a fresh I2G task/harness and human
authorization; I2G is not implemented or selected here.

## HISTORICAL / SUPERSEDED — PR22 I2F real closure / residual differential preparation

The authoritative I2F qualification consumed exactly one real self-enable
gate. It is closed and must not be rerun. The dedicated LocalService Service
SID received `SeSystemProfilePrivilege`, the service token materialized it as
`PRESENT + DISABLED`, native `AdjustTokenPrivileges` enabled it, and the
post-enable token showed exactly `DISABLED -> ENABLED`. The fixed
non-sampling AMD CLI then executed and returned the known no-counters result.

```text
I2F_RESULT = PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT
I2F_SCOPE = f68bf4d3d36547a0ba753cff489bb6eb
I2F_GATE_CONSUMED = true
I2F_RERUN = FORBIDDEN
SERVICE_SID_RIGHT_ASSIGNMENT = REAL_PASS
PRE_ENABLE_TOKEN_GATE = REAL_PASS
ADJUST_TOKEN_PRIVILEGES = REAL_PASS
POST_ENABLE_TOKEN_GATE = REAL_PASS
EXACT_TOKEN_DELTA = REAL_PASS
I2F_AMD_IDENTITY = REAL_PASS
I2F_AMD_COUNTER_DISCOVERY = REAL_POWER_UNAVAILABLE
I2F_AMD_CLI_EXIT_CODE = 0
I2F_POWER_CATEGORY_PRESENT = false
I2F_NO_COUNTERS_DIAGNOSTIC = true
I2F_NO_ORPHAN_CHILD = true
I2F_FULL_ROLLBACK = REAL_PASS
SE_SYSTEM_PROFILE_PRIVILEGE_ALONE_SUFFICIENT = false
SE_SYSTEM_PROFILE_PRIVILEGE_NECESSITY = UNRESOLVED
I2F_REAL_RUNTIME = 1
I2F_POWER_SAMPLING_RUNTIME = 0
I2F_HISTORICAL_ARTIFACT_SHA256 = F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D
I2F_POST_REPAIR_ARTIFACT_SHA256 = 9A13111B02D5AAA2886B7E1EA059643EAABD5F30C3A2522589EE8B124B7B735C
I2F_ARTIFACT_CHANGED_AFTER_REPAIR = true
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = NOT_COMPLETE
NEXT_GATE = REVIEW_RESIDUAL_DIFFERENTIAL_AND_SELECT_SINGLE_I2G_VARIABLE
```

`I2F_REAL_RUNTIME = 1` means one bounded counter-discovery CLI execution,
not a power sampling session. The CLI was actually spawned and completed;
`POWER_UNAVAILABLE` is the AMD result. The historical I2F launch/result JSON
is immutable and retains the legacy `amd_runtime_executed=false` field. New
counter-discovery evidence uses additive v1 fields:

```text
counter_discovery_cli_executed = true after Command::spawn succeeds
power_sampling_runtime_executed = false
sampling = false
```

This keeps the legacy sampling interpretation intact while making process
execution explicit. Spawn failure does not claim execution, and a spawned CLI
remains executed even with a nonzero exit code.

The residual SYSTEM-versus-I2F comparison is recorded in
`docs/upgrade/amd-system-vs-i2f-residual-differential.md`. Session 0, x64, and
AMD CLI identity are closed differentials. Account identity, Administrators
membership, other token privileges/groups, Service SID identity, and AMD
driver/device/backend authorization remain open. `I2G_VARIABLE = UNRESOLVED`;
no I2G runtime or production account selection is authorized.

The behavioral test executes the actual Resume wrapper in a child PowerShell
process, validates the canonical service and artifact identity in its plan, and
proves the machine snapshot is unchanged. A guarded authorized-entry sentinel
proves the treatment-only authorization switch survives helper loading without
performing any service, LSA, token, or AMD operation.

The retired wrapper is plan-only for ordinary invocation. Its historical
authorized path is now permanently fail-closed by the consumed-gate guard:

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

## HISTORICAL / SUPERSEDED — PR22 I2F entrypoint scope isolation closure

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
