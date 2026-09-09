# AMD-PRIVILEGE-I2G variable selection

This document preserves the original read-only closure for the AMD privilege
I2G variable. Its design-history sections do not implement, authorize, or run
I2G. The completed current state is recorded in the later
`CURRENT STATE — OFFLINE IMPLEMENTATION COMPLETE` section and in
[`amd-i2g-harness.md`](amd-i2g-harness.md). No AMD process, service, driver,
device, IOCTL, sampling session, LSA mutation, token mutation, ACL mutation,
registry mutation, or historical evidence mutation was performed while
creating this documentation closure.

```text
REVIEW = AMD-PRIVILEGE-I2G-VARIABLE-SELECTION
BASE_COMMIT = e77ec8dfe713a5f48362a728921cf79f10d17adf
ANALYSIS = OFFLINE_READ_ONLY_EVIDENCE_CLOSURE
I2G_VARIABLE_SELECTION = PASS_READ_ONLY
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_SINGLE_VARIABLE_ISOLATABLE = true
I2G_HARNESS = NOT_IMPLEMENTED
I2G_REAL_RUNTIME = 0
I2G_HARNESS_IMPLEMENTATION_AUTHORIZED = false
I2G_REAL_RUNTIME_AUTHORIZED = false
```

## Authoritative predecessor state

The review preserves the predecessor gates exactly:

```text
I2E = CLOSED / RERUN_FORBIDDEN
I2F = REAL_COMPLETED / PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT / RERUN_FORBIDDEN
I2F_SCOPE = f68bf4d3d36547a0ba753cff489bb6eb
I2F_HISTORICAL_ARTIFACT_SHA256 = F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D
I2F_POST_REPAIR_ARTIFACT_SHA256 = 9A13111B02D5AAA2886B7E1EA059643EAABD5F30C3A2522589EE8B124B7B735C
SE_SYSTEM_PROFILE_PRIVILEGE_ALONE_SUFFICIENT = false
SE_SYSTEM_PROFILE_PRIVILEGE_NECESSITY = UNRESOLVED
SYSTEM_COUNTER_RESULT = POWER_AVAILABLE / HISTORICAL_REAL
I2F_COUNTER_RESULT = POWER_UNAVAILABLE / HISTORICAL_REAL
SESSION_DIFFERENTIAL = CLOSED
ARCHITECTURE_DIFFERENTIAL = CLOSED
AMD_CLI_IDENTITY_DIFFERENTIAL = CLOSED
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_HARNESS = NOT_IMPLEMENTED
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = NOT_COMPLETE
```

`SeSystemProfilePrivilege` was enabled in the final I2F token. I2F therefore
disproves only that this one capability was sufficient in the tested
LocalService / dedicated Service SID / Session 0 context. The selected I2G
design retains that final I2F state and adds one different capability; it does
not claim that `SeSystemProfilePrivilege` is unnecessary.

## Evidence sources and precision

The normalized review uses the repository-preserved summaries and contracts
for the following immutable evidence:

| Evidence | Reference |
| --- | --- |
| SYSTEM comparison scope | `091a72e1d38341ca9eca0877b1625082` |
| I2F authoritative scope | `f68bf4d3d36547a0ba753cff489bb6eb` |
| I2F historical artifact | `F272E2D5E74A1F8CC7EFABF01A64BFF1ACE4A244BF6199530D30F9F3F90ED10D` |
| I2F post-repair artifact | `9A13111B02D5AAA2886B7E1EA059643EAABD5F30C3A2522589EE8B124B7B735C` |
| Primary residual differential | [`amd-system-vs-i2f-residual-differential.md`](amd-system-vs-i2f-residual-differential.md) |
| Read-only helper audited | [`i2d-readonly-forensics.ps1`](../../tools/amd-privilege-qualification/i2d-readonly-forensics.ps1) |

The known ProgramData paths for the SYSTEM and I2F context JSON were not
readable in this shell. No ownership change, ACL change, privileged copy, or
bypass was attempted. The matrix below therefore uses the repository
summaries/contracts and records unavailable raw fields as `UNKNOWN`.

The token collector recorded complete enabled/disabled privilege names for the
captured token, but did not preserve separate removed/default attributes.
`disabled_privileges` means present in `TokenPrivileges` with the enabled bit
clear; absence means absent from the recorded token set. The preserved group
summary contains the relevant SID set but not every raw group attribute for
each historical record. Accordingly, group `DENY_ONLY` state and unrelated
non-relevant groups remain `UNKNOWN` unless explicitly stated.

## Normalized security-context matrix

The I2F column below is the final post-enable token used for the authoritative
I2F result. The I2F pre-enable state is shown where it matters to prevent the
`SeSystemProfilePrivilege` enablement from being mistaken for an I2G variable.

| Dimension | SYSTEM historical state | I2F final historical state | Differential | Evidence precision |
| --- | --- | --- | --- | --- |
| Account name | `NT AUTHORITY\\SYSTEM` | `NT AUTHORITY\\LOCAL SERVICE` | `SYSTEM_ONLY` vs `I2F_ONLY` identity | Exact names preserved |
| Account SID / TokenUser | `S-1-5-18` | `S-1-5-19` | `SYSTEM_ONLY` vs `I2F_ONLY` identity | Exact SIDs preserved |
| Dedicated Service SID | Dedicated `NT SERVICE\\<SYSTEM qualification service>` | Distinct dedicated `NT SERVICE\\<I2F qualification service>` | `SYSTEM_ONLY` and `I2F_ONLY` identity values | Both are dedicated Service SID models; exact historical I2F numeric SID is not preserved in the readable summary |
| Service SID type | `UNRESTRICTED` qualification contract | `UNRESTRICTED` qualification contract/result gate | `SAME` | Type is a controlled contract constant; AMD installed service SID types were separately observed as `NONE` |
| Session ID | `0` | `0` | `SAME` / closed | Exact |
| Process architecture | `x64` | `x64` | `SAME` / closed | Exact |
| Integrity RID | `S-1-16-16384` / System | `S-1-16-16384` / System | `SAME` / closed | Exact level in preserved context summary |
| Token elevation boolean | `UNKNOWN` in preserved summary | `UNKNOWN` in preserved summary | `UNKNOWN` | Schema field exists; historical value is not preserved in readable docs |
| Administrators SID `S-1-5-32-544` | `PRESENT` | `ABSENT` | `SYSTEM_ONLY` group | Presence is authoritative; raw enabled/deny-only attribute is not preserved |
| Users SID `S-1-5-32-545` | `PRESENT` in relevant set | `PRESENT` in relevant set | `SAME` | Raw group attributes not preserved in summary |
| Service SID `S-1-5-6` | `PRESENT` in relevant set | `PRESENT` in relevant set | `SAME` | Raw group attributes not preserved in summary |
| Dedicated Service SID group | Present as a dedicated qualification identity | Present as a distinct dedicated qualification identity | `UNKNOWN` identity value; same model | Exact numeric I2F SID and raw attributes unavailable |
| Other token groups | Not enumerated in preserved summary | Not enumerated in preserved summary | `UNKNOWN` | Collector field is the relevant-access subset, not a complete raw group dump |

