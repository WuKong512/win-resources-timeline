# AMD-PRIVILEGE-I2G variable selection

This document is the read-only closure for the next AMD privilege experiment.
It selects a future single variable; it does not implement, authorize, or run
I2G. No AMD process, service, driver, device, IOCTL, sampling session, LSA
mutation, token mutation, ACL mutation, registry mutation, or historical
evidence mutation was performed for this review.

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
BASE_SECURITY_CONTEXT = final authoritative I2F post-enable context
BLOCKER = I2G_BASELINE_RECONSTRUCTION_CONTRACT_INCONSISTENT
BLOCKER_STATUS = CLOSED_OFFLINE
I2G_BASELINE_RECONSTRUCTION_RIGHT = SeSystemProfilePrivilege
I2G_TREATMENT_VARIABLE = SeProfileSingleProcessPrivilege
TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1
BASELINE_RECONSTRUCTION_ROLE = frozen final-I2F state; not a new treatment
TREATMENT_ROLE = selected single causal capability
SE_SYSTEM_PROFILE_PRIVILEGE_STATE_IN_LOGICAL_BASELINE = PRESENT + ENABLED
SE_PROFILE_SINGLE_PROCESS_PRIVILEGE_STATE_IN_LOGICAL_BASELINE = PRESENT + DISABLED
ADMINISTRATORS_MEMBERSHIP_MUTATION = FORBIDDEN
LOCAL_SYSTEM_AS_I2G_VARIABLE = FORBIDDEN_AS_NON_SINGLE_VARIABLE
SE_DEBUG_PRIVILEGE_MUTATION = FORBIDDEN
```

The review blocker was an inconsistent description of a fresh Service SID as
receiving only `SeProfileSingleProcessPrivilege` while also requiring the
final-I2F `SeSystemProfilePrivilege` state. It is closed by separating logical
baseline reconstruction from the treatment variable. A fresh I2G Service SID
must temporarily receive exactly two rights: `SeSystemProfilePrivilege` to
reconstruct the frozen final-I2F baseline, and `SeProfileSingleProcessPrivilege`
as the one intentional treatment capability. The two policy assignments do
not make this a two-variable experiment.

The historical I2F Service SID's policy assignment cannot persist across a
fresh Service SID. Therefore `SeSystemProfilePrivilege = PRESENT + ENABLED`
is a logical baseline state, while the temporary assignment of that right is
the physical operation required to recreate it. No right is added to the
global LocalService account. No Administrators membership, LocalSystem
account, AMD service ACL, driver ACL, or registry ACL is part of the selected
variable.

## Design-only I2G experiment contract

This contract is a specification for a later implementation review. It is not
executable code and does not create an authorization path.

```text
I2G_BASE_CONTEXT =
  LocalService (S-1-5-19), fresh unrestricted dedicated Service SID,
  Session 0, x64, System integrity, exact final-I2F token/group baseline,
  exact fixed AMD CLI identity, fixed working directory and environment

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
  enable SeSystemProfilePrivilege only; capture I2G-BASELINE-TOKEN.json;
  do not launch AMD CLI before this exact baseline gate passes

EXPECTED_BASELINE_TOKEN =
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = PRESENT + DISABLED;
  exact final-I2F relevant state; no Administrators SID; no SeDebugPrivilege

I2G_BASELINE_RECONSTRUCTION = EXACT_FINAL_I2F_RELEVANT_STATE
I2G_BASELINE_RECONSTRUCTION_TOKEN_DELTA = SeSystemProfilePrivilege DISABLED -> ENABLED

PHASE_C_TREATMENT =
  after baseline verification, enable SeProfileSingleProcessPrivilege only;
  capture I2G-TREATMENT-TOKEN.json

EXPECTED_TREATMENT_TOKEN =
  SeSystemProfilePrivilege = PRESENT + ENABLED;
  SeProfileSingleProcessPrivilege = PRESENT + ENABLED;
  all other final-I2F relevant privilege, group, account, session, and
  integrity states unchanged

I2G_TREATMENT_TOKEN_DELTA = SeProfileSingleProcessPrivilege DISABLED -> ENABLED
I2G_EXACT_TREATMENT_DELTA = ONE_PRIVILEGE_STATE_CHANGE

NO_BASELINE_AMD_RUN = true
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
  system_profile_remove_attempted; system_profile_remove_verified;
  profile_single_remove_attempted; profile_single_remove_verified;
  already_absent is recorded independently for each right

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
  TOKEN_MATERIALIZED; TOKEN_BASELINE; TOKEN_TREATMENT;
  BASELINE_RECONSTRUCTION_DELTA; TREATMENT_DELTA; relevant groups;
  AMD_CLI_IDENTITY; COUNTER_DISCOVERY_RESULT;
  POLICY_ROLLBACK_TREATMENT; POLICY_ROLLBACK_BASELINE; FULL_ROLLBACK;
  command; sampling=false; bounded stdout/stderr; exit code; normalized
  result; cleanup and rollback evidence

RESULT_CLASSIFICATION =
  POWER_AVAILABLE, POWER_UNAVAILABLE, DISCOVERY_FAILED, TOKEN_GATE_FAILED,
  IDENTITY_MISMATCH, CLEANUP_FAILED, or INVALID_NO_CAUSAL_INTERPRETATION

INVALID_RESULT_BASELINE_GATE =
  INVALID_NO_CAUSAL_INTERPRETATION if baseline SeSystemProfilePrivilege is
  not PRESENT + ENABLED; ProfileSingle is not PRESENT + DISABLED at baseline;
  any unexpected privilege is present; any expected I2F state changes;
  Administrators appears; TokenUser, session, or integrity changes; AMD CLI
  identity mismatches; sampling is not false; treatment delta contains more
  than ProfileSingle enablement; or rollback is incomplete

ONE_TIME_GATE =
  I2G_GATE_CONSUMED = false;
  I2G_REAL_EXECUTION_ALLOWED = false by default;
  a separate explicit human authorization is required after offline harness
  implementation and review; this task creates no executable gate
```

### Future result semantics

The result interpretation is preregistered:

- `SeProfileSingleProcessPrivilege enabled + POWER_AVAILABLE` means the
  selected right is sufficient only in the preserved final-I2F base context.
  It does not establish necessity, production suitability, or a minimum
  privilege in another account/context.
- `SeProfileSingleProcessPrivilege enabled + POWER_UNAVAILABLE` means that
  adding this right alone to the preserved I2F base is insufficient. It does
  not prove the right irrelevant in another context or prove that another
  combination is unnecessary.
- Any pre/post token, identity, group, CLI, sampling, or cleanup mismatch is
  a failed/invalid qualification result and must not be interpreted as a
  causal AMD result.

## Current-state handoff

```text
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_VARIABLE_SELECTION = PASS_READ_ONLY
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

A single I2G variable has been selected from read-only evidence. No I2G
harness exists yet and no real experiment is authorized. The next task is
offline I2G harness design/implementation followed by review. Human
authorization for one real non-sampling I2G run can only be considered after
that separate review passes.
