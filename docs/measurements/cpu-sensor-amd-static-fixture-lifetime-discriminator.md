# CPU-SENSOR-AMD STATIC FIXTURE LIFETIME / SHUTDOWN DISCRIMINATOR

This record prepares a new, separately built diagnostic fixture to distinguish
the two unresolved interpretations of the original minimal static fixture
failure. It does not rerun the original fixture, run B1, start profiling, or
begin `CPU-SENSOR-AMD-PROVIDER-DESIGN`.

## BASELINE

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = 5f83337c2a5253e00571962cc71642df49202e80
ORIGIN_MAIN = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
WORKING_TREE_AT_ENTRY = CLEAN
PREVIOUS_VENDOR_STARTUP_RESULT = PASS
PREVIOUS_VENDOR_SURVIVAL_DIVERGENCE = CONFIRMED
```

The accepted vendor no-op evidence shows that the signed AMD
`AMDuProf.exe` remains alive after a 3,000 ms native startup observation in
`D:\apps\AMDuProf\bin`. The preserved M1 evidence shows the original
minimal static fixture exiting after approximately 45.6 ms with signed `-1`
(`0xFFFFFFFF`), zero-byte stdout, and complete wrapper capture. The vendor
startup result is valid, but its causality is not established.

## VENDOR STARTUP CLOSURE

The vendor result is preserved in:

```text
EVIDENCE_ROOT = C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-vendor-noop-20260901T064515209Z
QUALIFICATION_SNAPSHOT = VENDOR-NOOP-STARTUP.qualification-before-cleanup.json
```

The snapshot recorded `process_started = true`, PID `41460`, no arguments,
the AMD `bin` working directory, `root_alive_at_deadline = true`,
`observation_elapsed_ms = 3048.247`, `timeout = false`, and no harness or
target failure. The target preflight matched the x64 AMD-signed
`AMDuProf.exe` SHA-256
`8F1195F9900C0FF6A5E18AA33127C83DF74622CA7BB68C0C532C3776F6FFD762`.

The shallow child snapshot observed `AMDProfilerService.exe` PID `44488`
with `--bypass-auth --cleanup --parent-pid 41460`; the child was alive at the
observation deadline. `AMDuProfHello.log` records application/runtime
bootstrap milestones and REST-client connection. These observations support:

```text
VENDOR_STARTUP_CONTROL = PASS
VENDOR_EXECUTABLE_SURVIVAL_DIVERGENCE = CONFIRMED
VENDOR_EXECUTABLE_SPECIFIC_CONTEXT = RUNTIME_SUPPORTED
VENDOR_APPLICATION_BOOTSTRAP_REACHED = CONFIRMED
VENDOR_SERVICE_BOOTSTRAP_OBSERVED = CONFIRMED
EXACT_VENDOR_EXECUTABLE_REQUIREMENT = UNPROVEN
STARTUP_CONTEXT_CAUSALITY = UNPROVEN
```

Qualification was persisted before cleanup. Graceful close was attempted but
did not succeed; force cleanup was not attempted, and the target was still
alive when cleanup bookkeeping completed. This is recorded separately as
`CLEANUP_RESULT = VENDOR_PROCESS_REMAINED_ALIVE`; no later process state is
inferred.

## UNRESOLVED FAILURE STAGE

The original M1 source uses a retained pointer to the imported
`AMDTPwrProfileInitialize` symbol, then buffered Rust `println!` output, and
returns immediately. Its zero-byte stdout therefore cannot distinguish:

```text
MODEL S: dependency initialization fails before main
MODEL D: main returns, buffered output is lost, and shutdown/detach fails
```

The accepted prior classification remains:

```text
MAIN_MARKER_RELIABILITY = BUFFERING_AMBIGUITY
STATIC_FAILURE_STAGE = PRE_MAIN_OR_PROCESS_SHUTDOWN
```

The new fixture is designed only to resolve this stage boundary.

## NEW FIXTURE

```text
NAME = amd-uprof-static-api-hold-fixture.exe
PATH = tools/amd-uprof-public-api-ab/target/release/amd-uprof-static-api-hold-fixture.exe
SIZE = 120320 bytes
SHA256 = B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676
ARCHITECTURE = x64 / PE machine 0x8664
SUBSYSTEM = Windows CUI
AUTHENTICODE = NotSigned (repository diagnostic artifact; not a vendor release artifact)
HOLD_WINDOW_MS = 3000
```

The original M1 binary remains untouched and its SHA remains
`9FAC63BD6B1FF1888DFFC8736F4152B972164DDAE8E1369584A53C1705354F53`.
The new fixture was built by targeting only the new Cargo binary; the M1
hash was checked before and after and remained unchanged.

## IMPORT QUALIFICATION

The fixture uses the installed official import library only:

```text
IMPORT_LIBRARY = D:\apps\AMDuProf\bin\AMDPowerProfileAPI.lib
IMPORT_LIBRARY_SHA256 = BF7CDA16612FC3F2B59443154ACB880363A2D5EABD5BFCAE4CF90F4C34371602
```

The final PE import table was inspected with the installed Microsoft
`dumpbin.exe` (`14.44.35228.0`) and recorded:

```text
DIRECT_IMPORT_AMDPOWERPROFILEAPI = true
DIRECT_IMPORT_CXLBASETOOLS = false
```

The accepted installed dependency graph remains:

```text
amd-uprof-static-api-hold-fixture.exe
  -> AMDPowerProfileAPI.dll
    -> AMDSysUtils.dll
      -> CXLBaseTools.dll
