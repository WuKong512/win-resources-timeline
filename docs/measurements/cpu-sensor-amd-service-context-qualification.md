# CPU-SENSOR-AMD SERVICE-CONTEXT QUALIFICATION

## STATUS

```text
TASK = AMD-SERVICE-CONTEXT-I1
RESULT = PASS
RUNTIME = COMPLETED_FROM_EXISTING_AUTHORITATIVE_EVIDENCE
AUTOMATED_PREPARATION = PASS
SERVICE_BROKER_CANDIDATE = EVIDENCE_SUPPORTED_PENDING_PRIVILEGE_AND_IPC
AMD_PRIVILEGE_ARCHITECTURE = WINDOWS_SERVICE_BROKER
DECISION_CONFIDENCE = MEDIUM_TO_HIGH
SERVICE_SESSION0_AMD_CLI_QUALIFIED = true
MINIMUM_REQUIRED_WINDOWS_PRIVILEGES = UNPROVEN
QUALIFICATION_ONLY = true
AMD_RUNTIME_EXECUTED_BY_PREPARATION = false
AMD_RUNTIME_EXECUTED = true
SERVICE_REGISTERED_DURING_AUTHORITATIVE_RUN = true
SERVICE_REGISTRATION_REMOVED = true
SERVICE_REGISTERED_CURRENT = false
```

The pre-runtime preparation was narrowly scoped and performed no AMD runtime.
One manually authorized feasibility run is documented below; its evidence is
immutable and was not regenerated during this parser repair.

## QUALIFICATION PROBE ARTIFACT

The final local release build used for the authoritative manual run was:

```text
path = tools/amd-cli-service-context-qualification/target/release/amd-cli-service-context-probe.exe
architecture = x64
sha256 = C437215E32A77B2AAA24351F41DACA9F7F37AF3642048C48E059278205BE04FC
```

The release `target` directory is build output and is not part of the
documentation commit. The authoritative Administrator command passed this exact
hash. Any separately authorized future rerun would need to pass this exact hash
(or a newly rebuilt, separately reviewed hash) to the wrapper; the wrapper does
not trust an arbitrary service executable.

The preparation ledger for this I1 branch, recorded before PR creation, was:

```text
BASE_COMMIT = 91bc4dfaf6e7358473f71be634b349d9b0a82101
START_HEAD = 91bc4dfaf6e7358473f71be634b349d9b0a82101
ORIGIN_MAIN = 91bc4dfaf6e7358473f71be634b349d9b0a82101
MERGE_BASE = 91bc4dfaf6e7358473f71be634b349d9b0a82101
PR20_MERGE_COMMIT = 91bc4dfaf6e7358473f71be634b349d9b0a82101
BRANCH = qualify/amd-service-context-i1
PR_AT_PREPARATION = NOT_CREATED_YET
```

The exact pre-existing Service gate and exact-path AMD CLI process gate both
passed during Administrator-equivalent read-only preparation. The authoritative
wrapper rechecked both gates immediately before the authoritative service
registration; preparation did not register a Service and did not launch the
AMD CLI.

## FIRST HUMAN WRAPPER INVOCATION

The first manually authorized wrapper invocation stopped during pre-runtime
evidence preparation, after administrator and artifact preflight and before
the pre-existing process evidence file or any Service registration could be
created:

```text
FIRST_HUMAN_WRAPPER_INVOCATION = BLOCKED_PRE_RUNTIME_HARNESS
EVIDENCE_ROOT = C:\ProgramData\ResourceTimeline\qualification\amd-service-context\20260904T071828677Z
ROOT_CAUSE = EMPTY_PIPELINE_RESULT_COLLAPSED_TO_NULL_BEFORE_JSON_SERIALIZATION
FAILURE_OCCURRED_BEFORE_NEW_SERVICE = true
AMD_RUNTIME_EXECUTED = false
PROCESS_SPAWNED = false
SERVICE_REGISTERED = false
REAL_AMD_RUNTIME_COUNT = 0
SERVICE_CONTEXT_RUNTIME_COUNT = 0
SERVICE_SESSION0_AMD_CLI_QUALIFIED = NOT_YET_TESTED
```

