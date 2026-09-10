# AMD LocalService active-sampling Q2 design and admission decision

This is a design, reconciliation, and admission record only. It does not
implement a Q2 harness, create a Q2 gate, authorize Q2 Live, execute an AMD
binary or API, create a Windows service, or change Windows state.

The decision in this document is whether one new, independently governed
experiment is warranted after the immutable Q1 closure. It is not a Q1 retry,
rerun, continuation, gate replacement, or evidence-completion step.

## Decision

| Field | Decision |
| --- | --- |
| RESULT | PASS_AMD_LOCALSERVICE_ACTIVE_SAMPLING_Q2_DESIGN_Q1 |
| TASK_ID | AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q2-DESIGN-Q1 |
| Q2_JUSTIFIED | YES |
| Q2_DESIGN_READY | YES |
| Q2_LIVE_AUTHORIZED | NO |
| REAL_EXECUTION_ALLOWED | false |
| NEXT_GATE | HUMAN_REVIEW_Q2_DESIGN |

Q2 is justified because the only Q1 attempt stopped before the scientific
intervention, the stop was a reviewed harness false negative rather than AMD
runtime evidence, and the unresolved question remains material: whether the
frozen active package-power command works from the intended LocalService,
Session-0, CONTROL-baseline context. Existing evidence does not answer that
question and a new Q2 can be isolated, bounded, and independently authorized.

Approval of this design does not approve implementation, create an
authorization token, create a gate, or change the production admission.

## Entry baseline and duplicate/predecessor audit

The required entry commands were completed before the design branch was
created:

| Field | Observed value |
| --- | --- |
| REPOSITORY | WuKong512/win-resources-timeline |
| REQUESTED_BASELINE | 7043322715562945a1b1a2ed5c69015b7a1ebafd |
| ENTRY_MAIN | 7043322715562945a1b1a2ed5c69015b7a1ebafd |
| ENTRY_HEAD | 7043322715562945a1b1a2ed5c69015b7a1ebafd |
| ENTRY_BRANCH | detached at origin/main before branch creation |
| ENTRY_WORKING_TREE | CLEAN |
| PR29_MERGED | YES |
| PR29_MERGE_COMMIT | 7043322715562945a1b1a2ed5c69015b7a1ebafd |
| PR29_ANCESTOR_OF_ORIGIN_MAIN | YES |
| DESIGN_BRANCH | design/amd-localservice-active-sampling-q2 |
| MERGE_BASE | 7043322715562945a1b1a2ed5c69015b7a1ebafd |

origin/main was fetched with prune and had not advanced beyond the requested
baseline. The branch was created directly from that clean authoritative
commit. No unrelated worktree was changed.

The duplicate audit covered:

| Surface | Result |
| --- | --- |
| Repository text, docs, decisions, execution plans, and qualification artifacts | No equivalent AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q2 task or design found |
| All local and fetched remote branch names | No Q2-equivalent branch found |
| All reachable commit messages and paths | No Q2-equivalent task, gate, or post-Q1 experiment found |
| Fetched remote heads | No Q2-equivalent head found; the only matching active-sampling head is the merged Q1 harness branch |
| PR #29 merge history | Confirmed as the merged Q1 predecessor |
| GitHub API/CLI listing | Not available: gh and the agent-reach executable were unavailable, and the browser pull-request page did not return a stable readable state |

Therefore:

    Q2_EXISTING_WORK_FOUND = NO
    DUPLICATE_TASK_GATE = PASS

The last limitation is recorded honestly: the result is no equivalent work in
the fetched repository state and reachable history, not a fabricated claim
that an unavailable GitHub API returned an empty list. If human review finds
an authoritative equivalent before implementation, this design must stop and
the duplicate must be reconciled before any Q2 branch or harness work.

## Immutable Q1 predecessor state

The following values are inherited facts. This document does not alter them:

    Q1_RESULT = BLOCKED_BY_HARNESS_VALIDATION_BUG
    LIVE_PREFLIGHT = PASS
    LSA_READ = PASS
    LSA_VALIDATION = FALSE_NEGATIVE
    LSA_MUTATION_ATTEMPTED = NO
    AMD_CLI_REAL_INVOCATIONS = 0
    AMD_API_REAL_INVOCATIONS = 0
    POWER_SAMPLING_RUNS = 0
    SCIENTIFIC_RESULT = NOT_OBTAINED
    PROXY_VERDICT = INSUFFICIENT
    ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN

    Q1_GATE_STATE = CONSUMED
    Q1_GATE_CONSUMED = YES
    Q1_RERUN_ALLOWED = NO
    Q1_MAX_RUNS = 1
    Q1_RETRIES = 0

    Q1_LIVE_RETIRED = YES
    Q1_LIVE_AUTHORIZATION_AVAILABLE = NO
    OLD_Q1_AUTHORIZATION_REUSABLE = NO

