# AMD LocalService Active-Sampling Design Q1

## 1. Baseline and entry gate

This document is an offline design record. It does not authorize or execute a
future AMD qualification run.

| Field | Frozen value |
| --- | --- |
| Task ID | AMD-LOCALSERVICE-ACTIVE-SAMPLING-DESIGN-Q1 |
| Repository | WuKong512/win-resources-timeline |
| Entry origin/main | e74153e74a416a1c8c542498b8b87cc6d146f5cf |
| PR | #28 |
| PR #28 merge commit | e74153e74a416a1c8c542498b8b87cc6d146f5cf |
| PR #28 relationship | The merge commit is an ancestor of entry origin/main |
| Design mode | AUDIT_MODE = OFFLINE_DESIGN_ONLY |
| Entry worktree | Clean |
| Entry branch | design/amd-localservice-active-sampling-q1 |

The entry check confirmed the repository remote is
https://github.com/WuKong512/win-resources-timeline.git. No post-PR #28
authoritative-state drift was present at entry. The merge commit is the
current origin/main pin used by this design. Any future main advance requires
human reconciliation before a live run is considered.

The historical records named by the task remain the source of truth. This
document adds a future design and does not rewrite their raw evidence.

## 2. Inherited authoritative state

The following state is inherited and remains unchanged by this design:

~~~text
AMD_RUNTIME_PREREQUISITE_AUDIT_Q1 = COMPLETE / INSUFFICIENT_EVIDENCE
RUNTIME_PREREQUISITE_AUDIT = COMPLETE / INSUFFICIENT_EVIDENCE
PROXY_VERDICT = INSUFFICIENT
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
AUTHORIZATION_EQUIVALENCE = UNKNOWN
PRODUCTION_ACCOUNT = UNRESOLVED
AMD_PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
LIVE_PAIR_AUTHORIZED = NO
SELECTED_NEXT_TASK = NONE / HUMAN_REVIEW_REQUIRED
NEXT_GATE = HUMAN_REVIEW_RUNTIME_PREREQUISITE_AUDIT
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
~~~

The new design does not turn any of these states into a production admission,
an authorization, or a sampling result. It only freezes the smallest future
operation-path qualification that could reduce the enumeration-versus-
sampling uncertainty.

## 3. Primary question and hypotheses

Historical negative evidence is:

~~~text
ACCOUNT = NT AUTHORITY\LocalService
SESSION = 0
AMD_OPERATION = timechart --list
RESULT = POWER_UNAVAILABLE
~~~

The future question is whether this discovery-path result predicts a bounded
production-relevant active package-power sampling result in the same controlled
context.

~~~text
H0 = LocalService discovery failure also predicts active-sampling failure.
H1 = LocalService can execute active package-power sampling even though
     timechart --list reported POWER_UNAVAILABLE.
~~~

The design changes only the AMD operation path from counter discovery to a
bounded active package-power sample. It does not add a privilege, group,
account, platform setting, device ACL, driver change, or production-provider
integration. It is a controlled cross-run comparison, not a claim that the
historical records form an in-run paired experiment.

## 4. Why this is the selected information-gain target

The current proxy verdict is INSUFFICIENT because the historical LocalService
records establish a discovery failure but do not establish an active-sampling
failure. The LocalSystem active-sampling PASS is useful positive-control
evidence, but it cannot answer the LocalService operation-path question and
cannot select LocalSystem as the production account.

A single bounded LocalService active-sampling attempt has the highest direct
information gain available without introducing a new privilege experiment:

1. It retains the account family and Session 0 service context that matter to
   the negative evidence.
2. It uses the already successful package-power CLI path, rather than testing
   a new metric or an open-ended workload.
3. It can produce either a directly useful LocalService counterexample or a
   controlled LocalService negative.
4. It does not silently answer installer, lifecycle, reliability, legal,
   distribution, or production-admission questions.

The design therefore targets operation-path equivalence only. It must not be
described as a paired equivalence result unless a future run actually closes
the explicitly listed residual confounders.

## 5. Historical comparator matrix

The following matrix records the role of the existing evidence without
reinterpreting its raw contents.