### Normalized privilege matrix

`PRESENT + ENABLED` and `PRESENT + DISABLED` describe the historical
`TokenPrivileges` state. `ABSENT` means absent from the recorded token
privilege arrays. The differential is always SYSTEM versus final I2F, not
I2F pre-enable versus I2F post-enable.

| Privilege | SYSTEM state | I2F pre-enable | I2F final | Differential |
| --- | --- | --- | --- | --- |
| `SeChangeNotifyPrivilege` | PRESENT + ENABLED | PRESENT + ENABLED | PRESENT + ENABLED | `SAME` |
| `SeCreateGlobalPrivilege` | PRESENT + ENABLED | PRESENT + ENABLED | PRESENT + ENABLED | `SAME` |
| `SeImpersonatePrivilege` | PRESENT + ENABLED | PRESENT + ENABLED | PRESENT + ENABLED | `SAME` |
| `SeSystemProfilePrivilege` | PRESENT + ENABLED | PRESENT + DISABLED | PRESENT + ENABLED | `SAME` final state; I2F pre/post `SAME_PRIVILEGE_DIFFERENT_STATE` |
| `SeAuditPrivilege` | PRESENT + ENABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME_PRIVILEGE_DIFFERENT_STATE` |
| `SeCreatePagefilePrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeCreatePermanentPrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeCreateSymbolicLinkPrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeDebugPrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeDelegateSessionUserImpersonatePrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeIncreaseBasePriorityPrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeIncreaseWorkingSetPrivilege` | PRESENT + ENABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME_PRIVILEGE_DIFFERENT_STATE` |
| `SeLockMemoryPrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeProfileSingleProcessPrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeTcbPrivilege` | PRESENT + ENABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeTimeZonePrivilege` | PRESENT + ENABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME_PRIVILEGE_DIFFERENT_STATE` |
| `SeAssignPrimaryTokenPrivilege` | PRESENT + DISABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME` |
| `SeIncreaseQuotaPrivilege` | PRESENT + DISABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME` |
| `SeShutdownPrivilege` | PRESENT + DISABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME` |
| `SeSystemtimePrivilege` | PRESENT + DISABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME` |
| `SeUndockPrivilege` | PRESENT + DISABLED | PRESENT + DISABLED | PRESENT + DISABLED | `SAME` |
| `SeBackupPrivilege` | PRESENT + DISABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeLoadDriverPrivilege` | PRESENT + DISABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeManageVolumePrivilege` | PRESENT + DISABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeRestorePrivilege` | PRESENT + DISABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeSecurityPrivilege` | PRESENT + DISABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeSystemEnvironmentPrivilege` | PRESENT + DISABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |
| `SeTakeOwnershipPrivilege` | PRESENT + DISABLED | ABSENT | ABSENT | `SYSTEM_ONLY` |

There is no recorded `I2F_ONLY` privilege. The I2F post-enable delta is
exactly `SeSystemProfilePrivilege: PRESENT + DISABLED -> PRESENT + ENABLED`;
that right is `SAME` as SYSTEM only after the I2F treatment and is not proof
that it is necessary.

## AMD authorization review

The observed I2F failure is not a generic process-launch failure. The signed
CLI launched from LocalService, completed with exit code `0`, and reported the
bounded no-counters diagnostic. The following read-only evidence was used:

| Surface | Evidence | Classification |
| --- | --- | --- |
| AMD service objects | `AMDPowerProfiler`, `AMDProfilerLoadService`, and `AmdPpkgSvc` had equivalent inspected service descriptors; SYSTEM and Administrators had broad service-object rights, while `NT AUTHORITY\\SERVICE` had a limited service-control/read-style set. No exact SDDL string was preserved in the repository summary. | Administrators is an explicit service-object access difference, but not proof of counter-device access |
| AMD installed files | The signed CLI, driver files, and relevant installation path were readable/executable through observed inherited ACLs for ordinary Users; LocalService already launched and identity-validated the CLI. | File/installation access is not a strong remaining explanation |
| AMD drivers/services | `AMDPowerProfiler.sys`, `AMDCpuProfiler.sys`, `AMDProfilerLoadService`, and `AmdPpkgSvc` were inventoried read-only; the driver/service path is a plausible backend boundary. | No device was opened and no IOCTL was sent |
| Device/interface/object metadata | No power-device symbolic link, interface GUID, named kernel object, named pipe, or device-object security descriptor was recovered by the bounded PnP/INF/registry/static-string pass. | Limitation, not negative proof |
| AMD static authorization strings | Bounded PE/import/string evidence identified the public power API/CXL/backend graph but no direct SYSTEM/LocalService SID authorization rule. | No direct account-identity evidence |

```text
ACCOUNT_IDENTITY_DIRECT_EVIDENCE = NOT_FOUND
AMD_DRIVER_DEVICE_AUTHORIZATION_EVIDENCE = INDIRECT
AMD_SERVICE_INTERNAL_AUTHORIZATION_EVIDENCE = INCONCLUSIVE
EXACT_AMD_DEVICE_OR_OBJECT_ACL = NOT_RECOVERED
```

The service-object descriptor observation is not converted into a device ACL
claim. Likewise, the fact that AMD services run as LocalSystem is not treated
as proof that the CLI requires LocalSystem. `SYSTEM` and `LocalService` remain
separate account-identity hypotheses, and the dedicated Service SID is kept
as a separate security dimension from the account SID.

## Candidate register and scoring

Scores use the required rubric: `A` evidence strength, `B` isolatability, `C`
mutation risk, and `D` interpretability. A higher `C` means lower mutation
risk. The total is supporting structure, not an automatic decision.

| Candidate | SYSTEM state | I2F state | Differential / potential mechanism | Evidence for / against | Confounders | Risk / one-variable testability | A+B+C+D |
| --- | --- | --- | --- | --- | --- | --- | ---: |
| Account identity (`S-1-5-18` vs `S-1-5-19`) | LocalSystem | LocalService | Vendor/account-specific check or token-user authorization | Correlated with success / no direct SID check found | All token, group, Service SID, and backend differences | Broad non-isolatable identity switch; forbidden as I2G variable | `1+0+0+0=1` |
| Administrators membership | PRESENT | ABSENT | Group-derived service/device/object access | Service SDDL explicitly grants Administrators broad service rights / no device or backend ACL identified | Broad group changes many access paths | High risk; not a narrow capability; no addition authorized | `2+1+0+1=4` |
| `SeProfileSingleProcessPrivilege` | PRESENT + ENABLED | ABSENT | Windows single-process profiling capability consumed by a profiler path | SYSTEM-only enabled; privilege name and AMD profiler path provide indirect mechanism / no direct AMD check | Device/backend ACL and remaining SYSTEM-only differences | Exact Service SID assignment plus one token enablement is bounded and reversible | `2+3+2+2=9` |
| `SeDebugPrivilege` | PRESENT + ENABLED | ABSENT | Broad process inspection/access or vendor debug path | SYSTEM-only enabled / no AMD debug authorization evidence | Many unrelated process-access effects | High risk by definition; explicitly not the first experiment | `1+3+0+1=5` |
| `SeAuditPrivilege` | PRESENT + ENABLED | PRESENT + DISABLED | Audit-related token capability | State differs / no counter mechanism evidence | State difference may be incidental | Narrowly variable but weakly interpretable | `0+3+1+1=5` |
| `SeCreatePagefilePrivilege` | PRESENT + ENABLED | ABSENT | Pagefile creation or memory setup | SYSTEM-only enabled / no pagefile action in bounded operation | No sampling and no direct memory evidence | Isolatable in principle, but weak mechanism | `0+3+1+1=5` |
| `SeCreatePermanentPrivilege` | PRESENT + ENABLED | ABSENT | Permanent kernel object creation | SYSTEM-only enabled / no named object recovered | Unknown backend object path | Narrow right but no object evidence | `1+3+1+1=6` |
| `SeCreateSymbolicLinkPrivilege` | PRESENT + ENABLED | ABSENT | Symbolic-link creation for a device/object path | SYSTEM-only enabled / no link or interface evidence | Installation/device boundary unknown | Bounded right but weak mechanism | `0+3+1+1=5` |
| `SeDelegateSessionUserImpersonatePrivilege` | PRESENT + ENABLED | ABSENT | Session/impersonation delegation | SYSTEM-only enabled / both runs are Session 0 and no delegation evidence | Account and Service SID differences | Bounded but weakly interpretable | `0+3+1+1=5` |
| `SeIncreaseBasePriorityPrivilege` | PRESENT + ENABLED | ABSENT | Priority setup for profiler | SYSTEM-only enabled / no priority evidence | No performance sampling was run | Narrow but weak mechanism | `0+3+2+1=6` |
| `SeIncreaseWorkingSetPrivilege` | PRESENT + ENABLED | PRESENT + DISABLED | Working-set expansion | State differs / no memory-pressure evidence | Disabled-vs-enabled state may be incidental | Bounded but weakly interpretable | `0+3+2+1=6` |
| `SeLockMemoryPrivilege` | PRESENT + ENABLED | ABSENT | Locked memory for driver/buffer paths | SYSTEM-only enabled; memory backend is plausible / no allocation evidence | No sampling and no device open | More authority than selected right; mechanism indirect | `1+3+1+1=6` |
| `SeTcbPrivilege` | PRESENT + ENABLED | ABSENT | Trusted-computing-base/token boundary | SYSTEM-only enabled / no TCB or token-manipulation evidence | Broad account/token confounding | Broad high-risk authority; rejected | `0+2+0+1=3` |
| `SeTimeZonePrivilege` | PRESENT + ENABLED | PRESENT + DISABLED | Time conversion or profile timestamp setup | State differs / no time-zone operation evidence | Incidental service-token state | Bounded but weakly interpretable | `0+3+2+1=6` |
| Other token-group difference | SYSTEM includes more groups | I2F relevant subset lacks them | Group-derived object access | SYSTEM-only group set is real / exact ACL target unknown | Administrators and Service SID already separate candidates | No clean single group identified | `0+1+1+1=3` |
| Dedicated Service SID identity | Dedicated SID A | Distinct dedicated SID B | SID-specific ACL or backend principal check | Distinct SIDs are real / no AMD ACL references either SID | Fresh-service isolation changes identifier each run | Not a capability by itself; no causal evidence | `0+1+2+1=4` |
| AMD driver/device ACL | Unknown object | Unknown object | Kernel/device handle authorization | Backend boundary is plausible; CLI/API access-denied evidence is indirect / exact object absent | Interface, ACL, and object unknown | High-risk ACL mutation and not yet isolatable | `2+1+1+2=6` |
| AMD service internal authorization | Unknown | Unknown | Broker/IPC identity or SID check | Service graph is present / no direct check or object recovered | Account, groups, and driver boundary all confounded | No single safe mutation defined | `1+1+1+1=4` |
| Combined privilege/group set | Multiple SYSTEM-only values | Smaller I2F set | Multi-capability dependency | I2F negative result leaves combinations possible / no minimum set | Violates one-variable requirement | Not eligible for first I2G | `1+0+1+0=2` |

`SeSystemProfilePrivilege` is not scored as a new candidate: it is enabled in
the final I2F base, and I2F already supplied the negative sufficiency result.
Testing it absent while adding another right would change two variables. The
future I2G base therefore retains it enabled.

The selected right has the best combination of (a) a real SYSTEM-only token
difference, (b) an indirect mechanism tied to profiling rather than generic
authority, (c) a clean one-right/one-enable transition, and (d) useful causal
interpretation. The selection is medium confidence because the review found no
direct AMD authorization check or exact device ACL.

## Selected variable

```text
I2G_VARIABLE_SELECTED = SeProfileSingleProcessPrivilege
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_SELECTION_RATIONALE = highest-scoring narrow SYSTEM-only capability with indirect profiling mechanism; no direct causal proof
I2G_SINGLE_VARIABLE_ISOLATABLE = true
BASE_ACCOUNT = NT AUTHORITY\LOCAL SERVICE
BASE_ACCOUNT_SID = S-1-5-19
BASE_SECURITY_CONTEXT = paired fresh-I2G control/treatment context; non-treatment dimensions are held constant within the experiment
BLOCKER = I2G_PAIRED_PHASE_CONFIGURATION_INVARIANT_CONTRADICTS_TREATMENT_MUTATION
BLOCKER_STATUS = CLOSED_OFFLINE
PREVIOUS_BLOCKER_1 = I2G_BASELINE_RECONSTRUCTION_CONTRACT_INCONSISTENT / CLOSED_OFFLINE
PREVIOUS_BLOCKER_2 = I2G_STAGED_TREATMENT_TOKEN_MISCLASSIFIED_AS_EXACT_I2F_BASELINE / CLOSED_OFFLINE
PREVIOUS_BLOCKER_3 = I2G_HISTORICAL_CONTROL_LEAVES_NON_TREATMENT_CONFOUNDERS_UNCONTROLLED / CLOSED_OFFLINE
I2G_BASELINE_RECONSTRUCTION_RIGHT = SeSystemProfilePrivilege
I2G_TREATMENT_VARIABLE = SeProfileSingleProcessPrivilege
I2G_SELECTION_CHANGED = false
I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT
TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1
HISTORICAL_I2F_PROFILE_SINGLE_STATE = ABSENT
I2G_CONTROL_PROFILE_SINGLE_STATE = ABSENT
I2G_TREATMENT_PROFILE_SINGLE_STATE = PRESENT + ENABLED
I2G_CONTROL_SYSTEM_PROFILE_STATE = PRESENT + ENABLED
I2G_TREATMENT_SYSTEM_PROFILE_STATE = PRESENT + ENABLED
CONTROL_SERVICE_NAME_EQUALS_TREATMENT = true
CONTROL_SERVICE_SID_EQUALS_TREATMENT = true
CONTROL_HARNESS_SHA_EQUALS_TREATMENT = true
BASELINE_RECONSTRUCTION_ROLE = frozen final-I2F state; not a new treatment
TREATMENT_ROLE = selected single causal capability
HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY
HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false
SE_SYSTEM_PROFILE_PRIVILEGE_STATE_IN_LOGICAL_BASELINE = PRESENT + ENABLED
SE_PROFILE_SINGLE_PROCESS_PRIVILEGE_STATE_IN_LOGICAL_BASELINE = PRESENT + DISABLED
ADMINISTRATORS_MEMBERSHIP_MUTATION = FORBIDDEN
LOCAL_SYSTEM_AS_I2G_VARIABLE = FORBIDDEN_AS_NON_SINGLE_VARIABLE
SE_DEBUG_PRIVILEGE_MUTATION = FORBIDDEN
```

The earlier review blockers concerned reconstruction terminology and staged
token misclassification; both are closed. The current blocker is that
historical I2F cannot serve as a fully paired future control. The authoritative
design therefore uses one fresh I2G Service SID twice: CONTROL receives only
`SeSystemProfilePrivilege` to reconstruct the known I2F security dimension and
perform the negative control run; after token teardown, TREATMENT adds
`SeProfileSingleProcessPrivilege` to that same SID. The two policy changes
remain one baseline reconstruction plus one scientific capability, not two
causal variables.

The historical I2F Service SID's policy assignment cannot persist across a
fresh Service SID. Therefore `SeSystemProfilePrivilege = PRESENT + ENABLED`
is a logical baseline state, while the temporary assignment of that right is
the physical operation required to recreate it. No right is added to the
global LocalService account. No Administrators membership, LocalSystem
account, AMD service ACL, driver ACL, or registry ACL is part of the selected
variable.

## HISTORICAL / SUPERSEDED — prior staged-only I2G contract

This contract is a specification for a later implementation review. It is not
executable code and does not create an authorization path.

```text
I2G_BASE_CONTEXT =
  LocalService (S-1-5-19), fresh unrestricted dedicated Service SID,
  Session 0, x64, System integrity; all non-treatment security-context
  dimensions reproduce the final authoritative I2F state; the selected
  treatment right is staged PRESENT + DISABLED before activation; exact fixed
  AMD CLI identity, fixed working directory, and environment

