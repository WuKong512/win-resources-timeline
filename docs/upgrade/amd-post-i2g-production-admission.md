# AMD POST-I2G — PRODUCTION ADMISSION PATH DECISION

This is the authoritative post-merge decision record for
`AMD-POST-I2G-PRODUCTION-ADMISSION-D1`. It reconciles immutable AMD evidence
through PR #24 and selects one bounded next task. It does not authorize or
execute an AMD, privilege, service, token, LSA, ACL, driver, device, or
sampling operation.

## Decision summary

```text
TASK_ID = AMD-POST-I2G-PRODUCTION-ADMISSION-D1
REPOSITORY = WuKong512/win-resources-timeline
BASELINE_MAIN = ecb461bf719c041b852fff113ddc25048c145c33
PR24 = MERGED
PR24_MERGE_COMMIT = ecb461bf719c041b852fff113ddc25048c145c33
I2G = COMPLETE / ATTEMPT3_AUTHORITATIVE
I2G_SCIENTIFIC_RESULT = PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT
I2G_CAUSAL_INTERPRETATION_VALID = true
PRODUCTION_ACCOUNT = UNRESOLVED
PRODUCTION_ACCOUNT_DECISION_READY = NO
AMD_PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
SELECTED_NEXT_TASK = AMD-CLI-LIST-PATH-VALIDITY-Q1
NEXT_GATE = HUMAN_REVIEW_SELECTED_POST_I2G_NEXT_TASK
RESULT = PASS_AMD_POST_I2G_PRODUCTION_ADMISSION_DECISION
```

The decision is `DEFER`, not `ADMIT`: the qualification harness and the
paired I2G causal gate are valid, but neither establishes a safe production
account, a valid production-representative operation path, or the required
installer/lifecycle/IPC/observability contract. The current CLI/service path
remains plausible because the same signed CLI produced `POWER_AVAILABLE` in
the historical SYSTEM counter comparison; it is not rejected solely because
LocalService failed.

The next task is an offline operation-path validity decision. It is more
informative than naming another privilege because I2G already tested the
selected privilege with a valid paired delta and the negative result leaves
the operation/API boundary and broader service-context boundary open.

## Authoritative I2G result and fail-closed state

Attempt #3 is immutable historical evidence:

```text
RUN_ID = d6d6c33003934dc5ad2b0b79307e5b2c
REAL_PRIVILEGED_RUN = YES
CONTROL_RUNS = 1
TREATMENT_RUNS = 1
TOTAL_DISCOVERY_RUNS = 2
RETRY_OCCURRED = false
POWER_SAMPLING_RUNS = 0
CONTROL_RESULT = POWER_UNAVAILABLE
TREATMENT_RESULT = POWER_UNAVAILABLE
CONTROL_TOKEN_GATE = PASS
TREATMENT_TOKEN_GATE = PASS
PAIRED_CONFIG_DELTA = PASS
PAIRED_TOKEN_DELTA = PASS
TREATMENT_SERVICE_PHASE_COMPLETED = true
HARNESS_RUNTIME_FAILURE = false
FAILURE_CLASS = NONE
SCIENTIFIC_RESULT = PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT
CAUSAL_INTERPRETATION_VALID = true
ROLLBACK = PASS
FINAL_MACHINE_STATE = CLEAN
RECOVERY_REQUIRED = false
```

The valid claim is only this: in the frozen paired context of the same fresh
qualification service, `NT AUTHORITY\LocalService`, unrestricted Service SID,
Session 0, x64 harness, exact AMD CLI, and non-sampling
`AMDuProfCLI.exe timechart --list`, adding
`SeProfileSingleProcessPrivilege` did not change `POWER_UNAVAILABLE` to
`POWER_AVAILABLE`. It does not generalize to another account, privilege
combination, AMD operation, platform, or production provider.

The following state is preserved and is not re-armed:

```text
I2G_REAL_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
ATTEMPT1_AUTHORIZATION = CONSUMED
ATTEMPT2_AUTHORIZATION = CONSUMED
ATTEMPT3_AUTHORIZATION = CONSUMED
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
NEW_REAL_RUN_REQUIRED = false
REAL_PRIVILEGED_RUN = NO                    # this D1 task
AMD_CLI_REAL_INVOCATIONS = 0                # this D1 task
SERVICE_MUTATIONS = 0                       # this D1 task
LSA_MUTATIONS = 0                            # this D1 task
TOKEN_MUTATIONS = 0                          # this D1 task
CONTROL_RUNS = 0                             # this D1 task
TREATMENT_RUNS = 0                           # this D1 task
ATTEMPT4_AUTHORIZATION = NOT_GRANTED        # this D1 task
```

