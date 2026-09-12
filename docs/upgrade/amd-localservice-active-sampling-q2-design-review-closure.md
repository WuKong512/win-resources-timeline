# AMD LocalService active-sampling Q2 design-review closure

This document is the post-merge human-review closure for PR #30. It corrects
Q2 contract semantics only. It does not implement a Q2 harness, create or
consume a Q2 gate, create an authorization token or environment marker, invoke
AMD, sample package power, create a Windows service, or mutate Windows state.

The corrected design is the combination of this closure and
[`amd-localservice-active-sampling-q2-design-q1.md`](amd-localservice-active-sampling-q2-design-q1.md).
The closure is authoritative for future Q2 implementation review and Live
authorization. PR #30's original design head is not a standalone reviewed
identity after this closure.

## Closure decision

| Field | Value |
| --- | --- |
| RESULT | PASS_AMD_LOCALSERVICE_ACTIVE_SAMPLING_Q2_DESIGN_REVIEW_CLOSURE |
| TASK_ID | AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q2-DESIGN-REVIEW-CLOSURE |
| REPOSITORY | WuKong512/win-resources-timeline |
| PR30_MERGE_COMMIT | `b3ae1779f13b15d73cebb2f4aa13439c60c78462` |
| PR30_DESIGN_HEAD | `e3d5c5a7c9198ea4ce42c1fe46c9444bdc359b57` |
| PR30_DESIGN_HEAD_STANDALONE_AUTHORIZATION | FORBIDDEN |
| Q2_DESIGN_READY | YES |
| Q2_HARNESS_IMPLEMENTATION_READY | YES |
| Q2_LIVE_AUTHORIZED | NO |
| REAL_EXECUTION_ALLOWED | false |
| Q2_GATE_CREATED | NO |
| Q2_GATE_CONSUMED | NO |
| NEXT_GATE | HUMAN_REVIEW_Q2_DESIGN_CLOSURE |

`Q2_HARNESS_IMPLEMENTATION_READY = YES` means only that a separately scoped
implementation review may begin. It does not authorize Q2 Live or any Windows
mutation.

## Machine-readable closure summary

~~~text
AMD_LOCALSERVICE_ACTIVE_SAMPLING_Q2_DESIGN_REVIEW_CLOSURE = PASS
PR30_DESIGN_REVIEWED = YES
REVIEW_FINDING_INVOCATION_ACCOUNTING = FIXED
REVIEW_FINDING_DYNAMIC_TOKEN_GROUPS = FIXED
REVIEW_FINDING_PROCESS_COMPLETION_PASS_GATE = FIXED
REVIEW_FINDING_SAMPLE_VARIATION_GATE = FIXED
CONFIRMED_ZERO_NOT_ATTEMPTED_PATH = GATE_CONSUMED + NO_LAUNCH_INTENT + DURABLE_CONTROLLER_PROOF_LAUNCH_STAGE_NOT_ENTERED + NO_LAUNCH_STARTED
CONFIRMED_ZERO_START_FAILED_PATH = EXACT_LAUNCH_INTENT + EXACT_MUTUALLY_EXCLUSIVE_START_FAILED_SUCCESSOR + NO_LAUNCH_STARTED
AMBIGUOUS_0_OR_1_PATH = LAUNCH_INTENT + MISSING_OR_CORRUPT_OR_CONTRADICTORY_OR_IDENTITY_MISMATCHED_SUCCESSOR
CONFIRMED_ONE_PATH = PROCESS_START_SUCCEEDED_OR_DURABLE_LAUNCH_STARTED
TOKEN_CONTRACT_MODE = SEMANTIC_INVARIANTS
DYNAMIC_LOGON_SID_MODELED = YES
EXACT_Q2_SERVICE_SID_REQUIRED = YES
FORBIDDEN_ADMINISTRATORS = S-1-5-32-544
PROCESS_COMPLETION_REQUIRED_FOR_PASS = YES
PROCESS_TIMEOUT_ALLOWED_FOR_PASS = NO
PROCESS_CANCEL_ALLOWED_FOR_PASS = NO
SUCCESS_EXIT_CODE_CONTRACT = 0
NON_CONSTANT_REQUIRED_FOR_PASS = NO
VARIATION_RECORDED_AS_OBSERVATION = YES
TEMPORAL_COVERAGE_CONTRACT = ROLE_FROZEN; EXACT_TOLERANCE_DEFINED_IN_REVIEWED_HARNESS_IMPLEMENTATION; OFFLINE_TESTED_BEFORE_HARNESS_APPROVAL_AND_LIVE_AUTHORIZATION
CADENCE_CONTRACT = ROLE_FROZEN; EXACT_TOLERANCE_DEFINED_IN_REVIEWED_HARNESS_IMPLEMENTATION; OFFLINE_TESTED_BEFORE_HARNESS_APPROVAL_AND_LIVE_AUTHORIZATION
TOLERANCE_DEFINED_DURING_HARNESS_IMPLEMENTATION = YES
TOLERANCE_OFFLINE_TEST_REQUIRED_BEFORE_HARNESS_APPROVAL = YES
TOLERANCE_FROZEN_BEFORE_LIVE_AUTHORIZATION = YES
POST_LIVE_TOLERANCE_CHANGE = FORBIDDEN
RAW_MANIFEST_AND_POST_SEAL_CLOSURE_SEPARATED = YES
Q2_DESIGN_READY = YES
Q2_HARNESS_IMPLEMENTATION_READY = YES
Q2_IS_NEW_EXPERIMENT = YES
Q2_GATE_INDEPENDENT = YES
Q2_MAX_RUNS = 1
Q2_RETRIES = 0
Q2_LIVE_AUTHORIZED = NO
REAL_EXECUTION_ALLOWED = false
Q2_GATE_CREATED = NO
Q2_GATE_CONSUMED = NO
Q1_GATE_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
I2G_REAL_GATE_CHANGED = NO
AMD_CLI_REAL_INVOCATIONS = 0
AMD_API_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
~~~