| Evidence | Identity and context | Operation | Result | Design role |
| --- | --- | --- | --- | --- |
| Administrator active sampling | Interactive Administrator; historical CLI identity; interactive session | timechart --event power, interval 1000, duration 10, CSV | PASS; package-power samples | Positive path evidence only; not same account or session |
| LocalSystem service qualification | NT AUTHORITY\LocalSystem, SID S-1-5-18, Session 0, non-interactive, x64 | timechart --event power, interval 1000, duration 10, CSV | PASS; 9 package-power samples and session output | Session 0 and hardware positive control; not production-account evidence |
| I2B LocalService discovery | NT AUTHORITY\LocalService, SID S-1-5-19, Session 0, dedicated service SID | timechart --list | POWER_UNAVAILABLE; exit 0; no sampling | Primary negative discovery evidence |
| I2F LocalService plus SystemProfile | LocalService, Session 0, dedicated service SID, SeSystemProfilePrivilege present and enabled | timechart --list | POWER_UNAVAILABLE; exit 0; no sampling | Shows that this baseline privilege did not close discovery |
| I2G Attempt #3 CONTROL | Fresh LocalService, Session 0, x64; SystemProfile baseline; ProfileSingle absent | timechart --list | POWER_UNAVAILABLE | Paired control for the ProfileSingle question |
| I2G Attempt #3 TREATMENT | Same historical pair design; ProfileSingle added to treatment | timechart --list | POWER_UNAVAILABLE | Historical result was PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT; no sampling |

The future contract reuses the known-good package-power command shape from
the Administrator and LocalSystem active-sampling evidence. It does not reuse
the I2G consumed run, does not create Attempt #4, and does not claim the
I2G discovery pair answered the sampling question.

## 6. Frozen future account, service, session, and token contract

### 6.1 Account and service SID

The future account is fixed as follows:

~~~text
WINDOWS_ACCOUNT = NT AUTHORITY\LocalService
ACCOUNT_SID = S-1-5-19
ACCOUNT_FAMILY = LocalService
DEDICATED_QUALIFICATION_SERVICE_SID = YES
SERVICE_SID_TYPE = UNRESTRICTED
FUTURE_SERVICE_NAME = ResourceTimelineAmdLocalServiceActiveSamplingQualification
~~~

The service is to be created only by a separately authorized future harness.
The account is the built-in LocalService account, while the dedicated Service
SID is an additional identity present in the service token. The exact numeric
Service SID will be fresh for the future service and therefore remains a
declared cross-run confounder. It must be captured in the run manifest and
compared with the frozen service configuration before execution.

This account is the most informative proxy for the question because it
matches the LocalService account family and Session 0 execution context of
I2F and I2G, while avoiding a new production-account or privilege selection.
LocalSystem is not selected as a production account. Its historical PASS is
positive-control evidence only.

### 6.2 Session and interactivity

~~~text
SESSION = 0
INTERACTIVE = NO
DESKTOP = NONE
ARCHITECTURE = x64
~~~

The future service must be started by the Service Control Manager in Session 0
and must not depend on an interactive desktop, logged-on user, or user
profile. If a future implementation cannot preserve Session 0, the run is
invalid and the resulting observation is a confounded result, not a
qualification result.

### 6.3 Baseline token and privilege state

The frozen baseline is the non-treatment I2F / I2G CONTROL contract:

~~~text
TOKEN_USER = S-1-5-19
INTEGRITY = SYSTEM
ADMINISTRATORS_MEMBERSHIP = ABSENT
SeSystemProfilePrivilege = PRESENT + ENABLED
SeProfileSingleProcessPrivilege = ABSENT
NEW_GROUP_MEMBERSHIP = NONE
NEW_PRIVILEGE = NONE
NEW_DEVICE_ACL = NONE
~~~

The required enabled baseline includes SeChangeNotifyPrivilege,
SeCreateGlobalPrivilege, SeImpersonatePrivilege, and
SeSystemProfilePrivilege. The service must not receive SeDebugPrivilege,
SeTcbPrivilege, SeProfileSingleProcessPrivilege, or another SYSTEM-only
privilege as part of this design. The historical disabled or absent
privileges remain a contract check rather than a reason to add a new
privilege experiment.