The persisted final summary contained no service or CLI lifecycle evidence;
the exact Service and exact AMD CLI process were absent in the subsequent
read-only checks. This incident is a harness classification, not evidence
that LocalSystem or Session 0 is incompatible with the vendor CLI. At that
point, the Service candidate remained pending qualification. The later
authoritative run completed the Service/Session 0 qualification; the current
result is PASS.

The repair makes the process-list evidence explicitly array-shaped for zero,
one, or multiple matches and writes a stable `count` plus `processes` object.
It also makes the qualification PowerShell evidence UTF-8/BOM-readable by
Windows PowerShell 5.1 for deterministic Unicode error round-tripping.

## AUTHORITATIVE SERVICE-CONTEXT RUNTIME

The first pre-runtime harness repair was followed by exactly one manually
authorized Administrator x64 PowerShell run. Its evidence is immutable:

```text
AUTHORITATIVE_RUN = C:\ProgramData\ResourceTimeline\qualification\amd-service-context\20260904T080323173Z
AUTHORITATIVE_EVIDENCE_MUTATED = false
SERVICE_CONTEXT_VALID = true
SERVICE_ACCOUNT = NT AUTHORITY\SYSTEM
ACCOUNT_SID = S-1-5-18
SESSION_ID = 0
SERVICE_PROCESS_ARCHITECTURE = x64
TOKEN_ELEVATED = true
AMD_RUNTIME_EXECUTED = true
PROCESS_SPAWNED = true
TARGET_PID = 28376
AMD_CLI_EXIT = 0x00000000
TIMEOUT = false
STDERR_BYTES = 0
CAPTURE_COMPLETE = true
HARNESS_FAILED = false
PACKAGE_POWER_STATUS = PASS
PACKAGE_POWER_SAMPLE_COUNT = 9
SERVICE_RUN_QUALIFICATION = TARGET_COMPLETED
SERVICE_COMPLETED_CLEANLY = true
SERVICE_DELETE_VERIFIED = true
SERVICE_PROCESS_GONE = true
CLI_PROCESS_GONE = true
REAL_AMD_RUNTIME_COUNT_BEFORE_TASK = 1
REAL_AMD_RUNTIME_COUNT_DURING_REPAIR = 0
SERVICE_CONTEXT_RUNTIME_COUNT_BEFORE_TASK = 1
SERVICE_CONTEXT_RUNTIME_COUNT_DURING_REPAIR = 0
```

The wrapper initially reported `CADENCE_INCONCLUSIVE` because the vendor
timestamp `16:3:25:895` did not match its former fixed-width-only expression.
The deterministic parser now accepts one- or two-digit hour/minute/second
fields and one- to three-digit milliseconds, with explicit range validation.
It accepts one reasonable midnight rollover only once; arbitrary timestamp
regression remains inconclusive and is not normalized as a day rollover.

The authoritative timestamp fixture is re-parsed offline without starting a
process or service:

```text
DERIVED_FROM_EXISTING_AUTHORITATIVE_RUN = true
SOURCE_RUN = 20260904T080323173Z
REAL_RUNTIME_REEXECUTED = false
OBSERVED_VENDOR_TIMESTAMP_FORMAT = H:m:s:fff
AUTHORITATIVE_SAMPLE_COUNT = 9
AUTHORITATIVE_DELTA_COUNT = 8
AUTHORITATIVE_DELTAS_MS = [1002,1004,1009,1006,1006,1004,1002,1009]
MIN_DELTA_MS = 1002
MAX_DELTA_MS = 1009
MEAN_DELTA_MS = 1005.25
CADENCE_POLICY_RESULT = PASS
```

The current non-elevated preparation shell cannot read the ACL-protected
authoritative directory directly, and the repository has no separate offline
replay entrypoint. The focused repository-native regression therefore uses
the exact persisted timestamp sequence as a derived fixture and does not
modify the evidence root.

