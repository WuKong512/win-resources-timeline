# CPU-SENSOR-AMD VENDOR EXECUTABLE CONTEXT DIFFERENTIAL AUDIT

This is a static/read-only audit of the installed AMD uProf 5.3.521 tree. No
AMD executable, sample, profiler, debugger, or repository diagnostic fixture
was launched for this audit. It does not qualify a live source and does not
authorize `CPU-SENSOR-AMD-PROVIDER-DESIGN`.

```text
RESULT = PASS
AUDIT_TYPE = STATIC_ONLY
RUNTIME_EXPERIMENT = NOT_PERFORMED
```

## BASELINE

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
AUDIT_START_HEAD = d536da607fbc35e7b178ceaecb7b8b820bcb68a3
ORIGIN_MAIN_AT_ENTRY = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
WORKING_TREE_AT_ENTRY = CLEAN
Q1_MERGE_ANCESTOR = PASS
DUPLICATE_TASK_GATE = PASS
```

`git fetch origin --prune` refreshed the existing refs. The requested branch
already existed locally and remotely and was continued; no competing task/PR
was found. GitHub CLI was not available for an independent PR query. There was
no main-branch drift relative to the recorded `origin/main` at entry; the
qualification branch was ahead of that base. No reset, rebase, merge,
cherry-pick, amend, or force-push was performed.

## INSTALL TREE AND COMPARED ARTIFACTS

The installed uProf root used for all vendor inspection was
`D:\apps\AMDuProf\`. The vendor signer shown below was
`CN=Advanced Micro Devices, O=Advanced Micro Devices, S=California, C=US`.

| ID | Role | Exact path | Size | SHA-256 | PE | Subsystem | File/product version | Authenticode |
| --- | --- | --- | ---: | --- | --- | --- | --- | --- |
| V1 | historically successful CLI | `D:\apps\AMDuProf\bin\AMDuProfCLI.exe` | 1,213,848 | `D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC` | x64 / `0x8664` | Windows CUI | 5.3.521.0 / 5.3.521.0 | Valid, AMD |
| V2 | smallest installed x64 direct-CXL EXE | `D:\apps\AMDuProf\bin\AMDuProf.exe` | 427,416 | `8F1195F9900C0FF6A5E18AA33127C83DF74622CA7BB68C0C532C3776F6FFD762` | x64 / `0x8664` | Windows GUI | 5.3.521.0 / 5.3.521.0 | Valid, AMD |
| V3 | optional service comparison | `D:\apps\AMDuProf\bin\AMDProfilerService.exe` | 699,800 | `DB2C5372B655CCB6F9651D4A020A13248AD6BF910857F21B72A57E3A99E7F255` | x64 / `0x8664` | Windows GUI | 5.3.521.0 / 5.3.521.0 | Valid, AMD |
| M1 | failed minimal static public-API fixture | `tools/amd-uprof-public-api-ab/target/release/amd-uprof-static-api-load-fixture.exe` | 126,464 | `9FAC63BD6B1FF1888DFFC8736F4152B972164DDAE8E1369584A53C1705354F53` | x64 / `0x8664` | Windows CUI | no version resource | NotSigned, repository-built |

PE timestamps from the headers were V1 `0x6A2AE886` (2026-06-12
00:55:34), V2 `0x6A2AE8A3` (2026-06-12 00:56:03), V3 `0x6A2AE885`
(2026-06-12 00:55:33), and M1 `0x6A94F978` (2026-08-31 11:48:08).
The timestamp difference is a build-history observation, not a causal claim.

The bounded install inventory also found the AMD runtime DLL set in `bin`, the
four public headers in `include`, the x64/x86 `AMDProfileController.lib` files,
the installed `Examples` trees, and `Help/AMDPowerProfilerAPI.pdf`.

## IMPORT LIBRARIES AND OFFICIAL SAMPLE LINK MODEL

The only AMD power-profile import archive found was:

```text
PATH = D:\apps\AMDuProf\bin\AMDPowerProfileAPI.lib
SIZE = 23,044
SHA256 = BF7CDA16612FC3F2B59443154ACB880363A2D5EABD5BFCAE4CF90F4C34371602
MACHINE = x64 / 0x8664 COFF import archive
IMPORTED_DLL = AMDPowerProfileAPI.dll
```

The other archives were `lib/x64/AMDProfileController.lib` and
`lib/x86/AMDProfileController.lib`; both are COFF object/static libraries and
do not contain a CXL import descriptor. No `CXLBaseTools.lib` or
`AMDSysUtils.lib` was found. No CXL header was found; the installed headers are
`AMDTPowerProfileApi.h`, `AMDTPowerProfileDataTypes.h`,
`AMDProfileController.h`, and `AMDTDefinitions.h`.

The installed `CollectAllCounters` source includes `AMDTPowerProfileApi.h`,
calls the public API directly, and has no `LoadLibrary`/`GetProcAddress`
loading model. Its Visual Studio project specifies
`AMDPowerProfileAPI.lib` and an AMD `bin` library directory, with the default
`C:\Program Files\AMD\AMDuProf` include/library paths. The installed project
uses the v140 toolset. No matching built sample binary or build log was
available, so project fidelity is not a claim about a particular executable.

```text
CXLBASETOOLS_IMPORT_LIBRARY_PRESENT = NO
AMDPOWERPROFILEAPI_IMPORT_LIBRARY_PRESENT = YES
OFFICIAL_SAMPLE_LINK_MODEL = STATIC_IMPORT
CXL_LINK_SURFACE = PRIVATE_INTERNAL
```

The public archive contains the decorated `AMDTPwr*` import symbols, including
initialize, supported-counter enumeration, enable/configure, timer,
start/read/stop, and close. CXL visibility in a vendor PE is packaging evidence,
not a supported CXL client surface. No fabricated `.def`, import stub, or CXL
library was created.

## DIRECT IMPORT GRAPH

The following normalized states come from the PE import tables plus a bounded
recursive walk of local AMD dependencies. `DIRECT_IMPORT` means the executable
has that import descriptor; `TRANSITIVE_IMPORT` means it is reachable through a
local dependency. Neither state claims a runtime load order.

| AMD/local module | V1 CLI | V2 GUI | M1 fixture | V3 service |
| --- | --- | --- | --- | --- |
| `CXLBaseTools.dll` | DIRECT_IMPORT | DIRECT_IMPORT | TRANSITIVE_IMPORT | DIRECT_IMPORT |
| `AMDPowerProfileAPI.dll` | DIRECT_IMPORT | NOT_PRESENT | DIRECT_IMPORT | TRANSITIVE_IMPORT |
| `AMDSysUtils.dll` | TRANSITIVE_IMPORT | TRANSITIVE_IMPORT | TRANSITIVE_IMPORT | TRANSITIVE_IMPORT |
| `CXLOSWrappers.dll` | DIRECT_IMPORT | DIRECT_IMPORT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDBaseUtils.dll` | DIRECT_IMPORT | DIRECT_IMPORT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDSharedUtils.dll` | DIRECT_IMPORT | DIRECT_IMPORT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDProfileCommon.dll` | DIRECT_IMPORT | NOT_PRESENT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDBackendUtils.dll` | DIRECT_IMPORT | NOT_PRESENT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDCpuPerfEventUtils.dll` | DIRECT_IMPORT | NOT_PRESENT | NOT_PRESENT | TRANSITIVE_IMPORT |
| `AMDProfileDataAccessor.dll` | DIRECT_IMPORT | NOT_PRESENT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDProfilerDAL.dll` | DIRECT_IMPORT | NOT_PRESENT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDRestApiUtils.dll` | NOT_PRESENT | DIRECT_IMPORT | NOT_PRESENT | DIRECT_IMPORT |
| `AMDApplicationFramework.dll` | NOT_PRESENT | DIRECT_IMPORT | NOT_PRESENT | NOT_PRESENT |
| `AMDApplicationViews.dll` | NOT_PRESENT | DIRECT_IMPORT | NOT_PRESENT | NOT_PRESENT |
| `AMDExecutableFormat.dll` | TRANSITIVE_IMPORT | NOT_PRESENT | NOT_PRESENT | TRANSITIVE_IMPORT |
| `AMDCountModeProfileAPI.dll` | TRANSITIVE_IMPORT | NOT_PRESENT | NOT_PRESENT | TRANSITIVE_IMPORT |
| `AMDThreadProfileAPI.dll` | TRANSITIVE_IMPORT | NOT_PRESENT | NOT_PRESENT | TRANSITIVE_IMPORT |
| `AMDPowerProfileAppAnalysis.dll` | TRANSITIVE_IMPORT | NOT_PRESENT | NOT_PRESENT | TRANSITIVE_IMPORT |

