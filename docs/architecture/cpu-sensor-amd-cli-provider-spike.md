# CPU-SENSOR-AMD CLI PROVIDER SPIKE

This record describes a spike-only implementation. It is not a production
provider admission, a privilege decision, or permission to run an unbounded
AMD collection session.

## STATUS AND BASELINE

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = 62d699111e0bf35f421d0da1de83640fdc753693
AMD_PROVIDER_ARCHITECTURE = CLI_SUBPROCESS
DECISION_CONFIDENCE = MEDIUM
STATUS = BOUNDED_SESSION_TECHNICALLY_QUALIFIED / NOT_PRODUCTION_ADMITTED
AMD_CLI_SPIKE_RUNTIME = PASS_RECOVERED_FROM_RAW_EVIDENCE
CPU_PACKAGE_POWER_W_RUNTIME_QUALIFIED = true
CPU_PACKAGE_POWER_W_PRODUCTION_QUALIFIED = false
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
```

The byte-identical directory counterfactual established that the AMD CXL
runtime accepts a process whose executable directory is
`InstallationPath\bin` (or `InstallationPath\bin\AMDPerf`) and rejects the
same public-API fixture from the repository build directory. The final root
cause record is
[`cpu-sensor-amd-executable-directory-runtime-confirmation.md`](../measurements/cpu-sensor-amd-executable-directory-runtime-confirmation.md).

The hard product constraint is:

```text
DIRECT_IN_PROCESS_AMD_UPROF_API_FROM_ARBITRARY_APP_DIRECTORY = NOT_VIABLE
MAIN_APP_PROCESS = NON_ELEVATED_BY_DEFAULT
```

The single manually authorized Administrator CLI control produced nine
parseable socket package-power records at approximately one-second cadence,
and its target process exited successfully. It did not establish all-day
lifecycle, privilege deployment, output stability across versions, or
production metric semantics.

## EXISTING PROVIDER SEAM

The implementation reuses the existing collector boundary rather than adding
a second provider framework:

| Concern | Existing seam | Spike use |
|---|---|---|
| Provider lifecycle | `MetricProvider` in `src-tauri/src/collector/provider.rs` | Future adapter boundary: probe/start/sample/stop |
| Bounded calls and isolation | `ProviderHost` | Future adapter gets the existing deadline, cancellation, retry, and health behavior |
| User intent and cadence | `CollectionPlan` | Future adapter uses requested category/interval without changing scheduler semantics |
| Capability/status | `ProviderDescriptor`, `ProviderCapabilitySpec`, `ProviderErrorCode` | Discovery and session failures map to existing vocabulary |
| Metric metadata | `ProviderMetricMetadata` | Package-power descriptor is prepared for a later value/storage contract |
| Current production registration | `collector::manager::run_collector` | AMD is intentionally not registered |
| Current sample value DTO | `SystemSample` | No AMD scalar is written because its additive value/writer contract is not part of this spike |

`src-tauri/src/collector/amd_uprof_cli.rs` is exported for focused tests and
future adapter work, but the default manager still registers only the existing
Windows baseline and NVIDIA providers. This keeps an unavailable, privileged,
or malformed AMD source from changing the normal collector.

## SPIKE IMPLEMENTATION

The module contains four small boundaries:

1. `AmdCliDiscovery` reads the installed root through the existing observed
   registry location, derives `bin\AMDuProfCLI.exe`, and records artifact
   metadata without launching it.
2. `AmdCliRunner` launches a supplied executable directly with an argument
   vector and explicit working directory. It captures both streams, records a
   signed and hexadecimal exit code, bounds timeout/cancellation, and cleans up
   the owned target process. On Windows it opportunistically assigns the target
   to a transient job so timeout/cancellation can terminate its owned child
   tree; if the OS refuses job assignment, the result records that limitation
   and still cleans up the target. No `cmd.exe /c`, global environment
   mutation, or process-wide current-directory change is used. The future
   runtime gate also records direct children and orphan state.
3. `AmdCliSession` models `IDLE -> STARTING -> RUNNING -> COMPLETED` and
   failure/cancellation transitions. A session owns one bounded file-producing
   command; it does not launch a process for each Resource Timeline sample.
4. `parse_cli_power_csv` parses the observed section and column structure by
   header, preserves raw values/units/timestamps, and rejects malformed,
   missing, negative, non-finite, duplicate, or locale-decimal values.

The runner and session return data/errors to their caller. They do not panic on
normal vendor failure. The existing `ProviderHost` remains the planned place
for bounded failure isolation and backoff when a production adapter is later
authorized.

## DISCOVERY AND ARTIFACT CONTRACT

The read-only discovery source is:

```text
HKLM\SOFTWARE\WOW6432Node\AMD\AMDProfiler\InstallationPath
<InstallationPath>\bin\AMDuProfCLI.exe
```

Discovery records:

- installation root and derived CLI path;
- SHA-256, file size, x64 PE architecture, file/product version, and
  Authenticode status;
- an explicit optional accepted-major-version policy.

The currently observed artifact is:

```text
CLI = D:\apps\AMDuProf\bin\AMDuProfCLI.exe
SHA256 = D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
VERSION = 5.3.521.0
ARCHITECTURE = x64
SIGNATURE = Valid / AMD artifact in accepted evidence
```

The historical hash is an evidence identity, not a universal version allowlist.
Future compatibility must be explicit. Discovery has separate states for
missing installation, unsupported version/architecture, permission,
driver/service unavailability, counter unavailability, runtime failure,
parse failure, and disabled operation; these map to the repository's existing
provider error codes. Raw vendor stderr remains diagnostic evidence, not a
product status string.

## SUBPROCESS AND SESSION MODEL

The command is constructed as an argument vector equivalent to:

```text
AMDuProfCLI.exe timechart --event power --interval 1000 --duration 10 \
  --format csv --output-dir <Resource-Timeline-owned-session-directory>
