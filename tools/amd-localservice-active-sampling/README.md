# AMD LocalService active-sampling qualification harness

This directory is a qualification-only harness for the already-frozen
AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1 contract. It is not a production
collector, provider, installer, broker, or Rust runtime component.

Current state:

~~~text
HARNESS_IMPLEMENTATION = REVIEW_FIX_COMPLETE
Q1_LIVE_RUN = HARNESS_READY_PENDING_REVIEW
Q1_LIVE_RUN_AUTHORIZED = NO
AMD_CLI_REAL_INVOCATIONS_DURING_IMPLEMENTATION = 0
POWER_SAMPLING_RUNS_DURING_IMPLEMENTATION = 0
~~~

## Files and responsibilities

- contract.ps1 contains the immutable account, Session 0, token, binary,
  driver, command, output, one-shot budget, LSA allowance, and reviewed
  source-hash values.
- run-amd-localservice-active-sampling.ps1 is the administrator-side
  controller. DryRun is the default. Live requires a new task-specific
  authorization and performs the preflight, exact service lifecycle, evidence
  collection, cleanup, and residue check.
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
service state. The observed user-mode 5.3.521.0 versus driver 5.3.481.0
split remains COMPATIBILITY = UNKNOWN; the harness does not reinterpret it
as an automatic mismatch.

The effective token must contain the I2F/I2G CONTROL semantics: System
integrity, no Administrators membership, a dedicated Service SID,
SeSystemProfilePrivilege enabled, and SeProfileSingleProcessPrivilege absent.
The future live path may materialize only the exact CONTROL baseline right on
the newly created Q1 Service SID. It must first prove that the right is absent,
durably record intent, read back the exact assignment, track ownership, and
remove the right only when this run added it. It never changes the
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
package-power CSV parsing, LSA ownership decisions, gate consumption semantics,
and irreversible post-start invocation accounting. It simulates lifecycle
state only. It does not create the ProgramData output base, register a
service, call sc.exe, read an effective service token, read or mutate LSA
policy, or invoke AMDuProfCLI.

The regression suite is:

~~~powershell
pwsh -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File .\tools\amd-localservice-active-sampling\test-harness.ps1
~~~

## Live path boundary

The live controller is disabled unless all of the following are present:

1. Live mode is explicitly selected.
2. The new task-specific switch AuthorizeLiveRun is present.
3. The new token AMD-LOCALSERVICE-ACTIVE-SAMPLING-Q1 is supplied.
4. The environment marker AMD_LOCALSERVICE_ACTIVE_SAMPLING_AUTHORIZATION has
  the value GRANTED_FOR_THIS_TASK_ONLY.
5. All read-only preflight gates pass.
6. The exact reviewed SHA256 identity of the controller, service host,
   contract, parser, sc.exe helper, and LSA/cleanup helpers passes before any
   live mutation.

The live path consumes Q1-LIVE-GATE.json before service registration. The gate
is one-shot with MAX_RUNS=1 and RETRIES=0; service failure, timeout, malformed
output, nonzero exit, and post-launch evidence failure do not permit retry.
The gate is separate from the consumed I2G gate.

The moment Process.Start() succeeds, the run is irreversibly counted as one
AMD CLI invocation and one sampling run. Durable launch-started evidence and
the final process state preserve that count even if later capture, parsing,
cleanup, or summary generation fails. A post-start harness error is never
reported as zero invocations.

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

Only a new run root is eligible for the exact output ACL. The root grants
LocalService Modify access and SYSTEM/Administrators FullControl, without
inheriting user-write access from an unrelated data directory. The root is
never the production Resource Timeline data directory or historical evidence
directory.

Raw process stdout, stderr, launch data, token data, process result, vendor
output, and output hashes are retained under raw. Summary data is separate.
Cleanup stops the exact service, terminates only the owned worker/child tree
when necessary, deletes only the exact temporary service, and records residue
status. An unexpected child or remaining service/process is a harness failure,
not a scientific POWER_UNAVAILABLE result.

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
