# AMD-OPERATION-PATH-EVIDENCE-GAP-D1

This is an offline, read-only decision record for the post-Q1 operation-path
evidence gap. It does not execute `AMDuProfCLI.exe`, load an AMD API, start a
profiling session, sample power, change a service/account/token/LSA/ACL/device/
driver/platform-security state, run I2H, or create Attempt #4.

## 1. Entry state

The required entry gate was completed before this document was created.

```text
TASK_ID = AMD-OPERATION-PATH-EVIDENCE-GAP-D1
REPOSITORY = WuKong512/win-resources-timeline
REMOTE = https://github.com/WuKong512/win-resources-timeline.git
ENTRY_MAIN = c4f972b4d0668f8fe9198f262d172a53cad2f2d8
ORIGIN_MAIN = c4f972b4d0668f8fe9198f262d172a53cad2f2d8
ENTRY_HEAD = c4f972b4d0668f8fe9198f262d172a53cad2f2d8
MERGE_BASE = c4f972b4d0668f8fe9198f262d172a53cad2f2d8
BRANCH = audit/amd-operation-path-evidence-gap-d1
ENTRY_WORKING_TREE = CLEAN
ENTRY_CHECKOUT_STATE = DETACHED_AT_ORIGIN_MAIN_BEFORE_BRANCH
PR26_MERGED = YES
PR26_VERIFICATION = origin/main merge commit title is "Merge pull request #26 from WuKong512/audit/amd-cli-list-path-validity-q1"
PR26_MERGE_COMMIT = c4f972b4d0668f8fe9198f262d172a53cad2f2d8
```

The repository state was fetched with `git fetch origin --prune`. The GitHub
CLI was not installed, so PR status was verified from the fetched merge commit
and remote history rather than an authenticated `gh` query. No newer
`origin/main` commit was present.

The required inherited invariants are unchanged:

```text
AMD_CLI_LIST_PATH_VALIDITY_Q1 = COMPLETE / INSUFFICIENT
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
AMD_PRODUCTION_ADMISSION = DEFER
PRODUCTION_ACCOUNT = UNRESOLVED
I2H_JUSTIFIED = NO
I2G_CURRENT_STATE = COMPLETE / ATTEMPT3_AUTHORITATIVE
I2G_REAL_GATE_CONSUMED = true
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
```

The pre-existing qualification artifact SHA mismatch remains separate and was
not repaired:

```text
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
QUALIFICATION_BLOCKER_TASK_CAUSED = NO
```

## 2. Q1 inventory, treated as input rather than new evidence

The parent Q1 record already established the following. D1 does not count
these facts as new information:

| Q1 input | Existing scope | D1 treatment |
| --- | --- | --- |
| `timechart --list` is a bounded account-sensitive counter-discovery operation | Existing qualification harness and immutable I2B/I2C/I2F/I2G records | Duplicate input; not reclassified |
| I2B/I2F/I2G LocalService discovery is `POWER_UNAVAILABLE`; I2C SYSTEM discovery is `POWER_AVAILABLE` | Session 0 service discovery, with I2G Attempt #3 authoritative for its paired privilege delta | Duplicate input; not a sampling result |
| I1 LocalSystem active sampling passes; Administrator active sampling passes | Different historical contexts using `timechart --event power` | Duplicate input; not a same-context pair |
| Public API/sample sequence and CLI public API/CXL static dependency evidence exist | Retained headers/sample summaries, PE/import records, and prior analysis | Duplicate input; static dependency is not a runtime trace |
| Exact per-command API call trace is absent | Q1 operation matrix and semantic mapping | Remains the decisive unresolved boundary |

The question under review is therefore narrower than Q1: whether the vendor
meaning of discovery and the documented sampling sequence is enough to prove
the same capability and authorization gate for the exact CLI operations.

## 3. Evidence ledger

`New for D1` means new semantic information relative to the Q1 evidence
record, not merely a fresh hash or a second citation to the same fact.