```

The spike uses a ten-second bounded default, a one-second requested interval,
and a fifteen-second library default timeout. The manual qualification wrapper
uses a thirty-second watchdog to leave room for startup and artifact flush;
the vendor duration remains ten seconds. The current directory is explicitly
the discovered `InstallationPath\bin` directory. A future production adapter
must not rely on the process-global current directory.

The installed help and accepted short run establish a bounded, file-producing
`timechart` model with duration, interval, CSV format, and output-directory
arguments. They do not establish a streaming protocol. Therefore:

```text
CLI_COLLECTION_MODEL = BOUNDED_SESSION / FILE_RESULT_AFTER_SESSION
STREAMING = UNKNOWN
PER_SAMPLE_PROCESS_LAUNCH = REJECTED_AS_DEFAULT
```

The intended future shape is one owned bounded collection session with several
records, followed by parsing and cleanup. Repeated one-second process launches
are not accepted as an all-day default without evidence that the vendor only
supports that model and that its cost is safe.

## OUTPUT AND PACKAGE-POWER CONTRACT

The parser is fixture-tested against the accepted report shape:

```text
PROFILED COUNTERS
COUNTER ID,NAME,CATEGORY,UNIT,DESCRIPTION
...
PROFILE RECORDS
RecordId,Timestamp,socket0-package-power,...
...
```

It identifies the package-power column by normalized header, resolves its
counter definition and unit, and emits:

```text
metric_key = cpu.package.power_w
category = power
device = cpu:package
vendor = AMD
original_unit = W
value = finite, non-negative f64 watts
timestamp = raw clock time plus parsed milliseconds
timestamp_semantics = CLOCK_TIME_WITHOUT_DATE
```

The accepted Administrator control observed package-power values including
approximately `49.44 W`, `42.26 W`, and `40.29 W`. The report timestamp has no
date in the observed format, so mapping to the Resource Timeline absolute
timeline remains `DEFER`; the raw relative/clock timing is retained. A file
write timestamp is never substituted for a sensor sample timestamp.

Only package power is implemented in this spike:

```text
CPU_PACKAGE_POWER_W = IMPLEMENTED_PARSER_PATH
CPU_PACKAGE_POWER_W_PRODUCTION_QUALIFIED = false
CPU_PACKAGE_TEMPERATURE_C = NOT_ESTABLISHED
CPU_EFFECTIVE_FREQUENCY_MHZ = NOT_ESTABLISHED
```

The CLI capability listing mentioned power, frequency, and P-state surfaces,
but no Resource Timeline temperature/frequency semantics or aggregation policy
has been qualified. No temperature/frequency parser or production metric is
approved here.

## PRIVILEGE AND FAILURE ISOLATION

On this installation, non-administrator power initialization returned
`0x80070005 / AMDT_ERROR_ACCESSDENIED`; the Administrator CLI control succeeded.
This does not authorize permanent elevation of Resource Timeline.

```text
PRIVILEGE_DEPLOYMENT_DECISION = DEFER_INSUFFICIENT_EVIDENCE
AMD_PRIVILEGE_ARCHITECTURE = DEFER_INSUFFICIENT_EVIDENCE
DECISION_CONFIDENCE = MEDIUM
ADMIN_CONSENT_MODEL = ONE_TIME_INSTALL_OR_ENABLE
```

The spike represents privilege as a capability/status boundary:

- the main application remains non-elevated by default;
- missing privilege maps to permission/unavailable state;
- no UAC request, service creation, registry edit, DLL copy, or vendor-tree
  mutation is performed;
- the baseline collector and other providers continue if AMD is missing,
  denied, unsupported, timed out, crashes, or produces invalid output;
- retry must be bounded/backed off through the existing `ProviderHost`, never a
  rapid crash/restart loop;
- a provider failure never becomes a synthetic zero sample.

The future provider must also decide whether a legitimate elevated helper or
user-approved session exists. A per-session elevation prompt is potentially
unsuitable for unattended collection; a privileged service has additional
installation and security cost. Both remain product decisions, not hidden
implementation details. The dedicated privilege-deployment record evaluates
service broker, Scheduled Task, per-session UAC, and elevated-main options. It
does not select one because Session 0/service-account behavior and
standard-user task-control ACLs remain unqualified:
[`cpu-sensor-amd-privilege-deployment.md`](cpu-sensor-amd-privilege-deployment.md).

## CONFIGURATION AND REGISTRATION BOUNDARY

Only internal spike configuration is represented:

```text
enabled
cli_path / auto-detect
duration
sample_interval
timeout
```

No user-facing settings or feature-default change was made. AMD remains
unregistered in `collector::manager`, so the current application does not
attempt discovery or profiling during ordinary startup. The prepared metadata
and descriptor can be adapted to `MetricProvider` after the value/storage
contract and runtime gates are approved.

## AUTOMATED QUALIFICATION

The focused Rust tests cover:

- registry-root and CLI path discovery through fakes, absent CLI, malformed
  relative root, architecture/signature/version metadata, and all status/error
  mappings;
- direct argument-vector construction, explicit working directory, empty
  arguments, paths containing spaces, nonzero/negative exits, empty stderr,
  timeout, cancellation, output capture, and owned-process cleanup;
- valid package-power series, header reordering, missing counter/unit/value,
  malformed/truncated rows, negative/non-finite/locale values, duplicate
  timestamps, and empty output;
- session success, `IDLE/STARTING/RUNNING/COMPLETED` transitions, permission
  mapping, timeout/cancellation, output-read failure, parser failure, and
  non-unwinding error handling;
- package-power metadata and the existing ProviderHost failure-isolation seam.

The tests use harmless synthetic processes and fixture strings. They do not
load AMD DLLs or start AMD executables. The focused command is:

```text
cargo test --manifest-path src-tauri/Cargo.toml --lib collector::amd_uprof_cli
```

Existing `collector::provider` tests remain the regression evidence for
healthy-provider continuity when another provider fails. No production
collector test was changed to register AMD.

## RECOVERED BOUNDED RUNTIME QUALIFICATION

The first and only real runtime session was completed once by the user on
2026-09-02. The wrapper persisted the target process result and raw streams,
then failed during post-runtime CSV-path processing. The target had already
completed successfully, so the qualification was recovered offline from the
authoritative raw evidence rather than rerun. See
[`cpu-sensor-amd-cli-spike-runtime.md`](../measurements/cpu-sensor-amd-cli-spike-runtime.md).

```text
REAL_RUNTIME_COUNT = 1
RERUN_PERFORMED = false
RERUN_REQUIRED = false
ORIGINAL_WRAPPER_RESULT = BLOCKED_POST_RUNTIME_HARNESS
HARNESS_ROOT_CAUSE = POWERSHELL_IF_USED_AS_COMMAND_ARGUMENT_EXPRESSION
AMD_CLI_SPIKE_RUNTIME = PASS_RECOVERED_FROM_RAW_EVIDENCE
PACKAGE_POWER_RUNTIME_PARSE = PASS
AMD_CLI_PROVIDER_SPIKE = TECHNICALLY_QUALIFIED_FOR_BOUNDED_SESSION
```

The recovered session used one manual Administrator x64 PowerShell context,
without CDB/debugger, with:

- discovered CLI identity/version/signature and exact arguments;
- same `InstallationPath\bin` working directory and unchanged inherited
  environment;
- ten-second power session at one-second interval;
- raw stdout/stderr and exit status;
- CSV/UPROF output paths and hashes;
- parsed sample count, raw package-power values, and raw timestamp/index data;
- process CPU time, peak working set, process/child count, output I/O, clean
  shutdown, and orphan-process check where safely available;
- explicit result when privilege, driver/service, counter, parser, or timeout
  fails.

The wrapper derives the installation root from the observed registry value,
refuses output under the AMD tree, checks for an already-running exact CLI,
captures a single bounded package-power session, and leaves its Resource
Timeline-owned evidence directory for analysis. It does not modify persistent
PATH/environment/registry state and does not self-elevate.

```text
CPU_PACKAGE_POWER_W_RUNTIME_QUALIFIED = true
CPU_PACKAGE_POWER_W_PRODUCTION_QUALIFIED = false
PRODUCTION_ADMISSION = DEFERRED
```

The repaired wrapper remains available for a separately authorized future
qualification, but this task performed no second AMD run. The recovered run
reported nine samples (`49.69`–`58.04 W`, arithmetic mean `54.503333 W`),
1000.375 ms mean cadence, 93.75 ms target CPU time, and a 43,040,768-byte
peak working set. These are bounded-run measurements, not all-day budgets.

## OVERHEAD AND PRODUCTION-ADMISSION BLOCKERS

No AMD-specific numeric overhead threshold is currently defined in the
repository. The next qualification must record a baseline and establish
provisional gates for:

1. startup and capability latency;
2. steady and peak CPU/working-set cost;
3. actual sample cadence and wakeups;
4. process/handle/thread growth and child lifetime;
5. disk I/O and temporary output growth;
6. UI/input responsiveness;
7. crash, hang, timeout, cancellation, restart, and disable/re-enable;
8. driver/service and sleep/resume behavior;
9. privilege loss and missing/unsupported counter transitions.

At minimum, the spike should show no visible input stutter, no runaway CPU or
memory growth, no orphaned owned process, cadence close to the requested
interval, and bounded clean shutdown. These are provisional spike gates, not
permanent product thresholds.

Production admission remains blocked until all of the following are resolved:

- legitimate privilege deployment without silently elevating the main app;
- stable supported session/output semantics and timestamp mapping;
- package-power scope/quality and additive storage/DTO contract;
- failure isolation and bounded recovery under real runtime behavior;
- all-day overhead and responsiveness evidence;
- installation/version/driver/service support policy;
- legal, licensing, redistribution, and security review for the external AMD
  dependency.

The vendor service observed in prior startup evidence is not a reusable public
telemetry interface. No private IPC/authentication surface is used.

## NEXT TASK

The next qualification family, after the privilege deployment model is chosen,
is:

```text
AMD_CLI_PRIVILEGE_CONTEXT_QUALIFICATION
```

The deployment architecture is documented but deliberately deferred: no
service or task has been implemented, and no account or cross-integrity IPC
contract has been qualified. The context qualification must select one exact
proposed principal, use a fixed semantic power request, and prove bounded
result delivery, cancellation, cleanup, and failure isolation. It must not
install a production privileged component or use per-sample UAC. Long-lived
session behavior, restart/recovery, timestamp mapping, temperature/frequency,
provider registration, user-facing settings, and production admission remain
deferred.

## PRIVILEGE-CONTEXT QUALIFICATION PREPARATION

The next single qualification is now prepared as
`AMD_CLI_SERVICE_CONTEXT_QUALIFICATION`. It uses a separate
qualification-only genuine SCM service harness under
[`tools/amd-cli-service-context-qualification`](../../tools/amd-cli-service-context-qualification/README.md)
and the first feasibility account is LocalSystem. The harness derives the
registry-installed `AMDuProfCLI.exe`, launches exactly one fixed ten-second
package-power session in `InstallationPath\\bin`, and persists Session 0,
token, service-status, raw-process, and output evidence under a protected
ProgramData run root. It uses the existing PowerShell package-power parser
after the target exits.

```text
RESULT = ADMIN_SERVICE_CONTEXT_QUALIFICATION_REQUIRED
SERVICE_BROKER_CANDIDATE = LEADING_PENDING_RUNTIME_QUALIFICATION
SERVICE_SESSION0_AMD_CLI_QUALIFIED = false
AMD_PRIVILEGE_ARCHITECTURE = DEFER_INSUFFICIENT_EVIDENCE
LONG_LIVED_SESSION_ORDERING = AFTER_PRIVILEGE_CONTEXT_QUALIFICATION
```

No service registration, AMD CLI launch, profiling, IPC, installer change, or
production provider registration was performed while preparing this harness.
