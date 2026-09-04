# CPU-SENSOR-AMD PUBLIC-API STATIC VS DYNAMIC MINIMAL A/B

This record covers preparation and the two A1 outcomes for a minimal
public-API static-versus-dynamic loader experiment. The initial A1 attempt
was unclassifiable because the wrapper failed before persistence. The repaired
A1-R1 attempt completed capture but the static fixture still terminated with
`-1 / 0xFFFFFFFF` and emitted no observable main marker. B1 was correctly
gated and was not launched. The static-vs-dynamic behavior remains untestable
with this A/B.

```text
RESULT = STATIC_CONTROL_INVALID
RUNTIME_EXPERIMENT = STATIC_CONTROL_INVALID_A1_GATE
INITIAL_A1_RESULT = EXECUTED_BUT_UNCLASSIFIABLE_DUE_TO_HARNESS_PERSISTENCE_FAILURE
A1_R1_RESULT = STATIC_CONTROL_INVALID
B1_EXECUTED = FALSE
STATIC_IMPORT_RETENTION = CONFIRMED
STATIC_FIXTURE_DIRECT_IMPORT_AMDPOWERPROFILEAPI = TRUE
STATIC_FIXTURE_DIRECT_IMPORT_CXLBASETOOLS = FALSE
DYNAMIC_FIXTURE_DIRECT_IMPORT_AMDPOWERPROFILEAPI = FALSE
DYNAMIC_FIXTURE_DIRECT_IMPORT_CXLBASETOOLS = FALSE
DEPENDENCY_CHAIN = AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll
```

`RESULT` describes the current handoff state. It is not a result for loader
survival, AMD API qualification, or production integration.

## BASELINE

- Repository: `WuKong512/win-resources-timeline`.
- Accepted start head: `d97a06ddb52068d6a634bcf9fd14fa29977d97e0`.
- Base commit: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- `origin/main`: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1` at preparation time.
- Branch: `spike/cpu-sensor-amd-uprof-live-qualification`.
- Existing branch was reused; no competing branch was created.
- The prior static-import surface audit is preserved. Its accepted findings
  are `CXLBASETOOLS_IMPORT_LIBRARY_PRESENT = NO`,
  `AMDPOWERPROFILEAPI_IMPORT_LIBRARY_PRESENT = YES`, and
  `CXL_LINK_SURFACE = PRIVATE_INTERNAL`.
- The prior official sample binary/build fidelity remains
  `OFFICIAL_SAMPLE_BUILD_FIDELITY = INCONCLUSIVE`; this experiment does not
  silently substitute the missing sample artifact.
- AMD install root: `D:\apps\AMDuProf\`.
- No persistent PATH, registry, service, driver, security, hypervisor, or
  installation mutation was performed.

## HYPOTHESIS

The experiment asks only whether the same installed public power-profile
dependency chain has different native loader survival behavior when the first
edge is a process-start/static PE import versus a runtime `LoadLibraryExW`
call:

```text
static fixture PE import:
  amd-uprof-static-api-load-fixture.exe
    -> AMDPowerProfileAPI.dll
      -> AMDSysUtils.dll
        -> CXLBaseTools.dll

dynamic fixture main:
  LoadLibraryExW(absolute AMDPowerProfileAPI.dll path)
    -> AMDPowerProfileAPI.dll
      -> AMDSysUtils.dll
        -> CXLBaseTools.dll
```

The experiment does not attempt to identify a private CXL branch or prove
that load mode alone is causal.

## PUBLIC IMPORT-LIBRARY PROVENANCE

The installed official import library is:

```text
Path = D:\apps\AMDuProf\bin\AMDPowerProfileAPI.lib
Size = 23044 bytes
SHA-256 = BF7CDA16612FC3F2B59443154ACB880363A2D5EABD5BFCAE4CF90F4C34371602
Archive = x64 COFF import library (machine 0x8664)
Imported DLL = AMDPowerProfileAPI.dll
```

The installed Windows header declares the API after the `AMDTDefinitions.h`
definitions block, so the C++-compiled import symbol is decorated. The
archive contains the exact symbol used by the fixture:

```text
?AMDTPwrProfileInitialize@@YAIW4AMDTPwrProfileMode@@@Z
```

The first release-link attempt using an unqualified C symbol failed with
`LNK2001`. The exact decorated name was then read from the official archive
and referenced with Rust `#[link_name]`. No `.def`, generated `.lib`, import
stub, export-derived linkage, or vendor binary modification was used.

