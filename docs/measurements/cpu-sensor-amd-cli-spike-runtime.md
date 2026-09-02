# CPU-SENSOR-AMD CLI SPIKE RUNTIME EVIDENCE

This record closes the single authorized bounded AMDuProfCLI runtime session
without rerunning it. The target process completed successfully; the original
wrapper failed afterward while resolving the discovered CSV path. The result
below is recovered from the preserved raw process result, streams, and output
artifacts.

## STATUS

```text
RESULT = PASS_RECOVERED_FROM_RAW_EVIDENCE
AMD_CLI_TARGET_RUNTIME = PASS
AMD_CLI_PROCESS_CAPTURE = PASS
ORIGINAL_WRAPPER_RESULT = BLOCKED_POST_RUNTIME_HARNESS
AMD_RUNTIME_RETRY_REQUIRED = false
REAL_RUNTIME_COUNT = 1
RERUN_PERFORMED = false
RERUN_REQUIRED = false
AMD_CLI_SPIKE_RUNTIME = PASS_RECOVERED_FROM_RAW_EVIDENCE
PACKAGE_POWER_RUNTIME_PARSE = PASS
CPU_PACKAGE_POWER_W_RUNTIME_QUALIFIED = true
AMD_CLI_PROVIDER_SPIKE = TECHNICALLY_QUALIFIED_FOR_BOUNDED_SESSION
CPU_PACKAGE_POWER_W_PRODUCTION_QUALIFIED = false
AMD_PROVIDER_PRODUCTION_ADMITTED = false
```

## EVIDENCE AND CONTEXT

Authoritative evidence root:

```text
C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-cli-spike-20260902T093613542Z
```

The manually supplied Administrator proof records a 64-bit PowerShell,
Administrator membership, High integrity (`S-1-16-12288`), and the unchanged
working directory `D:\apps\AMDuProf\bin`. The pre-existing exact-process gate
was empty before launch. No second AMD runtime was performed during evidence
consumption or harness repair.

The exact target and argument vector were:

```text
CLI = D:\apps\AMDuProf\bin\AMDuProfCLI.exe
SHA256 = D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC
VERSION = 5.3.521.0
SIGNATURE = Valid / AMD signer
WORKING_DIRECTORY = D:\apps\AMDuProf\bin
ARGUMENTS = timechart --event power --interval 1000 --duration 10 --format csv --output-dir <evidence>\timechart-output
```

## INCIDENT CLASSIFICATION AND RECOVERY

`AMD-CLI-PROCESS-RESULT.json` proves that process creation, waiting, stream
capture, and target-result persistence completed before the wrapper exception:

| Field | Recorded value |
|---|---|
| `process_started` | `true` |
| `target_pid` | `35484` |
| `started_at_utc` | `2026-09-02T09:36:14.1841237Z` |
| `finished_at_utc` | `2026-09-02T09:36:25.6033220Z` |
| `duration_ms` | `11403.504` |
| `timeout` | `false` |
| `target_exit_signed` | `0` |
| `target_exit_hex` | `0x00000000` |
| `target_process_failed` | `false` |
| `capture_complete` | `true` |
| `stdout_bytes` / `stderr_bytes` | `723` / `0` |
| `stdout_persisted` / `stderr_persisted` | `true` / `true` |
| `harness_error` in process result | `null` |

The original post-runtime expression passed `if` as though it were an
argument-producing expression:

```powershell
Parse-PackagePowerCsv -Path (if (...) { ... } else { ... })
```

PowerShell treated `if` as a command in that position. Consequently the
target had already exited and the raw process capture was available, but the
post-runtime summary was not written. The wrapper incident is therefore:

```text
HARNESS_ROOT_CAUSE = POWERSHELL_IF_USED_AS_COMMAND_ARGUMENT_EXPRESSION
ORIGINAL_WRAPPER_RESULT = BLOCKED_POST_RUNTIME_HARNESS
```

The repaired path resolves `$csvPath` in a preceding assignment, persists the
raw process result before post-processing, and keeps target failure separate
from harness failure. The same path is now exercised by the non-AMD synthetic
regression described below.

## CLI-REPORTED SESSION

