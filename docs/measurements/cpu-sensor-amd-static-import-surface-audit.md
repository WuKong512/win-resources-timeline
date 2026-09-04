# CPU-SENSOR-AMD STATIC-IMPORT SURFACE AUDIT

This is a static, read-only audit of the already installed AMD uProf tree. It
does not launch AMD executables, AMD samples, `metric-probe`, CDB, or any
profiling command. It does not create a new executable, fabricate an import
library, or alter AMD/Windows state. The audit answers whether a clean
vendor-provided static-import control already exists for a later, separately
authorized experiment.

```text
RESULT = PASS
AUDIT_SCOPE = STATIC_ONLY
STATIC_IMPORT_SURFACE_DECISION = EXISTING_VENDOR_CONTROL
CXLBASETOOLS_IMPORT_LIBRARY_PRESENT = NO
AMDPOWERPROFILEAPI_IMPORT_LIBRARY_PRESENT = YES
OFFICIAL_SAMPLE_LINK_MODEL = STATIC_IMPORT
OFFICIAL_SAMPLE_BUILD_FIDELITY = INCONCLUSIVE
CXL_LINK_SURFACE = PRIVATE_INTERNAL
BEST_VENDOR_STATIC_IMPORT_CONTROL = D:\apps\AMDuProf\bin\AMDuProf.exe
MINIMAL_AB_PATH = EXISTING_VENDOR_BINARY_VS_DYNAMIC_PROBE
RECOMMENDED_NEXT_EXPERIMENT = CPU-SENSOR-AMD STATIC-IMPORT VS DYNAMIC-LOAD MINIMAL A/B
```

`RESULT = PASS` means that the requested surface audit completed. It is not a
pass for AMD source qualification, a pass for the direct public API, or an
authorization to start `CPU-SENSOR-AMD-PROVIDER-DESIGN`.

## BASELINE