| Evidence | Already used by Q1? | New for D1? | Authority level | Can change proxy verdict? | Notes |
| --- | --- | --- | --- | --- | --- |
| Retained Q1 operation matrix, I1/I2B/I2C/I2F/I2G measurements, and current-state records | Yes | No | Repository-authoritative immutable records | No | Reconciled as input; no operation was rerun |
| Retained public API/header/sample semantic order | Yes at the high-level sequence | No for the sequence | Vendor-derived material retained by the repository | No by itself | D1 checks the original installed provenance but does not count the known order twice |
| Retained PE/import and CLI/API divergence analysis | Yes | No | Repository static analysis | No | Shared imports remain non-equivalent to a per-operation trace |
| `D:\apps\AMDuProf\bin\Help\text\AMDuProf_Timechart_Help.txt` | No; Q1 explicitly left authoritative CLI help unresolved | Yes | AMD uProf vendor CLI help | Partially | Establishes documented discovery/presentation semantics and the user-facing relationship to event selection; does not state internal calls or authorization equivalence |
| `D:\apps\AMDuProf\bin\Help\text\AMDProfilerService_Help.txt` | No | Yes | AMD uProf vendor service help | No direct change | Documents a separate remote-client authorization layer and a cautionary `--bypass-auth`; it does not map local `timechart --list` to active sampling authorization |
| `D:\apps\AMDuProf\Help\AMDPowerProfilerAPI.pdf` | The API order was already represented; the direct manual provenance was not | Yes as direct provenance, not as a new order | AMD Developer Tools Team, `AMDPowerProfile API User Guide`, Release v1.2 | Partially | Confirms driver initialization, counter enumeration, enable/start/read/stop/close semantics and distinct error classes; it does not describe the CLI wrapper's per-command implementation |
| Installed `AMDTPowerProfileApi.h`, `AMDTPowerProfileDataTypes.h`, `AMDTDefinitions.h`, and `CollectAllCounters.cpp` | Yes for the public sequence; explicit device/error details were not used as a CLI trace | Partially | AMD public headers and official sample source | Partially | `m_isAccessible`, category masks, `COUNTER_NOT_ACCESSIBLE`, driver/platform/BIOS/hypervisor/access errors identify possible API-stage failures but not account/SID/ACL equivalence |
| Installed version metadata, CLI/API hashes, signatures, import libraries, and PE import graph | Yes | No | Repository-retained static analysis, cross-checked locally | No | Same static dependency class; no exact `--list`/`--event` runtime trace |
| Installed release notes, user guide, source, SDK, and sample inventory beyond the files above | No matching additional semantic artifact found | Search result only | No additional artifact | No | No vendor source for the CLI command dispatcher or per-operation authorization was present |

## 4. New authoritative offline evidence

The installed tree is the machine-local AMD uProf `5.3.521.0 (Public)` tree.
The version was read from `bin\AMDPerf\metadata\version.txt` and
`bin\Data\Config\Version.txt`. The following files were read without
executing or loading any AMD image.

### 4.1 Vendor CLI help

```text
FILE_PATH = D:\apps\AMDuProf\bin\Help\text\AMDuProf_Timechart_Help.txt
VENDOR = Advanced Micro Devices / AMD Developer Tools
PRODUCT_VERSION = 5.3.521.0 (Public)
FILE_HASH_SHA256 = FD9615E1DA6BE4D14BF197B2F41442C1AB236A7687419E0DE69293BB2038492B
DOCUMENT_TITLE = AMDuProfCLI timechart
SECTION = DESCRIPTION / OPTIONS / EXAMPLES
EVIDENCE_CLASS = VENDOR_CLI_HELP
```

Relevant statements are:

- `--list Display all the supported devices and categories.`
- `--event ... Collect counters for specified combination of device type and/or category type.`
- `Use command 'timechart --list' for the list of supported devices and categories.`
- The power example is an active collection command using `--event power`, an
  interval, and a duration.

This is new authoritative evidence that `--list` is a documented
device/category discovery and presentation operation, and that its output is
the documented source for selecting event values. It does not say that
`--list` must perform the same internal initialization, accessibility check,
or authorization check as an active event, and it does not map the repository
`POWER_AVAILABLE`/`POWER_UNAVAILABLE` result contract to a specific API call.

### 4.2 Vendor public API guide and headers