The root import descriptor order is also different:

```text
V1: KERNEL32, CXLBaseTools, CXLOSWrappers, AMDBaseUtils, AMDProfileCommon,
    AMDSharedUtils, AMDBackendUtils, AMDCpuPerfEventUtils,
    AMDProfileDataAccessor, AMDPowerProfileAPI, AMDProfilerDAL, CRT...
V2: shcore, Qt5Widgets, Qt5Core, Qt5Gui, KERNEL32, CXLBaseTools,
    CXLOSWrappers, AMDBaseUtils, AMDSharedUtils, AMDRestApiUtils,
    AMDApplicationFramework, AMDApplicationViews, CRT...
V3: KERNEL32, CXLBaseTools, CXLOSWrappers, AMDProfileDataAccessor,
    AMDProfilerDAL, AMDBaseUtils, AMDProfileCommon, AMDSharedUtils,
    AMDBackendUtils, AMDRestApiUtils, CRT...
M1: AMDPowerProfileAPI, api-ms-win-core-synch, KERNEL32, ntdll, CRT...
```

### Dependency DAG and process-start CXL indegree

M1 has one narrow public-API route:

```text
M1 -> AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll
```

V1, V2, and V3 each have a direct executable-to-CXL edge and multiple other
local AMD parents that also import CXL. V1 includes the profile/common/backend,
base/shared, wrapper, perf, data, and DAL family; V2 includes the wrapper,
base/shared, REST, and application family; V3 includes the profile/data/DAL,
base/shared/backend, and REST family. In the bounded local graph, this is
`direct executable import plus multiple direct/transitive parents`, versus
`one transitive path` for M1.