## Historical evidence matrix

The repository contains no distinct authoritative `I2A` evidence slice. The
actual named sequence is legacy `I2`, `I2B`, `I2C`, `I2D`, `I2E`, `I2F`, and
`I2G`; this record does not manufacture an `I2A` result. References below are
the authoritative summaries, not replacements for raw evidence.

| Task / experiment | Question | Account / token context | Privilege state | AMD operation | Real or synthetic | Result | Causal validity | What it ruled out | What it did not rule out | Current relevance |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AMD uProf API/source qualification and direct-loader follow-up | Can the Resource Timeline-owned API path load and initialize the installed source? | Non-admin probe; later Administrator comparison; not a service | No treatment privilege | Explicit API/CXL load and init-only probes | Real probe plus read-only artifact analysis | Direct API/CXL load abort; no API samples; `DEFER` | Valid only for the direct loader boundary | Not a generic API/source pass; no value was fabricated | Does not prove the CLI path or account requirement is impossible | Keeps direct in-process API route unadmitted; see `cpu-sensor-amd-uprof-live-qualification.md` and `cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md` |
| Administrator CLI bounded runtime | Does the installed vendor CLI produce package-power data in a bounded elevated interactive session? | Administrator, interactive | Elevated administrator context; exact token not a production contract | `timechart --event power --interval 1000 --duration 5/10` | Real | `POWER_AVAILABLE`; parseable samples; exit 0 | Valid for that bounded CLI/session only | CLI installation, output parsing, and bounded package-power production are technically feasible | Does not qualify a service account, Session 0, unattended lifecycle, or production provider | Establishes a working vendor CLI control path; see `cpu-sensor-amd-cli-spike-runtime.md` |
| AMD-SERVICE-CONTEXT-I1 | Can the CLI run from a genuine non-interactive LocalSystem Session 0 service? | LocalSystem / `S-1-5-18`, Session 0, x64 | Service token as observed; no minimum-right claim | Fixed package-power `timechart --event power` | Real | `PACKAGE_POWER_STATUS = PASS`; 9 samples; cleanup pass | Valid for LocalSystem/Session 0 bounded feasibility | Desktop/interactive requirement and generic Session 0 incompatibility are not the cause | Does not select LocalSystem, prove least privilege, or qualify `--list` under that context | Shows the hardware and vendor runtime can work in a service context; see `cpu-sensor-amd-service-context-qualification.md` |
| AMD-PRIVILEGE-I2 / legacy broker qualification | Can a least-privilege qualification broker launch the vendor CLI and own its IPC/process lifecycle? | LocalService / `S-1-5-19`, dedicated Service SID, Session 0 | Baseline LocalService token; no broad account mutation | Historical bounded power-path broker launch | Real | IPC, launch, ownership, and cleanup passed; package-power sampling was not the result | Broker/process boundary was not the primary failure | Counter availability, minimum capability, and production account remain unresolved | Establishes qualification-only LocalService execution plumbing; see the historical I2 section in `tools/amd-privilege-qualification/README.md` |
| AMD-PRIVILEGE-I2A | Distinct repository-authoritative slice? | N/A | N/A | N/A | N/A | `NOT_FOUND_AS_DISTINCT_SLICE` | N/A | No claim | No evidence is inferred from the name | Prevents accidental evidence inflation |
| AMD-PRIVILEGE-I2B | Does LocalService expose AMD power counters through the fixed discovery path? | LocalService / Session 0 / dedicated Service SID | Historical broker token; no new selected right | `timechart --list` | Real | `POWER_UNAVAILABLE`, no-counters diagnostic, exit 0 | Valid for the LocalService counter-discovery scope | Output discovery was not the root cause; AMD itself reported no counters | Does not identify whether account, group, device ACL, runtime, API path, or privilege combination caused it | First direct negative counter result; scope `4b30b3d64b7e469cbce7c8080c84b7d4` |
| AMD-PRIVILEGE-I2C | Does the same counter-discovery path differ under SYSTEM? | LocalSystem / `S-1-5-18`, Session 0, dedicated Service SID | SYSTEM token with broader observed privilege/group set | Same `timechart --list` and exact CLI identity | Real | `POWER_AVAILABLE`, power category present, exit 0, cleanup pass | Valid as a context-level differential; not a minimum-right proof | LocalService failure is not a hardware-wide or generic CLI failure on this host | Does not isolate account SID from groups, Service SID, privileges, device access, or runtime state | Strongest remaining positive evidence for service-context/runtime authorization; scope `091a72e1d38341ca9eca0877b1625082` |
| AMD-PRIVILEGE-I2D | Which normalized token, group, service, and AMD-object differences remain? | Read-only comparison of SYSTEM and I2F/LocalService evidence | No mutation; normalized enabled/disabled/absent states | No AMD invocation | Read-only static/metadata analysis | Differential matrix complete with exact ACL/device gaps preserved as unknown | Causal mapping not claimed | Rules out neither account nor any individual remaining capability | No exact device ACL or direct AMD SID check was recovered | Supplies the evidence ranking; see `amd-system-vs-i2f-residual-differential.md` and `amd-i2g-variable-selection.md` |
| AMD-PRIVILEGE-I2E | Does assigning `SeSystemProfilePrivilege` materialize it enabled in a LocalService token? | LocalService / dedicated Service SID / Session 0 | Right assigned; materialized `PRESENT + DISABLED` | No AMD operation after token gate | Real token-only qualification | Token gate failed as expected; rollback pass; AMD not executed | Valid only for token materialization | Assignment alone does not imply enabled token capability | Does not classify AMD counter availability or prove the right unnecessary | Explains why I2F was needed; see I2E closure in `execution-plan.md` |
| AMD-PRIVILEGE-I2F | Is `SeSystemProfilePrivilege` enabled in the LocalService service token sufficient for counter discovery? | LocalService / dedicated Service SID / Session 0 / x64 | Exactly `SeSystemProfilePrivilege` enabled via `AdjustTokenPrivileges` | `timechart --list` | Real | `POWER_UNAVAILABLE`; exit 0; full rollback | Valid for the narrow I2F sufficiency question | `SeSystemProfilePrivilege` alone is not sufficient in that LocalService context | Does not prove necessity, global uselessness, or rule out combinations/context/API/runtime causes | Provides the frozen baseline that I2G reconstructed; scope `f68bf4d3d36547a0ba753cff489bb6eb` |
| AMD-I2G Attempt #1 | Can the paired harness complete control and treatment? | Fresh LocalService / same paired identity intended | Control baseline then ProfileSingle treatment | `timechart --list` | Real | Control `POWER_UNAVAILABLE`; treatment not run; harness `OrderedDictionary` clone error; rollback pass | `false`; `SCIENTIFIC_RESULT = NOT_OBTAINED` | Only the harness failure was classified | No conclusion about ProfileSingle or AMD | Immutable harness failure; run `9ae1e7898f6b4a438f1acc41b76c2715` |
| AMD-I2G Attempt #2 | Can repaired configuration publication complete the paired transition? | Fresh LocalService / paired identity | Treatment policy mutation completed; treatment service phase not genuinely started | `timechart --list` | Real | Control `POWER_UNAVAILABLE`; treatment not discovered; destination replacement failure; rollback pass | `false`; `SCIENTIFIC_RESULT = NOT_OBTAINED` | Only the second harness failure was classified | No conclusion about ProfileSingle or AMD | Immutable harness failure; run `2eee22d181dd4fc39415a9a227afcb2c` |
| AMD-I2G Attempt #3 | Does adding only ProfileSingle change the paired LocalService result? | Same fresh LocalService / Service SID / Session 0 / x64 context in both phases | Control: SystemProfile only; treatment adds and enables ProfileSingle | Same `timechart --list` | Real | Control and treatment both `POWER_UNAVAILABLE`; paired delta and token gates pass; rollback pass | `true`, but only in frozen paired I2G context | ProfileSingle sufficiency in that context | Does not rule out service context, operation/API path, runtime/device authorization, platform details, or other capability combinations | Authoritative post-I2G decision input; run `d6d6c33003934dc5ad2b0b79307e5b2c` |