The future preflight must capture the token user, integrity level, group
membership, Service SID, and full privilege state and compare them with this
contract. If the complete token contract cannot be frozen and observed, the
design is INSUFFICIENTLY CONTROLLED and live execution is not allowed.

This freezes the operation-path comparison while retaining the exact
I2F/I2G baseline. It intentionally does not test ProfileSingle, a new group,
a new integrity level, a different account, or a new device permission.

## 7. AMD binary and runtime version contract

The only permitted AMD executable identity for a future run is:

~~~text
FUTURE_BINARY = D:\apps\AMDuProf\bin\AMDuProfCLI.exe
FUTURE_BINARY_SHA256 = D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
FUTURE_BINARY_FILE_VERSION = 5.3.521.0
FUTURE_BINARY_PRODUCT_VERSION = 5.3.521.0
FUTURE_BINARY_ARCHITECTURE = x64 / PE 0x8664
FUTURE_BINARY_AUTHENTICODE = Valid; Advanced Micro Devices / AMD signer
~~~

The historical CLI-specific record retains the valid AMD signer identity but
does not retain an exact certificate-subject string for this executable. A
future preflight must recheck the Authenticode chain and record the exact
subject, without treating a different signer or hash as equivalent.

The version contract is:

~~~text
USER_MODE_UPROF_VERSION = 5.3.521.0
AMDPowerProfiler.sys PRODUCT_METADATA_VERSION = 5.3.481.0
AMDPowerProfiler.sys FILE_VERSION = 10.6.3.0
AMDCpuProfiler.sys FILE_VERSION = 4.4.1.0
USER_MODE_DRIVER_COMPATIBILITY = UNKNOWN
~~~

The user-mode and driver version difference is an observed version split, not
an automatic mismatch failure. A future run must recheck the installed driver
identity and report a launch, loader, compatibility, or driver result
separately if the operation fails.

If the SHA256, file version, product version, architecture, or signer does
not match the frozen identity, BINARY_IDENTITY_GATE = BLOCK and no AMD
executable may start.

## 8. Exact future command contract

The command shape is taken from the authoritative bounded package-power
sampling records. The future run must use exactly one bounded operation:

~~~text
FUTURE_COMMAND =
  D:\apps\AMDuProf\bin\AMDuProfCLI.exe timechart --event power --interval 1000 --duration 10 --format csv --output-dir <OUTPUT_ROOT>\raw\timechart-output

FUTURE_WORKING_DIRECTORY = D:\apps\AMDuProf\bin
FUTURE_DURATION = 10 seconds
FUTURE_INTERVAL = 1000 milliseconds
FUTURE_MAX_RUNS = 1
FUTURE_RETRIES = 0
FUTURE_OPERATION_TIMEOUT = 30 seconds
~~~

The command requests package power only. It does not request temperature,
frequency, an open-ended process, a discovery phase, or a retry. The future
harness must not run timechart --list as a prerequisite unless a later human
review explicitly changes the scientific question; adding discovery would
reintroduce the path under test and would not be a neutral preflight.

The acceptance record must preserve the raw CLI exit code, stdout, stderr,
session metadata, output file list, package-power row count, interval, and
elapsed duration. A successful observation means a clean bounded CLI
completion with package-power output; it does not require an invented sample
count threshold beyond the historical command's validated output shape.

## 9. Output-root contract

The future run must use one explicit, isolated root:

~~~text
OUTPUT_ROOT =
  C:\ProgramData\ResourceTimeline\qualification\amd-localservice-active-sampling-q1\<run-id>
~~~

The run ID must be unique and recorded before process launch. The root must
not exist before preflight. An existing root, any pre-existing content, or
any path collision is a fail-closed condition; the future harness must not
delete or overwrite it.

The root layout is:

~~~text
<OUTPUT_ROOT>\manifest.json
<OUTPUT_ROOT>\raw\cli-stdout.txt
<OUTPUT_ROOT>\raw\cli-stderr.txt
<OUTPUT_ROOT>\raw\process-result.json
<OUTPUT_ROOT>\raw\service-result.json
<OUTPUT_ROOT>\raw\timechart-output\timechart.csv
<OUTPUT_ROOT>\raw\timechart-output\session.uprof
<OUTPUT_ROOT>\summary\qualification-summary.json
~~~