This topology can alter which dependency graph is processed at process start,
but import descriptors alone cannot establish Windows DLL initialization order.

## DIRECT CXL IMPORTERS AND SYMBOL CATEGORIES

The bounded scan of installed `bin\\*.dll` found these direct CXL importers:

```text
AMDApplicationFramework.dll       AMDApplicationViews.dll
AMDBackendUtils.dll              AMDBaseUtils.dll
AMDCountModeProfilingTranslation.dll
AMDCpuCallstackSampling.dll      AMDCpuPerfEventUtils.dll
AMDCpuProfilingControl.dll       AMDCpuProfilingRawData.dll
AMDCpuProfilingTranslation.dll   AMDExecutableFormat.dll
AMDPowerProfileAppAnalysis.dll   AMDProfileCommon.dll
AMDProfileDataAccessor.dll       AMDProfilerDAL.dll
AMDProfilingAgentsData.dll       AMDProfilingDataWriter.dll
AMDRestApiUtils.dll              AMDSharedUtils.dll
AMDSysUtils.dll                  AMDTaskInfo.dll
AMDThreadProfileAPI.dll          AMDTranslateUtils.dll
CXLOSWrappers.dll
```

The compared vendor roots all directly import CXL. M1 does not; its CXL edge is
only reached through `AMDSysUtils`. Actual CXL import symbol counts and
categories were:

| Importer | Actual symbol count | Observed category |
| --- | ---: | --- |
| `AMDuProfCLI.exe` | 50 | `gtString`/`gtASCIIString` conversion and comparison |
| `AMDuProf.exe` | 8 | string construction, comparison, and UTF-8 conversion |
| `AMDProfilerService.exe` | 34 | string/tokenizer/path parsing |
| `AMDSysUtils.dll` | 4 | string/ASCII-string construction/assignment |
| `CXLOSWrappers.dll` | 92 | path, application identity, assertion, and string helpers |
| `AMDBaseUtils.dll` | 14 | string/path conversion |
| `AMDSharedUtils.dll` | 61 | string/tokenizer/path and filesystem helpers |
| `AMDProfileCommon.dll` | 72 | string/tokenizer and profile path helpers |
| `AMDBackendUtils.dll` | 78 | string/tokenizer, path and profile helpers |

The named symbols do not include a power-profile entry point. They show that
vendor roots bring a much wider common runtime around CXL than M1, but they do
not identify the private branch behind the historical `FatalExit(0xFFFFFFFF)`.

## DELAY IMPORTS

V1, V2, V3, M1, and the directly relevant AMD DLLs inspected all report a
zero-sized Delay Import Directory.

```text
AMD_DELAY_IMPORT_PATTERN = NONE_OBSERVED
IMPORT_TOPOLOGY_DIFFERENCE = PRESENT
```

## TLS / ENTRYPOINT

The executable TLS directories and callback arrays were inspected with
`dumpbin /all`:

| Image | Entry RVA | TLS directory | TLS callback result |
| --- | ---: | ---: | --- |
| V1 `AMDuProfCLI.exe` | `0xA6584` | `0xC0380`, size `0x28` | array terminated by null; `NONE_OBSERVED` |
| V2 `AMDuProf.exe` | `0x10ACC` | `0x19800`, size `0x28` | array terminated by null; `NONE_OBSERVED` |
| V3 `AMDProfilerService.exe` | `0x6D4E4` | `0x7DC00`, size `0x28` | array terminated by null; `NONE_OBSERVED` |
| M1 static fixture | `0x14A88` (`mainCRTStartup`) | `0x1BA80`, size `0x28` | one callback at `0x140006B90` (RVA `0x6B90`) |

Relevant vendor DLL entrypoints were CXL `0x10FC4`, `AMDSysUtils` `0x1AF8C`,
`AMDPowerProfileAPI` `0x2BC7C`, `CXLOSWrappers` `0x2DBD0`, `AMDProfileCommon`
`0xFF5F0`, and `AMDBackendUtils` `0xAD4DC`. The API DLL has no TLS directory;
the other listed vendor DLL callback arrays were null-terminated with no
callback observed. The M1 callback is a fixture/Rust-runtime difference; its
purpose was not inferred.

```text
EXE_TLS_CALLBACKS = PRESENT_ONLY_IN_M1_FIXTURE
```

TLS-directory presence and entrypoint RVA are structural evidence only. No
runtime callback or DllMain order was inferred.

## PE LOAD CONFIG / MITIGATIONS

V1, V2, V3, and M1 all reported:

```text
DLL_CHARACTERISTICS = 0x8160
  High Entropy VA, Dynamic base, NX compatible, Terminal Server Aware
RELOCATION_SECTION = PRESENT
LOAD_CONFIG_SIZE = 0x140
GUARD_FLAGS = 0x00000100 (CF instrumented)
SECURITY_COOKIE = PRESENT
CODE_INTEGRITY_FLAGS = 0
GUARD_EH_CONTINUATION_TABLE = 0 entries
DELAY_IMPORT_DIRECTORY = 0
```

No material ASLR, NX/DEP, CFG, relocation, or load-config difference was
observed between the executable roots. The available `dumpbin` output did not
expose a separate CET distinction; no CET claim is made. These common settings
are not immediate candidates.

## MANIFEST / EXECUTION CONTEXT

V1, V2, and V3 contain a minimal embedded manifest with:

```xml
<requestedExecutionLevel level="asInvoker" uiAccess="false" />
```

No compatibility, assembly dependency, DPI, long-path, alternate code-page,
or unusual activation section was observed. M1 has no resource section and no
embedded manifest.