```text
INCIDENT_CLASSIFICATION = POST_RUNTIME_EVIDENCE_PARSER_DEFECT
TIMESTAMP_PARSER_REPAIR = PASS
MIDNIGHT_ROLLOVER = PASS
AMD_SERVICE_CONTEXT_RUNTIME = PASS
PACKAGE_POWER_SAMPLING = PASS
CADENCE_PARSER = PASS
AMD_SERVICE_CONTEXT_I1 = PASS
AMD_PROVIDER_PRODUCTION_ADMITTED = false
```

## BASELINE AND QUESTION

The existing architecture keeps the Resource Timeline application
non-elevated by default and treats the AMD CLI provider as optional and
failure-isolated. A manually elevated interactive `AMDuProfCLI.exe` ten-second
power session is technically qualified for a bounded run, but the historical
non-administrator path returned `ACCESSDENIED`. The immutable authoritative
run documented below proves that the same vendor CLI works from a genuine
LocalSystem Windows Service in Session 0 for this bounded session.

The single qualification question was:

```text
Can the vendor-owned AMDuProfCLI.exe successfully execute the already-qualified
10-second package-power profile when launched by a genuine LocalSystem Windows
Service process in Session 0, without an interactive desktop?
```

This is a context feasibility qualification, not production broker admission,
least-privilege selection, IPC qualification, or an all-day test.

## SERVICE ACCOUNT AND CONTEXT CONTRACT

The first feasibility run used `LocalSystem`. This is an upper-bound service
context chosen because it requires no credential material and is a native SCM
identity. A pass must not be interpreted as proof that LocalSystem is the
minimum required account; a later least-privilege qualification remains
required.

The service must persist, before launching AMD, the following evidence:

- fixed service name `ResourceTimelineAmdQualification`;
- service process ID and shallow parent process ID;
- `account_sid = S-1-5-18` and canonical account
  `NT AUTHORITY\\SYSTEM`;
- `session_id = 0`;
- x64 process architecture;
- integrity SID, token elevation boolean, and token elevation type;
- current directory, a non-secret environment subset, and service start time.

If the LocalSystem/Session 0/x64 context is not established, the service stops
and does not launch the AMD CLI. The service reports SCM
`START_PENDING`, `RUNNING`, `STOP_PENDING`, and `STOPPED` states and persists a
status journal for the same run.

```text
SERVICE_ACCOUNT = LocalSystem
SERVICE_SESSION0_REQUIRED = true
SERVICE_SESSION0_AMD_CLI_QUALIFIED = true
INTERACTIVE_DESKTOP_REQUIRED = false
```

## QUALIFICATION-ONLY HARNESS

The repository-scoped package is:

```text
tools/amd-cli-service-context-qualification/
amd-cli-service-context-probe.exe
```

It is deliberately separate from `src-tauri` and is not registered in
`collector::manager`, the Tauri application, the installer, or any production
provider catalog. It has no production IPC, autostart, installer, or normal
application dependency.

The service accepts exactly one argument shape:

```text
--run-root <one child directory of ProgramData\\ResourceTimeline\\qualification\\amd-service-context>
```

It rejects arbitrary executable paths, raw vendor arguments, shell commands,
arbitrary environment variables, and arbitrary working directories. The
service itself derives the CLI path and fixed arguments. The run-root is
restricted to the machine-owned qualification base and is supplied only by
the qualification wrapper; a production broker would derive its output
directory from trusted configuration rather than expose a user-controlled
path.

## AMD CLI CONTRACT

The service reads:

```text
HKLM\\SOFTWARE\\WOW6432Node\\AMD\\AMDProfiler
InstallationPath
```

and derives:

```text
<InstallationPath>\\bin\\AMDuProfCLI.exe
working directory = <InstallationPath>\\bin
```

The preparation wrapper validates the exact current qualification artifact:

```text
version = 5.3.521.0
sha256 = D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
architecture = x64
Authenticode = Valid
signer = AMD / Advanced Micro Devices
```

The service additionally re-reads the registry-derived path, checks x64 PE
identity, and checks the expected SHA before launching. Signature and file
version are explicitly part of the Administrator wrapper preflight; the
service records that this wrapper gate was used. No user-provided CLI path or
vendor binary is accepted.

The only argument vector is:

```text
timechart --event power --interval 1000 --duration 10 --format csv
  --output-dir <service-owned run root>\\timechart-output
```

There is exactly one launch, with a 30,000 ms CLI timeout and a larger bounded
service wait. No temperature/frequency events, sampling loop, `cmd.exe`,
PowerShell, `runas`, debugger, PATH mutation, preload, or API call from the
Resource Timeline process is involved.

## OUTPUT AND ACL MODEL

The authoritative wrapper used:

```text
%ProgramData%\\ResourceTimeline\\qualification\\amd-service-context\\<run-id>
```

The run directory is intended to grant LocalSystem full control and the
authorized Administrator wrapper `Modify` access (needed to persist
qualification and cleanup evidence), with inheritance disabled for the
qualification directory. Ordinary non-administrator users receive no access
through this ACL. It is not placed in the interactive user's `%TEMP%`, the AMD
installation, or the Windows directory. The authoritative wrapper created and
protected this directory before service start; the preparation stage did not
create this directory on the real machine, and the later authoritative run
did.

The service persists, at minimum:

```text
SERVICE-CONTEXT.json
SERVICE-STATUS.json
CLI-ARTIFACT-IDENTITY.json
AMD-CLI-LAUNCH.json
AMD-SERVICE-CLI-PROCESS-RESULT.json
SERVICE-RUN-RESULT.json
AMD-CLI.stdout.txt
AMD-CLI.stderr.txt
timechart-output\\timechart.csv       # if the vendor creates it
timechart-output\\session.uprof       # if the vendor creates it
```

Raw process evidence is written before post-processing. The existing
`tools/amd-uprof-cli-spike/postprocess.ps1` is the single package-power parser;
this service harness does not create a third parser. The authoritative
qualification wrapper wrote
`AMD-SERVICE-CONTEXT.qualification-before-cleanup.json` before deleting the
temporary service registration.

The qualification-only harness now persists `AMD-CLI-LAUNCH.json` immediately
after `AMDuProfCLI.exe` spawn succeeds, before polling or post-processing. Its
typed lifecycle outcome carries the in-memory spawn fact through every
`run_cli()` exit path. The service summary and, when needed, the
`SERVICE-HARNESS-ERROR.json` fallback artifact derive `amd_runtime_executed`
from that fact rather than from launch-file existence or a successful
qualification result. The states are
`NOT_LAUNCHED`, `LAUNCHED_INCOMPLETE_RESULT`, and
`LAUNCHED_COMPLETE_RESULT`; the complete state additionally requires durable
process-result persistence. Therefore a launch-evidence write failure, timeout,
target failure, or later service post-processing error cannot be reported as
“not executed” after the CLI was actually launched. This repair was validated
with synthetic non-AMD evidence and the immutable authoritative timestamp
fixture; the Service/Session 0 qualification is `PASS`.

## PROCESS OWNERSHIP AND LIFECYCLE

The service launches the exact registry-derived CLI directly with a fixed
argument vector and the AMD `bin` working directory. Standard output and
standard error are redirected to service-owned files. The service records PID,
start/end time, signed/hex exit status, timeout, cancellation, capture
completeness, and output paths.

Where Windows permits it, the child is assigned to a transient job object so a
timeout or stop request can terminate only the owned process tree. The fallback
is an exact child-process termination, never a global image-name kill. A Job is
retained only after `AssignProcessToJobObject` succeeds; cleanup then observes
the exact child exit with a bounded poll, so a successful termination request
is not confused with target termination. A stop control is reported as
cancellation, not as a vendor success or failure.

