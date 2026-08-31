# CPU-SENSOR-AMD PUBLIC-API STATIC VS DYNAMIC MINIMAL A/B

This record covers preparation and static qualification of a minimal
public-API static-versus-dynamic loader experiment. The runtime A/B has not
been run. No AMD executable, AMD sample, `metric-probe`, CDB, profiling, or
sampling command was launched while preparing this record.

```text
RESULT = ADMIN_MINIMAL_AB_REQUIRED
RUNTIME_EXPERIMENT = NOT_RUN
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
SHA-256 = 9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A427
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

The dynamic control is gated on A1. If A1 does not reach `main` and exit 0,
the wrapper writes `STATIC_CONTROL_INVALID` and does not run B1. There is no
retry or sampling expansion.

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

## SECONDARY VENDOR CONTROL

The previously audited `D:\apps\AMDuProf\bin\AMDuProf.exe` remains a
secondary vendor control: AMD-signed, x64, and a direct CXL importer. It is
not used in this primary A/B because its GUI, Qt, application-framework, and
additional AMD runtime context introduce extra variables. It must not be
run automatically if the primary A/B is ambiguous.

## USER AUTHORIZATION BOUNDARY

Preparation and static qualification were performed without AMD runtime
execution. The next runtime step requires the user to manually open an
Administrator PowerShell. Codex must not invoke UAC, use `runas`, create an
elevated helper, or execute the wrapper on the user's behalf.

The exact manual wrapper is:

```powershell
$ErrorActionPreference = 'Stop'
$repoRoot = 'F:\File\codex\codex-worktrees\08bd\resource-timeline'
Set-Location -LiteralPath $repoRoot
& "$repoRoot\tools\amd-uprof-public-api-ab\run-admin-minimal-ab.ps1"
```

The script first proves the Administrator token and x64 PowerShell, verifies
the exact fixture/API hashes (repository fixture signatures are recorded but
not required; the AMD DLL signature is required), runs A1 once, gates B1 on
A1 success, and writes the evidence root. It does not run profiling,
sampling, initialize any AMD API, or modify system state.

## VALIDATION

- `cargo fmt --manifest-path tools/amd-uprof-public-api-ab/Cargo.toml -- --check`: PASS.
- `cargo test --manifest-path tools/amd-uprof-public-api-ab/Cargo.toml`: PASS, 3 passed, 0 failed; only the library tests ran because fixture bins have `test = false`.
- `AMD_UPROF_ROOT=D:\apps\AMDuProf cargo build --release --bins`: PASS.
- Static PE import assertion: PASS — static direct API `true`, static direct
  CXL `false`, dynamic direct API `false`, dynamic direct CXL `false`; both
  x64.
- Administrator wrapper PowerShell parse: PASS; it was not invoked.
- No AMD runtime command, CDB, profiling, sampling, or user workload was
  run.
- No production provider/catalog/schema/UI code changed.

## DELIVERY

- Added independent diagnostic crate under `tools/amd-uprof-public-api-ab`.
- Added manual-only wrapper `run-admin-minimal-ab.ps1`.
- Added this design/evidence record.
- Execution plan records the runtime handoff as pending; it does not mark the
  A/B completed.
- Runtime evidence: not yet available.
- System mutations: none.

## NEXT STEP

`ADMIN_MINIMAL_AB_COMMANDS_READY`: the user may run the exact wrapper once
from a manually elevated PowerShell. After the evidence is returned, consume
the raw A1/B1 records and classify the public API chain. Do not start
`CPU-SENSOR-AMD-PROVIDER-DESIGN`.