```text
MANIFEST_CONTEXT_DIFFERENCE = PRESENT_BUT_NO_ELEVATION_REQUEST
```

This is an activation-context difference, not evidence that the vendor image
automatically elevates.

## SUBSYSTEM / RUNTIME MODEL

- V1 and M1 are Windows CUI images; V2 and V3 are Windows GUI images.
- V1/V2/V3 import the vendor MSVC runtime family (`MSVCP140`, `VCRUNTIME140`,
  and UCRT API sets). V2 additionally imports Qt5 Widgets/Core/Gui.
- M1 is a Rust/MSVC release image with `mainCRTStartup`, `VCRUNTIME140`, and
  UCRT API sets; it has no Qt or vendor bootstrap DLLs.
- M1's committed source retains the official
  `AMDTPwrProfileInitialize` import through a `#[used]` function-pointer anchor
  and a volatile read, but never calls the function. It has no AMD API call in
  `main`.

The common CUI subsystem of V1 and M1 means GUI-vs-console is not a necessary
vendor property. The Rust TLS/runtime and vendor MSVC/bootstrap differences can
affect pre-main or shutdown behavior, but static evidence does not select one.

## PROCESS-IDENTITY AND CONFIGURATION SURFACES

Static imports and readable strings show a real possible identity/configuration
surface:

- `CXLBaseTools.dll` imports `GetModuleFileNameW`, `GetModuleHandleW`,
  `GetCurrentProcessId`, `RegOpenKeyW`, and `RegQueryValueExW`, and contains
  `SOFTWARE\\WOW6432Node\\AMD\\AMDProfiler` and `InstallationPath` strings.
- `CXLOSWrappers.dll` imports `QueryFullProcessImageNameW`,
  `GetModuleFileNameW`, `GetModuleHandleExW`, environment-variable and
  current-directory APIs, file-version APIs, registry APIs, and path/file APIs.
  It also provides current application name/path and DLL-directory helpers.
- `AMDBaseUtils.dll` imports `LoadLibraryExA`, `GetProcAddress`, and path APIs,
  and contains `AMDUPROF_ENABLE_CORE_BIND`,
  `AMDUPROF_MAX_DIRSEARCH_DEPTH`, and
  `AMDUPROF_RECURSIVE_DIRSEARCH_TIMEOUT`.
- `AMDProfileCommon.dll` and `AMDBackendUtils.dll` contain application-path,
  DLL-directory, environment-expansion, profile-session, JSON/XML, and
  `AMDUPROF_*` configuration surfaces, including output/session/timer/log
  variables.
- `AMDPowerProfileAPI.dll` imports service-manager APIs and contains driver
  device/configuration strings, but V1 and M1 both reach this same API module;
  that fact alone does not explain their different root graphs.

No `WinVerifyTrust`, `WinVerify`, `CryptCAT`, or equivalent Authenticode
verification import was observed in the bounded `bin\\*.exe`/`*.dll` scan.
`GetFileVersionInfo*` was observed, but that is not signature verification.

```text
PROCESS_IDENTITY_DEPENDENCE = PLAUSIBLE
SIGNATURE_DEPENDENCE_STATIC_SUPPORT = NONE_OBSERVED
```

This does not prove a basename check, signer check, or that M1 fails because it
is unsigned.

## VENDOR BOOTSTRAP CANDIDATES

1. `CXLOSWrappers.dll` — directly imported by V1/V2/V3, directly imports CXL,
   and carries application path/name, environment, registry, version, file,
   and module-loading surfaces. Version 5.3.521.0; SHA-256
   `B649EB9227F9DEA7596C1AC269E3CAF8935CE5AA8DF376E94855839987BBBB0B`.
2. `AMDBaseUtils.dll` — directly imported by V1/V2/V3 and directly imports
   CXL; it supplies path, `LoadLibraryExA`/`GetProcAddress`, logging/threading,
   and AMD environment-variable surfaces. Version 5.3.521.0; SHA-256
   `07562078FA84109EDF266B32EDE48826A499C7BF26BECA95571F4770122CE955`.
