# SYSTEM vs I2F residual AMD counter differential

This document records the latest offline interpretation of the completed I2F
qualification. It does not authorize another I2F run, a SYSTEM comparison, an
I2G experiment, or a production account change.

## Authoritative observations

| Input | SYSTEM comparison | I2F self-enable qualification |
| --- | --- | --- |
| Account | `S-1-5-18` / LocalSystem | `S-1-5-19` / LocalService |
| Session | 0 | 0 |
| Architecture | x64 | x64 |
| Integrity | System | System |
| Administrators SID | present | absent |
| `SeSystemProfilePrivilege` | enabled | enabled after `AdjustTokenPrivileges` |
| AMD CLI identity | `D:\apps\AMDuProf\bin\AMDuProfCLI.exe`, SHA-256 `D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC` | same |
| Operation | `timechart --list` | `timechart --list` |
| Result | `POWER_AVAILABLE` | `POWER_UNAVAILABLE` |

The I2F evidence scope is
`f68bf4d3d36547a0ba753cff489bb6eb`. Its dedicated Service SID received the
exact right, the service token contained the right as `PRESENT + DISABLED`, the
service enabled only that right, and the token delta was exactly
`DISABLED -> ENABLED`. The AMD CLI was spawned and completed with exit code
zero, but reported the bounded no-counters diagnostic. No sampling session was
requested or executed.

The earlier SYSTEM result is the successful comparison side. The I2F result is
therefore:

```text
I2F_RESULT = PASS_WITH_NEGATIVE_COUNTER_ACCESS_RESULT
SE_SYSTEM_PROFILE_PRIVILEGE_ALONE_SUFFICIENT = false
SE_SYSTEM_PROFILE_PRIVILEGE_NECESSITY = UNRESOLVED
I2F_RERUN = FORBIDDEN
```

This disproves sufficiency in the tested LocalService / Session 0 context. It
does not prove that the privilege is unnecessary, ineffective in every context,
or irrelevant to the AMD backend.

## Residual differential

```text
ACCOUNT_IDENTITY_DIFFERENTIAL = OPEN (SYSTEM vs LocalService)
ADMINISTRATORS_MEMBERSHIP_DIFFERENTIAL = OPEN (present vs absent)
SE_SYSTEM_PROFILE_DIFFERENTIAL = CLOSED (both tested enabled; not sufficient alone)
SE_PROFILE_SINGLE_PROCESS_DIFFERENTIAL = OPEN
SE_DEBUG_DIFFERENTIAL = OPEN
OTHER_TOKEN_PRIVILEGE_DIFFERENTIAL = OPEN
TOKEN_GROUP_DIFFERENTIAL = OPEN
SERVICE_ACCOUNT_SID_DIFFERENTIAL = OPEN (different dedicated Service SIDs)
AMD_DRIVER_DEVICE_ACL_DIFFERENTIAL = OPEN
AMD_SERVICE_INTERNAL_AUTHORIZATION_DIFFERENTIAL = OPEN
SESSION_DIFFERENTIAL = CLOSED (both Session 0)
ARCHITECTURE_DIFFERENTIAL = CLOSED (both x64)
AMD_CLI_IDENTITY_DIFFERENTIAL = CLOSED (same path/SHA/architecture/signature)
```

The Service SID difference is a controlled identity difference, not evidence
that either specific SID is causal. The SYSTEM result also includes additional
enabled privileges and the Administrators group, so those remain separate
hypotheses rather than being collapsed into “SYSTEM is required”.

## Conservative hypothesis ranking

### H1 — additional token privilege combination

- Evidence for: SYSTEM exposes additional enabled privileges beyond the single
  I2F privilege; I2F proves only that `SeSystemProfilePrivilege` alone is not
  sufficient.
- Evidence against: no additional privilege has yet been isolated as causal.
- Unknown: whether one privilege, or a combination, is consumed by the AMD
  backend.