## Review findings closed

### 1. Invocation accounting after a consumed-gate pre-launch block

The gate is consumed at `POST_PREFLIGHT_PRE_STAGING`, before service setup,
Service SID, ACL, LSA, token-worker, or AMD lifecycle. Therefore a consumed
gate can be followed by a block before the launch stage, with no launch intent
and no AMD invocation.

For a future run with a durably consumed Q2 gate, the controller must emit one
of these mutually exclusive states:

| `INVOCATION_STATE` | Required evidence | `AMD_CLI_REAL_INVOCATIONS` | `POWER_SAMPLING_RUNS` | `INVOCATION_CERTAINTY` |
| --- | --- | --- | --- | --- |
| `NOT_ATTEMPTED` | Gate consumed; durable launch intent absent; trusted durable source-reviewed controller lifecycle evidence proves the launch stage was never entered; durable launch-started evidence absent | `0` | `0` | `CONFIRMED_ZERO` |
| `START_FAILED` | Valid exact launch intent and valid exact mutually-exclusive launch-start-failed successor; no valid launch-started successor | `0` | `0` | `CONFIRMED_ZERO` |
| `AMBIGUOUS_0_OR_1` | Launch intent exists, but successor evidence is missing, corrupt, contradictory, or identity-mismatched | `UNKNOWN_0_OR_1` | `UNKNOWN_0_OR_1` | `AMBIGUOUS_0_OR_1` |
| `CONFIRMED_ONE` | `Process.Start` succeeded or trusted durable launch-started evidence exists | `1` | `1` | `CONFIRMED_ONE` |

`NOT_ATTEMPTED` must be established by the durable, source-reviewed state
machine/evidence, never merely by the absence of a file. `START_FAILED`
requires the exact successor because a durable launch intent proves that the
launch decision was reached. Conflicting successors are preserved as
`BLOCKED/HARNESS_FAILURE`; they are not normalized to zero and do not permit a
retry.

The following invariants remain explicit:

    GATE_CONSUMED != AMD_INVOKED
    AMD_INVOCATION_ZERO != GATE_AVAILABLE
    Q2_RERUN_ALLOWED = NO once the Q2 gate is consumed

### 2. Semantic token invariants and dynamic Windows groups

The token contract is semantic and strict. It freezes the security hypothesis
without requiring exact equality with every numeric group SID from a historical
token snapshot.

    TOKEN_USER_SID = S-1-5-19
    SESSION_ID = 0
    INTERACTIVE = false
    ARCHITECTURE = x64
    INTEGRITY = SYSTEM
    EXACT_Q2_SERVICE_SID = PRESENT
    ADMINISTRATORS_SID = S-1-5-32-544 = ABSENT

Required enabled privileges are exactly:

    SeChangeNotifyPrivilege
    SeCreateGlobalPrivilege
    SeImpersonatePrivilege
    SeSystemProfilePrivilege

Forbidden privileges are exactly:

    SeProfileSingleProcessPrivilege
    SeDebugPrivilege
    SeTcbPrivilege

The full raw privilege and group tables, including attributes, are evidence.
An additional enabled privilege is a fail-closed token-contract block. Groups
are validated by semantic identity and attributes:

    EXACT_HISTORICAL_GROUP_SET_EQUALITY = FORBIDDEN
    DYNAMIC_WINDOWS_GROUPS = ALLOWED_IF_SEMANTICALLY_VALID
    UNKNOWN_GROUP_POLICY = NOT_ALLOW_ALL; VALIDATE_OR_BLOCK
    LOGON_SID_FAMILY = S-1-5-5-X-Y with runtime X/Y
    DYNAMIC_LOGON_SID_MODELED = YES