```

No CXL import library, generated `.def`, import stub, copied vendor DLL, or
binary rewrite was used.

## MARKER IMPLEMENTATION

The source is
`tools/amd-uprof-public-api-ab/src/bin/amd-uprof-static-api-hold-fixture.rs`.
Its first practical operation in `main` reads the retained import anchor with
`read_volatile` and passes it to `black_box`; this reads an address and does
not call an AMD export. It then writes
`HOLD_FIXTURE_MAIN_REACHED=true` synchronously through Win32 `WriteFile` to
the inherited stdout handle and checks both the Boolean result and byte count.
If stdout is invalid, the same marker and a diagnostic are attempted through
the inherited stderr handle using the same synchronous path.

The fixture then calls only Win32 `Sleep(3000)`. Before normal return it
synchronously writes and checks `HOLD_FIXTURE_BEFORE_RETURN=true`. It does not
call `AMDTPwrProfileInitialize`, any other AMD API, `StartProfiling`, counter
read, `FreeLibrary`, or `ExitProcess`. It returns from `main` normally and
allows ordinary process shutdown.

```text
AMD_API_CALL_FROM_MAIN = false
DURABLE_MAIN_MARKER = true (checked WriteFile)
DURABLE_BEFORE_RETURN_MARKER = true (checked WriteFile)
NORMAL_RETURN = true (no explicit process termination)
```

## CLASSIFICATION CONTRACT

The future one-run Administrator result will be classified from raw stdout,
stderr, signed/hex exit status, timeout, and complete capture:

```text
MAIN marker absent, early 0xFFFFFFFF:
  STARTUP_FAILURE_SUPPORTED

MAIN marker present, BEFORE_RETURN absent, early abnormal exit:
  FAILURE_DURING_STABLE_MAIN_WINDOW

Both markers present, approximately 3000 ms hold, then 0xFFFFFFFF:
  PROCESS_STARTUP_SURVIVES = CONFIRMED
  SHUTDOWN_OR_DETACH_FAILURE = STRONGLY_SUPPORTED

Both markers present, approximately 3000 ms hold, then exit 0:
  HOLD_FIXTURE_NORMAL = PASS
