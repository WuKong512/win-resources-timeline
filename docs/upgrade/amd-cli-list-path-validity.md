# AMD-CLI-LIST-PATH-VALIDITY-Q1

This is an offline, read-only operation/API-path validity audit. It consumes
committed repository evidence and previously recorded vendor artifacts only. It
does not execute `AMDuProfCLI.exe`, load an AMD API, run sampling, change a
token/account/service/ACL/device/runtime, or modify historical raw evidence.

## Decision

```text
TASK_ID = AMD-CLI-LIST-PATH-VALIDITY-Q1
REPOSITORY = WuKong512/win-resources-timeline
BASELINE_MAIN = 94c0c98e057c25010294ff32dca99c430f81ce1d
AUDIT_MODE = OFFLINE_READ_ONLY
RESULT = PASS_WITH_EXTERNAL_BLOCKERS
TASK_RESULT = COMPLETE / INSUFFICIENT
PR_CREATION = BLOCKED_GITHUB_SIGN_IN_REQUIRED
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
QUALIFICATION_BLOCKER_TASK_CAUSED = NO
QUALIFICATION_BLOCKER_SCOPE = PRE_EXISTING / OUT_OF_SCOPE / NOT_REPAIRED
PRE_EXISTING_SHA_MISMATCH_REPAIRED = NO
I2G_CURRENT_STATE = COMPLETE / ATTEMPT3_AUTHORITATIVE
I2G_REAL_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
PRODUCTION_ACCOUNT = UNRESOLVED
AMD_PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
CLI_LIST_ROLE = ACCOUNT_SENSITIVE_COUNTER_ENUMERATION / DISCOVERY_ONLY
SAMPLING_PATH_ROLE = PRODUCTION_RELEVANT_PROFILE_CONFIGURE_START_READ_STOP_PATH
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
ACCOUNT_SENSITIVITY = CONFIRMED_FOR_DISCOVERY; SAMPLING_ACCOUNT_SEMANTICS_UNRESOLVED
OPERATION_PATH_CONFOUNDER = MATERIAL_AND_UNRESOLVED
ROUTE_VERDICT = INSUFFICIENT
VERDICT_CONFIDENCE = LOW
SELECTED_NEXT_TASK = NONE
NEXT_GATE = HUMAN_REVIEW_OPERATION_PATH_EVIDENCE_GAP
```
The audit does not establish `VALID_PROXY` or `NOT_VALID_PROXY`. The
repository does establish that the tested `--list` operation is a bounded
counter-discovery operation and that the documented public API/sample has
separate enumeration, configuration, start/read, stop, and close stages. It
does not establish the missing implication that a negative `--list` result is
a necessary failure of the production-relevant sampling path under the same
authorization semantics. Conversely, no same-context historical result shows
sampling succeeding while `--list` is unavailable. The correct route verdict
is therefore `INSUFFICIENT`, not a stronger binary conclusion.

For current admission safety, a negative `--list` result must remain scoped to
counter discovery. It is not admitted as a sole production-sampling blocker or
as a production-sampling capability proof until the missing semantic link is
closed.

## Entry gate and authoritative state

The required entry checks completed before documentation changes:

- `git fetch origin --prune` completed.
- `origin` resolves to `https://github.com/WuKong512/win-resources-timeline.git`.
- `origin/main` was `94c0c98e057c25010294ff32dca99c430f81ce1d`, the merge of the
  post-I2G decision PR.
- `docs/upgrade/amd-post-i2g-production-admission.md` exists on that commit.
- The authoritative handoff records `I2G = COMPLETE / ATTEMPT3_AUTHORITATIVE`;
  the exact `I2G_CURRENT_STATE = COMPLETE / ATTEMPT3_AUTHORITATIVE` marker is
  present in the qualification README. The same handoff records the consumed
  real gate, false real execution/cleanup permissions, `ATTEMPT4_AUTHORIZATION
  = NOT_GRANTED`, unresolved production account, deferred admission, no I2H,
  and this task as the selected next task at entry.
- The checkout was clean and detached at the verified `origin/main` before the
  requested branch was created. The audit branch is based directly on that
  commit.

The duplicate gate found no matching task ID in repository text, commit
history, local branches, fetched remote branches, or fetched pull-request head
refs. `gh` is not installed and the unauthenticated local GitHub API request
failed, so this record does not claim a complete authenticated GitHub PR-list
query. No matching implementation or PR head was present in the available
repository/remote evidence.