HISTORICAL_I2F_CONTROL =
  TokenUser = LocalService / S-1-5-19;
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = ABSENT;
  AMD_RESULT = POWER_UNAVAILABLE; no new control run

I2G_MATERIALIZED_TOKEN =
  SeSystemProfilePrivilege = PRESENT + DISABLED;
  SeProfileSingleProcessPrivilege = PRESENT + DISABLED

I2G_STAGED_TOKEN =
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = PRESENT + DISABLED;
  I2G_NON_TREATMENT_BASELINE_INVARIANTS = EXACT_FINAL_I2F

I2G_TREATMENT_TOKEN =
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = PRESENT + ENABLED;
  all non-treatment dimensions unchanged from I2G_STAGED_TOKEN

POLICY_ASSIGNMENT_BASELINE =
  fresh I2G Service SID temporarily receives SeSystemProfilePrivilege only
  for final-I2F baseline reconstruction

POLICY_ASSIGNMENT_TREATMENT =
  fresh I2G Service SID temporarily receives SeProfileSingleProcessPrivilege
  as the selected treatment capability

I2G_SINGLE_VARIABLE = SeProfileSingleProcessPrivilege
TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1

CONSTANTS =
  SeSystemProfilePrivilege is reconstructed on the fresh I2G Service SID and
  must reach PRESENT + ENABLED before the treatment gate; its logical state is
  frozen to final I2F. Account SID, session, architecture, integrity,
  elevation field,
  Administrators absence, all other privilege states, group set/model,
  AMD installation, driver/service state, CLI path/hash/version/signature,
  working directory, environment, protocol, timeout, job policy, output
  policy, cleanup policy, and result classifier remain unchanged