The preserved CLI stdout reports `Profile finished` and a final progress line
of `Profile Elapse Time in ms: 9094`. The output also identifies the generated
timechart directory and CSV. The initial progress line reporting zero is not
used as the elapsed result; the final recorded progress value is:

```text
CLI_REPORTED_PROFILE_ELAPSED_MS = 9094
CLI_REPORTED_PROFILE_FINISHED = true
```

These human-readable messages are evidence for this run only. They are not a
future production parser contract because they may be localized or change
across CLI versions.

## OUTPUT ARTIFACTS

The independently inventoried output directory is:

```text
C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-cli-spike-20260902T093613542Z\timechart-output\AMDuProf-SWP-Timechart_Sep-02-2026_17-36-15
```

| Artifact | Size | SHA-256 |
|---|---:|---|
| `timechart.csv` | 1,932 bytes | `42D2BA48C6C36E7109B829A224F6BC37B8A58E42369E82A355E12CE69628933B` |
| `session.uprof` | 10,884 bytes | `995E99F7F7B5B30B3965E20B21AEE79BE152DC051DD57C4EDC841FB5C2509BFE` |

```text
TIMECHART_CSV_PRESENT = true
SESSION_UPROF_PRESENT = true
SESSION_OUTPUT_TOTAL_BYTES = 12816
```

The total is the exact two-file inventory for this bounded session. It is not
an all-day disk-usage estimate.

## PACKAGE POWER PARSE

The repaired PowerShell parser independently read the preserved
`timechart.csv`. It located the `socket0-package-power` column and its `W`
unit, and returned nine finite, non-negative samples:

```text
58.04, 57.17, 52.40, 57.71, 57.78, 55.72, 51.65, 49.69, 50.37 W
```

```text
PACKAGE_POWER_PARSE_STATUS = PASS
PACKAGE_POWER_SAMPLE_COUNT = 9
PACKAGE_POWER_MIN_W = 49.69
PACKAGE_POWER_MAX_W = 58.04
PACKAGE_POWER_MEAN_W = 54.50333333333334
```

The Rust spike parser and PowerShell qualification parser use the same small
committed representative CSV fixture for their parser tests and agree on
sample count, values, and `W` unit. The real-session values above were read
from the preserved vendor CSV, not hard-coded into the parser.

## SAMPLE CADENCE AND COUNT

The raw timestamp values are:

```text
17:36:16:366
17:36:17:369
17:36:18:369
17:36:19:369
17:36:20:369
17:36:21:369
17:36:22:369
17:36:23:369
17:36:24:369
```

The eight consecutive deltas are:

```text
1003, 1000, 1000, 1000, 1000, 1000, 1000, 1000 ms
```

```text
CADENCE_DELTA_COUNT = 8
CADENCE_MIN_MS = 1000
CADENCE_MAX_MS = 1003
CADENCE_MEAN_MS = 1000.375
SAMPLE_CADENCE = PASS
```

The request was ten seconds at a 1000 ms interval, while the CLI reported
9094 ms of profile elapsed time. Nine samples spanning 8003 ms is therefore
recorded as:

```text
SAMPLE_COUNT_FOR_10S_REQUEST = 9
SAMPLE_COUNT_INTERPRETATION = CONSISTENT_WITH_VENDOR_SESSION_BOUNDARIES
```

No exact first/last-sample algorithm is inferred, and production behavior
must not require `sample_count == duration / interval`.

## TIMESTAMP SEMANTICS

The CSV provides raw clock timestamps with millisecond precision. The report
metadata provides `Profile Start Time: Sep-02-2026_17-36-15`; the observed
format does not independently establish a timezone or a complete absolute
timestamp contract for Resource Timeline storage.

```text
RAW_TIMESTAMP_AVAILABLE = true
PRODUCTION_TIMESTAMP_MAPPING = DEFER
```

The raw timestamp and relative cadence remain available to a future adapter.
File-write time is not substituted for the sensor sample time.

## PROCESS OVERHEAD AND LIFECYCLE

The captured target-process measurements were:

| Measurement | Value |
|---|---:|
| target CPU time | 93.75 ms |
| target wall duration | 11,403.504 ms |
| average one-core CPU equivalent | 0.8221157% |
| peak working set | 43,040,768 bytes |
| peak working set | 41.046875 MiB (approximately 41.05 MiB) |