## Evidence boundary

The stage vocabulary used below is deliberately explicit:

1. CLI process can start.
2. AMD runtime/library can load.
3. Counter/category enumeration succeeds.
4. A power counter/category is advertised.
5. A power session can be configured.
6. Power sampling can start.
7. Samples can be produced/read.
8. Samples are parseable and semantically usable.
9. The path works in a service context.
10. The path works under a production-admissible least-privilege account.

Stages 1–4 are discovery-side facts. Stages 5–8 are active sampling facts.
Stage 9 is context-specific, and stage 10 has not been established by any
historical AMD evidence.

## Historical evidence reconciliation

### Administrator CLI sampling

The immutable Administrator records in
[`cpu-sensor-amd-cli-spike-runtime.md`](../measurements/cpu-sensor-amd-cli-spike-runtime.md)
and the Administrator comparison in
[`cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md`](../measurements/cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md)
used the vendor-owned x64 `AMDuProfCLI.exe` 5.3.521.0 from its AMD `bin`
directory. A bounded command of the form
`timechart --event power --interval 1000 --duration ... --format csv`
completed with exit code zero and produced finite, parseable socket package
power samples. The records establish stages 1, 2, 5, 6, 7, and 8 for an
interactive Administrator context. They do not establish a separately traced
`--list` call, a service account result, Session 0 least privilege, or a
production admission contract. The provider architecture also records an
Administrator `--list` success, but that success does not make the discovery
and sampling operations equivalent.

### AMD-SERVICE-CONTEXT-I1

The immutable I1 record in
[`cpu-sensor-amd-service-context-qualification.md`](../measurements/cpu-sensor-amd-service-context-qualification.md)
used a genuine LocalSystem (`S-1-5-18`) Windows service in Session 0, x64,
with the registry-derived AMD CLI path and AMD `bin` working directory. Its
fixed operation was actual power sampling:

```text
timechart --event power --interval 1000 --duration 10 --format csv --output-dir <service-owned-root>
```

It did not use `timechart --list`. The service run completed with nine
parseable package-power samples and passed its cadence and cleanup gates. It
therefore establishes the vendor CLI/runtime sampling path in this specific
LocalSystem/Session 0 context, including stages 1, 2, 5–9. It does not show
that LocalService can sample, that `--list` is necessary for that sampling
path, that LocalSystem is least privilege, or that LocalSystem is an admitted
production account.

### I2B, I2C, I2F, and I2G discovery records

The historical I2 records intentionally used the same signed x64 CLI and the
fixed non-sampling operation `timechart --list`:

- I2B/I2C establish a LocalService `POWER_UNAVAILABLE` discovery result and a
  SYSTEM `POWER_AVAILABLE` discovery result. The SYSTEM side advertises the
  power category; it does not produce samples.
- I2F kept LocalService and enabled exactly `SeSystemProfilePrivilege` in the
  service token, then still obtained `POWER_UNAVAILABLE`. Its valid result is
  that this single right did not make discovery available in that tested
  LocalService context. It is not a sampling result.
- I2G Attempt #3 CONTROL and TREATMENT used the same fresh LocalService,
  unrestricted Service SID, Session 0, x64 harness, CLI identity, and
  `timechart --list`. Adding only `SeProfileSingleProcessPrivilege` left both
  results at `POWER_UNAVAILABLE`. The frozen causal conclusion remains only
  `PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT`.

Every I2 discovery run exercised the discovery operation through stages 1–3
and stage 9 for its specific service context. I2C established the positive
Stage 4 outcome because the power category was advertised; I2B, I2F, and I2G
produced the Stage 4 negative outcome because `POWER_UNAVAILABLE` means the
power category was not advertised. The records explicitly report zero
power-sampling runs, so stages 5–8 were not exercised by I2B, I2C, I2F, or
I2G. None establishes stage 10.
I2G Attempt #3 is immutable, authoritative historical evidence; this audit
does not broaden its account, privilege, operation, or production meaning.

### Direct API, source, and static qualification