## STATIC FIXTURE

Source:

`tools/amd-uprof-public-api-ab/src/bin/amd-uprof-static-api-load-fixture.rs`

The fixture links only the installed `AMDPowerProfileAPI.lib` through the
Rust/MSVC linker. It retains the official `AMDTPwrProfileInitialize` import
by storing its function address in a `#[used]` anchor and reading that anchor
with `read_volatile`. It does not call the function. There is no initialize,
enumerate, enable, timer, start, read, stop, close, driver, or sampling path.

After process-start dependency mapping, its only diagnostic output is the
main-reached marker and static contract metadata:

```text
STATIC_FIXTURE_MAIN_REACHED=true
STATIC_FIXTURE_API_IMPORT_REFERENCE=AMDTPwrProfileInitialize
STATIC_FIXTURE_API_CALLS=0
```

## DYNAMIC FIXTURE

Source:

`tools/amd-uprof-public-api-ab/src/bin/amd-uprof-dynamic-api-load-fixture.rs`

The dynamic fixture uses the same Rust/MSVC toolchain and x64 target. It does
not link the AMD import library and has no AMD API declaration. It requires
one absolute DLL path, emits:

```text
DYNAMIC_FIXTURE_MAIN_REACHED=true
BEFORE_LOADLIBRARY=true
```

and then calls only Windows `LoadLibraryExW` with the absolute path and
`LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` (`0x900`).
It does not resolve or invoke an AMD export. On a normal return it emits
`AFTER_LOADLIBRARY=true` and the module handle; if the vendor path terminates
the child, raw output can end after `BEFORE_LOADLIBRARY=true`.

## BUILD IDENTITY AND STATIC PE QUALIFICATION

Both fixtures were built with the installed toolchain:

```text
rustc = 1.97.0 (2d8144b7880597b6e6d3dfd63a9a9efae3f533d3)
host = x86_64-pc-windows-msvc
LLVM = 22.1.6
link.exe = C:\BuildTools\VC\Tools\MSVC\14.44.35207\bin\HostX64\x64\link.exe
link.exe version = 14.44.35228.0
release build = cargo build --release --bins
```

The repository-built fixtures are intentionally unsigned; their signature is
recorded but not a release gate. Their identity is hash-locked for the
manual comparison:

| Fixture | Size | SHA-256 | PE machine | Authenticode | Signature required |
| --- | ---: | --- | --- | --- | --- |
| `tools/amd-uprof-public-api-ab/target/release/amd-uprof-static-api-load-fixture.exe` | 126,464 | `9FAC63BD6B1FF1888DFFC8736F4152B972164DDAE8E1369584A53C1705354F53` | `0x8664` / x64 | `NotSigned` | no |
| `tools/amd-uprof-public-api-ab/target/release/amd-uprof-dynamic-api-load-fixture.exe` | 137,728 | `2111185AA7E9F162D864D4F8E9C72E17B1769D94A0A09B00543876877F36416A` | `0x8664` / x64 | `NotSigned` | no |

The vendor API DLL used by both controls is:

```text
Path = D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll
SHA-256 = 9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277
Architecture = x64 (PE machine 0x8664)
Authenticode = Valid, AMD signer
Signature required = yes
```

The release PE import directory was parsed after build:

| Fixture | Direct `AMDPowerProfileAPI.dll` | Direct `CXLBaseTools.dll` | Relevant imports |
| --- | --- | --- | --- |
| static | true | false | `AMDPowerProfileAPI.dll`, Windows/runtime DLLs |
| dynamic | false | false | Windows/runtime DLLs only; no AMD DLL |