PHASE_A_SERVICE_TOKEN_MATERIALIZATION =
  after fresh Service SID policy assignment and service start, both controlled
  rights are PRESENT + DISABLED; all other final-I2F relevant states remain
  unchanged; no Administrators SID; no SeDebugPrivilege; no other new
  SYSTEM-only right

EXPECTED_MATERIALIZED_TOKEN =
  SeSystemProfilePrivilege = PRESENT + DISABLED;
  SeProfileSingleProcessPrivilege = PRESENT + DISABLED;
  all other final-I2F relevant privilege, group, account, session, and
  integrity states unchanged

PHASE_B_BASELINE_RECONSTRUCTION =
  enable SeSystemProfilePrivilege only; capture I2G-STAGED-TOKEN.json;
  do not launch AMD CLI before the staged token gate passes

EXPECTED_STAGED_TOKEN =
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = PRESENT + DISABLED;
  all non-treatment dimensions MATCH FINAL I2F; no Administrators SID;
  no SeDebugPrivilege

I2G_NON_TREATMENT_BASELINE_INVARIANTS = EXACT_FINAL_I2F
I2G_BASELINE_RECONSTRUCTION = FINAL_I2F_NON_TREATMENT_STATE_RECONSTRUCTED
I2G_STAGED_TREATMENT_RIGHT = SeProfileSingleProcessPrivilege PRESENT + DISABLED
I2G_STAGED_TREATMENT_EXCEPTION = SeProfileSingleProcessPrivilege PRESENT + DISABLED
STAGED_EXCEPTION_COUNT = 1
BASELINE_RECONSTRUCTION_TOKEN_DELTA = SeSystemProfilePrivilege DISABLED -> ENABLED

PHASE_C_TREATMENT =
  after baseline verification, enable SeProfileSingleProcessPrivilege only;
  capture I2G-TREATMENT-TOKEN.json

EXPECTED_TREATMENT_TOKEN =
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = PRESENT + ENABLED;
  all non-treatment dimensions MATCH FINAL I2F and are unchanged from the
  staged token

