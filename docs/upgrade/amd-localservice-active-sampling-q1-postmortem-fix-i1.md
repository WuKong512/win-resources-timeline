# AMD LocalService active-sampling Q1 postmortem fix I1

This document records a harness repair after the single Q1 live attempt. It
does not reopen, rerun, or reinterpret that attempt, and it contains no new
live authorization.

## Immutable historical state

The historical run and gate remain read-only inputs:

~~~text
RUN_ID = q1-20260910T033830222Z-678c32a876384801a337d08f706bf994
Q1_GATE_STATE = CONSUMED
Q1_GATE_CONSUMED = YES
Q1_RUN_BUDGET_REMAINING = 0
Q1_RERUN_ALLOWED = NO
MAX_RUNS = 1
RETRIES = 0
Q1_GATE_FILE_CHANGED = NO
HISTORICAL_RAW_EVIDENCE_CHANGED = NO
~~~

The authoritative historical classification is:

~~~text
Q1_RESULT = BLOCKED_BY_HARNESS_VALIDATION_BUG
LIVE_PREFLIGHT = PASS
LSA_READ = PASS
LSA_VALIDATION = FALSE_NEGATIVE
LSA_MUTATION_ATTEMPTED = NO
AMD_CLI_REAL_INVOCATIONS = 0
AMD_API_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
SCIENTIFIC_RESULT = NOT_OBTAINED
CLEANUP = PASS
EVIDENCE_SEAL = PASS
FINAL_RESIDUE = CLEAN
~~~

This is not a `POWER_UNAVAILABLE`, scientific `FAIL`, or scientific `PASS`.
No conclusion about LocalService active sampling can be drawn from it.

## Primary bug and fix

The I2E read helpers returned `[ordered]`/`IDictionary` values. The old
validation path used a getter that only inspected `PSObject.Properties`, so it
treated valid nested `status = READ` records as unavailable and blocked before
the permitted LSA add. The sealed evidence supplied for the run showed:

~~~text
direct.status = READ
direct.account_object_state = ABSENT
direct.direct_rights = []
assignment.status = READ
Q1 Service SID in assignment.assigned_principals = NO
~~~

The Q1 Service SID was
`S-1-5-80-3346365147-1215615911-2069952304-2985730531-3677249097`. The
assignment list contained Administrators and a different Service SID, not the
Q1 SID. Therefore the correct precondition was
`RIGHT_ABSENT_BEFORE = TRUE`; no LSA mutation was attempted.

The repair makes `Get-ContractPropertyValue` dictionary-first and keeps
`Get-Q1DictionaryValue` as the single mixed-shape access abstraction. The Q1
LSA before, after, cleanup, and recovery validators now use it for outer
snapshots and all nested fields, including status, account state, rights,
assigned principals, and Service SID. Canonical SID checks remain fail-closed.

The protected historical run root could not be opened by the current workspace
principal because its sealed ACL denies read access. No ACL was changed. The
offline replay therefore uses the exact authoritative sealed evidence shape
provided for this postmortem, serialized/deserialized through the same JSON
boundary, and is explicitly diagnostic rather than a rewrite of the historical
file.

## Q1 object-shape audit

The audit was limited to the Q1 harness paths:

| Path | Input shapes | Result |
| --- | --- | --- |
| `Get-ContractPropertyValue` | dictionaries and property objects | Mixed-shape getter fixed |
| `Get-Q1DictionaryValue` | dictionaries, property objects, JSON objects | Shared safe access |
| LSA before/after/cleanup validators | I2E snapshots and JSON evidence | Mixed-shape risks fixed |
| `Get-Q1LsaRecoveryPlan` | durable mutation evidence and readback | Mixed-shape risks fixed |
| Driver contract validation | `[ordered]` driver dictionary | Explicit dictionary enumeration |
| Invocation accounting | process records, JSON successors, gate evidence | Mixed-shape access fixed |

No project-wide property-access refactor was performed. The only remaining
`PSObject.Properties` use in this scope is the property-object fallback inside
the shared getter; dictionary values are handled before that fallback.

## Real-shape regression and replay

The offline suite now exercises the exact I2E-shaped nested records as
`[ordered]` dictionaries, plus `PSCustomObject` and JSON-deserialized forms.
It covers readable and unavailable direct/assignment snapshots, pre-existing
direct or assigned rights, wrong Service SID, and malformed evidence.