The exact Q2 Service SID must come from the current exact SCM service, be
canonical, belong to the `S-1-5-80-...` family, agree with independent
controller/SCM readback, and not reuse a Q1 Service SID. A Logon SID's `X` and
`Y` values are runtime logon identifiers; a family-shaped string alone is not
sufficient. The validator must confirm expected Logon SID type/attributes and
token semantics. A dynamic group may be accepted only when it is not forbidden,
does not alter the Q2 privilege/security hypothesis, and its Windows-generated
semantics are demonstrated.

### 3. Clean bounded process completion is a scientific PASS prerequisite

`PROCESS_STARTED = YES` alone is insufficient. Scientific PASS requires all of
the following process conditions:

    PROCESS_STARTED = YES
    PROCESS_COMPLETED = YES
    PROCESS_TIMEOUT = NO
    PROCESS_CANCELLED = NO
    PROCESS_EXIT_CODE = 0
    STDOUT_CAPTURE_COMPLETE = YES
    STDERR_CAPTURE_COMPLETE = YES
    PROCESS_RESULT_DURABLE = YES
    PROCESS_OWNERSHIP_PID_VALID = YES
    NO_LAUNCH_ACCOUNTING_AMBIGUITY = YES

The current frozen success contract is exit code `0`. A different success-code
allowlist would require authoritative AMD/runtime evidence and explicit
registration before implementation and Live authorization; no alternative is
registered here.

A timeout after a confirmed launch remains `CONFIRMED_ONE` and therefore keeps
invocation count at `1`, but it cannot be scientific PASS. With trustworthy
vendor/runtime evidence that the preregistered bounded question failed, it may
be scientific FAIL. If timeout ownership, capture, process result, or evidence
persistence is not trustworthy, it is `BLOCKED/HARNESS_FAILURE` with
`SCIENTIFIC_RESULT = NOT_OBTAINED`. Partial CSV output never promotes timeout,
cancellation, or a non-zero exit to PASS.

### 4. Package-power validity without a hard variation gate

The frozen output validity contract is:

    CSV_FOUND = YES
    CSV_PARSE_STATUS = PASS
    PACKAGE_POWER_COLUMN_IDENTIFIED = YES
    PACKAGE_POWER_UNIT_VALID = YES
    PACKAGE_POWER_VALUES_FINITE = YES
    PACKAGE_POWER_VALUES_NON_NEGATIVE = YES
    TIMESTAMP_OR_SAMPLE_ORDER_VALID = YES
    TEMPORAL_COVERAGE_COMPATIBLE_WITH_FROZEN_COMMAND = YES
    SAMPLE_CADENCE_COMPATIBLE_WITH_1000MS_REQUEST = YES
    PACKAGE_POWER_SIGNAL_CREDIBILITY = PASS

The parser is frozen to the historically validated output shape of the exact
10-second, 1000-ms, CSV, `power` command. The design freezes the scientific
role of a reasonable bounded temporal/cadence tolerance, while the exact
numeric/algorithmic tolerance is defined as part of the source-reviewed Q2
harness implementation and covered by offline fixtures/tests. It must be
frozen before human approval of the harness and before any Live authorization;
it must never be selected, widened, relaxed, or changed after Live output is
observed. Exactly 10 rows are not required because vendor startup and ending
behavior can affect boundary rows.

These are recorded quality facts, not PASS gates:

    PACKAGE_POWER_SAMPLE_COUNT = RECORDED_OBSERVATION
    PACKAGE_POWER_VARIATION_OBSERVED = YES | NO
    NON_CONSTANT_REQUIRED_FOR_PASS = NO

Finite/non-negative numeric data is distinct from a credible package-power
signal. An all-zero series is recorded as an independent condition and is
classified only by preregistered vendor/output credibility semantics; no
all-zero acceptance/rejection rule may be invented after Live output is seen.
Equal values alone do not create a scientific failure.

## Scientific, failure, and blocked axes

### Scientific PASS

`SCIENTIFIC_RESULT = PASS_BOUNDED_PACKAGE_POWER` requires valid preflight,
service/context and semantic token contracts, the exact frozen command,
`CONFIRMED_ONE` invocation, clean bounded process completion, complete durable
process evidence, exact-run vendor output, the complete CSV/parser contract,
and credible package-power evidence under the exact temporal/cadence tolerance
frozen in the source-reviewed harness implementation and the preregistered
all-zero rules.