The recorded installed-header/sample evidence in
[`cpu-sensor-amd-uprof-live-qualification.md`](../measurements/cpu-sensor-amd-uprof-live-qualification.md)
and the static analysis in
[`cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md`](../measurements/cpu-sensor-amd-uprof-cli-vs-direct-api-divergence.md)
give the following public API semantic order:

```text
AMDTPwrProfileInitialize
  -> AMDTPwrGetSupportedCounters / counter metadata
  -> AMDTPwrEnableCounter
  -> AMDTPwrSetTimerSamplingPeriod
  -> AMDTPwrStartProfiling
  -> AMDTPwrReadAllEnabledCounters
  -> AMDTPwrStopProfiling
  -> AMDTPwrProfileClose
```

The official `CollectAllCounters` sample source is recorded as using that
sequence. The API header describes `GetSupportedCounters` as returning
descriptors and describes `ReadAllEnabledCounters` as returning vendor-owned
sample memory. The static CLI import graph contains the public power API,
metadata/enumeration symbols, and lifecycle symbols, while the saved CLI
debugger observation reached `AMDTPwrProfileInitialize(0)`. This confirms the
executable/static public API/CXL dependency graph recorded for the vendor CLI;
it supports, but does not confirm, per-operation runtime sharing or
authorization semantics. It does not trace which exact API call is made for
`--list`, nor prove that the CLI applies the same authorization check to
discovery and active sampling.

The direct-loader records are not sampling evidence. An isolated direct
`LoadLibraryExW`/dependency path reached the CXL fatal-exit boundary before
API initialization, while the full vendor CLI succeeded. The divergence
establishes that a direct API loader and the vendor CLI are materially
different process/load paths; it does not prove that a direct-load failure is
a CLI sampling failure, and it does not map the CLI `--list` command to one
specific API call.

## Operation-path matrix

`Not separately observed` means that the stage may be part of a vendor
implementation, but the committed evidence does not prove it. It must not be
converted into a positive or negative runtime claim.

| Evidence slice | Account/context | Operation | Enumeration path | Sampling path | Runtime/API stage outcomes | Result | Production relevance | Valid inference | Invalid inference |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Administrator successful sampling | Administrator, interactive x64 PowerShell, High integrity, AMD `bin` CWD | `timechart --event power --interval 1000 --duration 5/10 --format csv` | Not separately traced; source/API sequence supports selection | Yes: bounded configure/start/read/stop/close semantics | 1, 2, 5, 6, 7, 8 | Exit 0; parseable package-power samples | Strong bounded CLI sampling evidence; not least privilege | Admin interactive sampling works for the recorded CLI identity | `--list` failure under another context makes sampling impossible; service/production account works |
| AMD-SERVICE-CONTEXT-I1 | LocalSystem `S-1-5-18`, genuine service, Session 0, x64 | `timechart --event power --interval 1000 --duration 10 --format csv` | No `--list` run | Yes; nine samples and cadence passed | 1, 2, 5–9 | `PACKAGE_POWER_STATUS = PASS` | Strong service-context sampling feasibility; not minimum privilege | The CLI sampling path works in this LocalSystem/Session 0 context | LocalSystem is least privilege; LocalService `--list` failure blocks all sampling |
| I2B LocalService `--list` | LocalService `S-1-5-19`, qualification service, Session 0 | `timechart --list` | Yes; fixed counter discovery | No sampling arguments or output session | 1–3, 4 negative, 9 | `POWER_UNAVAILABLE` / no-counters result | Discovery-only account differential | This context did not advertise power through `--list` | Active sampling was attempted or proven impossible |
| I2C SYSTEM `--list` | LocalSystem `S-1-5-18`, dedicated service, Session 0, x64 | `timechart --list` | Yes; power category advertised | No | 1–4, 9 | `POWER_AVAILABLE`; power category present | Discovery positive under SYSTEM; no sample qualification | Discovery is account/context-sensitive and can succeed under SYSTEM | `--list` success proves parseable sampling or least privilege |
| I2F LocalService `--list` | LocalService, dedicated Service SID, Session 0; `SeSystemProfilePrivilege` enabled | `timechart --list` | Yes | No | 1–3, 4 negative, 9 | `POWER_UNAVAILABLE`; exit 0; no counters | Valid preregistered single-right discovery result | That right alone did not restore discovery in this context | The right is unnecessary/ineffective for every sampling context |
| I2G Attempt #3 CONTROL | Fresh LocalService + same Service SID, Session 0, x64; SystemProfile control state | `timechart --list` | Yes | No | 1–3, 4 negative, 9; paired control | `POWER_UNAVAILABLE` | Authoritative paired discovery baseline only | Control establishes the frozen I2G baseline | It is a production sampling baseline |
| I2G Attempt #3 TREATMENT | Same paired context; adds only `SeProfileSingleProcessPrivilege` | `timechart --list` | Yes | No | 1–3, 4 negative, 9; paired treatment | `POWER_UNAVAILABLE` | Causal validity is limited to the frozen discovery pair | The selected privilege was insufficient in paired I2G context | The privilege is irrelevant to active sampling or all accounts |
| Direct API/load qualification | Isolated diagnostic child versus full vendor CLI; separate loader contexts | `LoadLibraryExW`/init-only and static import analysis | No successful enumeration | No sampling | Direct child: process/load/dependency boundary; CLI comparator: public API/CXL graph | Direct load aborts before API init; CLI sampling succeeds | Loader/API-path qualification only | Direct loader and CLI are divergent process/load paths; CLI uses the public API graph | Direct-loader failure proves CLI sampling failure or API/CLI semantic identity |