Raw vendor output and launch evidence are immutable after capture. The
summary is written separately and must reference, rather than replace, the
raw files. The root must not be the historical evidence directory, a
production database directory, or the user's Resource Timeline data
directory. The LocalService token receives only the minimum write access to
this new root that is required by the already-authorized future contract.

Cleanup may remove temporary staging artifacts only after raw evidence is
persisted and hashed. It must not remove historical evidence. A future live
run is not allowed to create this root during the current task; this section
only defines the path contract.

## 10. Qualification artifact SHA256 blocker relevance

The current pinned qualification artifact mismatch is:

~~~text
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
PINNED_SHA256 = 2613129D179EA2A0496AD680E68E77A79FFFBB569D0802A11AC03346E162DD80
REBUILT_SHA256 = 37D4C3EC25F5F1607372BC78C0F35CF36511EBDF37E0F67D9F350475C36A1988
~~~

This design chooses Case B:

~~~text
QUALIFICATION_ARTIFACT_BLOCKER_RELEVANCE =
  NON_BLOCKING_FOR_NEW_FROZEN_CONTRACT
LIVE_RUN_DESIGN_READY = YES
~~~

The strict basis is:

1. The future LocalService run does not depend on the mismatched pinned I2G
   release artifact. The old I2G artifact is not a permitted executable for
   this design.
2. Using that mismatched artifact would make evidence identity untrusted.
   Therefore it is explicitly prohibited, not silently accepted.
3. The mismatch is a blocker for the pre-existing qualification-artifact
   validation test and for any future run that chooses to reuse that artifact.
   It is not a blocker to writing a separately pinned LocalService contract.
4. A future harness implementation must have its own exact executable hash,
   signer, version, source revision, and output manifest before human
   authorization. That future identity gate is independent of the old
   mismatch.
5. No rebuild, repin, SHA repair, or live harness implementation is performed
   in this task. If a future implementation elects to reuse the old artifact,
   artifact reconciliation becomes a prerequisite and this Case B decision no
   longer applies.

Thus the design is ready as a reviewed contract, while the live operation is
still unauthorized and execution remains false.

## 11. Mandatory future-live preflight

The future harness must fail closed. Every gate below is mandatory:

| Gate | Required condition |
| --- | --- |
| BASELINE_MAIN_PIN | Entry main pin e74153e74a416a1c8c542498b8b87cc6d146f5cf or a subsequently reconciled human-reviewed pin |
| WORKING_TREE_CLEAN | No uncommitted source, harness, or contract changes |
| BINARY_SHA256_MATCH | Exact full SHA256 above |
| BINARY_VERSION_MATCH | File and product versions both 5.3.521.0 |
| DRIVER_IDENTITY_RECHECK | Expected AMD driver names, versions, hashes, signer, and load state recorded |
| ACCOUNT_MATCH | NT AUTHORITY\LocalService |
| SID_MATCH | Token user S-1-5-19 and expected dedicated Service SID captured |
| SESSION_0_MATCH | Session 0 and non-interactive |
| TOKEN_CONTRACT_MATCH | Exact frozen user, groups, integrity, privileges, and no added rights |
| SERVICE_CONFIG_MATCH | Exact future service name, SID type, executable, start model, and no unexpected arguments |
| OUTPUT_ROOT_EMPTY | Unique future root is absent; no pre-existing content |
| NO_EXISTING_QUALIFICATION_RUN_ACTIVE | No other qualification service, wrapper, or run owns the target |
| NO_UNEXPECTED_AMD_PROCESS_STARTED_BY_HARNESS | Process tree is empty before launch and contains only the expected child during the bounded operation |
| PLATFORM_STATE_SNAPSHOT | Host, BIOS, hypervisor, VBS/HVCI, driver, and relevant platform state captured and unchanged |
| ROLLBACK_PLAN_PRESENT | Exact cleanup and timeout actions are loaded and reviewable |
| HUMAN_AUTHORIZATION_PRESENT | Explicit authorization for this exact run, identity, command, root, and duration |

Any failed gate sets REAL_EXECUTION_ALLOWED = false. A gate failure must
produce a preflight record without starting AMDuProfCLI.exe. Preflight must
not repair the failed state by adding a privilege, changing an ACL, changing
service configuration, or changing the platform.

