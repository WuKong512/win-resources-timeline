# CPU-SENSOR-AMD EXISTING VENDOR EXECUTABLE NO-OP STARTUP CONTROL

This record defines the next native startup control for the AMD uProf source
investigation. It is deliberately narrower than profiling qualification: it
tests only whether the existing AMD-signed GUI executable remains alive during
a bounded startup window. It does not explain the vendor behavior and does
not authorize `CPU-SENSOR-AMD-PROVIDER-DESIGN`.

```text
RESULT = ADMIN_VENDOR_STARTUP_CONTROL_REQUIRED
RUNTIME_EXECUTION = NOT_PERFORMED
PROFILING = NOT_PERFORMED
```

## BASELINE

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = 404b0fd16571eefef5f14c4beb605daeb0492203
ORIGIN_MAIN_AT_ENTRY = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
WORKING_TREE_AT_ENTRY = CLEAN
ENTRY_GATE = PASS
DUPLICATE_TASK_GATE = PASS
GH_PR_VISIBILITY = UNAVAILABLE_GH_CLI
```

The intended branch already existed locally and remotely and was continued.
No reset, rebase, merge, cherry-pick, amend, or force-push was performed.
The branch was twelve commits ahead of the recorded `origin/main`; that drift
was recorded and not reconciled silently.

## HYPOTHESIS

Does the existing `D:\apps\AMDuProf\bin\AMDuProf.exe`, which directly
process-start imports `CXLBaseTools.dll`, survive native startup in the same
Administrator/AMD-bin context where the failed minimal static fixture exits
`-1 / 0xFFFFFFFF` after approximately 45.6 ms?

This control measures process survival only. It does not infer a cause from
survival and it does not call the AMD public API, start profiling, read
counters, or interact with the GUI.

## TARGET ARTIFACT AND PREFLIGHT

The wrapper targets exactly:

```text
PATH = D:\apps\AMDuProf\bin\AMDuProf.exe
EXPECTED_SHA256 = 8F1195F9900C0FF6A5E18AA33127C83DF74622CA7BB68C0C532C3776F6FFD762
EXPECTED_ARCHITECTURE = x64 / PE machine 0x8664
EXPECTED_SIGNATURE = Authenticode Valid
EXPECTED_SIGNER = Advanced Micro Devices
SUBSYSTEM = Windows GUI
DIRECT_IMPORT(CXLBaseTools.dll) = true
```

The read-only preflight performed during preparation observed the expected
size `427,416` bytes, the expected SHA-256, `Valid` Authenticode status, and
subject `CN=Advanced Micro Devices, O=Advanced Micro Devices, S=California,
C=US`. The manual wrapper repeats the hash, x64 PE-machine, signature, and
signer checks immediately before launch. Any failed check writes evidence and
does not launch the target.

The failed M1 comparison remains the previously accepted repository artifact:

```text
M1 = tools/amd-uprof-public-api-ab/target/release/amd-uprof-static-api-load-fixture.exe
M1_SHA256 = 9FAC63BD6B1FF1888DFFC8736F4152B972164DDAE8E1369584A53C1705354F53
M1_RESULT = exit -1 / 0xFFFFFFFF, capture complete
```

M1 is not rerun by this control.

## STARTUP CONTROL DESIGN

The manual-only wrapper is
[`run-admin-vendor-noop-startup.ps1`](../../tools/amd-uprof-public-api-ab/run-admin-vendor-noop-startup.ps1).
It performs this bounded sequence:

1. record a read-only Administrator proof (`whoami.exe /groups`,
   `WindowsPrincipal`, x64 PowerShell, current directory);
2. preflight the exact target identity;
3. query for any pre-existing `AMDuProf.exe` process;
4. launch exactly one target instance with `Arguments = @()` and working
   directory `D:\apps\AMDuProf\bin`;
5. wait for `3,000` ms without a debugger, profiling command, or watchdog kill;
6. snapshot only direct children of the launched PID;
7. persist the startup classification before cleanup;
8. attempt `CloseMainWindow()` on the exact launched process only;
9. persist cleanup and final raw evidence.

The wrapper uses `UseShellExecute = false`, an explicit absolute executable
path, and an explicit working directory. It does not mutate PATH, registry,
services, drivers, Hyper-V/VBS/HVCI, or installation state. It never kills by
process name. The optional exact-PID force cleanup exists only inside the
non-AMD synthetic regression and is not enabled by the Administrator command.

## OBSERVATION WINDOW AND CLASSIFICATION

```text
OBSERVATION_WINDOW_MS = 3000
```

The primary pass condition is `process_started = true` and
`root_alive_at_deadline = true`. A direct child that is still alive at the
deadline is recorded as a possible `DELEGATED_CHILD_PROCESS`; child evidence
is secondary and is not treated as proof without a successful shallow process
query. If neither root nor an observable direct child survives, the raw result
is `FAIL` when the process query is complete, or `INCONCLUSIVE` when process
identity/delegation inspection is unavailable.

An exit code of zero is not required for the GUI control. If the root exits
early, the wrapper records signed and hexadecimal exit values, elapsed time,
and available streams. The qualification decision is independent of cleanup
behavior.

## PRE-EXISTING PROCESS GATE

Before launch, the wrapper performs a read-only `Win32_Process` query for
`AMDuProf.exe`. Any existing row produces:

```text
RESULT = BLOCKED_PREEXISTING_VENDOR_PROCESS
```

with PID, parent PID, image path, command line, and creation metadata where
available. The wrapper does not kill or launch another instance. A failure to
perform the query is conservatively recorded as a harness block.

## DELEGATED-CHILD HANDLING

The wrapper takes one shallow direct-child snapshot using the launched PID as
the parent filter. It records child PID, parent PID, name, path, command line,
creation metadata, and alive-at-observation status. It does not recursively
walk the system and does not terminate children. A root that exits while an
attributable direct GUI child survives is classified as delegated startup,
not as an automatic startup failure.

## CLEANUP SEPARATION

The `*.qualification-before-cleanup.json` file is written before any close
request and contains the survival decision. Only after that write does the
wrapper attempt `CloseMainWindow()` against the exact launched `Process`
object. Cleanup fields are written separately into the final result, including
whether graceful close was attempted/returned/succeeded and whether a force
cleanup was requested. The real Administrator command never requests force
cleanup; if graceful close fails, the result records the exact cleanup state
without killing an unrelated process.

The process object is disposed in a `finally` block. No cleanup exit status is
used as startup qualification evidence.

## NON-AMD SYNTHETIC VALIDATION

The wrapper's `-SyntheticTest` mode was run locally against only
`cmd.exe`, `whoami.exe`, and the current PowerShell host. It does not resolve
or load an AMD executable or DLL. The final validation evidence root was:

```text
C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-vendor-noop-synthetic-final-20260901T063942273Z
```

Observed result:

```text
SYNTHETIC_REGRESSION = PASS
AMD_RUNTIME_EXECUTED = false
negative early exit = signed -1 / 0xFFFFFFFF
negative stdout/stderr = persisted; capture_complete = true
zero exit = 0 / 0x00000000; stderr length = 0; capture_complete = true
survivor = alive at 250 ms; qualification persisted before cleanup = true
survivor cleanup = graceful attempt recorded; exact-PID force cleanup recorded for synthetic only
empty argument array = argument_count 0; capture_complete = true
```

The direct-child fixture also exercised the read-only parent query. The
non-admin validation host returned `Access Denied` for `Win32_Process`; the
wrapper recorded `INCONCLUSIVE_NONADMIN_QUERY_ACCESS` with no guessed child
relationship. The manually elevated Administrator run will repeat the same
query in its authorized context. This limitation did not affect the signed
exit, stream persistence, cleanup, or argument-array checks.

PowerShell parser validation passed before and during the synthetic run. The
synthetic survivor never used a timeout watchdog; its exact-PID force cleanup
was an explicit test-only option, not the production/vendor command path.

## AUTHORIZATION BOUNDARY

Codex did not launch `AMDuProf.exe`, `AMDuProfCLI.exe`, AMD samples, or any
profiling command. No Administrator elevation was requested or performed by
Codex. The next runtime step requires the user to manually open an elevated
x64 PowerShell. No system or AMD state change is authorized.

## ADMINISTRATOR COMMANDS READY

Run this once from a manually launched **Administrator x64 PowerShell**. The
parent working directory and target working directory are both the AMD `bin`
directory. Do not add arguments, PATH changes, profiling options, or cleanup
commands. The wrapper performs one launch only and prints its evidence root.

```powershell
$repoRoot = '<LOCAL_CHECKOUT_ROOT>'
Set-Location 'D:\apps\AMDuProf\bin'
& (Join-Path $repoRoot 'tools\amd-uprof-public-api-ab\run-admin-vendor-noop-startup.ps1') `
    -InstallRoot 'D:\apps\AMDuProf' `
    -OutputRoot (Join-Path $env:TEMP ('resource-timeline-amd-vendor-noop-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ')))
```

The command does not auto-elevate. If `AMDuProf.exe` is already running, it
records the existing process and stops without launching another one. Do not
rerun solely to repair formatting; preserve the first runtime evidence root.

## CURRENT DELIVERY STATE

```text
VENDOR_STARTUP_CONTROL = NOT_RUN
ADMIN_VENDOR_STARTUP_CONTROL = REQUIRED
PROFILING = false
SAMPLING = false
SYSTEM_MUTATIONS = false
PRODUCTION_PROVIDER = unchanged
SCHEMA = unchanged
METRIC_CATALOG = unchanged
UI = unchanged
```

## NEXT STEP

After the user supplies the one runtime evidence root, consume its raw JSON
and classify only the vendor startup survival result against M1. Do not
automatically run a second vendor executable, B1, profiling, sampling,
debugger tracing, or provider design.