The CLI/API and vendor-context records add an important non-privilege fact:
the successful CLI path loads the public `AMDPowerProfileAPI`/CXL graph, while
the direct minimal API probe has a different process/loader context and aborts.
That divergence is real evidence, but the repository has not yet reconciled
whether `timechart --list` is a production-representative telemetry gate or an
account-sensitive enumeration path. That is the selected next task.

## Confounders and invariants

The first column is the strict paired I2G comparison. The second describes
generalization across historical slices; a value can be controlled within I2G
while remaining open across I2C/I2F/I2G because those are different evidence
scopes.

| Variable | Within paired I2G | Across historical slices | Evidence consequence |
| --- | --- | --- | --- |
| Account identity | `CONTROLLED` — LocalService in both phases | `OBSERVED_BUT_NOT_CONTROLLED` — SYSTEM and LocalService both have real results | Context family remains open; no account minimum is proved |
| Service SID | `CONTROLLED` — same fresh SID in control/treatment | `OBSERVED_BUT_NOT_CONTROLLED` — distinct qualification SIDs | SID-specific ACL effects remain possible |
| Token groups | `CONTROLLED` for the paired token delta | `OBSERVED_BUT_NOT_CONTROLLED` — SYSTEM includes Administrators and other differences | Group-derived access is not isolated |
| Integrity level | `CONTROLLED` by paired token gates | `OBSERVED_BUT_NOT_CONTROLLED` across all prior probe types | No integrity causal claim |
| Session | `CONTROLLED` — Session 0 | `CONTROLLED` for the I1/I2B/I2C/I2F/I2G service comparisons | Session 0 is not the remaining differentiator in those comparisons |
| Interactive vs service | `CONTROLLED` — service only | `OBSERVED_BUT_NOT_CONTROLLED` — early Administrator CLI was interactive | Interactive success cannot be substituted for service admission |
| CLI binary SHA/version | `CONTROLLED` — exact pinned CLI identity | `CONTROLLED` for the counter-discovery comparisons | CLI image drift is not the leading explanation for I2G |
| CLI arguments | `CONTROLLED` — fixed `timechart --list` | `OBSERVED_BUT_NOT_CONTROLLED` across sampling vs discovery slices | Operation path remains a live hypothesis |
| AMD installation | `OBSERVED_BUT_NOT_CONTROLLED` — exact CLI pinned, full install state not frozen as a counterfactual | `OBSERVED_BUT_NOT_CONTROLLED` | Driver/service/component coherence remains open |
| Working directory | `CONTROLLED` for vendor CLI runs — AMD `bin` directory | `OBSERVED_BUT_NOT_CONTROLLED` against direct API probes | Direct API and CLI are not the same loader context |
| Environment | `UNKNOWN` — no complete per-process environment snapshot | `UNKNOWN` | Do not invent an environment prerequisite |
| Driver/runtime state | `OBSERVED_BUT_NOT_CONTROLLED` — read-only inventory; user/driver versions differ | `OBSERVED_BUT_NOT_CONTROLLED` | Runtime prerequisite audit remains valuable |
| Hardware | `CONTROLLED` for this single host/CPU during paired work | `OBSERVED_BUT_NOT_CONTROLLED` over time and machines | Hardware-wide incompatibility is disfavored, not globally ruled out |
| Windows state | `OBSERVED_BUT_NOT_CONTROLLED` — no D1 mutation, but no time-invariant proof | `OBSERVED_BUT_NOT_CONTROLLED` | VBS/HVCI/hypervisor effects remain contextual |
| VBS/HVCI/hypervisor | `OBSERVED_BUT_NOT_CONTROLLED` — observed enabled and unchanged | `OBSERVED_BUT_NOT_CONTROLLED` | No counterfactual supports changing platform security state |
| Privilege assignment | `CONTROLLED` — only ProfileSingle policy delta after teardown | `OBSERVED_BUT_NOT_CONTROLLED` across I2B/I2C/I2F/I2G | I2G rules out only the selected right in its frozen context |
| Privilege enablement | `CONTROLLED` — token gates and exact enablement passed | `OBSERVED_BUT_NOT_CONTROLLED` across historical token designs | Other rights/combinations remain untested, but not automatically justified |
| Service lifecycle | `CONTROLLED` — explicit treatment completion and rollback pass | `OBSERVED_BUT_NOT_CONTROLLED` — earlier harness incidents differ | I2G is not a harness-failure explanation |