3. `AMDSharedUtils.dll` — directly imported by V1/V2/V3 and directly imports
   CXL; its path/filesystem/archive/user-app-data surface can establish shared
   state. No version resource was reported; SHA-256
   `42482006ACD752A6A8BFB06F02C278216F7BFA64D529F46BE1D2D2C64502F055`.
4. `AMDProfileCommon.dll` / `AMDBackendUtils.dll` — directly present in V1
   and V3, with broad profile/configuration/path graphs and transitive
   API/SysUtils reachability. They are not shared direct imports of V2, so they
   are a secondary family.
5. `AMDRestApiUtils.dll` and V2 application modules — direct in V2/V3 and part
   of the GUI/service graph; lower priority because they are not common to V1,
   V2, and V3.

## HYPOTHESIS RANKING

```text
VENDOR_IMPORT_TOPOLOGY_HYPOTHESIS = STRONG
VENDOR_PROCESS_IDENTITY_HYPOTHESIS = PLAUSIBLE
```

The topology rating is strong only as a structural hypothesis: vendor roots
directly import CXL, common AMD modules also directly import CXL, and M1 reaches
CXL only through the public API chain. It is not runtime proof of initialization
order or causality. Identity/path discovery is kept independent and is only
plausible from imports and strings.

## MINIMAL DIFFERENCE SET

The useful candidate set is reduced to five dimensions:

### P1 — direct CXL and multi-parent import topology

- Evidence: V1/V2/V3 have a direct CXL edge plus multiple local CXL parents;
  M1 has only `M1 -> API -> SysUtils -> CXL`.
- Why it could matter: the loader processes a materially different dependency
  DAG and vendor bootstrap dependencies can be present at process start.
- Confidence: strong structural evidence; causal mechanism unproven.
- Cheapest safe discriminator: native, non-debugger, no-op startup observation
  of existing signed V2 against M1 under the same CWD/environment, using raw
  process/exit evidence only.

### P2 — common vendor bootstrap set

- Evidence: `CXLOSWrappers`, `AMDBaseUtils`, and `AMDSharedUtils` are direct
  imports of all three vendor roots and each directly imports CXL; M1 imports
  none of them.
- Why it could matter: these modules provide application-path, environment,
  dynamic-module, filesystem, registry, and shared-state surfaces absent from
  M1.
- Confidence: strong candidate family; individual required module unproven.
- Cheapest safe discriminator: the same V2 no-op startup with a read-only
  module inventory; do not preload or permute DLLs.

### P3 — CXL/application identity and install-context discovery

- Evidence: CXL/wrappers import process-image, current-directory, registry,
  environment, version, and application-name helpers and contain the AMD
  profiler installation key/string.
- Why it could matter: vendor code may resolve configuration/resources from
  executable or installation context after mapping CXL.
- Confidence: plausible, not confirmed; no basename check was found.
- Cheapest safe discriminator: a bounded native comparison recording image
  path/CWD/environment; isolating image-name effects would require a separately
  authorized design and must not copy vendor DLLs.

### P4 — vendor bootstrap/runtime versus M1 Rust/TLS runtime

- Evidence: vendor roots have manifests and no executable TLS callback observed;
  M1 has one TLS callback, no resource manifest, and no vendor bootstrap DLLs.
- Why it could matter: pre-main TLS/CRT/activation-context or shutdown ordering
  may differ before a marker is observable.
- Confidence: plausible but weakly isolated by static evidence.
- Cheapest safe discriminator: short native no-op startup comparison without a
  debugger; do not map an exit difference to a specific TLS/CRT cause.

### P5 — vendor-specific profile graph

- Evidence: V1/V3 directly import `AMDProfileCommon`, `AMDBackendUtils`,
  profile/DAL modules; V2 uses the application/REST family; M1 has none.
- Why it could matter: extra graph components may establish profile/session/
  configuration state before public API use.
- Confidence: plausible, but not common to every vendor root.
- Cheapest safe discriminator: compare V2 first; only if useful should a later
  V1/V3 comparison be considered.

## DEPRIORITIZED DIFFERENCES