The qualification wrapper waits for the service to reach `STOPPED`, reads all raw
evidence, and computes the qualification snapshot. Only then does it stop if
needed, delete the exact service name, verify that SCM no longer returns it,
and check the recorded service/CLI PIDs. It does not claim that unrelated AMD
processes are gone.

## QUALIFICATION CONTRACT

The qualification result is `PASS` only if all of the following are true:

- LocalSystem SID and Session 0 proof is complete;
- x64 service context and token evidence are present;
- registry-derived CLI identity passes the exact preflight;
- the target starts and completes without timeout/cancellation;
- target exit is signed `0`;
- stdout/stderr are persisted and capture is complete;
- `timechart.csv` and `session.uprof` exist;
- the shared package-power parser returns finite samples greater than zero;
- sample cadence approximately follows the requested 1,000 ms interval;
- service status reaches `STOPPED` and cleanup is verified.

Failure categories stay distinct:

```text
SERVICE_HARNESS_FAILED
SERVICE_CONTEXT_NOT_ESTABLISHED
CLI_ACCESS_DENIED
CLI_TIMEOUT_OR_SESSION_FAILURE
CLI_RUNTIME_FAILED
OUTPUT_OR_COUNTER_FAILED
PASS
```

A service-harness failure does not classify AMD behavior. An access-denied
result with valid Session 0 proof establishes a service-context permission
failure, but does not automatically select another account. A timeout does not
justify a retry in the same task.

## SYSTEM MUTATION AND AUTHORIZATION BOUNDARY

The authoritative run required one explicit Administrator operation to create
the manual/demand-start service, start it once, and delete the exact registration.
It must not configure `Automatic` or `AutomaticDelayedStart`, alter AMD files,
registry, PATH, drivers, services, security settings, or reboot the machine.

This parser repair performed none of those operations and did not self-elevate.

## AUTOMATED NON-AMD VALIDATION

The focused Rust tests cover:

- fixed command and controlled run-root policy;
- rejection of arbitrary executable/argv surfaces;
- signed exit-code representation;
- context evidence and qualification JSON serialization;
- service status protocol state mapping;
- LocalSystem/Session 0/x64 context gate.

The PowerShell synthetic test covers:

- synthetic exit 0 and signed `-1` capture;
- empty stderr and raw stdout/stderr persistence;
- timeout and owned cleanup bookkeeping;
- fixed service command shape and service-name safety checks;
- shared package-power parser PASS/FAIL and cadence assessment;
- deterministic 1/2/3-width timestamp parsing and range rejection;
- the authoritative 9-sample cadence fixture and one-time midnight rollover;
- zero/one/multiple pre-existing CLI process-list evidence serialization;
- pre-runtime gate ordering before `New-Service` and `Start-Service`;
- Unicode wrapper-error evidence round-tripping;
- qualification-before-cleanup persistence and exact-file cleanup.

All tests use harmless synthetic processes or fixture data and report
`AMD_RUNTIME_EXECUTED=false`. No service registration is made by the tests.

## REMAINING GATES

The completed I1 qualification does not claim:

- minimum privilege account;
- service SID and production named-pipe ACL;
- standard-user IPC start/query/cancel;
- multi-user arbitration;
- sleep/resume or reboot behavior;
- long-lived/all-day lifecycle;
- update/uninstall transaction;
- temperature or frequency;
- production metric/storage admission.

Long-lived session qualification remains ordered after privilege-context
qualification:

```text
LONG_LIVED_SESSION_ORDERING = AFTER_PRIVILEGE_CONTEXT_QUALIFICATION
```

## NEXT RUNTIME QUALIFICATION

```text
NEXT_RUNTIME_QUALIFICATION = AMD_PRIVILEGE_I2
RESULT = PASS
```

The existing LocalSystem/Session 0 bounded package-power run passed after
offline parser repair. The architecture review may therefore use the Windows
Service Broker as an evidence-supported candidate, while still requiring
least-privilege and IPC qualification. No additional service or AMD CLI was
run during this parser-repair task.