## Causal elimination

| Hypothesis | Status | Evidence | Confidence |
| --- | --- | --- | --- |
| `SeProfileSingleProcessPrivilege` alone is sufficient | `RULED_OUT_IN_FROZEN_CONTEXT` | Valid I2G control/treatment pair remained `POWER_UNAVAILABLE` after the one permitted delta | High within I2G; no global claim |
| `SeSystemProfilePrivilege` alone is sufficient | `RULED_OUT_IN_FROZEN_CONTEXT` | I2F and the I2G control both had the right enabled and returned `POWER_UNAVAILABLE` | High within LocalService counter-discovery context |
| Another token privilege is required | `WEAKLY_SUPPORTED` | SYSTEM had additional enabled rights, but no direct AMD authorization evidence identifies one | Low to medium |
| A privilege combination is required | `NOT_TESTED` | SYSTEM differs in multiple rights/groups; no minimum combination was isolated | Low |
| LocalService/service-context authorization is incompatible with the current path | `SUPPORTED` | LocalService `POWER_UNAVAILABLE` versus SYSTEM `POWER_AVAILABLE` on the same fixed `--list` path; I2G did not change the LocalService result | Medium; context is confounded |
| LocalSystem itself is the minimum required account | `BLOCKED_BY_MISSING_EVIDENCE` | SYSTEM success is correlated with account, groups, Service SID, and broader token differences | Low |
| AMD runtime/driver/device authorization or component coherence is causal | `WEAKLY_SUPPORTED` | The driver/backend boundary is plausible; exact device ACL and direct authorization check were not recovered; user/driver version split is observed | Low to medium |
| `timechart --list` is operation/API-path-specific rather than generic telemetry access | `PARTIALLY_TESTED` | CLI uses public API/CXL graph; sampling and direct API paths have different evidence; `--list` semantics are not reconciled to production sampling | Medium |
| CPU/platform is unsupported for AMD power telemetry | `DISFAVORED` | Same host produced package power in Administrator and LocalSystem CLI sessions and SYSTEM produced `POWER_AVAILABLE` | Medium; does not rule out a counter-specific platform issue |
| VBS/HVCI/hypervisor is the cause | `NOT_TESTED` | These states were observed enabled and unchanged; no safe counterfactual was run | Low |
| Stop/defer AMD production admission | `SUPPORTED` | No production account, path validity, lifecycle, installer, legal, storage, or provider integration gate is complete | High for current admission decision |