The protected ProgramData Q1 evidence root cannot be read by the current
workspace principal because its sealed ACL denies access. No ACL was changed
and no direct historical-file claim is made here. The repository-authoritative
Q1 classification is taken from the merged postmortem and execution-plan
markers, including the preserved sealed evidence shape replay.

The Q1 postmortem records that the old validator inspected only
PSObject.Properties while valid nested LSA records were [ordered]/IDictionary
values. It therefore treated readable, right-absent evidence as unavailable
and stopped before the permitted LSA add. The repair made dictionary-first
access the shared abstraction, added mixed-shape regression/replay coverage,
separated gate consumption from AMD invocation accounting, and retired Q1
Live at source level. Those repairs are predecessor evidence; they are not a
Q2 execution.

Q1 remains historically closed forever. Q2 must never inspect, delete, reset,
consume, rename, recreate, or reuse the Q1 gate or Q1 authorization.

## Q2 justification analysis

| QUESTION | EVIDENCE | CONCLUSION |
| --- | --- | --- |
| Did Q1 obtain a scientific result? | Q1 stopped during live preflight/LSA validation; scientific result is NOT_OBTAINED. | No. |
| Did Q1 invoke AMD? | Authoritative Q1 accounting is AMD CLI 0, AMD API 0, sampling runs 0, invocation certainty CONFIRMED_ZERO. | No. |
| Was Q1 blocker environmental or harness-caused? | Live preflight and LSA read passed. The blocker was the dictionary-shaped evidence false negative; no AMD or driver operation was reached. | Harness-caused. Q1 provides no environmental pass or fail about active sampling. |
| Has the blocker been repaired and reviewed? | The merged postmortem repair uses dictionary-first mixed-shape access, replays the sealed Q1 evidence shape, adds launch/gate accounting regressions, and source-retires Q1 Live. PR #29 is merged. | Yes as a repository-reviewed repair record; Q2 still requires fresh human review of its own exact source head. |
| Would Q2 answer an unresolved material question? | LocalService I2F/I2G evidence is discovery-only timechart --list. Active LocalService package-power output is absent from all authoritative evidence. | Yes. Q2 directly tests the unresolved operation-path question. |
| Would Q2 merely duplicate existing evidence? | Historical LocalSystem active sampling passed, but it is a different account. Historical LocalService evidence is negative discovery, not active sampling. | No. It is a new account/operation-path combination, with the historical contexts retained as comparators. |
| Can Q2 preserve the original comparator? | The I2F/I2G LocalService CONTROL baseline and the known-good package-power command are frozen in repository records. | Yes. |
| Can Q2 be isolated from Q1? | New task ID, output base, gate filename, service name, SCM-generated Service SID, source identity, and authorization are all separate. The Q2 controller has no Q1 gate access. | Yes. |
| Can Q2 be bounded to one run and zero retries? | A new atomic CreateNew gate is consumed before service/ACL/LSA lifecycle; max_runs is 1 and retries is 0. | Yes. |
| Does Q2 require broader privileges than Q1? | Q2 uses the same I2F/I2G CONTROL baseline and only the exact Q2 Service SID may receive SeSystemProfilePrivilege if the before-state proves absence. It forbids ProfileSingle, Debug, Tcb, Administrators, fallback, and expansion. | No. |
| Does Q2 require production code changes? | The target is a qualification-only vendor CLI operation. No src-tauri, provider, schema, UI, installer, or production runtime change is needed. | No. |
| Does Q2 require I2G Attempt4? | I2G is a separate consumed historical experiment and its Attempt4 authorization is not granted. Q2 does not read or consume its gate. | No. |
| Does Q2 require I2H? | Q2 changes the operation path, not a privilege variable. It uses the CONTROL baseline and does not nominate a new treatment. | No. |

The decision follows from the table: Q2 is a single, material, unresolved
operation-path experiment, not a duplicate and not a privilege experiment.
Its value is bounded to evidence about LocalService active package-power
sampling. It cannot by itself decide production admission, production account,
I2H, lifecycle reliability, or distribution.

## Frozen primary scientific question

Q2 has exactly one primary question:

    Under the corrected, reviewed harness semantics, can AMD uProf 5.3.521.0
    execute the frozen bounded package-power timechart command from the
    intended LocalService Session-0 service context using the CONTROL baseline
    privilege contract and produce parseable, credible package-power samples?