- file size, PE timestamp, icon/resource volume, and version-resource text;
- GUI versus CUI by itself (V1 and M1 are both CUI);
- Qt imports, because they are V2-specific;
- AMD signature status alone; no static Authenticode-verification API was
  observed, so the unsigned fixture is not proven causal;
- ASLR, NX/DEP, CFG, relocation, and load-config differences, because the
  compared executable roots share the observed settings;
- TLS-directory presence without a callback/call-site observation;
- delay imports, because no AMD delay-import distinction was found.

## STATIC IMPORT SURFACE DECISION

```text
STATIC_IMPORT_SURFACE_DECISION = EXISTING_VENDOR_CONTROL
BEST_VENDOR_STATIC_IMPORT_CONTROL = D:\apps\AMDuProf\bin\AMDuProf.exe
```

`AMDuProf.exe` is the smallest installed x64 AMD-signed executable found that
directly imports `CXLBaseTools.dll` (427,416 bytes). The smaller process-enum
and load-service helpers do not directly import CXL. `AMDProfilerService.exe`
and `AMDuProfCLI.exe` are valid additional vendor roots but have more context.

No supported public CXL link surface exists, so a new fixture that directly
links CXL is not cleanly buildable from the installed SDK.

## RECOMMENDED NEXT EXPERIMENT

```text
RECOMMENDED_NEXT_EXPERIMENT = CPU-SENSOR-AMD EXISTING VENDOR EXECUTABLE NO-OP STARTUP CONTROL
```

Use the existing signed V2 `AMDuProf.exe` as a native, non-debugger startup
control with no profiling operation. Keep the installed root, AMD-bin CWD,
inherited environment, and manually authorized token aligned with the previous
comparison. Capture only raw process/exit/module evidence and bound the owned
process lifetime. Do not invent undocumented flags, preload DLLs, alter the
installation, or use CDB as the first instrument. If the GUI has no documented
non-profile exit path, stop at that boundary rather than guessing a command or
killing unrelated processes.

This is one existing-vendor-control experiment family, not a new static CXL
fixture. It can test whether the vendor executable context family survives
native startup, but it cannot by itself prove the private CXL condition.

## PRODUCT IMPLICATION

The audit does not approve package power, per-identity frequency, temperature,
or any Resource Timeline provider. No collector, ProviderHost, CollectionPlan,
MetricCatalog, schema, dashboard, or UI was changed.

## REJECTED APPROACHES

No CXL `.def`, fabricated import library, export-derived import stub, PE rewrite,
random AMD DLL preload, hook/injection, vendor patch, debugger-first experiment,
or AMD executable launch was performed. Such methods would test a repository-
constructed artifact rather than a vendor-supported static-import surface.

## VALIDATION

- Hashes, file versions, signatures, PE machine/subsystem/timestamp, import
  descriptors, delay-import directories, TLS directories/callback arrays,
  manifests, and load-config output were cross-checked with installed
  `dumpbin.exe`, `mt.exe`, and PowerShell read-only inspection.
- The installed import-library/sample files and committed M1 source/build
  identity were cross-checked.
- The bounded dependency graph was checked against direct PE imports.
- `git diff --check` passed and the working tree was checked for unrelated
  changes before delivery.

No full Rust suite was needed because no source code changed.

## DELIVERY

This is a documentation-only result committed on the existing
`spike/cpu-sensor-amd-uprof-live-qualification` branch and pushed normally
without force-push. No AMD DLL, driver, installer, header, PDF, sample,
license, dump, or machine-specific runtime artifact is committed.

```text
PRODUCTION_PROVIDER = NO
METRIC_CATALOG_CHANGED = NO
SCHEMA_CHANGED = NO
UI_CHANGED = NO
SYSTEM_MUTATIONS = NONE
```

## NEXT STEP

Do not start `CPU-SENSOR-AMD-PROVIDER-DESIGN`. If separately authorized, perform
only the bounded native V2 no-op startup control above; keep the public API/CXL
failure as an unresolved vendor-context/runtime question.