```text
PDF_PATH = D:\apps\AMDuProf\Help\AMDPowerProfilerAPI.pdf
VENDOR = Advanced Micro Devices / AMD Developer Tools Team
PRODUCT_VERSION = Installed beside AMD uProf 5.3.521.0 (Public); manual title reports Release v1.2
FILE_HASH_SHA256 = DF36EA5E0F87893F39F3DA546A4AAF97060424C3BA1FD42FBC6ACB24C642E017
DOCUMENT_TITLE = AMDPowerProfile API User Guide
SECTIONS = 1.1, 1.2.1-1.2.10, 1.3, 1.4, 1.5
EVIDENCE_CLASS = VENDOR_API_MANUAL
```

The manual and matching headers state that:

1. `AMDTPwrProfileInitialize` loads and initializes the AMDT Power Profile
   drivers and should be called first. Documented failures include driver
   unavailable, driver-version mismatch, unsupported platform, and prior
   session state.
2. `AMDTPwrGetSupportedCounters` returns the counters supported by the
   platform and requires successful profile initialization.
3. The official `CollectAllCounters.cpp` sample obtains supported counters,
   enables counters, sets the timer, starts profiling, reads samples, stops,
   and closes the profile.
4. The public API documents `AMDT_ERROR_COUNTER_NOT_ACCESSIBLE`,
   `AMDT_ERROR_SMU_ACCESS_FAILED`, `AMDT_ERROR_ACCESSDENIED`,
   `AMDT_ERROR_HYPERVISOR_NOT_SUPPORTED`, and
   `AMDT_ERROR_BIOS_VERSION_NOT_SUPPORTED` as distinct possible failure or
   support conditions at different stages.
5. `AMDTPwrDevice` has an `m_isAccessible` field and a category mask, while
   `AMDTPwrCounterDesc` identifies a device and category.

These statements support a general API-level relationship between supported
counter discovery and active counter configuration. They do not provide the
missing exact CLI call trace or identify whether an account, group, service
SID, privilege, device ACL, or backend authorization is checked identically by
`--list` and `--event power`.

### 4.3 Vendor service help

```text
FILE_PATH = D:\apps\AMDuProf\bin\Help\text\AMDProfilerService_Help.txt
VENDOR = Advanced Micro Devices / AMD Developer Tools
PRODUCT_VERSION = 5.3.521.0 (Public)
FILE_HASH_SHA256 = CE9DFD667F2E064BDBA01C98458B5FFF7A219454B652BE3C425268ABFFC227B4
DOCUMENT_TITLE = AMDProfilerService
SECTION = DESCRIPTION / OPTIONS / USAGE
EVIDENCE_CLASS = VENDOR_SERVICE_HELP
```

The document says that `AMDProfilerService` lets remote AMD uProf clients
execute performance and power profiling sessions, provides `--add`,
`--clear-user`, and `--clear-all`, and has a cautionary `--bypass-auth` option
for client authorization. This establishes a distinct documented service
client authorization layer. It does not establish that the historical
LocalService/SYSTEM CLI runs used this service, nor that its client check is
the counter capability gate in `timechart --list`.

### 4.4 Provenance cross-checks

```text
CLI_FILE = D:\apps\AMDuProf\bin\AMDuProfCLI.exe
CLI_VERSION = 5.3.521.0
CLI_SHA256 = D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
API_DLL_SHA256 = 9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277
API_HEADER_SHA256 = 4608E7BD9DDDDE2EB4CEE4618BA870E6F46A8423840B58EC7629D235C3A26C2C
DATA_TYPES_HEADER_SHA256 = 99274EB2BDB612C878D94A38FAA9AA7BF6822581AD91637292170F9A0D9D9D66
DEFINITIONS_HEADER_SHA256 = A8FD41F29EC69B1208A9DCFFE3FA837522109ADCC452E62BF99259B58F24B747
SAMPLE_SHA256 = 918B4C11F91C20CBC4F464CF926D3EA64D430E3A29D9F8EDDC2ABFFF0979BF20
```

The current installed CLI hash and import relationships match the
repository-retained static analysis. This is a provenance check, not new
runtime evidence.

## 5. Duplicate evidence rejected as new information

The following were deliberately not promoted to D1 findings:

- The existing LocalService/SYSTEM discovery differential. It is the same
  `timechart --list` evidence already used by Q1.