```

No outcome alone proves the private CXL branch or makes the AMD source
production-ready.

## CAPTURE WRAPPER

The new minimal wrapper is
`tools/amd-uprof-public-api-ab/run-admin-static-api-hold.ps1`. It performs,
in order, Administrator proof, exact fixture/API SHA and architecture/signature
preflight, one captured target launch, raw stdout/stderr persistence, signed
and hexadecimal exit serialization, and a small JSON result. The target CWD
is `D:\apps\AMDuProf\bin`; the default process timeout is `10,000` ms so the
fixture itself owns the 3,000 ms hold. A timeout may terminate only the owned
target process tree, with its cleanup fields recorded. There is no debugger,
profiling call, sampling, root-cause parser, or persistent environment change.

## SYNTHETIC VALIDATION

The wrapper's `-SyntheticTest` mode used only the local PowerShell host,
`cmd.exe`, and `whoami.exe`; no AMD executable or DLL was started. Evidence:

```text
EVIDENCE_ROOT = C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-static-hold-synthetic-20260901T065945125Z
RESULT = SYNTHETIC_REGRESSION_PASS
AMD_RUNTIME_EXECUTED = false
NO_AMD_EXECUTABLE_OR_DLL_STARTED = true
EXIT_CONVERSION = 0 -> 0x00000000; 1 -> 0x00000001; -1 -> 0xFFFFFFFF
NEGATIVE_EXIT = signed -1; stdout/stderr persisted; capture_complete = true
HOLD_MIMIC = both markers; exit 0; elapsed 3330.2268 ms; capture_complete = true
EMPTY_OUTPUT = stdout/stderr files persisted at zero bytes; exit 0
EMPTY_ARGUMENTS = exercised with whoami.exe and argument_count 0
TIMEOUT = true; owned kill-tree cleanup succeeded; no harness failure
```

This validates the capture machinery and marker/lifetime model without
executing the AMD-linked fixture. PowerShell parser validation also passed.

## PRE-RUNTIME STATUS

```text
STATIC_FIXTURE_BUILD = PASS
PE_IMPORT_ASSERTION = PASS
NO_AMD_API_INVOCATION_FROM_MAIN_AUDIT = PASS
MARKER_AUDIT = PASS
SYNTHETIC_HARNESS = PASS
AMD_RUNTIME_EXPERIMENT = NOT_RUN
```

No Administrator execution is authorized by this preparation record. The
original M1 is not rerun, and B1 is not run.

## ADMINISTRATOR COMMANDS READY

After manually opening an elevated x64 PowerShell, run exactly one target
invocation from the AMD `bin` directory. Replace only the checkout root:

```powershell
$repoRoot = '<LOCAL_CHECKOUT_ROOT>'
Set-Location 'D:\apps\AMDuProf\bin'
& (Join-Path $repoRoot 'tools\amd-uprof-public-api-ab\run-admin-static-api-hold.ps1') `
    -InstallRoot 'D:\apps\AMDuProf' `
    -OutputRoot (Join-Path $env:TEMP ('resource-timeline-amd-static-hold-' + (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssfffZ')))
```

The command does not elevate itself, modify PATH, alter services/drivers,
change hypervisor/VBS/HVCI state, or invoke profiling. Do not run the fixture
from Codex and do not repeat the run solely to repair formatting; preserve the
first evidence root.

## AUTHORIZATION BOUNDARY

```text
USER_AUTHORIZATION_REQUIRED = MANUAL_ADMINISTRATOR_RUN_OF_ONE_HOLD_FIXTURE
SYSTEM_MUTATIONS_PERFORMED = false
AMD_INSTALLATION_MUTATIONS = none
HYPER_V_VBS_HVCI_MUTATIONS = none
PRODUCTION_PROVIDER = unchanged
METRIC_CATALOG = unchanged
SCHEMA = unchanged
UI = unchanged
```

## NEXT STEP

`ADMIN_STATIC_LIFETIME_DISCRIMINATOR_REQUIRED`. Consume the one resulting raw
evidence set and classify `M1_FAILURE_FAMILY` as `STARTUP`, `STABLE_MAIN`,
`SHUTDOWN_OR_DETACH`, `NO_FAILURE`, or `INCONCLUSIVE`. Do not run B1, extend
into profiling/sampling, or start provider design before that classification.