## 12. Rollback contract

Rollback is mandatory even for the bounded command:

1. The harness must launch the expected child under an explicit process/job
   ownership model, record its PID and descendants, and enforce the 30-second
   operation timeout.
2. On normal completion, timeout, hang, loader failure, or partial output,
   the harness must terminate the owned process tree, verify that no expected
   child remains, and classify the result before cleanup.
3. An unexpected child process is a harness failure. The harness must stop
   only owned descendants according to its pre-reviewed ownership model,
   preserve the raw process evidence, and fail closed. It must not kill
   unrelated system processes.
4. If a temporary qualification service is used, it must be a new exact
   service instance. The service must be stopped, its PID and token must be
   gone, and the exact temporary service configuration must be removed after
   raw evidence is persisted. Reusing the consumed I2G service identity is
   not permitted.
5. Any temporary service configuration, temporary files, staging directories,
   and process handles must be removed only by exact-path or exact-identity
   cleanup. No broad directory deletion is allowed.
6. Token and privilege state must be restored by service teardown. The future
   harness must not leave a privilege assignment, Service SID policy, group
   membership, or elevated token behind. Any LSA change, if separately
   authorized in a future run, needs a before/after snapshot and a verified
   dual-check rollback.
7. Temporary output ownership must be limited to the exact future root. Raw
   output is preserved for evidence; only explicitly temporary staging is
   eligible for cleanup.
8. A CLI hang, driver wait, or timeout is a runtime/harness outcome, not a
   POWER_UNAVAILABLE scientific result. The timeout and cleanup evidence must
   be retained.

The rollback contract is not weakened by the bounded duration. The design
does not reuse the existing I2G harness because that harness is consumed,
historical, and explicitly not authorized for another live attempt.

## 13. Outcome interpretation matrix

The interpretation is frozen before any future authorization.

| Future observation | Allowed conclusion | Prohibited conclusion |
| --- | --- | --- |
| LocalService active sampling is POWER_AVAILABLE | The historical LocalService discovery negative is not sufficient as an active-sampling failure proxy. If all same-context gates pass, this is a same-context counterexample and operation-path uncertainty falls materially. | LocalService production admission, installer/lifecycle PASS, long-lived reliability PASS, legal/distribution PASS, or production account selection |
| LocalService active sampling is POWER_UNAVAILABLE | In the frozen LocalService, Session 0, token, binary, and platform context, both discovery and active sampling were unavailable. | LocalService can never work, LocalSystem must be production, or any arbitrary privilege is proven to be the root cause |
| CLI/API/runtime/harness failure | Classify separately as launch failure, loader failure, version/compatibility failure, access denied, driver unavailable, BIOS unsupported, hypervisor unsupported, no counter, timeout, or harness failure using the captured evidence | Treating an infrastructure, runtime, or harness failure as a scientific POWER_UNAVAILABLE result |

If the observed context differs from the frozen contract, the result is
CONFOUNDED / INVALID_FOR_PROXY and the operation-path question remains
unresolved. A POWER_AVAILABLE result does not alter AMD_PRODUCTION_ADMISSION
or authorize an I2H, production account, Attempt #4, or any product change.

## 14. Residual confounders and control assessment

The design controls the following variables:

~~~text
ACCOUNT_FAMILY = LocalService
ACCOUNT_SID = S-1-5-19
SESSION = 0
INTERACTIVE = NO
HARDWARE = SAME_HOST, RECHECKED
AMD_INSTALLATION = SAME_PINNED_INSTALL, RECHECKED
CLI_IDENTITY = FROZEN
PLATFORM_STATE = SNAPSHOTTED AND MUST REMAIN UNCHANGED
NO_NEW_PRIVILEGE_EXPERIMENT = YES
OPERATION_PATH_VARIABLE = discovery (--list) versus bounded active sampling
~~~

The following remain explicit residual confounders:

- The dedicated Service SID is fresh in the future run; historical I2F and
  I2G used different dedicated Service SID values.
- Historical raw records do not provide a complete counterfactual identity
  for every group, environment variable, inherited handle, or service
  process detail.
- User-mode uProf and driver product/file versions are split, and
  compatibility remains UNKNOWN until a future preflight and operation.