I2G_TREATMENT_MATERIALIZATION_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + DISABLED
I2G_TREATMENT_ACTIVATION_DELTA = SeProfileSingleProcessPrivilege DISABLED -> ENABLED
I2G_EXACT_TREATMENT_ACTIVATION_DELTA = ONE_PRIVILEGE_STATE_CHANGE
I2G_TOTAL_CAUSAL_TREATMENT_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + ENABLED
I2G_TOTAL_CAUSAL_TREATMENT = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + ENABLED
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1

SUPERSEDED_NO_BASELINE_AMD_RUN = true
AMD_LAUNCH_GATE = baseline token gate must pass before any AMD CLI launch

MUST_REMAIN_ABSENT =
  SeDebugPrivilege, SeCreatePagefilePrivilege, SeCreatePermanentPrivilege,
  SeCreateSymbolicLinkPrivilege, SeDelegateSessionUserImpersonatePrivilege,
  SeIncreaseBasePriorityPrivilege, SeLockMemoryPrivilege, SeTcbPrivilege,
  SeBackupPrivilege, SeLoadDriverPrivilege, SeManageVolumePrivilege,
  SeRestorePrivilege, SeSecurityPrivilege, SeSystemEnvironmentPrivilege,
  SeTakeOwnershipPrivilege

MUST_REMAIN_DISABLED =
  SeAuditPrivilege, SeIncreaseWorkingSetPrivilege, SeTimeZonePrivilege,
  SeAssignPrimaryTokenPrivilege, SeIncreaseQuotaPrivilege,
  SeShutdownPrivilege, SeSystemtimePrivilege, SeUndockPrivilege

MUST_REMAIN_ENABLED =
  SeChangeNotifyPrivilege, SeCreateGlobalPrivilege, SeImpersonatePrivilege;
  SeSystemProfilePrivilege after baseline reconstruction;
  SeProfileSingleProcessPrivilege only after treatment enablement

FORBIDDEN_PRIVILEGE_AND_IDENTITY_CHANGES =
  no newly present SYSTEM-only right outside the two controlled rights;
  no Administrators SID; no LocalSystem account; no global LocalService right;
  no combined-right treatment; no account, Service SID model, session,
  architecture, integrity, or group mutation

AMD_CLI_IDENTITY =
  D:\apps\AMDuProf\bin\AMDuProfCLI.exe;
  SHA-256 D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC;
  x64, version 5.3.521.0, valid AMD signature

FIXED_OPERATION = timechart --list
SAMPLING = false
TIMEOUT = 30,000 ms counter-discovery bound with the existing 90,000 ms child safety cap
FRESH_SERVICE_NAME = ResourceTimelineAmdProfileSingleProcessQualification
FRESH_SERVICE_SID = derived at future setup from the fresh service name; must be an unrestricted NT SERVICE SID and must not reuse historical I2E/I2F identities

ROLLBACK_ORDER =
  stop accepting work; wait/terminate the exact owned AMD child if needed;
  verify AMD child absent; stop service; verify service PID = 0 and effective
  token gone; verify fresh Service SID direct rights; remove treatment right;
  dual LSA readback; remove baseline reconstruction right; dual LSA readback;
  delete fresh qualification service; verify service absent; verify broker and
  AMD-owned processes absent; persist final rollback evidence

BASELINE_POLICY_RIGHT_ADDED = SeSystemProfilePrivilege
TREATMENT_POLICY_RIGHT_ADDED = SeProfileSingleProcessPrivilege
I2G_BASELINE_POLICY_ROLLBACK = SeSystemProfilePrivilege
I2G_TREATMENT_POLICY_ROLLBACK = SeProfileSingleProcessPrivilege
POLICY_ROLLBACK =
  after effective token teardown, remove SeProfileSingleProcessPrivilege if
  added by this run and verify dual LSA readback; then remove
  SeSystemProfilePrivilege if added by this run and verify dual LSA readback;
  if a right is already absent, record ALREADY_ABSENT and issue no duplicate
  removal; no LsaRemoveAccountRights(all=true)

TOKEN_TEARDOWN_BEFORE_POLICY_RIGHT_REMOVAL = true

PARTIAL_FAILURE_ACCOUNTING =
  system_profile_right_added_by_run;
  profile_single_right_added_by_run;
  control_amd_run_started; control_amd_run_completed;
  treatment_policy_mutation_started; treatment_policy_mutation_verified;
  treatment_amd_run_started; treatment_amd_run_completed;
  system_profile_remove_attempted; system_profile_remove_verified;
  profile_single_remove_attempted; profile_single_remove_verified;
  already_absent is recorded independently for each right

CONTROL_DRIFT_CLEANUP =
  if CONTROL drifts before ProfileSingle is assigned, stop and verify token and
  process teardown, remove SeSystemProfilePrivilege only, verify dual readback,
  and delete the fresh service; profile_single_right_added_by_run remains false
  and no ProfileSingle removal is attempted

TOKEN_TEARDOWN =
  service stop and process-absence proof; token dies with the stopped service
  process; no production token is adjusted

SERVICE_TEARDOWN =
  stop first, drain owned child, verify absent, delete only the fresh I2G
  qualification service, verify absent

PROCESS_OWNERSHIP =
  qualification service owns the direct CLI child under the existing
  kill-on-close/job-owned policy; no orphan child is acceptable

EVIDENCE_SCHEMA =
  immutable I2G scope; gate state; service/SID metadata;
  POLICY_ASSIGNMENT_BASELINE; POLICY_ASSIGNMENT_TREATMENT;
  TOKEN_MATERIALIZED; TOKEN_STAGED; TOKEN_TREATMENT;
  BASELINE_RECONSTRUCTION_DELTA; I2G_TREATMENT_MATERIALIZATION_DELTA;
  I2G_TREATMENT_ACTIVATION_DELTA; I2G_TOTAL_CAUSAL_TREATMENT_DELTA;
  relevant groups;
  AMD_CLI_IDENTITY; COUNTER_DISCOVERY_RESULT;
  POLICY_ROLLBACK_TREATMENT; POLICY_ROLLBACK_BASELINE; FULL_ROLLBACK;
  command; sampling=false; bounded stdout/stderr; exit code; normalized
  result; cleanup and rollback evidence

RESULT_CLASSIFICATION =
  POWER_AVAILABLE, POWER_UNAVAILABLE, DISCOVERY_FAILED, TOKEN_GATE_FAILED,
  IDENTITY_MISMATCH, CLEANUP_FAILED, or INVALID_NO_CAUSAL_INTERPRETATION

INVALID_RESULT_BASELINE_GATE =
  INVALID_NO_CAUSAL_INTERPRETATION if any non-treatment invariant differs from
  final I2F; ProfileSingle is enabled before treatment; ProfileSingle is absent
  from the staged token or has unexpected attributes; staged SystemProfile is
  not PRESENT + ENABLED; Administrators appears; an unexpected SYSTEM-only
  privilege appears; baseline reconstruction changes another privilege;
  treatment activation changes more than ProfileSingle's enabled bit; AMD CLI
  identity differs; sampling is not false; or rollback is incomplete