The one-core equivalent is only `target_cpu_time / target_wall_duration *
100`; it is not whole-machine CPU utilization. The process result observed a
direct `conhost.exe` child (PID `38740`) during the run. A subsequent exact
query for remaining `AMDuProfCLI.exe` processes returned none:

```text
AMDUProfCLI_ORPHAN_AFTER_RUN = NOT_OBSERVED
```

The final state of the observed `conhost.exe` child was not separately
established. Kernel/driver cost, child-process CPU cost, complete disk I/O,
and Resource Timeline UI responsiveness were not measured. Accordingly:

```text
CLI_PROCESS_OVERHEAD = LOW_IN_THIS_BOUNDED_RUN
ALL_DAY_OVERHEAD_QUALIFIED = false
```

The low-overhead label is limited to the captured CLI process in this one
bounded run and is not an all-day admission.

## OFFLINE QUALIFICATION SCOPE

The preserved target result, output artifacts, parser result, and cadence
support:

```text
AMD_CLI_SPIKE_RUNTIME = PASS_RECOVERED_FROM_RAW_EVIDENCE
PACKAGE_POWER_RUNTIME_PARSE = PASS
CPU_PACKAGE_POWER_W_RUNTIME_QUALIFIED = true
AMD_CLI_PROVIDER_SPIKE = TECHNICALLY_QUALIFIED_FOR_BOUNDED_SESSION
AMD_PROVIDER_PRODUCTION_ADMITTED = false
```

This means only that one manually elevated, bounded ten-second
`AMDuProfCLI.exe` power session completed, generated parseable package-power
data at approximately one-second cadence, and had bounded captured
CLI-process CPU/memory measurements. It does not qualify unattended privilege
deployment, a long-lived session, all-day recovery, version compatibility,
timestamp/storage semantics, temperature, frequency, total driver/kernel
overhead, input responsiveness, licensing, or distribution.

Temperature and frequency remain deferred. The privilege deployment decision
also remains:

```text
PRIVILEGE_DEPLOYMENT_DECISION = DEFER_PENDING_RUNTIME_AND_PRODUCT_DECISION
```

## HARNESS REPAIR AND REGRESSION

The repair is limited to the spike tooling:

- `postprocess.ps1` owns CSV/UPROF discovery, parsing, inventory, and
  post-runtime classification.
- The wrapper persists `AMD-CLI-PROCESS-RESULT.json` before post-processing.
- CSV path resolution uses an explicit prior assignment, not `if` inside a
  command argument expression.
- `test-fixtures/package-power.csv` is shared by the PowerShell regression and
  Rust parser tests, preventing two incompatible representative contracts.
- `test-post-runtime.ps1` exercises synthetic CSV/UPROF/process-result inputs
  for successful output, missing CSV, missing UPROF, parser failure, target
  failure, timeout, harness failure, and summary persistence.

The non-AMD regression passed:

```text
CSV_PATH_RESOLUTION = PASS
PARSER_INVOKED = PASS
SUMMARY_WRITTEN = PASS
FULL_POST_RUNTIME_SYNTHETIC_PATH = PASS
AMD_RUNTIME_EXECUTED = false
```

The historical first runtime remains preserved as a single run; no retry was
performed to obtain this closure.

## REMAINING PRODUCTION BLOCKERS

- legitimate unattended privilege deployment while the main application
  remains non-elevated by default;
- supported long-lived/session lifecycle and bounded restart/recovery;
- stable output and absolute timestamp mapping;
- version, installation, driver/service, and counter compatibility policy;
- package-power storage/DTO admission in the production catalog;
- all-day CPU/memory/I/O and UI responsiveness qualification;
- temperature/frequency semantics and aggregation qualification;
- legal, licensing, redistribution, and security review.

The next task is intentionally not another runtime retry:

```text
NEXT_TASK = AMD_CLI_PRIVILEGE_DEPLOYMENT_ARCHITECTURE
```

It should select a legitimate deployment model for the known Administrator
boundary before production registration or all-day qualification is
considered.