This proves the static fixture has a genuine direct public-API PE import and
does not directly import the private CXL DLL. It also proves the dynamic
fixture has neither direct AMD dependency, so the runtime load is not
silently converted into a static import.

## DEPENDENCY-CHAIN QUALIFICATION

The installed PE graph, independently audited before this experiment, is:

```text
AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll
```

The static fixture adds only the first edge at the executable level. The
dynamic fixture adds no AMD edge at the executable level and asks Windows to
load the same absolute API DLL from `main`.

No CXL import library exists in the installation, and neither fixture links
directly to CXL. The test therefore compares a supported public import
library surface with the existing dynamic public-DLL path; it does not
construct a private CXL client.

## CONTROLS

The pending runtime comparison is intentionally one-pass and minimal:

| Control | Executable | CWD | Arguments | Timeout | Expected gate |
| --- | --- | --- | --- | ---: | --- |
| A1 static | repository-built static fixture | `D:\apps\AMDuProf\bin` | none | 20,000 ms | main marker, exit 0, no timeout |
| B1 dynamic | repository-built dynamic fixture | `D:\apps\AMDuProf\bin` | absolute API DLL path | 20,000 ms | raw `BEFORE_LOADLIBRARY`; then success or abnormal termination |

Both controls must run from the same manually elevated x64 PowerShell, with
the same inherited environment, CWD, wrapper, timeout, and vendor API SHA.
The wrapper does not mutate PATH or current directory globally. It captures
raw stdout/stderr, timestamps, signed/hex target exit status, timeout, PID,
and owned-process-tree cleanup fields. It does not parse loader internals.

The repaired wrapper supports an explicit `-A1Only` handoff for the current
repair rerun and an explicit `-RunB1` mode for a later, separately authorized
full A/B. The dynamic control remains gated on A1. If A1 does not reach
`main` and exit 0, the wrapper writes `STATIC_CONTROL_INVALID` and does not
run B1. There is no automatic retry or sampling expansion.

## CLASSIFICATION CONTRACT

Only after real evidence is supplied:

```text
CASE A:
  A1 reaches main and exits 0;
  B1 stops after BEFORE_LOADLIBRARY or exits abnormally;
  => STATIC_API_DEPENDENCY_CHAIN_SURVIVES = true
     DYNAMIC_API_DEPENDENCY_CHAIN_ABORTS = true
     STATIC_VS_DYNAMIC_LOAD_BEHAVIOR_DIVERGENCE = CONFIRMED
     EXACT_CXL_INTERNAL_BRANCH = UNPROVEN
     LOAD_MODE_IS_SOLE_CAUSE = UNPROVEN

CASE B:
  both controls return normally;
  => STATIC_VS_DYNAMIC_LOAD_BEHAVIOR_DIVERGENCE = DISPROVEN_FOR_PUBLIC_API_CHAIN

CASE C:
  A1 fails before main;
  => STATIC_CONTROL_INVALID; B1 is not run

CASE D:
  wrapper capture itself is incomplete or invalid;
  => BLOCKED_HARNESS; no retry in this handoff
```

No result from this A/B would by itself approve a production provider or
prove a specific private CXL condition.

## INITIAL A1 ATTEMPT AND EVIDENCE CONSUMPTION

The first real A1 invocation is preserved as:

```text
A1_ID = A1-INITIAL-20260901T021841634Z
EVIDENCE_ROOT = C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-public-api-ab-20260901T021841634Z
STATUS = EXECUTED_BUT_UNCLASSIFIABLE_DUE_TO_HARNESS_PERSISTENCE_FAILURE
A1_EXECUTED = TRUE
A1_TARGET_EXIT_SIGNED_OBSERVED = -1
A1_TARGET_EXIT_HEX_EXPECTED = 0xFFFFFFFF
A1_STDOUT_PERSISTED = FALSE
A1_STDERR_PERSISTED = FALSE
A1_RESULT_JSON_PERSISTED = FALSE
A1_MAIN_REACHED = UNPROVEN
STATIC_CONTROL_VALIDITY = INCONCLUSIVE
B1_EXECUTED = FALSE
```