ONE_TIME_GATE =
  I2G_GATE_CONSUMED = true;
  I2G_REAL_EXECUTION_ALLOWED = false by default;
  a separate explicit human authorization is required after offline harness
  implementation and review; this task creates no executable gate
```

## Design-only I2G paired CONTROL -> TREATMENT contract

This is the authoritative future design for a later implementation review. It
is documentation only, does not implement a harness, and does not create an
authorization path. Historical I2F remains predecessor evidence; the active
causal comparison is within one fresh I2G experiment.

```text
I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT
HISTORICAL_I2F_REFERENCE =
  I2F_RESULT = POWER_UNAVAILABLE;
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = ABSENT
HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY
HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false

I2G_BASE_CONTEXT =
  one fresh experiment ID; one fresh unrestricted Service SID; one service
  name reused across CONTROL and TREATMENT; LocalService (S-1-5-19), Session 0,
  x64, System integrity; one qualification harness artifact; fixed working
  directory, environment, timeout, output policy, protocol, and ownership
  policy; all causal comparisons are within these paired phases

PAIRED_CONSTANTS =
  same experiment ID, service name, Service SID, Service SID type, account,
  harness binary, harness SHA256, working directory, environment, AMD CLI
  path/SHA256/architecture/version/signature, Session 0, x64, System
  integrity, timeout, output policy, job/process ownership, cleanup semantics,
  FIXED_OPERATION, and sampling mode
CONTROL_SERVICE_NAME_EQUALS_TREATMENT = true
CONTROL_SERVICE_SID_EQUALS_TREATMENT = true
CONTROL_SERVICE_SID_TYPE = UNRESTRICTED
TREATMENT_SERVICE_SID_TYPE = UNRESTRICTED
CONTROL_HARNESS_SHA_EQUALS_TREATMENT = true
NO_REBUILD_BETWEEN_PHASES = true
NO_CODE_CHANGE_BETWEEN_PHASES = true
NO_HARNESS_REBUILD_BETWEEN_PHASES = true
NO_NON_TREATMENT_CONFIGURATION_CHANGE_BETWEEN_PHASES = true
ALLOWED_TREATMENT_CONFIGURATION_DELTA = SeProfileSingleProcessPrivilege assignment to same Service SID only
CONTROL_TO_TREATMENT_POLICY_DELTA = SeProfileSingleProcessPrivilege assignment to same Service SID only
CONTROL_TO_TREATMENT_POLICY_DELTA_COUNT = 1
NON_TREATMENT_CONFIGURATION_INVARIANTS = UNCHANGED
FORBIDDEN_NON_TREATMENT_CONFIGURATION_CHANGES =
  service name, service binary path, service account, Service SID type, start
  mode, service command line, harness binary/SHA256, working directory,
  environment, AMD CLI path/hash/version/signature, timeout, protocol, output
  policy, job/process ownership, sampling mode, result classifier, ACLs,
  registry, or AMD installation

CONTROL_POLICY_RIGHTS = SeSystemProfilePrivilege only
CONTROL_POLICY_ASSIGNMENT =
  assign SeSystemProfilePrivilege to the fresh Service SID before CONTROL;
  do not assign SeProfileSingleProcessPrivilege before CONTROL
CONTROL_DIRECT_SERVICE_SID_RIGHTS = SeSystemProfilePrivilege

I2G_CONTROL_MATERIALIZED_TOKEN =
  SeSystemProfilePrivilege = PRESENT + DISABLED;
  SeProfileSingleProcessPrivilege = ABSENT
I2G_CONTROL_FINAL_TOKEN =
  enable SeSystemProfilePrivilege only;
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = ABSENT;
  all other controlled privilege, group, account, session, architecture, and
  integrity dimensions match the final-I2F non-treatment baseline as closely
  as the fresh paired service model permits
I2G_CONTROL_TOKEN = I2G-CONTROL-TOKEN.json
CONTROL_SAMPLING = false
CONTROL_COUNTER_DISCOVERY = timechart --list
PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1
CONTROL_EXPECTED_RESULT = POWER_UNAVAILABLE
CONTROL_VALIDITY =
  valid only with POWER_UNAVAILABLE, exit code 0, expected no-counters
  diagnostic, exact identity, sampling=false, and no owned child

CONTROL_DRIFT =
  any CONTROL result other than the preregistered valid negative result,
  including POWER_AVAILABLE, DISCOVERY_FAILED, TOKEN_GATE_FAILED,
  IDENTITY_MISMATCH, or TIMEOUT
CONTROL_DRIFT_STOP_BEFORE_TREATMENT = true
CONTROL_DRIFT_RESULT = INVALID_NO_CAUSAL_INTERPRETATION

CONTROL_TEARDOWN =
  1. CONTROL AMD child finishes; 2. verify the exact owned AMD child is
  absent; 3. stop service; 4. verify service PID=0; 5. verify the CONTROL
  token no longer exists; 6. verify the broker-owned process set is empty;
  7. only then mutate Service SID policy
CONTROL_TOKEN_TEARDOWN_BEFORE_TREATMENT_POLICY_MUTATION = true

TREATMENT_POLICY_RIGHTS =
  SeSystemProfilePrivilege + SeProfileSingleProcessPrivilege
TREATMENT_POLICY_ADDITION = SeProfileSingleProcessPrivilege only
TREATMENT_POLICY_MUTATION_GATE = CONTROL_TOKEN_TEARDOWN_BEFORE_TREATMENT_POLICY_MUTATION
TREATMENT_DIRECT_SERVICE_SID_RIGHTS = SeSystemProfilePrivilege + SeProfileSingleProcessPrivilege
EXACT_TREATMENT_POLICY_DELTA = SeProfileSingleProcessPrivilege added
SERVICE_RESTART = REQUIRED_TECHNICAL_MATERIALIZATION_BOUNDARY
SERVICE_RESTART_IS_SECOND_SCIENTIFIC_VARIABLE = false

I2G_TREATMENT_MATERIALIZED_TOKEN =
  restart the same service after CONTROL teardown and the single treatment
  policy addition; SeSystemProfilePrivilege = PRESENT + DISABLED;
  SeProfileSingleProcessPrivilege = PRESENT + DISABLED
I2G_TREATMENT_FINAL_TOKEN =
  enable SeSystemProfilePrivilege first and SeProfileSingleProcessPrivilege
  second; SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = PRESENT + ENABLED
I2G_TREATMENT_TOKEN = I2G-TREATMENT-TOKEN.json
TREATMENT_SAMPLING = false
TREATMENT_COUNTER_DISCOVERY = timechart --list
PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1