The important boundary is that I2G reduced confidence in the selected
privilege hypothesis without proving a replacement. It is evidence for a
different next question, not a license to expand privilege scope.

## Ranked candidate next slices

| Rank / candidate ID | Hypothesis family | Question | Single variable or delta | Expected information gain | Safety risk | Implementation cost | Real machine mutation required | Production relevance | Evidence already available | Blockers |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 — `AMD-CLI-LIST-PATH-VALIDITY-Q1` | Operation / API path | Is `AMDuProfCLI.exe timechart --list` a valid production-representative telemetry gate, or only an account-sensitive counter enumeration path? | Read-only operation contract: reconcile `--list` with the documented `AMDPowerProfileAPI` enumeration path and the already-qualified `timechart --event power` path | `HIGH` — can prevent another privilege/account run against the wrong diagnostic | `LOW` | `MEDIUM` | `NO` for the selected task | `HIGH` | CLI/API divergence, installed headers/sample, I1 sampling, I2C/I2F/I2G `--list` records | Vendor semantics may remain incomplete; any live path comparison needs separate authorization |
| 2 — `AMD-RUNTIME-PREREQUISITE-AUDIT-Q1` | AMD runtime / environment | Do existing AMD installation, driver, service, environment, and platform records identify a supported prerequisite gap? | Read-only prerequisite inventory and version/coherence reconciliation | `MEDIUM` — may distinguish runtime/device gaps from token theories without mutation | `LOW` | `MEDIUM` | `NO` | `HIGH` | Signed user-mode files, driver/service inventory, version split, VBS/HVCI/hypervisor observations | No undocumented dependency may be asserted; static evidence may remain inconclusive |
| 3 — `AMD-SERVICE-CONTEXT-DIFFERENTIAL-Q1` | Account / service context | Does a deliberately chosen service context explain the counter differential? | Account/context change while holding CLI identity, operation, Session 0, and harness constant; token/group consequences must be observed, not hidden | `MEDIUM` — the family is already supported by I2C, but the variable is broad and partly repeated | `MEDIUM/HIGH` | `HIGH` | `YES` in a future separately authorized run; `NO` here | `HIGH` | I1 LocalSystem sampling, I2C SYSTEM `--list`, I2G LocalService pair, normalized token matrix | No production account is authorized; a LocalSystem run would not prove least privilege |
| 4 — `AMD-PRODUCTION-ADMISSION-CLOSURE` | Stop / defer | Is there enough evidence to retire AMD research for the current product? | No machine variable; formal product/security/legal decision | `MEDIUM` — high decision value if no admissible path remains | `LOW` | `LOW` | `NO` | `HIGH` | Current unresolved account/provider/lifecycle/installer gaps | Premature until the operation-path validity question is closed |
| 5 — `AMD-HARDWARE-PLATFORM-SUPPORT-AUDIT-Q1` | Hardware / platform | Is there positive evidence for a CPU/platform-specific counter limitation? | Read-only reconciliation of CPU family/model, driver support, and unchanged VBS/HVCI/hypervisor state | `LOW` — SYSTEM success on the same host already weakens this family | `LOW` | `LOW/MEDIUM` | `NO` | `MEDIUM` | Same Ryzen host, SYSTEM success, CLI package-power success, platform flags | No evidence justifies changing VBS/HVCI/hypervisor or claiming unsupported hardware |