The evidence directory contains the Administrator proof and artifact
preflight, but no A1 stdout, stderr, result JSON, or summary. The wrapper's
execution order proves that `Process.Start()` occurred, `WaitForExit()`
completed, and the signed target exit code was read as `-1`. The old wrapper
then attempted `[uint32]$targetExitSigned` while formatting the hexadecimal
exit code. PowerShell threw before stdout/stderr and result persistence. The
missing streams therefore do not prove that `main` was or was not reached.

B1 was not executed because the wrapper failed during A1 handling. This is a
wrapper persistence failure, not a `STATIC_CONTROL_INVALID` classification.

## HARNESS REPAIR

The two pre-existing intentional fixes are retained:

- `AMDPowerProfileAPI.dll` expected SHA metadata is corrected to
  `9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277`.
  The earlier truncated value ending in `A427` is classified as
  `API_SHA_PREFLIGHT_BUG = EXPECTED_HASH_METADATA_TYPO`; it is not vendor
  artifact drift. Historical evidence is not rewritten.
- `Invoke-CapturedProcess.Arguments` accepts `@()` through
  `[AllowEmptyCollection()]`.

The wrapper now uses `Convert-ExitCodeToHex`, which preserves the signed 32-bit
bit pattern through `BitConverter`:

```text
0  -> 0x00000000
1  -> 0x00000001
-1 -> 0xFFFFFFFF
```

Raw stdout and stderr are captured and persisted independently of exit-code
formatting. A negative target exit is represented as
`target_process_status = TARGET_PROCESS_FAILED`, not as a harness exception.
The process object is disposed in `finally`; timeout tree-kill fields remain
separate from target failure fields. The result JSON records persistence flags,
`capture_complete`, target status, harness status, and `result_path`.

The wrapper now requires an explicit runtime mode. `-SyntheticTest` is a
non-AMD regression mode, `-A1Only` is the current repair rerun, and `-RunB1`
is reserved for a later explicit full A/B authorization. No default mode can
silently launch B1.

## NON-AMD SYNTHETIC REGRESSION

The repaired wrapper was executed only in `-SyntheticTest` mode. It launched
the current PowerShell host for synthetic child processes and `whoami.exe` for
the empty-argument check; no AMD artifact was loaded and no AMD executable,
profiling, CDB, or sampling command was run.

```text
RESULT = SYNTHETIC_REGRESSION_PASS
AMD_RUNTIME_EXECUTED = FALSE
NEGATIVE_EXIT = -1 / 0xFFFFFFFF
NEGATIVE_CAPTURE_COMPLETE = TRUE
NEGATIVE_STDOUT_PERSISTED = TRUE
NEGATIVE_STDERR_PERSISTED = TRUE
ZERO_EXIT = 0 / 0x00000000
ZERO_CAPTURE_COMPLETE = TRUE
ZERO_STDERR_BYTES = 0
EMPTY_ARGUMENTS_EXIT = 0
EMPTY_ARGUMENTS_CAPTURE_COMPLETE = TRUE
RESULT_JSON_PERSISTED = TRUE
TIMEOUT = FALSE
KILL_TREE = FALSE
```

The negative fixture persisted both known stdout/stderr markers and its result
JSON. The zero-exit fixture persisted a non-empty stdout marker and a
zero-length stderr file, proving that an empty string is a valid captured
stream. The `whoami.exe` invocation used `Arguments = @()` and completed with
exit 0, retaining coverage for the empty-array binding fix.

## A1-R1 EVIDENCE CLOSURE

A1-R1 was executed by the user with the repaired wrapper. It is a clean
capture of the static fixture, not a rerun performed by Codex:

```text
A1_R1_EVIDENCE_ROOT = C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-public-api-ab-20260901T033613032Z
A1_R1_EXECUTED = TRUE
PROCESS_STARTED = TRUE
TIMEOUT = FALSE
TARGET_EXIT_SIGNED = -1
TARGET_EXIT_HEX = 0xFFFFFFFF
A1_R1_TARGET_EXIT = 0xFFFFFFFF
STDOUT_BYTES = 0
STDERR_BYTES = 0
STDOUT_PERSISTED = TRUE
STDERR_PERSISTED = TRUE
CAPTURE_COMPLETE = TRUE
A1_R1_CAPTURE_COMPLETE = TRUE
TARGET_PROCESS_FAILED = TRUE
HARNESS_FAILED = FALSE
B1_EXECUTED = FALSE
RUNTIME_DURATION = approximately 45.6 ms
```

The summary is `STATIC_CONTROL_INVALID`. The static-control contract required
`STATIC_FIXTURE_MAIN_REACHED=true`, target exit 0, and no timeout. It was not
met, so:

```text
STATIC_CONTROL_INVALID = CONFIRMED
STATIC_FIXTURE_NORMAL_STARTUP = FAILED
STATIC_PUBLIC_API_STATIC_SURVIVAL_PREMISE = FALSIFIED_FOR_THIS_FIXTURE
STATIC_VS_DYNAMIC_LOAD_BEHAVIOR_DIVERGENCE = NOT_TESTABLE_WITH_THIS_A/B
B1_STATUS = NOT_AUTHORIZED_BECAUSE_STATIC_CONTROL_INVALID
```

The empty stdout file is now reliable evidence of an empty captured stream,
but not by itself reliable evidence that `main` was never reached; the source
marker uses buffered `println!` without an explicit flush. A later abrupt
vendor/runtime shutdown could lose buffered output. Therefore:

```text
MAIN_MARKER_RELIABILITY = BUFFERING_AMBIGUITY
STATIC_FIXTURE_MAIN_REACHED = UNPROVEN
STATIC_FAILURE_STAGE = PRE_MAIN_OR_PROCESS_SHUTDOWN
```

This intentionally does not upgrade the result to a proven before-main
failure.

## STATIC FIXTURE SOURCE AND IMPORT RETENTION AUDIT

The committed source is
`tools/amd-uprof-public-api-ab/src/bin/amd-uprof-static-api-load-fixture.rs`.
The first statement in `main` reads the retained imported function pointer
through `std::ptr::read_volatile` and passes the pointer to
`std::hint::black_box`. It does not invoke the AMD function. The following
statements emit the marker and metadata using Rust `println!`; there is no
explicit `stdout().flush()` or direct OS `WriteFile` call. `main` then returns
normally after the four print calls.

The import is retained by the official `AMDPowerProfileAPI.lib` link directive,
the decorated `AMDTPwrProfileInitialize` symbol reference, a `#[used]` anchor,
and the volatile anchor read. `build.rs` links only the installed
`D:\apps\AMDuProf\bin\AMDPowerProfileAPI.lib`; it does not fabricate a `.lib`
or link CXL directly.

Static PE inspection of the unchanged release artifact confirms:

```text
STATIC_SHA256 = 9FAC63BD6B1FF1888DFFC8736F4152B972164DDAE8E1369584A53C1705354F53
PE_MACHINE = 0x8664 / x64
DIRECT_IMPORT_AMDPOWERPROFILEAPI = TRUE
DIRECT_IMPORT_CXLBASETOOLS = FALSE
DIRECT_IMPORT_SYMBOL = AMDTPwrProfileInitialize
AMD_API_CALL_FROM_MAIN = FALSE
```

The installed dependency graph remains:

```text
fixture -> AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll
```

Thus the fixture has a genuine public API PE import, but no AMD API call is
reachable from `main`. The imported DLLs can run process-start initialization
before `main`; after normal `main` return, Rust/runtime shutdown and DLL
`PROCESS_DETACH` can occur before the wrapper observes process termination.
That shutdown possibility is why the missing buffered marker cannot isolate the
failure stage.

## STATIC-VS-DYNAMIC HYPOTHESIS UPDATE