- Lowest-risk read-only test: normalize the immutable SYSTEM and I2F token
  snapshots and compare enabled/disabled privilege and group sets without
  running a process.

### H2 — Administrators or another token-group access path

- Evidence for: Administrators is present in SYSTEM and absent in I2F; AMD
  service/device ACLs may grant access through that group.
- Evidence against: the exact object or device ACL has not been identified.
- Unknown: whether the backend checks membership or uses group-derived object
  access.
- Lowest-risk read-only test: inspect captured group evidence and read-only
  service/device/interface metadata; do not add group membership.

### H3 — LocalSystem-specific identity or service-token composition

- Evidence for: SYSTEM succeeded while LocalService failed after the tested
  privilege was explicitly enabled.
- Evidence against: the account identity is confounded with several remaining
  token/group/access differences.
- Unknown: whether AMD performs an account/SID-specific check.
- Lowest-risk read-only test: static inspection of AMD binaries and installed
  service metadata for account/SID checks; do not run LocalSystem again.

### H4 — AMD driver/device ACL or backend object authorization

- Evidence for: kernel/device and internal IPC boundaries can differ from
  service-object ACLs and are consistent with a counter backend failure.
- Evidence against: no exact device interface or object descriptor has yet been
  recovered.
- Unknown: the device name/interface and its security descriptor.
- Lowest-risk read-only test: inspect INF, registry, SetupAPI, strings/imports,
  and any safely queryable security descriptor without opening the device or
  issuing IOCTLs.

### H5 — Combined account/group and privilege dependency

- Evidence for: the SYSTEM side differs in multiple security dimensions and a
  single-right test was negative.
- Evidence against: no minimum combination has been isolated.
- Unknown: the smallest sufficient set.
- Lowest-risk read-only test: maintain a structured differential matrix before
  proposing any mutation experiment.

## I2G boundary

```text
I2G_VARIABLE = SeProfileSingleProcessPrivilege
I2G_VARIABLE_SELECTION = PASS_READ_ONLY
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_SINGLE_VARIABLE_ISOLATABLE = true
I2G_BASELINE_RECONSTRUCTION_RIGHT = SeSystemProfilePrivilege
I2G_TREATMENT_VARIABLE = SeProfileSingleProcessPrivilege
TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1
I2G_REAL_RUNTIME = 0
I2G_HARNESS = NOT_IMPLEMENTED
I2G_HARNESS_IMPLEMENTATION_AUTHORIZED = false
I2G_REAL_RUNTIME_AUTHORIZED = false
PRODUCTION_ACCOUNT = UNRESOLVED
LOCAL_SYSTEM_PRODUCTION_SELECTION = NOT_AUTHORIZED
PRODUCTION_ADMISSION = NOT_COMPLETE
NEXT_GATE = I2G_HARNESS_DESIGN_AND_OFFLINE_IMPLEMENTATION_REVIEW
```

Any future I2G must be a fresh, human-authorized, non-sampling
`timechart --list` qualification with one intentional variable, a fresh
Service SID, exact rollback, and no bulk privilege or group grants. This file
does not implement, authorize, or execute that experiment. The detailed
normalized matrix, candidate scoring, and design-only contract are recorded in
[`amd-i2g-variable-selection.md`](amd-i2g-variable-selection.md).

## I2G variable-selection closure

The read-only review selected exactly one future variable:

```text
I2G_VARIABLE_SELECTED = SeProfileSingleProcessPrivilege
BASE_ACCOUNT = LocalService / S-1-5-19
BASE_CONTEXT = final-I2F non-treatment dimensions; ProfileSingle staged PRESENT + DISABLED
BLOCKER = I2G_STAGED_TREATMENT_TOKEN_MISCLASSIFIED_AS_EXACT_I2F_BASELINE
BLOCKER_STATUS = CLOSED_OFFLINE
PREVIOUS_BLOCKER = I2G_BASELINE_RECONSTRUCTION_CONTRACT_INCONSISTENT / CLOSED_OFFLINE
I2G_BASELINE_RECONSTRUCTION_RIGHT = SeSystemProfilePrivilege
I2G_TREATMENT_VARIABLE = SeProfileSingleProcessPrivilege
TEMPORARY_POLICY_ASSIGNMENT_COUNT = 2
SCIENTIFIC_TREATMENT_VARIABLE_COUNT = 1
EXPECTED_MATERIALIZED_TOKEN = SystemProfile PRESENT + DISABLED; ProfileSingle PRESENT + DISABLED
HISTORICAL_I2F_CONTROL = SystemProfile PRESENT + ENABLED; ProfileSingle ABSENT; POWER_UNAVAILABLE
EXPECTED_STAGED_TOKEN = SystemProfile PRESENT + ENABLED; ProfileSingle PRESENT + DISABLED
EXPECTED_TREATMENT_TOKEN = SystemProfile PRESENT + ENABLED; ProfileSingle PRESENT + ENABLED
I2G_NON_TREATMENT_BASELINE_INVARIANTS = EXACT_FINAL_I2F
I2G_STAGED_TREATMENT_EXCEPTION = SeProfileSingleProcessPrivilege PRESENT + DISABLED
STAGED_EXCEPTION_COUNT = 1
BASELINE_RECONSTRUCTION_TOKEN_DELTA = SeSystemProfilePrivilege DISABLED -> ENABLED
I2G_TREATMENT_MATERIALIZATION_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + DISABLED
I2G_TREATMENT_ACTIVATION_DELTA = SeProfileSingleProcessPrivilege DISABLED -> ENABLED
I2G_TOTAL_CAUSAL_TREATMENT_DELTA = SeProfileSingleProcessPrivilege ABSENT -> PRESENT + ENABLED
NO_BASELINE_AMD_RUN = true
ADMINISTRATORS_MEMBERSHIP_MUTATION = FORBIDDEN
SE_DEBUG_PRIVILEGE_MUTATION = FORBIDDEN
LOCAL_SYSTEM_AS_I2G_VARIABLE = FORBIDDEN_AS_NON_SINGLE_VARIABLE
I2G_SELECTION_CONFIDENCE = MEDIUM
I2G_HARNESS = NOT_IMPLEMENTED
I2G_REAL_RUNTIME = 0
```

The fresh Service SID cannot inherit the historical I2F Service SID's LSA
assignment. The future contract therefore temporarily assigns exactly two
rights with different roles: `SeSystemProfilePrivilege` reconstructs the
frozen final-I2F non-treatment state, and `SeProfileSingleProcessPrivilege`
is the one scientific treatment variable. Historical I2F had ProfileSingle
`ABSENT`; the fresh I2G materialized and staged token has it
`PRESENT + DISABLED`, and only treatment enables it. No baseline AMD run is
planned. The future staged token is not an exact copy of historical I2F.

`SeProfileSingleProcessPrivilege` remains selected because it is a recorded
SYSTEM-only enabled privilege with the strongest indirect semantic connection
to a profiler path and the cleanest reversible one-right/one-enable treatment
shape. This is a future sufficiency test in the reconstructed I2F context, not
a claim of necessity. The exact device/backend authorization remains indirect
and no direct AMD SID check or device ACL was recovered.

## Counter-discovery evidence semantics

The historical I2F launch/result JSON is byte-preserved. Its
`amd_runtime_executed=false` field was emitted even though the counter-
discovery CLI process was spawned and completed. The field is retained as a
legacy power-sampling indicator for compatibility; new counter-discovery
evidence adds:

```text
counter_discovery_cli_executed = true after Command::spawn succeeds
power_sampling_runtime_executed = false
sampling = false
```

The additive fields keep schema v1 readers compatible while making CLI
execution, sampling, and counter availability independent facts. Spawn failure
does not produce a successful execution flag; a spawned process remains
`counter_discovery_cli_executed=true` even if its exit code is nonzero.