- The I1 LocalSystem and Administrator sampling results. They remain useful
  sampling-path feasibility evidence, but neither is a same-context pair with
  a negative `--list` result.
- The public API initialization/enumeration/start sequence as a second copy of
  Q1's sequence. The D1 addition is the direct vendor provenance and the
  explicit stage-specific error/accessibility taxonomy, not a new call trace.
- Shared `AMDPowerProfileAPI.dll`, `CXLBaseTools.dll`, and backend imports.
  Static dependency is not per-operation runtime call or authorization
  evidence.
- The installed CLI version, signature, and hash. They confirm image identity
  but do not identify command-specific semantics.
- Arbitrary strings or symbols from AMD binaries. No binary string was treated
  as authoritative semantics without a contextual vendor document, and no
  binary was executed.

## 6. CLI/API semantic findings

The requested questions are answered separately.

### Q1 — `--list` internal semantic role

```text
Q1_LIST_ROLE = SUPPORTED_AS_DOCUMENTED_DISCOVERY_PRESENTATION
Q1_EXACT_INITIALIZATION_MAPPING = UNKNOWN
Q1_SAME_POWER_CAPABILITY_GATE_AS_ACTIVE_SAMPLING = UNKNOWN
```

The vendor help supports the discovery/presentation role directly. The public
API guide supports a related API-level concept: a profile session is
initialized and supported counters are enumerated before counters are
enabled. Neither document gives the CLI source or a per-command API trace.
Therefore D1 does not claim that `--list` is only formatting, and does not
claim that it necessarily invokes the exact active-sampling gate.

### Q2 — active power sampling prerequisite

```text
Q2_PUBLIC_API_SEQUENCE = SUPPORTED
Q2_EXACT_CLI_POWER_AVAILABLE_MAPPING = UNKNOWN
Q2_EXACT_CLI_SAMPLING_PREREQUISITE = UNKNOWN
```

The vendor API sample makes supported-counter enumeration part of the public
API configuration sequence before enabling and starting counters. The vendor
CLI help tells users to use `--list` to select event values. That is meaningful
support for a conceptual relationship, but it is not proof that
`timechart --event power` must first pass the exact repository
`POWER_AVAILABLE` capability represented by `timechart --list`.

### Q3 — authorization semantics

```text
AUTHORIZATION_EQUIVALENCE = UNKNOWN
SERVICE_BACKEND_EQUIVALENCE = UNKNOWN
DRIVER_DEVICE_PATH_EQUIVALENCE = UNKNOWN
ACCOUNT_GROUP_SID_EQUIVALENCE = UNKNOWN
PRIVILEGE_EQUIVALENCE = UNKNOWN
ACL_OBJECT_EQUIVALENCE = UNKNOWN
API_CAPABILITY_GATE_EQUIVALENCE = UNKNOWN
```

The vendor material names drivers, platform support, counter accessibility,
SMU access, hypervisor support, BIOS support, and a separate remote-client
authorization layer. It does not identify the Windows account, token group,
Service SID, privilege, device/object ACL, or backend check used by either
exact CLI command, much less establish that the checks are identical.

### Q4 — same-context counterexample

```text
SAME_CONTEXT_COUNTEREXAMPLE_FOUND = NO
```

No immutable evidence holds account identity, service identity, Service SID,
token/group/privilege state, Session 0 state, CLI image, working directory,
environment, runtime/driver state, and close time sufficiently fixed while
showing a negative `--list` followed by successful active sampling.

- Administrator sampling versus LocalService discovery is not a controlled
  pair because account, token, session/interaction, and operation differ.
- LocalSystem I1 sampling versus a differently constructed LocalSystem
  discovery service is not a controlled pair without evidence that all of the
  requested identity and runtime fields are equal.
- Any historical Administrator `--list` success recorded in architecture
  material is not a same-context counterexample to a negative discovery result
  because the required immutable operation-pair state is not recorded.

## 7. Proxy criteria matrix and verdict