No temperature, frequency, per-core, GPU, long-duration, restart-resilience,
production-runtime, automatic-privilege-discovery, or feature-expansion
question is part of Q2.

The Q2 result is an operation-path qualification only. It must not be
described as production suitability or a general claim about all AMD metrics.

## Comparator and interpretation

    Q2_PRIMARY_COMPARATOR =
      Historical I2F/I2G LocalService CONTROL baseline:
      LocalService, Session 0, dedicated Service SID, SeSystemProfilePrivilege
      present/enabled, SeProfileSingleProcessPrivilege absent, Administrators
      absent, discovery operation retained as historical evidence only.

    Q2_POSITIVE_CONTROL =
      Historical LocalSystem active-sampling PASS:
      Session 0, x64, non-interactive, bounded package-power command, 9
      package-power samples. Comparator/positive control only.

LocalSystem must never be used as a Q2 fallback. A Q2 LocalService PASS is a
same-context counterexample to treating LocalService discovery failure as an
active-sampling failure proxy. A Q2 LocalService scientific FAIL is limited to
the exact established context and command. A harness, context, token, launch
accounting, or evidence failure is BLOCKED, not scientific FAIL.

## Q2 account, service, and context contract

    Q2_ACCOUNT = NT AUTHORITY\LocalService
    Q2_ACCOUNT_SID = S-1-5-19
    Q2_SESSION_ID = 0
    Q2_INTERACTIVE = false
    Q2_ARCHITECTURE = x64
    Q2_SERVICE_NAME =
      ResourceTimelineAmdLocalServiceActiveSamplingQ2Qualification
    Q2_SERVICE_SID_POLICY =
      SCM-generated fresh dedicated Service SID, sid type unrestricted,
      exact SID captured and checked; Q1 Service SID and Q1 registration
      must not be reused.

The service must be a temporary, own-process, demand/manual-start SCM
registration created only by a future separately authorized Q2 harness. The
worker must not depend on a desktop, logged-on user, user profile, PATH
mutation, shell, debugger, or interactive input. The service definition must
prove the exact Q2 name, account, image, start mode, own-process type, SID
type, and no unexpected arguments before the worker can launch AMD.

If Session 0, non-interactivity, x64, account SID, or the exact fresh Q2
Service SID cannot be proved, the run is BLOCKED before AMD launch.

## Q2 effective-token contract

The future service worker must capture the complete effective token before
launch and compare it to the reviewed Q2 CONTROL contract:

    USER_SID = S-1-5-19
    SESSION_ID = 0
    INTERACTIVE = false
    ARCHITECTURE = x64
    INTEGRITY = SYSTEM
    DEDICATED_Q2_SERVICE_SID = PRESENT
    ADMINISTRATORS = ABSENT
    FORBIDDEN_GROUP_SID = S-1-5-32-544

Required enabled privileges:

    SeChangeNotifyPrivilege
    SeCreateGlobalPrivilege
    SeImpersonatePrivilege
    SeSystemProfilePrivilege

Forbidden privileges:

    SeProfileSingleProcessPrivilege
    SeDebugPrivilege
    SeTcbPrivilege

The full privilege and group tables must be captured, not inferred from the
account name. No additional enabled privilege, group, Q1 Service SID, or
account-wide LocalService capability may be silently introduced. Any
unexpected enabled privilege or group outside the reviewed CONTROL snapshot
is a fail-closed token-contract block and requires a separately reviewed
design, not automatic privilege discovery or expansion.

## Q2 LSA materialization contract

Q2 may assign one right to one principal only when the future authorization
explicitly includes this contract:

    Q2_ALLOWED_LSA_RIGHT = SeSystemProfilePrivilege
    Q2_ALLOWED_LSA_TARGET = EXACT_Q2_SERVICE_SID_ONLY

The exact behavior is:

1. Capture a readable BEFORE snapshot of both the direct rights for the exact
   Q2 Service SID and all principals assigned SeSystemProfilePrivilege.
2. Require both reads to report status READ. Record the canonical Q2 Service
   SID, direct rights, assigned principals, read timestamps, and source
   shapes.
3. Require the exact Q2 Service SID not to possess the right directly and not
   to appear in the assignment list. A contradictory, malformed, unavailable,
   or ambiguous state blocks closed.
4. Durably write mutation intent before the mutation call. The intent must
   contain task ID, service name, exact Q2 Service SID, right, target policy,
   right_absent_before=true, and the planned ADD operation.
5. Durably write mutation-started immediately before the exact add call.
6. Read the AFTER state and require the exact Q2 Service SID assignment to be
   present with no unexplained change. Record an ownership record saying
   whether the run owns the exact add.