CONTROL_TO_TREATMENT_NON_TREATMENT_INVARIANTS = UNCHANGED
  TokenUser = S-1-5-19 / LocalService; Session = 0; Architecture = x64;
  Integrity = System; Administrators SID = ABSENT;
  Service SID type = UNRESTRICTED; SeSystemProfilePrivilege = PRESENT + ENABLED
  in both final tokens; all unrelated privilege states and groups identical;
  AMD CLI identity, operation, sampling, timeout, environment, and ownership
  policy identical

CONTROL_TREATMENT_TOKEN_INVARIANT_COMPARISON =
  PASS_EXACT_ONE_PRIVILEGE_DELTA when all unrelated token/group/account,
  session, architecture, integrity, Administrators, and SystemProfile states
  are equal and only ProfileSingle changes ABSENT -> PRESENT + ENABLED

I2G_PAIRED_CAUSAL_TREATMENT_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + ENABLED
I2G_TREATMENT_MATERIALIZATION_DELTA =
  SeProfileSingleProcessPrivilege ABSENT -> PRESENT + DISABLED
I2G_TREATMENT_ACTIVATION_DELTA =
  SeProfileSingleProcessPrivilege DISABLED -> ENABLED
TREATMENT_MATERIALIZATION_AND_ACTIVATION = ONE_CAPABILITY_INTERVENTION
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1

CONTROL_TO_TREATMENT_RUNS =
  PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1;
  PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1;
  PLANNED_VALID_PAIR_COUNTER_DISCOVERY_RUNS = 2;
  MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1;
  MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1;
  MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2;
  POWER_SAMPLING_RUNS = 0

PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1
PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
PLANNED_VALID_PAIR_COUNTER_DISCOVERY_RUNS = 2
MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1
MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2
ACTUAL_RUN_COUNTS =
  record ACTUAL_CONTROL_COUNTER_DISCOVERY_RUNS,
  ACTUAL_TREATMENT_COUNTER_DISCOVERY_RUNS, and
  ACTUAL_TOTAL_I2G_COUNTER_DISCOVERY_RUNS from future evidence; never infer
  counts from planned values or evidence-file presence
CONTROL_DRIFT_EXPECTED_ACTUAL_COUNTS = control 1; treatment 0; total 1
PRE_CONTROL_FAILURE_ACTUAL_COUNTS = 0 / 0 / 0
CONTROL_RETRY_ALLOWED = false
TREATMENT_RETRY_ALLOWED = false
COUNTER_DISCOVERY_RETRY_POLICY = NO_RETRY

MUST_REMAIN_ABSENT =
  SeDebugPrivilege, SeCreatePagefilePrivilege, SeCreatePermanentPrivilege,
  SeCreateSymbolicLinkPrivilege, SeDelegateSessionUserImpersonatePrivilege,
  SeIncreaseBasePriorityPrivilege, SeLockMemoryPrivilege, SeTcbPrivilege,
  SeBackupPrivilege, SeLoadDriverPrivilege, SeManageVolumePrivilege,
  SeRestorePrivilege, SeSecurityPrivilege, SeSystemEnvironmentPrivilege,
  SeTakeOwnershipPrivilege
MUST_REMAIN_DISABLED =
  SeAuditPrivilege, SeIncreaseWorkingSetPrivilege, SeTimeZonePrivilege,
  SeAssignPrimaryTokenPrivilege, SeIncreaseQuotaPrivilege, SeShutdownPrivilege,
  SeSystemtimePrivilege, SeUndockPrivilege
MUST_REMAIN_ENABLED =
  SeChangeNotifyPrivilege, SeCreateGlobalPrivilege, SeImpersonatePrivilege;
  SeSystemProfilePrivilege in both final tokens;
  SeProfileSingleProcessPrivilege only in final TREATMENT

AMD_CLI_IDENTITY =
  D:\\apps\\AMDuProf\\bin\\AMDuProfCLI.exe;
  SHA-256 D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC;
  x64, version 5.3.521.0, valid AMD signature
CONTROL_HARNESS_SHA256 = TREATMENT_HARNESS_SHA256
FIXED_OPERATION = timechart --list
POWER_SAMPLING_RUNS = 0
TIMEOUT = 30,000 ms counter-discovery bound with the existing 90,000 ms child safety cap
FRESH_SERVICE_NAME = one fixed fresh name reused by both phases
FRESH_SERVICE_SID = one SID derived from that name and reused by both phases

ROLLBACK_ORDER =
  after TREATMENT, stop accepting work; wait/terminate the exact owned AMD
  child if needed; verify AMD child absent; stop service; verify service PID=0,
  effective token gone, and broker-owned processes absent; remove
  SeProfileSingleProcessPrivilege and verify dual LSA readback; remove
  SeSystemProfilePrivilege and verify dual LSA readback; delete the same fresh
  qualification service; verify service absent; persist final rollback evidence
TOKEN_TEARDOWN_BEFORE_POLICY_RIGHT_REMOVAL = true
I2G_BASELINE_POLICY_ROLLBACK = SeSystemProfilePrivilege
I2G_TREATMENT_POLICY_ROLLBACK = SeProfileSingleProcessPrivilege
POLICY_ROLLBACK = exact independent removal/readback of both additions;
  no LsaRemoveAccountRights(all=true); no global LocalService mutation
PARTIAL_FAILURE_ACCOUNTING =
  system_profile_right_added_by_run; profile_single_right_added_by_run;
  system_profile_remove_attempted; system_profile_remove_verified;
  profile_single_remove_attempted; profile_single_remove_verified;
  already_absent is recorded independently for each right

PROCESS_OWNERSHIP =
  both phases use qualification service -> exact direct AMD CLI child ->
  job-owned / kill-on-close; CONTROL tree absent before treatment mutation;
  TREATMENT tree absent before final rollback

EVIDENCE_SCHEMA =
  HISTORICAL_I2F_REFERENCE; PLANNED_RUN_COUNTS; MAX_RUN_COUNTS;
  ACTUAL_RUN_COUNTS; CONTROL_POLICY_STATE; CONTROL_TOKEN;
  CONTROL_AMD_IDENTITY; CONTROL_COUNTER_DISCOVERY_RESULT; CONTROL_TEARDOWN;
  TREATMENT_POLICY_MUTATION; TREATMENT_POLICY_READBACK; TREATMENT_TOKEN;
  TREATMENT_AMD_IDENTITY; TREATMENT_COUNTER_DISCOVERY_RESULT;
  CONTROL_TREATMENT_CONFIGURATION_INVARIANT_COMPARISON;
  CONTROL_TREATMENT_TOKEN_INVARIANT_COMPARISON; PAIRED_CAUSAL_DELTA;
  POLICY_ROLLBACK_TREATMENT; POLICY_ROLLBACK_BASELINE; FINAL_ROLLBACK

INVALID_RESULT_CONTROL_DRIFT_GATE =
  INVALID_NO_CAUSAL_INTERPRETATION if CONTROL is not valid POWER_UNAVAILABLE;
  stop before treatment on drift or operational failure