| Criterion | Required for `VALID_PROXY` | D1 result |
| --- | --- | --- |
| Necessary prerequisite | Negative `--list` must reflect a capability active power sampling necessarily requires | Not established; vendor API sequence is generic and exact CLI mapping is unknown |
| Shared operation/API dependency | The exact discovery and sampling operations must share the relevant capability gate | Partially supported conceptually; exact per-command mapping unknown |
| Shared authorization semantics | Account/service/driver/device/API authorization must be materially equivalent | Unknown across every requested authorization family |
| No valid counterexample | No same-context active sampling success while the relevant discovery capability is absent | No counterexample found, but no same-context pair exists |
| Production relevance | The result must apply to a materially equivalent production context | Production account remains unresolved |

The evidence is not sufficient for `VALID_PROXY`. It is also not sufficient
for `NOT_VALID_PROXY`: no authoritative statement says that active sampling
can succeed after a negative discovery result, and no valid same-context
counterexample exists. The fail-closed classification is:

```text
PROXY_VERDICT = INSUFFICIENT
PROXY_VERDICT_CONFIDENCE = MEDIUM
ROUTE_VERDICT = INSUFFICIENT
```

The new vendor help and manual increase confidence in the discovery/API
semantic description, but they do not resolve operation-specific authorization
equivalence. Historical negative `--list` results remain discovery evidence;
they neither independently block nor admit production sampling.

## 8. Minimal live-pair justification (evaluated, not executed)

The proposed future slice is:

```text
PHASE_A = timechart --list
PHASE_B = one bounded timechart --event power session
STATE_HOLD = same account, service identity, Service SID, token/groups/privileges, Session 0, CLI hash, AMD bin CWD, environment, runtime/driver/device state, and close time
MUTATION_BETWEEN_PHASES = NONE
LIVE_PAIR_AUTHORIZED = NO
```

The current hypotheses are H1 `VALID_PROXY`, H2 `NOT_VALID_PROXY`, and H3
`RELATED_BUT_NON_EQUIVALENT_OR_OTHER_FAILURE`. The pair has asymmetric
information value: a negative Phase A followed by successful Phase B would be
a strong same-context counterexample and would rule out H1; a negative Phase A
followed by failed Phase B would not prove H1 because both operations could
fail through an unisolated shared runtime condition. A positive Phase A does
not test the negative-discovery premise.

| Required field | D1 decision | Reason |
| --- | --- | --- |
| `LIVE_PAIR_INFORMATION_GAIN` | MEDIUM / ASYMMETRIC | Only the negative-list plus successful-sampling branch is decisive; the other branches leave H1/H3 confounded |
| `LIVE_PAIR_CAUSAL_INTERPRETABILITY` | CONDITIONAL | Fixed state would make a counterexample interpretable, but a failed sampling phase would not identify the cause |
| `LIVE_PAIR_SAFETY_RISK` | MEDIUM | No between-phase mutation is needed, but Phase B is a real vendor profiling operation that can touch the AMD backend/driver and produce session state |
| `LIVE_PAIR_PRODUCTION_RELEVANCE` | LOW_TO_MEDIUM / CONDITIONAL | The production account and production service contract are unresolved; a qualification LocalService pair would not itself admit production |
| `LIVE_PAIR_REQUIRED` | NO_FOR_CURRENT_ADMISSION | Current admission remains deferred for independent account, runtime, lifecycle, and deployment gates |
| `LIVE_PAIR_JUSTIFIED` | NO | The pair is not sufficient in all outcome branches and would not change `AMD_PRODUCTION_ADMISSION` from `DEFER`; a lower-risk read-only runtime prerequisite audit now has higher expected production information gain |
| `LIVE_PAIR_AUTHORIZED` | NO | This D1 task authorizes no live AMD operation |

This is not a claim that a pair could never be useful. If a reviewed
production candidate remains after the runtime audit and the proxy is the only
remaining admission discriminator, a new task may preregister the exact pair.
D1 does not authorize or design its operational launcher.

## 9. Ranked next-route decision

Exactly one route is selected.