- Repository: `WuKong512/win-resources-timeline`.
- Base commit: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- Start head: `a08cf9245a7374361ed582cbd9d588556345124c`.
- `origin/main`: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1` at audit time.
- Branch: `spike/cpu-sensor-amd-uprof-live-qualification`.
- The branch already existed and was reused. No competing branch was created.
- The entry working tree was clean. No open-PR inspection tool was available
  (`gh` was not available).
- The previous loader-trace and direct-load evidence records remain preserved.
- AMD install root audited: `D:\apps\AMDuProf\`.
- No AMD executable, sample, `metric-probe`, CDB, profiling command, or
  Administrator experiment was run for this audit.

## INSTALL-TREE RELEVANT INVENTORY

The installed root contains these relevant top-level areas:

| Area | Observed contents | Bounded inventory |
| --- | --- | ---: |
| `bin` | AMD executables, DLLs, and one `.lib` | 1,274 files; 40 DLL, 8 EXE, 1 LIB |
| `include` | Public AMD headers | 4 headers |
| `lib` | `x64` and `x86` libraries | 2 files |
| `Examples` | sample sources and Visual Studio projects | 12 files |
| `Help` | installed API documentation | 1 PDF |
| `Legal` | installed vendor legal material | 15 files |

Installed version metadata in `bin\AMDPerf\metadata\version.txt` and
`bin\Data\Config\Version.txt` reports `5.3.521.0 (Public)`. The installed
`include` directory contains `AMDProfileController.h`, `AMDTDefinitions.h`,
`AMDTPowerProfileApi.h`, and `AMDTPowerProfileDataTypes.h`; no
`CXLBaseTools.h` was found.

No installed `*.a`, `*.def`, `CMakeLists.txt`, makefile, or batch/PowerShell
build setup was found in the bounded install-tree audit.

## IMPORT LIBRARIES

Only three `*.lib` files were found under the installed root. Their archive
identity was inspected rather than inferred from filename alone.

| Path | Size | SHA-256 | Archive identity / architecture | Imported DLL or role |
| --- | ---: | --- | --- | --- |
| `D:\apps\AMDuProf\bin\AMDPowerProfileAPI.lib` | 23,044 | `BF7CDA16612FC3F2B59443154ACB880363A2D5EABD5BFCAE4CF90F4C34371602` | COFF import archive; x64 members (`0x8664`) | Imports `AMDPowerProfileAPI.dll`; contains public power API symbols including initialize, counter enumeration, enable, timer, start/read/stop/close |
| `D:\apps\AMDuProf\lib\x64\AMDProfileController.lib` | 306,856 | `3934D02F53DA84A14C49174C8361AFD936EBAED063CCF3B37E5A429351C6B420` | x64 COFF object/static library; no import descriptor | Controller implementation library, not a CXL import library |
| `D:\apps\AMDuProf\lib\x86\AMDProfileController.lib` | 299,982 | `9649C420605B7D07F649084456B182D795375BD1404D9E43451F74CA27521002` | x86 COFF object/static library; no import descriptor | Controller implementation library, not a CXL import library |

The archive files are not PE images; Authenticode inspection returned
`UnknownError` for these `.lib` files. Their hash, archive type, machine type,
and import/object identity are the applicable evidence. No `CXLBaseTools.lib`
or `AMDSysUtils.lib` was found.

Therefore:

```text
CXLBASETOOLS_IMPORT_LIBRARY_PRESENT = NO
AMDPOWERPROFILEAPI_IMPORT_LIBRARY_PRESENT = YES
AMDSYSUTILS_IMPORT_LIBRARY_PRESENT = NO
```

The public power API import library is a usable surface for an ordinary
client of `AMDPowerProfileAPI.dll`, but it does not provide a supported static
link to the failing `CXLBaseTools.dll` dependency.

## OFFICIAL SAMPLE LINK MODEL

The installed `CollectAllCounters` sample is at:

`D:\apps\AMDuProf\Examples\CollectAllCounters\CollectAllCounters.cpp`

Its source includes `AMDTPowerProfileApi.h` and calls the public API directly:

- `AMDTPwrProfileInitialize`;
- `AMDTPwrGetSupportedCounters`;
- `AMDTPwrEnableCounter`;
- `AMDTPwrSetTimerSamplingPeriod`;
- `AMDTPwrStartProfiling` and `AMDTPwrReadAllEnabledCounters`;
- `AMDTPwrStopProfiling` and `AMDTPwrProfileClose`.

No `LoadLibrary`, `GetProcAddress`, `dlopen`, or equivalent runtime loading
was found in the sample source, and no pragma-based library directive was
found. `CollectAllCounters.vcxproj` has x64 Debug and Release configurations
with:

- `AdditionalDependencies = AMDPowerProfileAPI.lib`;
- an `AdditionalLibraryDirectories` entry for the AMD `bin` directory;
- an `AdditionalIncludeDirectories` entry for the AMD `include` directory.

The source/project link model is therefore:

```text
OFFICIAL_SAMPLE_LINK_MODEL = STATIC_IMPORT
```

This means a PE import dependency through the public import library; it does
not mean that AMD DLL code was statically linked into the sample.

The installed project files still contain default `C:\Program Files\AMD\AMDuProf`
paths, while this machine's actual installation is under `D:\apps\AMDuProf`.
The previously used `CollectAllCounters.exe` was not present in the bounded
workspace/temporary artifact search, and no saved build log was available.
Consequently:

```text
OFFICIAL_SAMPLE_BUILD_FIDELITY = INCONCLUSIVE
```

That uncertainty does not change the source/project link-model finding and
does not justify silently rebuilding or rerunning the sample in this task.

`ClassicCpuProfileCtrl` links the `AMDProfileController.lib` object library;
that is a different controller surface and is not evidence of a CXL import
library. `AMDTClassicMatMul` did not expose a relevant AMD power/CXL link
setting.

## VENDOR EXECUTABLE IMPORT GRAPH

The installed AMD executables were inspected as PE files only. All listed
AMD executables were x64 or x86 as shown, version `5.3.521.0` where reported,
and had valid AMD Authenticode signatures. “Direct” below means the DLL name
is in that executable's PE import table; it is not a transitive conclusion.

| Binary | Size | Arch | SHA-256 | CXL direct | API direct | SysUtils direct | ProfileCommon direct | BackendUtils direct |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- |
| `AMDProcessEnum-x86.exe` | 23,448 | x86 | `011730B675ECB2C6ED99BB9BD8817F32FFA18FCC1E0E058CD0ECE70AF3B383F4` | no | no | no | no | no |
| `AMDProcessEnum.exe` | 26,520 | x64 | `D6CA884C9C0437622DE127352CA02E512CB242AA8394DE058D5D574BA3766CE7` | no | no | no | no | no |
| `AMDProfilerLoadService.exe` | 81,816 | x64 | `43A747955DA05EBF51C7A9B612ABC7D0C5564F82168405A003A08F51CD114516` | no | no | no | no | no |
| `AMDProfilerService.exe` | 699,800 | x64 | `DB2C5372B655CCB6F9651D4A020A13248AD6BF910857F21B72A57E3A99E7F255` | yes | no | no | yes | yes |
| `AMDReportGenerator.exe` | 924,568 | x64 | `D9182B62E1910A775816D985E0DB16A39AAF2A5CBEF2F1C7970F6D04F62C57B6` | no | no | no | no | no |
| `AMDuProf.exe` | 427,416 | x64 | `8F1195F9900C0FF6A5E18AA33127C83DF74622CA7BB68C0C532C3776F6FFD762` | yes | no | no | no | no |
| `AMDuProfCLI.exe` | 1,213,848 | x64 | `D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC` | yes | yes | no | yes | yes |
| `AMDuProfPcm.exe` | 1,630,616 | x64 | `7DF6DA7B3E0E089826712D6E3A91ED99B563C0A02CA2BCFF69A24481B3883218` | no | no | no | no | no |

No relevant delay import of these AMD modules was observed in the scanned
executable set. The installed AMD executable signatures reported the AMD
signer `CN=Advanced Micro Devices, O=Advanced Micro Devices, S=California,
C=US`.

The key direct/transitive relationships in the installed DLL graph are:

```text
AMDuProfCLI.exe       -> CXLBaseTools.dll                 (direct)
AMDuProfCLI.exe       -> AMDPowerProfileAPI.dll           (direct)
AMDPowerProfileAPI.dll -> AMDSysUtils.dll                 (direct)
AMDSysUtils.dll       -> CXLBaseTools.dll                 (direct from SysUtils;
                                                            transitive from API)
