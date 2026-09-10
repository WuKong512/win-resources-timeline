# AMD LocalService active-sampling qualification harness

This directory is a qualification-only harness for the already-frozen
AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1 contract. It is not a production
collector, provider, installer, broker, or Rust runtime component.

Current state:

~~~text
HARNESS_IMPLEMENTATION = Q1_POSTMORTEM_FIX_I1_COMPLETE
Q1_LIVE_RUN = CLOSED_BLOCKED_BY_HARNESS_VALIDATION_BUG
Q1_LIVE_RUN_AUTHORIZED = NO
HISTORICAL_Q1_RUN = q1-20260910T033830222Z-678c32a876384801a337d08f706bf994
Q1_GATE_CONSUMED = YES
LIVE_RUNS_COMPLETED = 0
Q1_RUN_BUDGET_REMAINING = 0
Q1_RERUN_ALLOWED = NO
AMD_CLI_REAL_INVOCATIONS_DURING_POSTMORTEM = 0
POWER_SAMPLING_RUNS_DURING_POSTMORTEM = 0
~~~

The single historical Q1 attempt is permanently closed. It reached the live
preflight and created the temporary service, but was blocked before LSA
materialization by a false-negative validation result. The sealed read evidence
showed `status = READ`, an absent Q1 Service SID right, and no attempted LSA
mutation. The historical classification is therefore
`BLOCKED_BY_HARNESS_VALIDATION_BUG`, not `PASS`, `FAIL`, or
`POWER_UNAVAILABLE`. The Q1 gate remains consumed and the run cannot be rerun.

The postmortem fix makes nested LSA evidence access safe for
`IDictionary`/`[ordered]` values as well as property-backed and JSON-deserialized
objects. It also takes Q1 gate consumption from authoritative gate evidence,
independently of AMD launch evidence. A consumed gate with zero AMD invocations
is valid historical accounting; zero invocation evidence does not reopen the
gate. See `docs/upgrade/amd-localservice-active-sampling-q1-postmortem-fix-i1.md`
for the frozen historical classification and replay boundary.

## Files and responsibilities

- contract.ps1 contains the immutable account, Session 0, token, binary,
  driver, command, output, one-shot budget, LSA allowance, and reviewed
  source-hash values.
- run-amd-localservice-active-sampling.ps1 is the administrator-side
  controller. DryRun is the default. Preflight performs the same host-facing
  checks as Live without authorization or mutation. Live requires a new
  task-specific authorization and performs the preflight, exact service
  lifecycle, evidence collection, cleanup, and residue check.
- service-host.ps1 is the dedicated Windows ServiceBase host. SCM starts it
  as LocalService in Session 0. It launches one worker, captures the worker's
  effective token, captures the exact SCM-generated Service SID, and only then
  can the worker launch the fixed AMD command.
- test-harness.ps1 is an offline regression suite. It uses fixtures and the
  dry-run path; it does not create a service or invoke an AMD binary.

The implementation reuses only the side-effect-free I2E helper library for the
qualified sc.exe argument contract and exact LSA helper. It does not dot-source
or execute the I2G real runner, does not use the I2G authorization marker, and
does not modify the existing LocalSystem service-context harness.

## Frozen command and identities

The service worker accepts no caller-supplied executable, arbitrary arguments,
working directory, environment, or output path. The manifest is checked
against contract.ps1 before the worker can launch anything.

~~~text
ACCOUNT = NT AUTHORITY\LocalService
ACCOUNT_SID = S-1-5-19
SERVICE_SID_TYPE = unrestricted
SESSION = 0
INTERACTIVE = NO
AMD_CLI = D:\apps\AMDuProf\bin\AMDuProfCLI.exe
AMD_CLI_SHA256 = D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
COMMAND = timechart --event power --interval 1000 --duration 10 --format csv --output-dir <RUN_ROOT>\raw\timechart-output
MAX_RUNS = 1
RETRIES = 0
~~~