| Rank | Route | Status | Decision basis |
| ---: | --- | --- | --- |
| 1 | Route D: `AMD-RUNTIME-PREREQUISITE-AUDIT-Q1` | SELECTED | New vendor material identifies concrete read-only prerequisite families: uProf/driver/backend version coherence, driver availability, platform/BIOS/hypervisor support, device/counter accessibility, installation completeness, service/backend state, and environment/output-path requirements. This is the highest-information read-only path for current production admission. |
| 2 | Route C: `AMD-OPERATION-PATH-PAIR-C1` | NOT SELECTED / DEFERRED | Revisit only after a production-relevant context exists and the pair can change an admission decision. |
| 3 | `AMD-PRODUCTION-ADMISSION-CLOSURE` | NOT SELECTED | Premature while the documented runtime prerequisite families have not been reconciled. |
| 4 | Route A or B | NOT APPLICABLE | The proxy is neither `VALID_PROXY` nor `NOT_VALID_PROXY`. |

The selected future audit is read-only and must not infer undocumented
requirements or change platform state. Its scope is limited to the installed
AMD uProf version, AMD driver/backend/service state, CPU family/model support
evidence, SDK/runtime coherence, installation completeness, environment/path
requirements, observed VBS/HVCI/hypervisor state, device/interface exposure,
and documented vendor prerequisites. It must not execute AMD binaries or APIs,
start sampling, mutate accounts/privileges/services/ACLs/devices/drivers, or
alter platform security state.

## 10. I2H gate

```text
I2H_JUSTIFIED = NO
```

The unresolved proxy does not identify a new right or mechanism. D1 does not
nominate `SeDebugPrivilege`, a privilege combination, or any other treatment.
The I2G real gate remains consumed and Attempt #4 remains unauthorized.

## 11. Explicit non-execution record

```text
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
RUNTIME_CODE_CHANGED = NO
RUST_RUNTIME_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
I2H_EXECUTED = NO
ATTEMPT4_CREATED = NO
```

Read-only actions were limited to Git metadata, repository text, installed
vendor text/PDF/header/source files, file hashes, Authenticode metadata, and
static provenance checks. No AMD executable, AMD API, or profiling session was
invoked or loaded.

## 12. Validation record

```text
VALIDATION_GIT_DIFF_CHECK = PASS
VALIDATION_POWERSHELL_PARSE = NOT_APPLICABLE; NO_POWERSHELL_FILE_MODIFIED
VALIDATION_CURRENT_STATE = PASS
AMD_RUNTIME_VALIDATION = NOT_RUN / PROHIBITED_BY_TASK
RUST_RUNTIME_CHANGED = NO
```

The current-state check covered the Q1 parent result, proxy verdict,
production account/admission, I2H gate, selected task, and next gate across
the authoritative handoff documents. The only retained stale `NONE`/human
operation-path markers are inside the explicitly labeled historical Q1
decision snapshot.

## 13. Current-state handoff

```text
AMD_OPERATION_PATH_EVIDENCE_GAP_D1 = COMPLETE / INSUFFICIENT
AMD_CLI_LIST_PATH_VALIDITY_Q1 = COMPLETE / INSUFFICIENT
PROXY_VERDICT = INSUFFICIENT
PROXY_VERDICT_CONFIDENCE = MEDIUM
NEW_AUTHORITATIVE_VENDOR_EVIDENCE_FOUND = YES
NEW_EVIDENCE_CAN_CHANGE_PROXY_VERDICT = PARTIAL_ONLY
CLI_LIST_ROLE = ACCOUNT_SENSITIVE_COUNTER_ENUMERATION / DISCOVERY_PRESENTATION
CLI_LIST_INTERNAL_ROLE = SUPPORTED_DISCOVERY_PRESENTATION; EXACT_INITIALIZATION_GATE UNKNOWN
SAMPLING_PATH_ROLE = PRODUCTION_RELEVANT_PROFILE_CONFIGURE_START_READ_STOP_PATH
ACTIVE_SAMPLING_PREREQUISITE_MAPPING = SUPPORTED_PUBLIC_API_SEQUENCE_ONLY; EXACT_CLI_MAPPING UNKNOWN
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
AUTHORIZATION_EQUIVALENCE = UNKNOWN
SAME_CONTEXT_COUNTEREXAMPLE_FOUND = NO
I2G_CURRENT_STATE = COMPLETE / ATTEMPT3_AUTHORITATIVE
I2G_REAL_GATE_CONSUMED = true
I2G_REAL_EXECUTION_ALLOWED = false
I2G_REAL_CLEANUP_ALLOWED = false
ATTEMPT4_AUTHORIZATION = NOT_GRANTED
PRODUCTION_ACCOUNT = UNRESOLVED
AMD_PRODUCTION_ADMISSION = DEFER
I2H_JUSTIFIED = NO
LIVE_PAIR_EVALUATED = YES
LIVE_PAIR_INFORMATION_GAIN = MEDIUM / ASYMMETRIC
LIVE_PAIR_CAUSAL_INTERPRETABILITY = CONDITIONAL
LIVE_PAIR_SAFETY_RISK = MEDIUM
LIVE_PAIR_PRODUCTION_RELEVANCE = LOW_TO_MEDIUM / CONDITIONAL
LIVE_PAIR_REQUIRED = NO_FOR_CURRENT_ADMISSION
LIVE_PAIR_JUSTIFIED = NO
LIVE_PAIR_AUTHORIZED = NO
SELECTED_NEXT_TASK = AMD-RUNTIME-PREREQUISITE-AUDIT-Q1
SELECTED_NEXT_TASK_GOAL = AUDIT_DOCUMENTED_AMD_RUNTIME_AND_ENVIRONMENT_PREREQUISITES_BEFORE_ANY_LIVE_OPERATION
NEXT_GATE = HUMAN_REVIEW_RUNTIME_PREREQUISITE_AUDIT
HUMAN_AUTH_REQUIRED_LATER = YES_FOR_ANY_FUTURE_LIVE_AMD_OPERATION
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
QUALIFICATION_BLOCKER_TASK_CAUSED = NO
QUALIFICATION_BLOCKER_SCOPE = PRE_EXISTING / OUT_OF_SCOPE / NOT_REPAIRED
```