AMDuProf.exe          -> CXLBaseTools.dll                 (direct)
AMDProfilerService.exe -> CXLBaseTools.dll                (direct)
AMDProfilerService.exe -> AMDProfileCommon.dll            (direct)
AMDProfilerService.exe -> AMDBackendUtils.dll             (direct)
```

Other AMD DLLs also reference CXL, but the table above is sufficient to keep
the direct-versus-transitive distinction for this audit. `AMDPowerProfileAPI.lib`
imports only `AMDPowerProfileAPI.dll`; it does not turn CXL into a public link
dependency.

## BEST STATIC-IMPORT CANDIDATE

```text
BEST_VENDOR_STATIC_IMPORT_CONTROL = D:\apps\AMDuProf\bin\AMDuProf.exe
```

Evidence for this candidate:

- AMD Authenticode status: `Valid`.
- Architecture: x64.
- Version: `5.3.521.0`.
- Size: 427,416 bytes.
- SHA-256: `8F1195F9900C0FF6A5E18AA33127C83DF74622CA7BB68C0C532C3776F6FFD762`.
- PE import table directly names `CXLBaseTools.dll`.
- It is smaller than the other installed x64 executables that directly import
  CXL (`AMDProfilerService.exe` and `AMDuProfCLI.exe`).
- Among the requested target names it does not directly import the public API,
  `AMDSysUtils`, `AMDProfileCommon`, or `AMDBackendUtils`, making it a narrower
  CXL mapping control than the CLI.

It is not a minimal purpose-built loader: it has Qt and application-framework
dependencies and is a GUI executable. It is nevertheless the smallest
installed AMD-signed executable found with a direct CXL import, so it is a
better future vendor control than using the full CLI if its normal invocation
can be bounded safely. It was not run in this audit.

`AMDProfilerService.exe` is also a direct-CXL-import candidate, but its
service/backend role makes it a less isolated control. `AMDProfilerLoadService.exe`
is smaller but has no direct CXL import and therefore does not satisfy the
required control property.

## CXL LINK-SURFACE CLASSIFICATION

```text
CXL_LINK_SURFACE = PRIVATE_INTERNAL
```

The audit found no `CXLBaseTools.h`, no `CXLBaseTools.lib`, no CXL sample, no
official CXL build instruction, and no installed public documentation that
defines CXL as a client API. The DLL's visible exports and its presence in
other AMD PE import tables establish vendor packaging, not public support.
The classification is therefore `PRIVATE_INTERNAL` for this installed
artifact set.

The public supported link surface found here is `AMDPowerProfileAPI.h` plus
`AMDPowerProfileAPI.lib` for `AMDPowerProfileAPI.dll`. It cannot be used to
fabricate a supported static CXL import surface.

## STATIC-IMPORT MINIMAL A/B FEASIBILITY

The evidence selects the existing-vendor-control path:

```text
MINIMAL_AB_PATH = EXISTING_VENDOR_BINARY_VS_DYNAMIC_PROBE
STATIC_IMPORT_SURFACE_DECISION = EXISTING_VENDOR_CONTROL
```

The future experiment, if separately authorized, should compare the
unmodified installed `AMDuProf.exe` PE static-CXL path with the existing
dynamic direct probe while holding token, CWD, environment, architecture, and
CXL SHA constant. It must remain a short, non-profiling loader observation;
this audit did not execute it and does not infer that the candidate survives.

No clean supported fixture was found that statically imports CXL through a
public AMD import library. The unsupported-buildable alternative is therefore
not selected.

## REJECTED APPROACHES

The following were explicitly not used:

- generating a `.lib` from DLL exports;
- writing a `.def` or import stub;
- using `lib.exe /def` or equivalent archive fabrication;
- patching an import table or rewriting a vendor binary;
- preloading arbitrary AMD DLLs;
- injection, hooks, IAT patching, or Detours;
- copying vendor DLLs, headers, drivers, samples, PDFs, or licenses into the
  repository;
- launching the candidate or repeating any AMD/CDB experiment.

These approaches would test a repository-constructed loader artifact rather
than a vendor-supported static-import surface and are outside this task.

## PRODUCT / LEGAL BOUNDARY

No production provider, ProviderHost behavior, CollectionPlan contract,
MetricCatalog, schema, UI, or product metric changed. No live metric was
accepted. The installed AMD dependency remains an external dependency, and
redistribution remains subject to legal review. This audit does not establish
permission to bundle or redistribute any AMD artifact.

## RECOMMENDATION

```text
RECOMMENDED_NEXT_EXPERIMENT = CPU-SENSOR-AMD STATIC-IMPORT VS DYNAMIC-LOAD MINIMAL A/B
```

Use the existing signed `AMDuProf.exe` direct-CXL import as the first candidate
only in a separately authorized, short, controlled experiment. Before running
it, verify its normal invocation can be bounded without service/system
mutation and keep the dynamic probe unchanged. If that control cannot be
used safely or does not provide a clean loader-only observation, do not replace
it with fabricated linkage; use the already planned non-invasive CLI image
observation or defer the static-vs-dynamic question.

This recommendation does not start `CPU-SENSOR-AMD-PROVIDER-DESIGN` and does
not qualify package power, frequency, temperature, cadence, lifecycle, or
overhead.

## VALIDATION

- Static evidence cross-check completed for the install tree, three `.lib`
  archives, four headers, three Visual Studio sample/project surfaces, all
  eight installed AMD executables, and the relevant AMD DLL import edges.
- No AMD executable, sample, `metric-probe`, CDB, profiling command, or new
  Administrator experiment was launched.
- No temporary PE parser or diagnostic script was added to the repository.
- No production source or Rust code changed; full Rust suites were not rerun
  for this documentation-only audit.
- `git diff --check`: PASS after documentation edits.

## DELIVERY

- Documentation added: this file.
- Existing trace wording received only the minimal provenance clarification
  described in [`cpu-sensor-amd-uprof-cli-direct-loader-trace.md`](cpu-sensor-amd-uprof-cli-direct-loader-trace.md).
- Execution plan updated only with this audit's completed status; the future
  A/B remains explicitly not completed.
- Commit and push are recorded after delivery.
- Draft PR status is separate because PR tooling (`gh`) was unavailable.
- System mutations: none.

## NEXT STEP

`CPU-SENSOR-AMD STATIC-IMPORT VS DYNAMIC-LOAD MINIMAL A/B` — design review and
then a separately authorized experiment using the existing vendor control.
Do not begin `CPU-SENSOR-AMD-PROVIDER-DESIGN`.