The result disproves the assumption that a minimal executable automatically
becomes a valid surviving control merely by statically importing the official
public API library:

```text
MINIMAL_PUBLIC_API_STATIC_CONTROL_ASSUMPTION = DISPROVEN
STATIC_VS_DYNAMIC_LOAD_HYPOTHESIS = DOWNGRADED
```

This does not fully disprove load-mode effects. The static fixture itself does
not survive, so static-vs-dynamic behavior cannot be compared with the planned
matched A/B. Historical successful `AMDuProfCLI.exe` behavior and failed
minimal/direct paths make the following the primary hypothesis family without
selecting a specific cause:

```text
VENDOR_EXECUTABLE_SPECIFIC_CONTEXT = PRIMARY_HYPOTHESIS_FAMILY
EXACT_VENDOR_EXECUTABLE_REQUIREMENT = UNPROVEN
```

Candidate dimensions include executable import topology, PE/process metadata,
bootstrap state, initialization ordering, path behavior, and another vendor
pre-main or shutdown condition. None is established here.

## SECONDARY VENDOR CONTROL

The previously audited `D:\apps\AMDuProf\bin\AMDuProf.exe` remains a
secondary vendor control: AMD-signed, x64, and a direct CXL importer. It is
not used in this primary A/B because its GUI, Qt, application-framework, and
additional AMD runtime context introduce extra variables. It must not be
run automatically if the primary A/B is ambiguous.

## USER AUTHORIZATION BOUNDARY

A1-R1 was run manually by the user from an elevated x64 Administrator
PowerShell. Codex did not invoke UAC, use `runas`, create an elevated helper,
or execute any AMD command during evidence consumption. No further runtime
command is authorized in this closure; in particular, B1 remains closed.

## VALIDATION

- `cargo fmt --manifest-path tools/amd-uprof-public-api-ab/Cargo.toml -- --check`: PASS.
- `cargo test --manifest-path tools/amd-uprof-public-api-ab/Cargo.toml`: PASS, 3 passed, 0 failed; only the library tests ran because fixture bins have `test = false`.
- `AMD_UPROF_ROOT=D:\apps\AMDuProf cargo build --release --bins`: PASS.
- Static PE import assertion: PASS — static direct API `true`, static direct
  CXL `false`, dynamic direct API `false`, dynamic direct CXL `false`; both
  x64.
- Administrator wrapper PowerShell parse: PASS.
- A1-R1 evidence cross-check: PASS; capture complete, no timeout, no kill,
  target failure recorded separately from harness failure.
- Static fixture source/build/import cross-check: PASS; the committed source,
  official import library link, and release PE import table agree.
- Non-AMD synthetic regression: PASS, including signed exit `-1`, signed exit
  `0`, stdout/stderr persistence, empty stderr, result JSON, and `Arguments =
  @()`.
- No new A1, B1, AMD runtime command, CDB, profiling, sampling, or user
  workload was run during this evidence-consumption task.
- No production provider/catalog/schema/UI code changed.

## DELIVERY

- Added independent diagnostic crate under `tools/amd-uprof-public-api-ab`.
- Added manual-only wrapper `run-admin-minimal-ab.ps1`.
- Added this design/evidence record.
- Execution plan records A1-R1 closure and keeps the static-vs-dynamic A/B
  incomplete; B1 was not authorized.
- First A1 runtime evidence: preserved but unclassifiable as documented above.
- A1-R1 runtime evidence: consumed and closed as `STATIC_CONTROL_INVALID`.
- System mutations: none.

## NEXT STEP

`CPU-SENSOR-AMD VENDOR EXECUTABLE CONTEXT DIFFERENTIAL AUDIT`: design the
next static/read-only comparison among the successful historical
`AMDuProfCLI.exe`, the audited `AMDuProf.exe` direct-CXL candidate, and the
failed minimal static fixture. Do not run B1 merely to complete an invalid
A/B, and do not start `CPU-SENSOR-AMD-PROVIDER-DESIGN`.