## API / CLI semantic mapping

This mapping distinguishes vendor/API semantics from repository operation
labels. `CONFIRMED` means the evidence directly records the edge. `SUPPORTED`
means the installed header/sample or static graph supports it but does not
provide a per-command trace. `PLAUSIBLE` is not used as established behavior.

| CLI/API edge | Classification | Evidence boundary |
| --- | --- | --- |
| `timechart --list` is the fixed repository counter-discovery operation | `CONFIRMED` | Fixed arguments, `sampling=false`, `COUNTER_DISCOVERY`, and discovery result artifacts in the I2 harness/source |
| `timechart --list` starts the CLI and reaches a vendor command path | `CONFIRMED` for launched historical runs | Spawn evidence, exit status, bounded stdout/stderr, and no-counters/category output |
| `timechart --list` loads the same installed public API/CXL graph as the vendor CLI | `SUPPORTED` | CLI PE imports, saved initialization observation, and successful CLI process; no event-level `--list` module trace |
| `timechart --list` invokes `AMDTPwrProfileInitialize` | `SUPPORTED`, exact command mapping unresolved | Header ordering plus saved CLI initialization observation; the observation is not a complete command-to-call trace |
| `timechart --list` invokes `AMDTPwrGetSupportedCounters` or equivalent counter metadata calls | `SUPPORTED` at semantic level; exact function `UNKNOWN` | The command is discovery-only and the API/sample define descriptor enumeration, but no vendor source/call trace is recorded |
| `--list` result `POWER_AVAILABLE` means a power category is advertised | `CONFIRMED` at the repository result contract | SYSTEM stdout/result evidence records category presence; exact vendor status-to-output mapping is not independently documented |
| `--list` configures, starts, reads, stops, or closes a power sampling session | `UNKNOWN` / not exercised by the qualification contract | No sampling arguments, no output CSV, and zero recorded sampling runs; internal vendor preflight behavior is not documented |
| `timechart --event power` follows the public initialize/enumerate/enable/timer/start/read/stop/close semantic sequence | `SUPPORTED` for API semantics; active CLI sampling is `CONFIRMED` | Installed header/sample plus successful parseable CLI sessions; exact CLI call trace is absent |
| `timechart --event power` reaches active start/read/stop semantics | `SUPPORTED` at API-call identity; `CONFIRMED` at CLI-operation level | Successful session, profile-finished output, CSV samples, cadence, and clean completion; no API hook/trace |
| Direct `AMDPowerProfileAPI.dll` load is equivalent to the vendor CLI path | `NOT_SUPPORTED` | Direct loader abort and full-CLI success occur in materially different process/module contexts |
| A `--list` negative is a necessary failure of `timechart --event power` sampling | `UNKNOWN` | No authoritative CLI source/help or same-context operation pair establishes necessity |

The decisive unknown is the last edge. Shared DLL imports and the API sample's
enumeration-before-start order do not prove that the CLI's `--list` result is
the exact prerequisite that its `--event power` handler requires, or that both
operations use the same account-sensitive authorization check.

## Confounder matrix