The authoritative evidence-shape replay result is:

~~~text
SEALED_Q1_LSA_BEFORE_REPLAY = PASS
LSA_READ_STATUS = READ
RIGHT_ABSENT_BEFORE = TRUE
ORIGINAL_BLOCK_CLASSIFICATION = FALSE_NEGATIVE_VALIDATION
~~~

This replay validates only the before-state interpretation. It does not turn
the historical run into a valid experiment and does not authorize continuation.

## Gate accounting repair

Q1 gate consumption and AMD invocation accounting are now separate. The
accounting function accepts the actual Q1 gate evidence; the runner obtains it
from the gate state after acquisition and passes it through finalization. A
launch-intent file is no longer allowed to imply gate consumption.

The corrected historical interpretation is:

~~~text
Q1_GATE_CONSUMED = YES
SECOND_RUN_FORBIDDEN = TRUE
DURABLE_LAUNCH_INTENT = FALSE
PROCESS_STARTED = FALSE
AMD_CLI_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
INVOCATION_CERTAINTY = CONFIRMED_ZERO
~~~

Consequently, zero AMD invocations do not reopen the consumed gate, and a
consumed gate does not imply that AMD was invoked. The actual Q1 gate was only
read for the allowed postmortem/preflight checks; it was not rewritten, reset,
or recreated by this repair.

## Validation status

The offline harness suite passed the dictionary/object-shape, historical gate
accounting, LSA recovery, evidence sealing, invocation ambiguity, token,
command, source-identity, and read-only preflight regressions. The test suite
uses temporary fixtures and did not touch the historical run root or gate.

The known unrelated I2G blocker remains unchanged:

~~~text
PRE_EXISTING_I2G_ARTIFACT_SHA256_MISMATCH
EXPECTED = 2613129D179EA2A0496AD680E68E77A79FFFBB569D0802A11AC03346E162DD80
ACTUAL   = 061082A9FD65BB768CF450CFBE1F9523A06FA4362BBD1853154B17A9C90E0BC8
~~~

No I2G run, Attempt #4, Q2 experiment, AMD CLI/API invocation, service
creation, LSA mutation, ACL mutation, token mutation, driver/device mutation,
platform-security mutation, or production-runtime change occurred.

## Current-state handoff

~~~text
AMD_LOCALSERVICE_ACTIVE_SAMPLING_Q1_POSTMORTEM_FIX_I1 = PASS
PRIMARY_BUG = FIXED
LSA_IDICTIONARY_VALIDATION = FIXED
LSA_REAL_SHAPE_REGRESSION = PASS
SEALED_Q1_LSA_BEFORE_REPLAY = PASS
SECONDARY_BUG = FIXED
Q1_GATE_ACCOUNTING = AUTHORITATIVE
HISTORICAL_Q1_RESULT = BLOCKED_BY_HARNESS_VALIDATION_BUG
SCIENTIFIC_RESULT = NOT_OBTAINED
PROXY_VERDICT = INSUFFICIENT
ENUMERATION_AND_SAMPLING_EQUIVALENCE = UNKNOWN
AMD_PRODUCTION_ADMISSION = DEFER
PRODUCTION_ACCOUNT = UNRESOLVED
I2H_JUSTIFIED = NO
REAL_EXECUTION_ALLOWED = false
AMD_CLI_REAL_INVOCATIONS = 0
POWER_SAMPLING_RUNS = 0
CURRENT_TASK_SERVICE_MUTATIONS = 0
CURRENT_TASK_LSA_MUTATIONS = 0
CURRENT_TASK_ACL_MUTATIONS = 0
OLD_AUTHORIZATION_REUSABLE = NO
NEW_LIVE_AUTHORIZATION_CREATED = NO
SELECTED_NEXT_TASK = HUMAN_REVIEW_PR29_Q1_POSTMORTEM_FIX
NEXT_GATE = HUMAN_REVIEW_PR29_Q1_POSTMORTEM_FIX
~~~

The next action is human review of PR #29 and this postmortem repair. The Q1
one-shot gate remains permanently closed; this document does not propose a Q2
run or authorize any new experiment.