- Backend/device/firmware/platform state cannot be made counterfactual by
  documentation alone; it is only snapshotted and required to remain
  unchanged.
- The future wrapper and output root are new artifacts, and the old I2G
  artifact mismatch is not evidence for or against AMD counter availability.
- The historical comparison is cross-run rather than a simultaneous
  control/treatment pair.

Because of these residuals, this document deliberately does not use the
phrase paired equivalence. The strongest permitted positive interpretation is
a same-context counterexample only when all frozen gates pass; otherwise it is
a close-context observation with the listed limitations.

## 15. Authorization boundary and non-goals

This document does not grant authorization. The following remain mandatory:

~~~text
LIVE_RUN_AUTHORIZED = NO
REAL_EXECUTION_ALLOWED = false
HUMAN_AUTHORIZATION_REQUIRED = YES
NEXT_GATE = HUMAN_AUTHORIZATION_LOCALSERVICE_ACTIVE_SAMPLING
~~~

No step in this task may start AMDuProfCLI.exe or AMDuProf.exe, load
AMDPowerProfileAPI.dll, call an AMD API, run timechart --list, run active
sampling, create a real qualification run, create or mutate a Windows
service, modify LSA policy, modify token privileges, modify ACLs, open an AMD
device handle, send an IOCTL, modify registry/environment/driver/BIOS,
change Hyper-V/VBS/HVCI, execute I2H, create Attempt #4, or switch the
production account.

The task does not modify the production Provider, AMD provider, MetricCatalog,
schema, UI, or legal/distribution state. It does not begin long-lived
sampling, temperature/frequency sampling, soak testing, privilege fishing, or
qualification artifact repair.

## 16. Current-state handoff

The successful design result is:

~~~text
AMD_LOCALSERVICE_ACTIVE_SAMPLING_DESIGN_Q1 = COMPLETE
QUALIFICATION_ARTIFACT_BLOCKER_RELEVANCE = NON_BLOCKING_FOR_NEW_FROZEN_CONTRACT
LIVE_RUN_DESIGN_READY = YES
LIVE_RUN_AUTHORIZED = NO
REAL_EXECUTION_ALLOWED = false
SELECTED_NEXT_TASK =
  AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1 / AWAITING_HUMAN_AUTHORIZATION
NEXT_GATE = HUMAN_AUTHORIZATION_LOCALSERVICE_ACTIVE_SAMPLING
PROXY_VERDICT = INSUFFICIENT
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
AMD_PRODUCTION_ADMISSION = DEFER
PRODUCTION_ACCOUNT = UNRESOLVED
I2H_JUSTIFIED = NO
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
~~~

The future run, if separately authorized and if it passes all gates, may
update the operation-path evidence. Until then, the inherited authoritative
state remains unchanged.

## 17. Offline validation and non-execution record

This change is documentation-only:

~~~text
RUNTIME_CODE_CHANGED = NO
RUST_RUNTIME_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
AMD_CLI_REAL_INVOCATIONS = 0
AMD_API_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
SERVICE_MUTATIONS = 0
LSA_MUTATIONS = 0
TOKEN_MUTATIONS = 0
ACL_MUTATIONS = 0
DEVICE_MUTATIONS = 0
DRIVER_MUTATIONS = 0
PLATFORM_SECURITY_MUTATIONS = 0
~~~

Validation is limited to repository state, document inspection, and
git diff --check. No live output root is created. No AMD binary, API,
service, driver, token, LSA, ACL, device, registry, or platform state is
touched.

The authoritative source set for this design is:

- docs/upgrade/amd-runtime-prerequisite-audit-q1.md
- docs/upgrade/amd-cli-list-path-validity.md
- docs/upgrade/amd-post-i2g-production-admission.md
- docs/upgrade/amd-operation-path-evidence-gap-d1.md
- docs/upgrade/amd-i2g-harness.md
- docs/upgrade/amd-i2g-variable-selection.md
- docs/upgrade/amd-system-vs-i2f-residual-differential.md
- docs/upgrade/execution-plan.md
- tools/amd-privilege-qualification/README.md

Historical active-sampling and service-context evidence remains in the
measurement records referenced by those authoritative documents. This
document records the exact identity and command needed for a future
preflight, without executing or regenerating the historical evidence.