| Confounder | Classification | Boundary |
| --- | --- | --- |
| Account identity | `OBSERVED_BUT_NOT_CONTROLLED` across evidence; `CONTROLLED` within I2G | SYSTEM/LocalService/Admin differ; I2G holds LocalService fixed |
| LocalService vs LocalSystem | `OBSERVED_BUT_NOT_CONTROLLED` | The discovery differential and I1 sampling comparison change account and operation together |
| Service SID | `CONTROLLED` within I2G; `OBSERVED_BUT_NOT_CONTROLLED` across I1/I2 | I2G reuses one Service SID; SYSTEM and other qualification services differ |
| Token groups | `OBSERVED_BUT_NOT_CONTROLLED` | SYSTEM includes Administrators; no group was normalized for a sampling comparison |
| Privileges | `CONTROLLED` for the I2G treatment delta and I2F enablement; otherwise `OBSERVED_BUT_NOT_CONTROLLED` | I2G changed only ProfileSingle in its paired context; SYSTEM has broader residual differences |
| Integrity | `OBSERVED_BUT_NOT_CONTROLLED` across Admin/service evidence; `CONTROLLED` within service pairs | Admin is High; service runs are System integrity |
| Session 0 | `CONTROLLED` for I1/I2 service records; `OBSERVED_BUT_NOT_CONTROLLED` versus Admin | I1/I2 are service contexts; Admin sampling is interactive |
| Working directory | `CONTROLLED` for recorded CLI harnesses; `OBSERVED_BUT_NOT_CONTROLLED` versus direct loader | CLI uses AMD `bin`; direct probes use a different diagnostic directory |
| Process environment | `OBSERVED_BUT_NOT_CONTROLLED` / partially recorded | No complete per-process environment equality record exists for every slice |
| CLI arguments | `CONTROLLED` within each operation; intentionally different across operations | `--list` and `--event power` are distinct argument vectors |
| CLI executable identity/version | `CONTROLLED` for the recorded I1/I2 CLI evidence | Same recorded signed x64 5.3.521.0 identity; direct API is a different executable |
| AMD runtime/backend/service/driver state | `OBSERVED_BUT_NOT_CONTROLLED` | Installed driver/service/version observations exist; no state counterfactual was run here |
| Device/object authorization | `UNKNOWN` | No authoritative device/interface ACL or backend authorization semantics were recovered |
| Public API versus CLI wrapper behavior | `OBSERVED_BUT_NOT_CONTROLLED` | Static graph supports shared API use; exact wrapper call behavior is not available |
| Enumeration versus active sampling | `CONTROLLED` as an operation distinction | Harness explicitly separates `--list` from actual power-session execution |
| Output path/session lifecycle | `OBSERVED_BUT_NOT_CONTROLLED` across operations | `--list` has bounded stdout/stderr; sampling creates CSV/UPROF and a session |
| Interactive versus service execution | `OBSERVED_BUT_NOT_CONTROLLED` across evidence | Admin, LocalSystem, and LocalService contexts are not a single controlled matrix |

No causal claim is made from a confounder marked `OBSERVED_BUT_NOT_CONTROLLED`
or `UNKNOWN`.

## Proxy validity criteria and verdict

The criteria were fixed before the route verdict:

| Criterion | Required for `VALID_PROXY` | Evidence status |
| --- | --- | --- |
| Necessary prerequisite | `--list` must exercise a prerequisite that production sampling necessarily requires | `SUPPORTED` only at the public API sequence level; exact CLI necessity `UNKNOWN` |
| Shared authorization/runtime/device/API dependency | The relevant dependency and authorization semantics must be shared | Executable/static public API/CXL dependency graph `CONFIRMED`; per-operation runtime sharing and authorization semantics `UNKNOWN` |
| No counterexample | No historical same-context sampling success while the corresponding discovery capability is unavailable | No counterexample is recorded, but no same-context pair was run; criterion remains unproven |
| No material discovery confounder | Discovery must not depend on materially different account-sensitive semantics | Account-sensitive discovery is positively observed; criterion is not satisfied |

The evidence is not strong enough for `VALID_PROXY`. It is also not strong
enough for the stronger `NOT_VALID_PROXY` route as a factual claim about
sampling impossibility: no same-context `--list` negative plus successful
sampling result exists, and the public API sample leaves open whether
enumeration is a necessary sampling prerequisite. The exact CLI command-to-API
mapping is absent from the recorded vendor artifacts. Under the task's
fail-closed rule, the route verdict is:

```text
ROUTE_VERDICT = INSUFFICIENT
```

The operational consequence is narrower than either binary route: historical
negative `--list` results remain valid discovery evidence, but they cannot by
themselves block or admit production sampling. `AMD_PRODUCTION_ADMISSION`
therefore remains `DEFER` and the production account remains unresolved.

## Decision consequences for future AMD work

### If later evidence establishes `VALID_PROXY`

The prior LocalService `--list` negatives could then legitimately establish a
production-relevant capability failure for that exact account/context and
operation, but still would not select LocalSystem or prove a least-privilege
account. I2F and I2G would remain scoped to their preregistered variables.
Future work would move to read-only runtime/prerequisite and deployment review,
not automatically to another privilege experiment. A new live run would be
needed only if a separately reviewed production account still required active
qualification.

### If later evidence establishes `NOT_VALID_PROXY`

I2B, I2C, and I2F would remain scientifically valid counter-discovery records,
and I2G Attempt #3 would retain its paired discovery-only causal result, but
their negative results would lose production-admission force for active
sampling. The next production-representative operation would be a bounded
`timechart --event power` sampling qualification in a separately chosen,
supportable context. That live operation would require separate human
authorization; this audit does not authorize it.

### Current `INSUFFICIENT` consequence

The missing evidence can first be sought offline: an authoritative AMD CLI
help/source/API artifact that maps `timechart --list` to exact enumeration and
authorization stages, and states whether active power sampling requires the
same capability. If that research remains unavailable, the minimal
distinguishing live slice would be an operation-specific, same-context pair:
run `--list` and then one bounded `timechart --event power` session with the
same CLI identity, working directory, environment, token, service context, and
runtime state, without changing privileges or account state. That slice is a
future proposal only. It is not authorized by this task, and no operation-path
experiment was performed here.

No next task is selected until a human reviews this evidence gap; choosing a
new privilege, account, or runtime experiment from this unresolved route would
exceed the evidence.

## Current-state handoff

```text
AMD_CLI_LIST_PATH_VALIDITY_Q1 = COMPLETE / INSUFFICIENT
CLI_LIST_ROLE = ACCOUNT_SENSITIVE_COUNTER_ENUMERATION / DISCOVERY_ONLY
SAMPLING_PATH_ROLE = PRODUCTION_RELEVANT_PROFILE_CONFIGURE_START_READ_STOP_PATH
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
I2G_CURRENT_STATE = COMPLETE / ATTEMPT3_AUTHORITATIVE
I2G_REAL_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
I2G_CAUSAL_INTERPRETATION_CHANGED = NO
PRODUCTION_ACCOUNT = UNRESOLVED
AMD_PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
SELECTED_NEXT_TASK = NONE
SELECTED_NEXT_TASK_GOAL = NONE_PENDING_HUMAN_REVIEW_OF_OPERATION_PATH_EVIDENCE_GAP
NEXT_GATE = HUMAN_REVIEW_OPERATION_PATH_EVIDENCE_GAP
HUMAN_AUTH_REQUIRED_LATER = YES_FOR_ANY_FUTURE_LIVE_AMD_OPERATION
RESULT = PASS_WITH_EXTERNAL_BLOCKERS
TASK_RESULT = COMPLETE / INSUFFICIENT
PR_CREATION = BLOCKED_GITHUB_SIGN_IN_REQUIRED
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
QUALIFICATION_BLOCKER_TASK_CAUSED = NO
QUALIFICATION_BLOCKER_SCOPE = PRE_EXISTING / OUT_OF_SCOPE / NOT_REPAIRED
PRE_EXISTING_SHA_MISMATCH_REPAIRED = NO
```

I2G Attempt #3 remains immutable and authoritative. This audit changes no
I2G causal interpretation and creates no new real gate.

## Audit non-execution record

```text
AMD_CLI_REAL_INVOCATIONS = 0
AMD_API_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
SERVICE_MUTATIONS = 0
LSA_MUTATIONS = 0
TOKEN_MUTATIONS = 0
ACL_MUTATIONS = 0
DEVICE_MUTATIONS = 0
RUNTIME_CODE_CHANGED = NO
RUST_RUNTIME_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
```