The next task is selected for read-only prerequisite closure only. It does not
authorize the live pair, a privilege test, a production account switch, or any
AMD runtime operation.

## CURRENT STATE — AMD-RUNTIME-PREREQUISITE-AUDIT-Q1

The runtime prerequisite audit is complete as an offline-only
characterization. Its detailed source of truth is
[amd-runtime-prerequisite-audit-q1.md](amd-runtime-prerequisite-audit-q1.md).
This additive marker supersedes the earlier “selected next task” marker for
current handoff purposes; the D1 evidence and historical snapshots remain
immutable.

~~~text
AMD_RUNTIME_PREREQUISITE_AUDIT_Q1 = COMPLETE / INSUFFICIENT_EVIDENCE
RUNTIME_PREREQUISITE_AUDIT = COMPLETE / INSUFFICIENT_EVIDENCE
RUNTIME_PREREQUISITE_CONFIDENCE = MEDIUM
INSTALLATION_IDENTITY = SATISFIED
INSTALLATION_COMPLETENESS = UNKNOWN
RUNTIME_COMPONENT_COHERENCE = UNKNOWN
AMD_DRIVER_PREREQUISITE = UNKNOWN
AMD_BACKEND_SERVICE_PREREQUISITE = UNKNOWN
CPU_PLATFORM_SUPPORT = UNKNOWN
BIOS_PREREQUISITE = UNKNOWN
HYPERVISOR_PREREQUISITE = UNKNOWN
VBS_HVCI_PREREQUISITE = UNKNOWN
DEVICE_INTERFACE_PREREQUISITE = UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE
COUNTER_ACCESSIBILITY_PREREQUISITE = UNKNOWN_REQUIRES_LIVE_OR_MUTATING_EVIDENCE
ENVIRONMENT_PREREQUISITE = UNKNOWN
WORKING_DIRECTORY_REQUIREMENT = UNKNOWN
OUTPUT_PATH_REQUIREMENT = UNKNOWN
ACCOUNT_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
PRIVILEGE_REQUIREMENT_DOCUMENTED = UNKNOWN / NOT_ESTABLISHED
DOCUMENTED_BLOCKER_FOUND = NO
MATERIAL_UNKNOWNS_REMAIN = YES
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
RUNTIME_CODE_CHANGED = NO
RUST_RUNTIME_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
QUALIFICATION_TEST = BLOCKED
QUALIFICATION_TEST_BLOCKER = PRE_EXISTING_PINNED_RELEASE_ARTIFACT_SHA256_MISMATCH
~~~