The read-only driver preflight rechecks AMDPowerProfiler.sys file version
10.6.3.0, AMDCpuProfiler.sys file version 4.4.1.0, both product metadata
version 5.3.481.0, AMD Authenticode identity, file SHA256, and running
service state. The frozen driver contract is an OrderedDictionary and is
validated to contain exactly AMDPowerProfiler and AMDCpuProfiler; adapter
properties such as Count, Keys, Values, SyncRoot, and similar noise are never
treated as drivers. Missing files, version drift, and host query failures are
returned as structured fail-closed evidence. The observed user-mode 5.3.521.0
versus driver 5.3.481.0 split remains COMPATIBILITY = UNKNOWN; the harness
does not reinterpret it as an automatic mismatch.

The effective token must contain the I2F/I2G CONTROL semantics: System
integrity, no Administrators membership, a dedicated Service SID,
SeSystemProfilePrivilege enabled, and SeProfileSingleProcessPrivilege absent.
The future live path may materialize only the exact CONTROL baseline right on
the newly created Q1 Service SID. It first proves that the right is absent,
durably records the exact task/service/right pre-state and mutation intent,
then reads back the exact assignment. Cleanup is driven by that durable intent
and an independently captured controller/service-definition Service SID, even
if the materialization function fails after LSA add. Recovery cross-checks the
intent, mutation-started, before, current, and available `sc.exe showsid`
identities before any remove. An unavailable recovery readback fails closed
and leaves the exact service registration for human diagnosis. It never changes the
LocalService account-global rights, LocalSystem, Administrators membership,
ProfileSingle, device ACLs, drivers, or platform security. A token mismatch
blocks before AMD CLI launch.

## Dry/offline validation

From the repository root:

~~~powershell
pwsh -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File .\tools\amd-localservice-active-sampling\run-amd-localservice-active-sampling.ps1 -Mode DryRun
~~~

The dry run validates command construction, contract fields, fixture binary
and driver identity, token expectations, run-root policy, one-shot budget, and
package-power CSV parsing, LSA ownership/recovery decisions, independent SID
anchor checks, exact Service SID ACL phases, evidence manifest hashing/sealing
decisions, gate consumption semantics, and irreversible/ambiguous post-start
invocation accounting. It simulates lifecycle
state only. It does not create the ProgramData output base, register a
service, call sc.exe, read an effective service token, read or mutate LSA
policy, or invoke AMDuProfCLI.

The regression suite is:

~~~powershell
pwsh -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File .\tools\amd-localservice-active-sampling\test-harness.ps1
~~~

The controller also exposes a true read-only host preflight:

~~~powershell
pwsh -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File .\tools\amd-localservice-active-sampling\run-amd-localservice-active-sampling.ps1 -Mode Preflight
~~~

Preflight reuses the live `Get-LivePreflight` checks for administrator
context, Git baseline and clean tree, reviewed source identity, LocalService
SID, AMD binary and driver identity, platform snapshot, qualification residue,
candidate run-root feasibility, service-host presence, and Q1 gate state. It
uses a hypothetical run id only in memory. It does not require
`AuthorizeLiveRun`, the authorization token, or the environment marker; it
does not create the output base/run root, service, gate, LSA right, ACL, token
state, or AMD process. A blocked result is a safe preflight result, not a
scientific qualification result.

## Live path boundary

The live controller is disabled unless all of the following are present:

1. Live mode is explicitly selected.
2. The new task-specific switch AuthorizeLiveRun is present.
3. The new-head token AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1-REVIEWED-PREFLIGHT-FIX-I1 is supplied.
4. The environment marker AMD_LOCALSERVICE_ACTIVE_SAMPLING_AUTHORIZATION has
  the value GRANTED_FOR_NEW_REVIEWED_HEAD_ONLY.
5. All read-only preflight gates pass.
6. The exact reviewed SHA256 identity of the controller, service host,
   contract, parser, sc.exe helper, and LSA/cleanup helpers passes before any
   live mutation.

The live path consumes Q1-LIVE-GATE.json before service registration. The gate
is one-shot with MAX_RUNS=1 and RETRIES=0; service failure, timeout, malformed
output, nonzero exit, and post-launch evidence failure do not permit retry.
The gate is separate from the consumed I2G gate.

The previous reviewed head `3ff66c258ffb2f6aafe64790abbd7287b70ddc4e` and its
authorization are obsolete after this preflight repair. A new human review
and new explicit live authorization are required before any Q1 live attempt.