INVALID_RESULT_CAUSAL_GATE =
  INVALID_NO_CAUSAL_INTERPRETATION if service name/SID/SID type, harness SHA,
  AMD CLI identity, TokenUser, session, architecture, integrity, unrelated
  privilege/group state, SystemProfile final state, operation, timeout,
  environment, or sampling changes; Administrators appears; treatment adds
  more than ProfileSingle; owned cleanup fails; or rollback is incomplete

INVALID_CONFIGURATION_DELTA =
  INVALID_NO_CAUSAL_INTERPRETATION if any CONTROL-to-TREATMENT configuration
  change occurs beyond adding SeProfileSingleProcessPrivilege to the same
  Service SID; examples include harness, service binary, environment, timeout,
  working directory, AMD CLI, Service SID, account, ACL, registry, or any
  additional privilege change

CONTROL_TREATMENT_CONFIGURATION_INVARIANT_COMPARISON =
  PASS_EXACT_ONE_ALLOWED_TREATMENT_CONFIGURATION_DELTA only when all
  non-treatment configuration is unchanged and the sole delta is the
  SeProfileSingleProcessPrivilege assignment to the same Service SID

ACTUAL_RUN_COUNT_RULE =
  a valid full pair records 1 CONTROL / 1 TREATMENT / 2 total; CONTROL drift
  after CONTROL records 1 / 0 / 1; failure before CONTROL launch records
  0 / 0 / 0; no retry or third discovery run is permitted

RESULT_CLASSIFICATION =
  CONTROL_DRIFT, PROFILE_SINGLE_SUFFICIENT_IN_PAIRED_I2G_CONTEXT,
  PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT, DISCOVERY_FAILED,
  TOKEN_GATE_FAILED, IDENTITY_MISMATCH, CLEANUP_FAILED, or
  INVALID_NO_CAUSAL_INTERPRETATION

ONE_TIME_GATE =
  I2G_GATE_CONSUMED = true;
  I2G_REAL_EXECUTION_ALLOWED = false by default;
  separate explicit human authorization is required after offline harness
  implementation and review; this task creates no executable gate
```

### Future result semantics

The result interpretation is preregistered:

- A valid CONTROL must produce the preregistered `POWER_UNAVAILABLE` result.
  `POWER_AVAILABLE` or any operational/identity/token drift is
  `CONTROL_DRIFT`; treatment must not execute and the experiment has no causal
  interpretation.
- Only when CONTROL is valid negative, TREATMENT `POWER_AVAILABLE` means that
  introducing usable `SeProfileSingleProcessPrivilege` from `ABSENT` to
  `PRESENT + ENABLED` was sufficient to change counter discovery in the
  paired fresh-I2G context. It does not establish necessity, production
  suitability, or a minimum privilege, and does not attribute causality to
  enablement alone outside the paired intervention.
- When CONTROL is valid negative and TREATMENT is `POWER_UNAVAILABLE`, the
  selected capability was insufficient in the paired fresh-I2G context. It
  does not prove universal irrelevance.
- Any pre/post token, identity, group, CLI, sampling, or cleanup mismatch is
  a failed/invalid qualification result and must not be interpreted as a
  causal AMD result.

## Current-state handoff

```text
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_VARIABLE_SELECTION = PASS_READ_ONLY
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_SELECTION_CHANGED = false
I2G_SINGLE_VARIABLE_ISOLATABLE = true
I2G_BASELINE_RECONSTRUCTION_RIGHT = SeSystemProfilePrivilege
I2G_TREATMENT_VARIABLE = SeProfileSingleProcessPrivilege
I2G_EXPERIMENT_SHAPE = PAIRED_CONTROL_TREATMENT
HISTORICAL_I2F_ROLE = PREDECESSOR_EVIDENCE_ONLY
HISTORICAL_I2F_IS_ACTIVE_CAUSAL_CONTROL = false
TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1
PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1
PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
PLANNED_VALID_PAIR_COUNTER_DISCOVERY_RUNS = 2
MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1
MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2
ACTUAL_RUN_COUNT_EVIDENCE_SCHEMA = DEFINED
POWER_SAMPLING_RUNS = 0
I2G_HARNESS = NOT_IMPLEMENTED
I2G_REAL_RUNTIME = 0
I2G_HARNESS_IMPLEMENTATION_AUTHORIZED = false
I2G_REAL_RUNTIME_AUTHORIZED = false
NEXT_GATE = I2G_HARNESS_DESIGN_AND_OFFLINE_IMPLEMENTATION_REVIEW
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = NOT_COMPLETE
I1_HISTORICAL_HARNESS = OUT_OF_SCOPE
```

A single I2G variable remains selected from read-only evidence. Historical I2F
is predecessor evidence only; the future causal design is a paired
CONTROL -> TREATMENT experiment using one fresh Service SID and one unchanged
harness artifact. No I2G harness exists yet and no real experiment is
authorized. The next task is offline I2G harness design/implementation
followed by review. Human authorization for the paired non-sampling runs can
only be considered after that separate review passes.

## CURRENT STATE — OFFLINE IMPLEMENTATION COMPLETE

The preceding sections are preserved as read-only design history. The default
implementation is the fail-closed, synthetic harness in
`tools/amd-privilege-qualification`; see
[`amd-i2g-harness.md`](amd-i2g-harness.md) for the complete contract.

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
PLANNED_CONTROL_COUNTER_DISCOVERY_RUNS = 1
PLANNED_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
PLANNED_VALID_PAIR_COUNTER_DISCOVERY_RUNS = 2
MAX_CONTROL_COUNTER_DISCOVERY_RUNS = 1
MAX_TREATMENT_COUNTER_DISCOVERY_RUNS = 1
MAX_TOTAL_I2G_COUNTER_DISCOVERY_RUNS = 2
POWER_SAMPLING_RUNS = 0
CONTROL_RETRY_ALLOWED = false
TREATMENT_RETRY_ALLOWED = false
CONTROL_DRIFT_STOP_BEFORE_TREATMENT = true
TOKEN_TEARDOWN_BEFORE_TREATMENT_POLICY_MUTATION = true
NEW_REAL_RUN_REQUIRED = false
NEXT_GATE = HUMAN_FINAL_REVIEW_BEFORE_MARKING_PR24_READY
```

The implementation adds no production account selection. The default I2G surface
is synthetic/offline-only and fail-closed, while
the shared qualification executable still contains historical non-I2G
entrypoints; this does not make the whole EXE offline-only and does not
authorize those entrypoints. Attempt #3 completed the one authorized paired
qualification and obtained the narrowly scoped result
`PROFILE_SINGLE_INSUFFICIENT_IN_PAIRED_I2G_CONTEXT`; no further real run is
required or authorized. The task-local real runner remains one-shot and
exact-token gated; it is not a reusable Windows backend or production integration.