7. On teardown, recover only a right proven to be Q2-owned by the durable
   intent, mutation-started record, BEFORE state, AFTER state, current
   readback, and exact Service SID anchor. Write recovery state whether
   removal is attempted or deliberately blocked.
8. Read the final state and require the exact Q2 assignment to be absent after
   a successful owned recovery. Any unreadable or contradictory recovery
   state fails closed and keeps the exact registration/residue visible for
   human diagnosis.

The implementation must use a dictionary-first mixed-shape access abstraction
for all nested LSA values and must include offline fixtures for
[ordered]/IDictionary, property-backed, and JSON-deserialized evidence. A
readable dictionary-shaped BEFORE state must not regress to the Q1 false
negative. Q2 never adds the right to S-1-5-19, Administrators, LocalSystem,
generic service groups, or any Q1 identity. This task performs no LSA
mutation.

## AMD binary and driver contract

The design-entry read-only metadata check found no drift from the historical
identity. Future Q2 preflight must recheck all fields and block on mismatch;
it must not update the contract silently.

    Q2_AMD_BINARY =
      D:\apps\AMDuProf\bin\AMDuProfCLI.exe
    Q2_AMD_BINARY_SHA256 =
      D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
    Q2_AMD_VERSION = FILE_VERSION 5.3.521.0; PRODUCT_VERSION 5.3.521.0
    Q2_AMD_ARCHITECTURE = x64
    Q2_AMD_SIGNER = Advanced Micro Devices

The read-only design-entry observation was file version 5.3.521.0,
product version 5.3.521.0, Valid Authenticode, and signer
CN=Advanced Micro Devices, O=Advanced Micro Devices, S=California, C=US.
This is identity evidence only; the binary was not executed.

Frozen driver contract:

    AMDPowerProfiler
      file_version = 10.6.3.0
      product_version = 5.3.481.0
      signer = Advanced Micro Devices
    AMDCpuProfiler
      file_version = 4.4.1.0
      product_version = 5.3.481.0
      signer = Advanced Micro Devices

The design-entry observations matched those versions and valid AMD signatures.
The observed driver hashes were AMDPowerProfiler.sys =
C3C95925D69CA06D63B52B8465D10A902B85276D1DF2199F8A310DC8EEC04123 and
AMDCpuProfiler.sys =
39EB5C974A66A1AC29440212E9E136368E851BAE8460426741EE8D387D512BCD. A future
preflight must record exact file identity, signer, service/load state, and
compatibility evidence. The user-mode 5.3.521.0 versus driver 5.3.481.0
version split remains:

    AMD_BINARY_DRIVER_COMPATIBILITY = UNKNOWN

No compatibility claim is made from matching metadata alone. Q2 does not
repair, update, unload, reload, or otherwise mutate drivers.

## Exact active command

The future run must use exactly one bounded command:

    D:\apps\AMDuProf\bin\AMDuProfCLI.exe timechart --event power --interval 1000 --duration 10 --format csv --output-dir <Q2_RUN_ROOT>\raw\timechart-output

    EVENT = power
    INTERVAL_MS = 1000
    DURATION_SECONDS = 10
    FORMAT = csv
    WORKING_DIRECTORY = D:\apps\AMDuProf\bin
    CLI_TIMEOUT = 30000 ms

The command must not add --list, temperature, frequency, additional counters,
an open-ended process, a second launch, a sampling loop, or a retry. The
historical --list result remains the comparator input; running --list in Q2
would add a second operation and blur the one-question contract.