### Scientific FAIL

Scientific FAIL is allowed only when the exact LocalService Session-0 CONTROL
context is established, invocation is validly reached, process/runtime/vendor
evidence is trustworthy, and that evidence demonstrates failure of the
preregistered package-power question. Examples include clean vendor completion
reporting unsupported/unavailable capability or trustworthy output failing a
preregistered credibility condition.

### BLOCKED

The result is `BLOCKED` with `SCIENTIFIC_RESULT = NOT_OBTAINED` for invalid
source identity or authorization, service/SID/LSA/token contract failure,
launch-accounting ambiguity, persistence or evidence corruption, parser
inability to establish a trustworthy interpretation, manifest/evidence-chain
failure, or a harness timeout/ownership defect. A timeout after confirmed
launch is not invocation ambiguity; its invocation state remains
`CONFIRMED_ONE`.

Scientific and operational axes remain separate. Cleanup failure is recorded as
an operational failure and cannot be hidden by an overall PASS or repaired by a
second run.

## Evidence sealing and post-seal closure

The evidence model has two layers:

1. Sealed raw experiment evidence: preflight, source identity, Git/material
   manifest, platform/binary/driver identity, service/SID, LSA before/intent/
   started/after/ownership/recovery, token, gate, launch intent/success or
   start-failed successor, controller lifecycle, complete stdout/stderr,
   process result, exact vendor output/CSV, parser/package-power evidence,
   writer quiescence, ACL state, raw manifest, and independent hash
   verification. These are sealed after writer quiescence.
2. Post-seal closure/summary evidence: service registration deletion result,
   final residue scan, cleanup closure, final classification, and limitations.
   If stored under `summary/*`, these files are excluded from the raw manifest.

The contract is therefore:

    RAW_MANIFEST_COVERAGE != POST_SEAL_CLOSURE_SUMMARY

The post-seal summary references the immutable raw-manifest hash; it must not
claim that all final closure evidence is covered by the earlier raw manifest.

## Source identity and authorization boundary

Future Q2 authorization must bind to a new exact reviewed implementation head,
the exact material-file path list, each file's SHA-256, the manifest SHA-256,
exact AMD CLI hash/version, exact driver contract, exact command, exact account,
exact Service SID policy, exact output base, exact gate semantics, exact timeout,
and the frozen parser/tolerance/credibility contract. The PR #30 design head
`e3d5c5a7c9198ea4ce42c1fe46c9444bdc359b57` is not sufficient by itself.

No Q1 gate, Q1 authorization, Q1 Service SID, I2G Attempt4 authorization, or
historical token/environment marker may be reused. This closure creates no Q2
authorization token, environment marker, or gate.

## Current-task non-execution record

    Q2_LIVE_AUTHORIZED = NO
    REAL_EXECUTION_ALLOWED = false
    Q2_GATE_CREATED = NO
    Q2_GATE_CONSUMED = NO
    Q1_GATE_CHANGED = NO
    HISTORICAL_RAW_EVIDENCE_CHANGED = NO
    I2G_REAL_GATE_CHANGED = NO
    AMD_CLI_REAL_INVOCATIONS = 0
    AMD_API_REAL_INVOCATIONS = 0
    POWER_SAMPLING_RUNS = 0
    CURRENT_TASK_SERVICE_MUTATIONS = 0
    CURRENT_TASK_LSA_MUTATIONS = 0
    CURRENT_TASK_TOKEN_MUTATIONS = 0
    CURRENT_TASK_ACL_MUTATIONS = 0
    CURRENT_TASK_DEVICE_MUTATIONS = 0
    CURRENT_TASK_DRIVER_MUTATIONS = 0
    CURRENT_TASK_PLATFORM_SECURITY_MUTATIONS = 0
    I2G_ATTEMPT4 = NO
    AMD_PRODUCTION_ADMISSION = DEFER
    PRODUCTION_ACCOUNT = UNRESOLVED

Q1 historical meaning, the protected Q1 live gate, I2G real gate state, AMD
runtime, Windows SCM/LSA/ACL/token/device/driver/platform state, and production
runtime are outside this closure's mutation scope.

## Handoff

    Q2_DESIGN_READY = YES
    Q2_HARNESS_IMPLEMENTATION_READY = YES
    Q2_LIVE_AUTHORIZED = NO
    SELECTED_NEXT_TASK = AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q2-HARNESS-I1
    NEXT_GATE = HUMAN_REVIEW_Q2_HARNESS_IMPLEMENTATION

The immediate gate for this document is
`HUMAN_REVIEW_Q2_DESIGN_CLOSURE`. Any later Q2 harness implementation must be
reviewed against this closure before a separate Live authorization can even be
considered.