## I2H gate

```text
I2H_JUSTIFIED = NO
```

There is no concrete untested privilege with all five required properties:
positive evidence, an evidence-based mechanism, clean isolation, production
relevance, and priority over the operation/runtime questions. The prior
selection of `SeProfileSingleProcessPrivilege` was medium-confidence and is
now causally negative in its frozen paired context. Testing `SeDebugPrivilege`,
another SYSTEM-only right, or a privilege combination would be speculative,
broader, and less interpretable than first closing the operation/API path.

## Production account and admission gates

```text
PRODUCTION_ACCOUNT_DECISION_READY = NO
PRODUCTION_ACCOUNT_CURRENT_STATE = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
AMD_PRODUCTION_ADMISSION = DEFER
```

The missing evidence is specific:

1. Whether the current `--list` operation is the right production-relevant
   capability gate, or whether production should use a different documented
   CLI/API operation.
2. A bounded, least-privilege service context whose account, Service SID,
   token groups, device/object access, installer ownership, and lifecycle are
   supportable. SYSTEM success alone is not an authorization to promote
   LocalSystem.
3. A production architecture for IPC, output ownership, start/stop/cancel,
   update/uninstall, rollback, observability, and failure isolation.
4. Runtime/driver/version/platform compatibility and legal/distribution
   decisions, plus production metric/storage semantics. The qualification
   executable remains separate from the production Resource Timeline
   collector; no AMD Provider has been registered.

The current CLI/service approach is therefore deferred, not globally rejected.
If the selected path-validity task cannot establish a production-relevant
operation, the next disposition should become `REJECT_CURRENT_PATH` and the
AMD research path should close or move to a different documented interface.

## Selected next task