Machine-readable summary of the frozen Q2 contract:

    Q2_PRIMARY_SCIENTIFIC_QUESTION =
      The corrected, reviewed LocalService Session-0 CONTROL context can
      execute the frozen AMD uProf 5.3.521.0 bounded package-power command and
      produce parseable, credible package-power samples.
    Q2_PRIMARY_COMPARATOR = Historical I2F/I2G LocalService CONTROL baseline
    Q2_POSITIVE_CONTROL = Historical LocalSystem active-sampling PASS
    Q2_TOKEN_CONTRACT = Exact LocalService Session-0 x64 CONTROL token:
      dedicated Q2 Service SID present, Administrators absent, required
      privileges enabled, forbidden privileges/groups absent
    Q2_DRIVER_CONTRACT = AMDPowerProfiler 10.6.3.0/5.3.481.0;
      AMDCpuProfiler 4.4.1.0/5.3.481.0; AMD-signed; compatibility UNKNOWN
    Q2_COMMAND =
      D:\apps\AMDuProf\bin\AMDuProfCLI.exe timechart --event power --interval
      1000 --duration 10 --format csv --output-dir
      <Q2_RUN_ROOT>\raw\timechart-output
    Q2_DURATION_SECONDS = 10
    Q2_INTERVAL_MS = 1000
    Q2_GATE_CONSUMPTION_POINT =
      POST_PREFLIGHT_PRE_STAGING; after human/source authorization and
      read-only preflight, before run-root, service, ACL, LSA, token-worker,
      or AMD lifecycle
    Q2_SOURCE_IDENTITY =
      SOURCE_SHA256_PINNED; exact reviewed Git head plus per-material-file
      SHA-256 manifest and authorization-bound manifest hash
    Q2_NEW_AUTHORIZATION_REQUIRED = YES
    Q1_AUTHORIZATION_REUSE = FORBIDDEN
    Q2_PASS_CRITERIA = Valid context and launch; CSV parse PASS; at least
      two finite, non-negative, non-constant package-power samples
    Q2_FAIL_CRITERIA = Exact context and valid AMD invocation established,
      then trustworthy vendor runtime/output evidence fails the preregistered
      package-power criteria
    Q2_BLOCKED_CRITERIA = Preflight, identity, service/SID, LSA,
      token, launch-accounting, harness, persistence, parser, ACL, manifest,
      or cleanup/evidence-chain validity failure before a trustworthy
      scientific observation
    Q2_CLEANUP_CONTRACT =
      Stop and own processes; prove writer quiescence; recover only owned
      Q2 LSA change; remove exact Q2 SID write; seal/hash evidence; delete
      exact service only when safe; preserve visible residue on ambiguity
    Q2_EVIDENCE_CONTRACT =
      Preflight, source/git/platform/binary/driver/service/SID/token/LSA/gate/
      launch/stdout/stderr/process/raw-output/CSV/parser/package-power/
      quiescence/cleanup/ACL/manifest/hash/residue/summary evidence

## Independent one-shot governance

Q2 has a new gate and a new output base:

    TASK_ID = AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q2
    Q2_MAX_RUNS = 1
    Q2_RETRIES = 0
    Q2_OUTPUT_BASE =
      C:\ProgramData\ResourceTimeline\qualification\amd-localservice-active-sampling-q2
    Q2_GATE_FILE = Q2-LIVE-GATE.json
    Q2_RUN_ROOT = <Q2_OUTPUT_BASE>\<run-id>

No Q2 gate file is created by this design. The future controller must use an
atomic create-new operation and reject any existing, malformed, or ambiguous
gate path. The gate record must contain at least:

    schema = amd-localservice-active-sampling-q2/gate/v1
    task_id = AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q2
    run_id = unique Q2 run identifier
    state = CONSUMED
    max_runs = 1
    retries = 0
    consumed_at_utc = durable UTC timestamp
    consumption_phase = POST_PREFLIGHT_PRE_STAGING
    real_execution_allowed = true only after all reviewed gates pass

The exact consumption point is after the future human authorization,
source-identity verification, and read-only preflight all pass, and before
run-root creation, service registration, ACL authorization, LSA materialization,
token-dependent worker start, or AMD launch. The create-new Q2 gate file is
the first irreversible entry into the authorized Q2 lifecycle. A failed gate
creation enters no experiment lifecycle. Once consumed, any later block,
failure, timeout, cleanup defect, or evidence defect leaves
Q2_RERUN_ALLOWED = NO; there is no automatic retry.

Q2 must not inspect, delete, reset, consume, alter, or depend on Q1-LIVE-GATE.json.
The Q2 controller must not use Q1 gate state as a precondition. Q1_GATE_CHANGED
is verified as a predecessor invariant by review, not by mutating or reopening
the Q1 gate.

## New authorization boundary and source identity

This design creates no authorization token, environment marker, or Q2 gate.
Future Q2 Live requires all of the following:

    HUMAN_REVIEW_Q2_HARNESS_HEAD = REQUIRED
    EXACT_REVIEWED_SOURCE_IDENTITY = REQUIRED
    EXPLICIT_HUMAN_ONE_SHOT_AUTHORIZATION = REQUIRED
    NEW_Q2_SPECIFIC_AUTHORIZATION = REQUIRED
    Q1_AUTHORIZATION_REUSE_FOR_Q2 = FORBIDDEN

The future authorization must bind to SOURCE_SHA256_PINNED:

1. a full reviewed Git source commit identity;
2. a SHA-256 manifest for every file that can materially affect the live
   experiment, including the Q2 contract, controller, service host/worker,
   imported LSA/SCM/cleanup/parser helpers, and any native helper source or
   reviewed executable used by the run;