The moment Process.Start() succeeds, the run is irreversibly counted as one
AMD CLI invocation and one sampling run. Durable launch-started evidence and
the final process state preserve that count even if later capture, parsing,
cleanup, or summary generation fails. If the worker stops after durable launch
intent but before either successor record is durable, accounting is
`UNKNOWN_0_OR_1` with `INVOCATION_CERTAINTY = AMBIGUOUS`; it is never serialized
as numeric zero and the consumed gate still forbids a second run.

The live path creates only the exact service
ResourceTimelineAmdLocalServiceActiveSamplingQualification. It uses a
Windows PowerShell ServiceBase host so SCM provides Session 0 and the
non-interactive service context. The service SID type is set to unrestricted;
the controller records the exact Service SID and validates the SCM-reported
LocalService account, demand start mode, own-process type, image path, SID
type, and token membership before launch.
The isolated run root is:

~~~text
C:\ProgramData\ResourceTimeline\qualification\amd-localservice-active-sampling-q1\<run-id>
~~~

Only a new run root is eligible for the output ACL. Staging grants only
SYSTEM/Administrators control. After service creation and exact Service SID
validation, only that Q1 Service SID receives Modify access; account-wide
`S-1-5-19`/LocalService write access is never granted. After the worker and
service stop, writer quiescence is verified independently of LSA cleanup. The
controller then inventories and hashes raw evidence, recursively removes Q1
Service SID write access, re-reads the ACL, and verifies the hashes even when
LSA cleanup failed closed. If writers are not quiesced, sealing is explicitly
blocked. The completed run root is preserved after any live attempt, including FAIL,
timeout, parser failure, or cleanup failure.

Raw process stdout, stderr, launch data, token data, process result, vendor
output, and output hashes are retained under raw. Summary data is separate. A
valid `cli-launch-start-failed.json` successor must match the frozen schema,
command, working directory, and run output path before accounting can be
confirmed zero; corrupt or mismatched successor evidence remains ambiguous.
Cleanup stops the exact service, terminates only the owned worker/child tree
when necessary, deletes the exact temporary service only after verified LSA
recovery and evidence sealing, and otherwise keeps registration for
recovery/diagnosis. An unexpected child or remaining service/process is a
harness failure, not a scientific POWER_UNAVAILABLE result.

Live mutation capabilities and current-task accounting are separate. The
harness supports exact temporary service lifecycle mutation, isolated output
ACL mutation, and exact Q1 CONTROL-baseline LSA mutation. The current review
fix performed zero of each: no real service, ACL, LSA, token, driver, device,
platform-security, or AMD operation occurred.

## Failure classifications

The harness keeps infrastructure and scientific outcomes separate:

| Condition | Classification |
| --- | --- |
| Preflight, manifest, account, Session 0, token, binary, driver, service, or residue gate fails | BLOCKED |
| CLI process never starts | LAUNCH_FAILURE or HARNESS_FAILED |
| CLI starts and a later harness operation fails | HARNESS_FAILURE_AFTER_PROCESS_START |
| CLI starts but times out | TIMEOUT |
| CLI starts and exits nonzero | ACCESS_DENIED, LOADER_FAILURE, VERSION_MISMATCH, DRIVER_UNAVAILABLE, BIOS_UNSUPPORTED, HYPERVISOR_UNSUPPORTED, NO_COUNTER, or CLI_RUNTIME_FAILURE |
| CLI completes but output is absent or malformed | OUTPUT_ARTIFACT_MISSING or PARSE_FAILED |
| Service/process cleanup or residue verification fails | HARNESS_CLEANUP_FAILED |
| CSV has finite, non-negative, non-constant package-power values | PASS_BOUNDED_PACKAGE_POWER |
| CSV has no package-power column | POWER_UNAVAILABLE |

The harness does not convert a service, loader, timeout, or evidence failure
into a scientific negative. It also does not make a production-account or
production-admission decision.

## Reuse and change boundary

The existing package-power postprocessor is reused for the same historical
CSV shape. I2F, I2G, the existing LocalSystem service-context harness, and
historical raw evidence are read-only inputs. No production Rust source is
changed and no production runtime behavior is changed.