```text
SELECTED_NEXT_TASK = AMD-CLI-LIST-PATH-VALIDITY-Q1
TASK_ID = AMD-CLI-LIST-PATH-VALIDITY-Q1
GOAL = Close the operation-path question before any new privilege or account experiment.
QUESTION = Is AMDuProfCLI.exe timechart --list a production-representative AMD telemetry capability gate, or an account-sensitive counter-enumeration path whose negative result cannot justify production admission?
SCOPE = Read-only reconciliation of existing CLI/API static evidence, installed AMD header/sample semantics already recorded in the repository, the I1 sampling result, and the I2B/I2C/I2F/I2G counter-discovery results.
NON_GOALS = No AMD CLI invocation; no API load; no privilege hypothesis; no service/account mutation; no LocalSystem comparison; no provider, installer, collector, sampling, or production implementation.
ENTRY_GATE = Start from the latest clean main; confirm PR24 is merged; confirm I2G gate consumed and real execution/cleanup false; use only immutable evidence and repository/offline artifact records.
VARIABLE / INFORMATION GAP = Operation/API path selection (`timechart --list` versus the documented telemetry/sampling path); no machine variable is changed.
EXPECTED_OUTPUT = A documented route verdict (`VALID_PROXY`, `NOT_VALID_PROXY`, or `INSUFFICIENT`) with the next admission consequence and explicit remaining blockers.
STOP_CONDITIONS = Stop if vendor semantics are not present in authoritative artifacts; do not infer undocumented prerequisites; stop before any live trace or operation and request a separate task if static evidence cannot distinguish the routes.
REAL_EXECUTION_NEEDED_LATER = MAYBE, only if the read-only closure is insufficient and a separately reviewed bounded operation-specific qualification is justified.
REAL_EXECUTION_AUTHORIZED_BY_THIS_TASK = NO
NEW_HUMAN_AUTHORIZATION_REQUIRED_LATER = YES for any live AMD, service, account, privilege, API, or CLI execution; no for the read-only audit itself.
```

This task is selected over another privilege experiment because it changes the
decision boundary that I2G could not change: whether the negative `--list`
result is the right product signal. A further privilege test would add another
token variable without resolving the known account/context confounding or the
CLI/API operation mismatch.

## P2 disposition

```text
P2_EVIDENCE_CREATE_ONLY_DISPOSITION = AFTER_AMD_RESEARCH_DECISION
```

`Write-I2gAtomicJson` currently supports replacement generally, while some
evidence paths are conceptually create-once. This is not a prerequisite for
the selected read-only task and is not fixed here. Before any future real
slice, the evidence ownership decision must either restore create-only
semantics or explicitly retire that requirement with a reviewed contract. If
the AMD path is retired after research, the repair is never needed for a new
AMD run.

## Post-merge reconciliation

PR #24 is already merged. The following stale markers were found in
authoritative current-state blocks and are replaced by this decision:

```text
POST_MERGE_STALE_CURRENT_STATE_MARKERS_FOUND = YES
STALE_MARKERS = HUMAN_FINAL_REVIEW_BEFORE_MARKING_PR24_READY; PR24_FINAL_HUMAN_READY_REVIEW; I2G_REAL_RUNTIME=0; I2G_HARNESS=NOT_IMPLEMENTED in current handoff blocks
POST_MERGE_STALE_CURRENT_STATE_MARKERS_FIXED = YES
HISTORICAL_PR_TEXT_REWRITTEN = NO
```

Historical/superseded sections retain their original transition language for
traceability. The authoritative current pointers now refer to this document
and to `AMD-CLI-LIST-PATH-VALIDITY-Q1`.

## Delivery and non-execution contract

```text
RUNTIME_CODE_CHANGED = NO
REAL_PRIVILEGED_RUN = NO
AMD_CLI_REAL_INVOCATIONS = 0
SERVICE_MUTATIONS = 0
LSA_MUTATIONS = 0
TOKEN_MUTATIONS = 0
CONTROL_RUNS = 0
TREATMENT_RUNS = 0
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
```

The intended validation is documentation/current-state consistency plus
`git diff --check`. If the PowerShell current-state assertion file is updated,
only PowerShell parse validation is required for that file. No Rust suite is
required because runtime code is unchanged.

The performed validation is:

```text
GIT_DIFF_CHECK = PASS
POWERSHELL_ASSERTION_PARSE = PASS
CURRENT_STATE_MARKERS = PASS
EXISTING_QUALIFICATION_TEST = BLOCKED_BEFORE_DOCUMENTATION_ASSERTIONS
EXISTING_QUALIFICATION_TEST_BLOCKER = PINNED_RELEASE_ARTIFACT_SHA_MISMATCH
REBUILT_ARTIFACT_SHA256 = 37D4C3EC25F5F1607372BC78C0F35CF36511EBDF37E0F67D9F350475C36A1988
PINNED_ARTIFACT_SHA256 = 2613129D179EA2A0496AD680E68E77A79FFFBB569D0802A11AC03346E162DD80
RUST_RUNTIME_CHANGED = NO
```

The failed check is an artifact identity/environment mismatch in the existing
qualification contract. It was not weakened or repaired in this decision task.