3. the manifest hash and exact path list recorded in the authorization;
4. the exact Q2 command, binary hash, driver contract, account, Service SID
   policy, output base, gate semantics, and timeout recorded in the
   authorization.

Source changes after review invalidate authorization before any gate or
lifecycle action. The Q1 historical token and environment marker must be
rejected even if supplied. A Q1 runner, Q1 gate helper, or Q1 authorization
must not be treated as a Q2 source or authorization.

## Output ACL contract

The future harness must use a fresh, collision-free Q2 run root and fail
closed if it already exists or contains content.

| Phase | Required writer policy |
| --- | --- |
| Before the Q2 Service SID exists | Only the minimum reviewed staging/controller principals may write the root; no LocalService account SID write |
| After the Q2 Service SID is captured | The service-writer grant is Modify for the exact Q2 Service SID only; no Q1 SID, no S-1-5-19 account-wide write, no generic service group |
| After worker/service writers stop | Remove Q2 Service SID write capability before final evidence sealing completes |
| Sealed | Raw evidence is immutable by the Q2 service; ACL inspection, hashes, manifest verification, and any remaining residue are recorded |

The controller must use exact paths and exact SIDs. It may retain only the
minimum separately reviewed lifecycle/evidence control needed to stop the
service, recover an owned LSA right, and seal evidence. It must not turn that
control into LocalService account-wide write.

    LOCALSERVICE_ACCOUNT_WIDE_WRITE = false

## Evidence contract

The future Q2 implementation must produce the following evidence before a
result can be reviewed. The names are a minimum contract; a source-reviewed
implementation may use equivalent names only if the manifest maps them
one-to-one without dropping a required fact.

| Evidence | Required content |
| --- | --- |
| preflight | All read-only gates, result, failures, and proof that no mutation/AMD launch occurred before pass |
| source identity | Reviewed Git head, per-file SHA-256 records, manifest hash, and authorization binding |
| git baseline | Requested/current main, merge base, branch/source head, clean-tree result |
| platform snapshot | OS/build, CPU identity, hypervisor/VBS/HVCI and relevant AMD platform state; no unreviewed change |
| AMD binary identity | Exact path, SHA-256, file/product versions, x64 PE identity, Authenticode status and signer |
| AMD driver identity | Both driver names, paths, hashes, file/product versions, signer, service/load state, compatibility classification |
| service definition | Exact Q2 service name, account, image, arguments, start mode, own-process type, SID type |
| service SID evidence | SCM-generated exact Q2 Service SID, source/readback, no Q1 SID reuse |
| LSA before | Direct Q2 SID rights and all SeSystemProfilePrivilege principals, both readable, right absent from Q2 SID |
| LSA mutation intent | Exact task/service/SID/right/target, ownership plan, and right_absent_before |
| LSA mutation started | Durable record immediately before the exact add call |
| LSA ownership | Whether the change is Q2-owned, with before/after/current identity anchors |
| LSA after | Exact Q2 SID assignment readback and unchanged unrelated principals |
| LSA recovery | Attempt/state/result, final readback, ambiguity and residue if blocked |
| effective token | User SID, session, interactivity, x64, integrity, full groups, Service SID, full privileges |
| Q2 gate record | Exact Q2-LIVE-GATE.json record and hash, including consumption phase |
| CLI launch intent | Frozen executable, arguments, working directory, output path, gate state, and start result UNKNOWN |
| CLI launch started | Durable post-Process.Start evidence, PID/ownership, and exact command; mutually exclusive with valid start-failed successor |
| CLI launch start-failed | Valid successor with exact command/path and process_started=false when Process.Start did not succeed |
| stdout | Raw vendor standard output |
| stderr | Raw vendor standard error |
| process result | PID, descendants/ownership, signed exit code, timeout, cancellation, capture completeness |
| AMD raw output | All vendor-produced files under the exact output directory, including session metadata if present |
| CSV | The exact vendor timechart CSV, not a regenerated or synthesized file |
| parser result | Parser version/source identity, schema/column interpretation, row and timestamp checks |
| package-power evidence | Package-power column, sample count, finite/non-negative/non-constant checks, values and units |
| writer quiescence | Service/worker/AMD descendants stopped and no writer can still change evidence |
| cleanup evidence | Service stop, process absence, LSA recovery, temporary-resource cleanup and any failure |
| ACL seal evidence | Before/after ACL records, exact Q2 SID write removal, no account-wide LocalService write |
| evidence manifest | Relative paths, sizes, SHA-256 values, and excluded summary rules |
| SHA256 manifest verification | Independent post-seal hash verification and result |
| final residue | Exact Q2 service/process/LSA/ACL/run-root residue and unrelated-state non-interference |
| summary | Final classification, scientific result, operational cleanup result, invocation accounting, and limitations |

Raw live evidence must be made immutable after sealing. Evidence capture,
manifest generation, and cleanup must not overwrite or delete the protected
Q1 evidence root or any unrelated qualification root.

## Invocation accounting

Q2 gate accounting is separate from AMD invocation accounting:

    Q2_GATE_CONSUMED = YES
    AMD_CLI_REAL_INVOCATIONS = 0
    INVOCATION_CERTAINTY = CONFIRMED_ZERO
    SECOND_RUN_FORBIDDEN = YES

The three certainty states are:

| State | Meaning |
| --- | --- |
| CONFIRMED_ZERO | Gate was consumed, no durable launch-started evidence exists, and a valid exact launch-start-failed successor proves Process.Start did not succeed |
| AMBIGUOUS_0_OR_1 | Durable launch intent exists but successor evidence is missing, corrupt, contradictory, or not exact enough to prove whether Process.Start succeeded; numeric zero is forbidden |
| CONFIRMED_ONE | Process.Start succeeded or durable launch-started evidence exists, even if later capture, parsing, cleanup, or summary fails |

If both launch successors appear, the evidence conflict is preserved and the
classification is BLOCKED/HARNESS_FAILURE; it must not be normalized into a
zero or a retry. A consumed Q2 gate never implies that AMD was invoked, and
zero invocation evidence never reopens the gate.

## Scientific pass criteria

Q2 may report:

    SCIENTIFIC_RESULT = PASS_BOUNDED_PACKAGE_POWER

only when all of the following are true:

    LIVE_PREFLIGHT = PASS
    SERVICE_CONTEXT = PASS
    TOKEN_CONTRACT = PASS
    FROZEN_COMMAND = PASS
    PROCESS_STARTED = YES
    CSV_FOUND = YES
    CSV_PARSE_STATUS = PASS
    PACKAGE_POWER_SAMPLE_COUNT >= 2
    PACKAGE_POWER_VALUES_FINITE = YES
    PACKAGE_POWER_VALUES_NON_NEGATIVE = YES
    PACKAGE_POWER_VALUES_NON_CONSTANT = YES

The process result, raw output, parser, package-power evidence, writer
quiescence, ACL seal, manifest, and cleanup records must still be retained.
A cleanup failure remains an independent operational failure even if the
scientific result is otherwise established; it cannot be hidden by a PASS.

Q2 PASS means only that the frozen bounded command produced credible
package-power samples in the established Q2 context. It does not claim
production suitability, long-term reliability, installer/lifecycle safety,
distribution approval, account minimization, or a production account.

## FAIL versus BLOCKED

The future result has separate scientific and operational axes.

Use BLOCKED, with SCIENTIFIC_RESULT = NOT_OBTAINED, when the intended
scientific context never became valid because of any of:

    preflight failure
    source-identity or authorization failure
    service setup or Service SID failure
    unreadable/ambiguous LSA BEFORE, ownership, AFTER, or recovery evidence
    token-contract failure
    launch-accounting ambiguity
    harness, persistence, parser, manifest, ACL, or evidence-chain failure

Use scientific FAIL only when the exact LocalService Session-0 CONTROL
context is established, AMD invocation is validly reached, and trustworthy
vendor runtime/output evidence demonstrates failure of the bounded
package-power question. Examples include a valid completed invocation whose
authoritative output reports no package-power capability, or a valid
package-power result that fails the preregistered scientific criteria. Missing
output caused by a harness defect, corrupt evidence, or ambiguous launch is
not scientific FAIL.

Cleanup failure is always separately visible as an operational failure. The
run must not be upgraded to PASS, retried, or repaired and rerun because
cleanup failed.

## Cleanup contract

The future Q2 teardown order is:

1. Stop the Q2 service/worker and record SCM status.
2. Confirm the exact worker and AMD process tree is absent; never kill by a
   broad image-name rule or touch unrelated processes.
3. Confirm writer quiescence before changing ACLs or sealing.
4. Recover exactly the Q2-owned SeSystemProfilePrivilege assignment, if any.
   If ownership or current state is ambiguous, fail closed and retain the
   exact registration/residue for human diagnosis.
5. Remove the exact Q2 Service SID write capability and record ACL seal
   evidence before final evidence sealing completes.
6. Seal raw evidence, generate and verify the SHA-256 manifest, and preserve
   the sealed run root.
7. Delete the exact temporary Q2 service only when process absence, writer
   quiescence, owned LSA recovery, and evidence sealing are verified. If any
   prerequisite fails, keep the exact registration/residue visible.
8. Perform a final residue scan covering Q2 service, worker/AMD processes,
   Q2 Service SID ACLs, Q2-owned LSA right, temporary staging, and unrelated
   state.

No unrelated right, service, ACL, process, driver, device, platform setting,
or historical evidence may be removed. Cleanup failure is not a reason to
retry or to broaden authority.

## Stop conditions and forbidden follow-up

The future Q2 attempt stops after exactly one live attempt. The following are
forbidden within this task and within the future Q2 authorization:

    retry
    repair + rerun
    second AMD invocation
    LocalSystem fallback
    alternate account fallback
    privilege expansion or automatic privilege discovery
    driver repair/update/reset
    BIOS repair
    VBS, HVCI, Hyper-V, or other platform-security change
    Q1 gate or authorization reuse
    I2G Attempt4
    I2H execution
    production code/runtime change

Any follow-up that changes the question, account, privilege, driver, platform,
or operation shape requires a separately designed and reviewed experiment.

## Relationship to I2H, I2G, and production admission

    Q2_IS_I2H = NO
    I2H_JUSTIFIED_BEFORE_Q2 = NO
    I2H_RECONSIDERATION_GATE = AFTER_VALID_Q2_RESULT
    I2G_ATTEMPT4 = NO
    I2G_REAL_GATE_CHANGED = NO
    AMD_PRODUCTION_ADMISSION = DEFER
    PRODUCTION_ACCOUNT = UNRESOLVED

Q2 does not nominate SeProfileSingleProcessPrivilege or another treatment.
Only after a valid Q2 result and a new evidence review may the project
reconsider whether any I2H-style question has a concrete, isolated,
production-relevant hypothesis. A valid Q2 result does not automatically
justify I2H.

The known unrelated I2G blocker remains unchanged:

    I2G_PINNED_ARTIFACT_SHA256_EXPECTED =
      2613129D179EA2A0496AD680E68E77A79FFFBB569D0802A11AC03346E162DD80
    I2G_PINNED_ARTIFACT_SHA256_ACTUAL =
      061082A9FD65BB768CF450CFBE1F9523A06FA4362BBD1853154B17A9C90E0BC8
    I2G_ATTEMPT4_AUTHORIZATION = NOT_GRANTED

Q2 must not repair, rebuild, repin, reinterpret, consume the I2G gate, or
authorize Attempt4. Q2 does not depend on I2G Attempt4.

## Required invariants

    Q1_GATE_IMMUTABLE = YES
    Q1_LIVE_RETIRED = YES
    Q1_RERUN_ALLOWED = NO
    Q2_IS_NEW_EXPERIMENT = YES
    Q2_GATE_INDEPENDENT = YES
    Q2_MAX_RUNS = 1
    Q2_RETRIES = 0
    Q1_AUTHORIZATION_REUSE = FORBIDDEN
    Q2_LIVE_AUTHORIZED = NO
    LOCALSYSTEM_FALLBACK = FORBIDDEN
    PRIVILEGE_EXPANSION_FALLBACK = FORBIDDEN
    REPAIR_AND_RERUN = FORBIDDEN
    PRODUCTION_RUNTIME_CHANGE = NO
    I2G_ATTEMPT4 = NO

## Current-task non-execution record

This design task performed no live or Windows mutation:

    REAL_EXECUTION_ALLOWED = false
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
    HISTORICAL_RAW_EVIDENCE_CHANGED = NO
    Q1_GATE_CHANGED = NO
    I2G_REAL_GATE_CHANGED = NO
    PRODUCTION_RUNTIME_CHANGED = NO
    RUST_RUNTIME_CHANGED = NO

The only host inspection beyond repository text was read-only file metadata
for the frozen AMD binary and two drivers. No AMD image was executed or
loaded, no service or LSA API was invoked, and no ProgramData Q1/Q2 evidence
root or gate was created.

## Final design handoff

    Q2_JUSTIFIED = YES
    Q2_DESIGN_READY = YES
    Q2_LIVE_AUTHORIZED = NO
    REAL_EXECUTION_ALLOWED = false
    NEXT_GATE = HUMAN_REVIEW_Q2_DESIGN
    DESIGN_DOC = docs/upgrade/amd-localservice-active-sampling-q2-design-q1.md
    EXECUTION_PLAN_UPDATED = YES; additive current-state marker only
    PRODUCTION_ADMISSION = DEFER

This document is ready for human review. The next permitted project action
after review is a separately scoped Q2 harness implementation decision. No
implementation, live gate, authorization, or Q2 service is created here.
