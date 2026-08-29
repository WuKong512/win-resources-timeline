# CPU-SENSOR-AMD CLI VS DIRECT API DIVERGENCE

This record consumes the already-generated Administrator comparison and performs a read-only static and artifact investigation. It does not rerun AMD uProf, does not run Resource Timeline sampling, and does not change the installation or Windows configuration.

```text
RESULT: PASS_WITH_UNRESOLVED_VENDOR_INTERNAL
CLI_RUNTIME_USES_PUBLIC_POWER_API_PATH: YES
CLI_LOADS_AMDPOWERPROFILEAPI: YES
CLI_LOADS_CXLBASETOOLS: YES
CXL_FATAL_EXIT: CONTEXT_DEPENDENT
DIRECT_API_VS_VENDOR_CLI: DIVERGENT
```

The result is a pass for this divergence investigation only. It is not a live-source pass, a production-provider approval, or permission to begin `CPU-SENSOR-AMD-PROVIDER-DESIGN`.

## BASELINE

- Repository: `WuKong512/win-resources-timeline`.
- Base commit: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- Start head: `68c2b67bdfa575b17add62d03a10f85c03bfa71f` (`docs: record AMD uProf administrator comparison`).
- `origin/main`: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- Branch: `spike/cpu-sensor-amd-uprof-live-qualification`.
- Entry worktree: clean; the qualification branch was already attached to the current worktree.
- `git merge-base --is-ancestor` for the Q1 merge commit: `PASS`.
- Drift: none relative to `origin/main`; the qualification branch is intentionally ahead by its existing qualification commits.
- Duplicate-task gate: `PASS` for the existing qualification branch; no competing local/remote branch was found. `gh` was unavailable, so open-PR visibility is not claimed.
- Historical blocked records are preserved in `cpu-sensor-amd-uprof-live-qualification.md` and `cpu-sensor-amd-uprof-load-abort-followup.md`.

## AUTHORITATIVE PRIOR EVIDENCE

The historical live qualification remains blocked with zero Resource Timeline API samples. The preceding root-cause record established:

- `ROOT_CAUSE = DEPENDENCY_LOAD_FAILURE`.
- `SUBCAUSE = CXLBASETOOLS_LOAD_PATH_FATAL_EXIT`.
- `EXACT_CXL_INTERNAL_CONDITION = UNPROVEN`.
- Direct `CXLBaseTools.dll` loading terminates the isolated process with signed `-1` / `0xFFFFFFFF`; prior CDB evidence caught `KERNEL32!FatalExit(0xFFFFFFFF)`.
- Direct `AMDPowerProfileAPI.dll` loading reaches the transitive `AMDSysUtils.dll -> CXLBaseTools.dll` path and terminates before API initialization.
- The same direct load boundary persisted under the Administrator comparison.
- The official Administrator CLI completed a short power timechart and produced real output, while its earlier non-admin initialization reached `0x80070005 / AMDT_ERROR_ACCESSDENIED`.

This investigation refines the CLI/direct-API relationship without rewriting any of those observations.

## STATIC CLI DEPENDENCY GRAPH

The analysis used the installed files under `D:\apps\AMDuProf\bin` and a bounded recursive graph of AMD-local PE imports. No new tool was installed. `AMDuProfCLI.exe`, `AMDPowerProfileAPI.dll`, `AMDSysUtils.dll`, and `CXLBaseTools.dll` have no observed delay-import directory and are x64 PE images (`machine 0x8664`).

### CLI image

| Item | Evidence |
|---|---|
| Image | `D:\apps\AMDuProf\bin\AMDuProfCLI.exe` |
| File/product version | `5.3.521.0` |
| SHA-256 | `D0812D64963DD98F7C339CAC72F650461F95FF84E757A99767C7981B4111FBAC` |
| Authenticode | Valid; Advanced Micro Devices signer |
| Subsystem | Console (`3`) |
| Delay imports | Not observed |

Its direct AMD-local imports are:

```text
CXLBaseTools.dll
CXLOSWrappers.dll
AMDBaseUtils.dll
AMDProfileCommon.dll
AMDSharedUtils.dll
AMDBackendUtils.dll
AMDCpuPerfEventUtils.dll
AMDProfileDataAccessor.dll
AMDPowerProfileAPI.dll
AMDProfilerDAL.dll
```

The other direct imports are `KERNEL32.dll`, Microsoft CRT/API-set libraries, `MSVCP140.dll`, `VCRUNTIME140.dll`, and `VCRUNTIME140_1.dll`.

### Relevant AMD edges

| Image | Direct relevant edges | Relevant imported API evidence |
|---|---|---|
| `AMDuProfCLI.exe` | `AMDPowerProfileAPI.dll`, `CXLBaseTools.dll`, `AMDProfileCommon.dll`, `AMDBackendUtils.dll`, and other AMD profile modules | Direct imports from `AMDPowerProfileAPI.dll` include `AMDTPwrGetSupportedCounters`, `AMDTPwrGetCategoryInfo`, `AMDTPwrGetUnitInfo`, `AMDTPwrReadCumulativeCounter`, and `AMDTPwrReadCounterHistogram`; `AMDProfileCommon.dll` supplies the profile manager/time-line calls used by the CLI |
| `AMDProfileCommon.dll` | `AMDPowerProfileAPI.dll`, `AMDSysUtils.dll`, `CXLBaseTools.dll`, `AMDPowerProfileAppAnalysis.dll`, and companion modules | Imports `AMDTPwrProfileInitialize`, `AMDTPwrStartProfiling`, `AMDTPwrReadAllEnabledCounters`, `AMDTPwrStopProfiling`, `AMDTPwrProfileClose`, timer and topology functions; also has `LivePowerDataProduced` and time-line collection symbols |
| `AMDBackendUtils.dll` | `AMDPowerProfileAPI.dll`, `AMDSysUtils.dll`, `CXLBaseTools.dll` | Imports `AMDTPwrProfileInitialize`, `AMDTPwrGetCategoryInfo`, `AMDTPwrGetDeviceTreeCounters`, `AMDTPwrGetSystemTopology`, and related metadata functions |
| `AMDPowerProfileAppAnalysis.dll` | `AMDPowerProfileAPI.dll`, `CXLBaseTools.dll`, and AMD support modules | Imports `AMDTPwrStartProfiling`, `AMDTPwrStopProfiling`, `AMDTPwrGetTimerSamplingPeriod`, `AMDTPwrGetTargetSystemInfo`, `PowerTrace`, and `Pwr*` functions |
| `AMDPowerProfileAPI.dll` | `AMDSysUtils.dll` | Exports the public `AMDTPwrProfileInitialize`, enumeration, enable, timer, start/read/stop, close, and power-driver functions |
| `AMDSysUtils.dll` | `CXLBaseTools.dll` | Direct transitive edge in the failing public API chain |
| `CXLOSWrappers.dll` and several AMD support DLLs | `CXLBaseTools.dll` | Shared AMD base-layer dependency; not an independently proven power backend |

Relevant image identities from the same install are coherent at the user-mode file level:

| Image | Version | SHA-256 |
|---|---:|---|
| `AMDPowerProfileAPI.dll` | `5.3.521.0` | `9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277` |
| `AMDSysUtils.dll` | `5.3.521.0` | `3BCD209D8B2AF3EA45098400A0F95A7D25D9EA659F27A57624C95E5C84570839` |
| `CXLBaseTools.dll` | `5.3.521.0` | `4815D4631BCA9C051DC4293538DF8D402BD848E705228F497DF718EDCA1F8931` |
| `AMDProfileCommon.dll` | `5.3.521.0` | `FC198B1CE35481C1EF438B1D0D984A950975643F78811D182A87ABF225E49764` |
| `AMDPowerProfileAppAnalysis.dll` | `5.3.521.0` | `ADB766D3287C3172F6477B7951B42E00614F8F25015F98DF59F9CAB2AB0998CE` |
| `AMDBackendUtils.dll` | version metadata absent | `89ABDE1BB551AB030D298082FFEF7CC74B084D508A43087D50226F3DF7A29C28` |

The direct-import evidence is stronger than a string match: a successful process cannot resolve a non-delay direct import without loading the referenced module. The bounded duplicate search found these named modules only under the D: installation; the checked standard Program Files AMD roots contained no second copy.

## REFERENCE SEARCH

A bounded ASCII and UTF-16 search of the files directly under `D:\apps\AMDuProf\bin` found the following supporting references:

- `AMDuProfCLI.exe`: `AMDPowerProfileAPI`, `AMDTPwrGetSupportedCounters`, `CXLBaseTools`, `AMDProfileCommon`, `LivePowerDataProduced`, and `timechart`.
- `AMDProfileCommon.dll`: `AMDTPwrProfileInitialize`, `AMDPowerProfileAPI`, `AMDSysUtils`, `CXLBaseTools`, `AMDPowerProfileAppAnalysis`, and `LivePowerDataProduced`.
- `AMDBackendUtils.dll`: `AMDTPwrProfileInitialize`, `AMDPowerProfileAPI`, `AMDSysUtils`, `CXLBaseTools`, and `PowerProfiler`.
- `AMDPowerProfileAppAnalysis.dll`: `AMDPowerProfileAPI` and `CXLBaseTools`.
- `AMDSysUtils.dll`: `CXLBaseTools`.

These strings support the import graph and identify likely ownership, but strings alone are not treated as call semantics.

## EXISTING CLI SESSION EVIDENCE

Evidence directory:

```text
C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-admin-comparison-v2-20260829T031818104Z
```

The existing Administrator D2 record is complete, not timed out, and has empty stderr. It ran:

```text
D:\apps\AMDuProf\bin\AMDuProfCLI.exe timechart --event power --interval 1000 --duration 5 --output-dir <evidence>\ADMIN-D2-power-output
```

It exited `0` / `0x00000000` after `5,446.921 ms`, reported profiling start and finish, and created:

| Artifact | Size | SHA-256 | Readable result |
|---|---:|---|---|
| `timechart.csv` | 1,349 bytes | `D18CCC886AFC1AB40EACF7A8183BCFD3DA1D14EFA8ED21050909069191EF8BC5` | 3 records; package power `49.44`, `42.26`, `40.29 W`; package counter ID `48`; core power IDs `50`–`64` |
| `session.uprof` | 10,883 bytes | `82A785C4A0BC18AFDFBAF1D89BA18229017CB7276A02FE064E20FBA017E5877F` | JSON session metadata; `categoriesCollected=["livePower"]`, 1000 ms interval, `profileType=64` |

The session metadata identifies the live-power category and the package/core descriptors, but does not name a backend DLL. It records `D:\apps\AMDuProf\bin` as the working directory, `AMD Ryzen 7 9700X 8-Core Processor`, and `hypervisorEnabled=true`. The CSV is vendor CLI evidence only; it is not a Resource Timeline API sample or production metric admission.

## CLI RUNTIME MODULE GRAPH

No event-level module-load trace was present in the supplied Administrator directory. The conclusion below therefore uses the stronger available PE/runtime implication and the earlier saved CLI debugger evidence:

1. `AMDuProfCLI.exe` has non-delay direct imports of both `AMDPowerProfileAPI.dll` and `CXLBaseTools.dll`.
2. A successful CLI process that exits normally after `timechart --event power` must have resolved those direct imports.
3. `AMDPowerProfileAPI.dll` directly imports `AMDSysUtils.dll`, which directly imports `CXLBaseTools.dll`; the CLI graph also contains the direct CXL edge.
4. The prior saved CDB observation of the non-admin CLI reached `AMDTPwrProfileInitialize(0)` and returned `0x80070005 / AMDT_ERROR_ACCESSDENIED`. Thus this is not merely a static name coincidence: the CLI timechart initialization path reached a public API entry point.

Accordingly:

| Question | Result | Evidence boundary |
|---|---|---|
| CLI loads `AMDPowerProfileAPI.dll` | `YES` | Non-delay direct import plus successful CLI process; prior CLI debugger reached `AMDTPwrProfileInitialize(0)` |
| CLI loads `AMDSysUtils.dll` | `YES` | Required direct dependency of the loaded API and imported by `AMDProfileCommon.dll` |
| CLI loads `CXLBaseTools.dll` | `YES` | Non-delay direct CLI import and transitive API/support-module dependency; successful CLI process |
| CLI uses the public power API path | `YES` | Static public API imports plus saved runtime `AMDTPwrProfileInitialize(0)` observation |
| Event timestamps/order and child/helper module graph | `INCONCLUSIVE` | Not captured by the supplied evidence; no claim of a particular load order or helper process |

`CLI_RUNTIME_USES_PUBLIC_POWER_API_PATH = YES` means the successful CLI path uses the public API/CXL module path. It does not mean the direct minimal probe and CLI establish identical process state or that the public API is safe for Resource Timeline.

## PUBLIC API PATH USED BY CLI

The evidence disproves the hypothesis that the CLI is simply avoiding the public API. The CLI imports the public API directly and through its profile-common/backend components, and the prior CLI debugger run reached `AMDTPwrProfileInitialize(0)`. The Administrator D2 run then completed through that installed CLI path.

The evidence does not identify whether the CLI calls the public API directly from its command handler or through `AMDProfileCommon`/`AMDPowerProfileAppAnalysis`; those are implementation details of AMD's binary. It also does not establish that the CLI's initialization sequence is safe to reproduce by preloading undocumented modules.

`LIKELY_CLI_POWER_BACKEND_COMPONENTS` (medium-to-high confidence as CLI-side components, not a separate sensor source) are:

- `AMDProfileCommon.dll`, whose imported symbols include the time-line collection and public power lifecycle calls.
- `AMDBackendUtils.dll`, which supplies live counter/device metadata and imports public power API functions.
- `AMDPowerProfileAppAnalysis.dll`, which imports public power start/stop, power trace, driver, and shared-memory functions.
- `AMDProfileDataAccessor.dll` and `AMDProfilerDAL.dll`, which are session/data infrastructure in the same graph.

No alternative undocumented backend was proven. These modules form the CLI's larger frontend/session/backend graph around the same public API.

## LOADER ORDER COMPARISON

The static import directory lists `CXLBaseTools.dll` early among the CLI's AMD imports, followed by `CXLOSWrappers.dll`, profile/common/backend modules, and `AMDPowerProfileAPI.dll`; however, PE import-table order is not a reliable record of runtime load order. No event trace was supplied, so:

- `CLI_PRE_CXL_PREREQUISITE_CANDIDATES = NOT_PROVEN`.
- The larger CLI module set is a context difference, not proof that any named module ran before CXL.
- The direct probe loads one explicit absolute target with `LoadLibraryExW` flags `0x00000900` (`LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32`) and does not resolve or invoke AMD exports in the load-only child.
- The CLI runs from `D:\apps\AMDuProf\bin` and has the complete AMD import graph, whereas the direct child has only its own process/runtime state plus the requested target's dependency load.

No preload combination, import hook, patch, loader snap, or environment mutation was attempted.

## PROCESS CONTEXT DIFF

| Context dimension | Direct A/B/E probe | Successful CLI control | Status |
|---|---|---|---|
| Architecture | x64 | x64 | Same |
| Token | Administrator for A/B/E comparison; non-admin historical direct run | Administrator for D2; non-admin historical CLI reached access denied | Elevation changes CLI permission outcome but not direct CXL abort |
| Entry point | Repository `metric-probe.exe` diagnostic child | AMD `AMDuProfCLI.exe` | Different |
| Working directory | `C:\Users\Hello\.codex\worktrees\08bd\resource-timeline` | `D:\apps\AMDuProf\bin` | Different; not silently normalized |
| DLL request | Explicit canonical absolute path, safe `LoadLibraryExW`, flags `0x900` | CLI's PE imports resolved by normal process loader | Different loading context |
| AMD module set | Minimal target/dependency path | Full CLI AMD profile/common/backend graph | Different |
| PATH | Current environment includes `D:\apps\AMDuProf\bin`; probe does not use arbitrary PATH selection | Same machine environment was not fully snapshotted in D2; CLI working directory and application directory are known | Full equality not proven |
| AMD-specific environment | `AMDPROFILERPATH=D:\apps\AMDuProf\bin` was observable in the inspection environment | No per-process D2 environment snapshot in supplied evidence | Inconclusive |
| Parent/process tree | Capture harness -> metric-probe child | PowerShell -> AMDuProfCLI; child/helper PIDs not recorded | Inconclusive |
| Registry/configuration | No mutation; no targeted configuration read used to explain the result | No mutation; CLI-readable configuration not enumerated by this task | Inconclusive |

The bounded installation search found no duplicate copy of the named AMD DLLs under the checked roots. That reduces, but does not mathematically eliminate, an unobserved loader-context substitution hypothesis.

## CXL FATAL EXIT CALLSITE

The existing direct-child CDB record remains the callsite evidence:

```text
KERNEL32!FatalExit(0xFFFFFFFF)
CXLBaseTools!gtString::asWideString+0x458
CXLBaseTools!gtStringTokenizer::getNextToken+0x2f96
ntdll!LdrLoadDll
KERNELBASE!LoadLibraryExW
<metric-probe load-only child>
```

Stable observed vendor frames are therefore:

```text
CXL_FATAL_EXIT_CALLSITE =
  CXLBaseTools!gtString::asWideString+0x458 /
  CXLBaseTools!gtStringTokenizer::getNextToken+0x2f96
```

The existing trace did not preserve a private AMD symbol for the exact call instruction or the internal condition. ADMIN-A did not attach a debugger, so reproduction of the exact offset under Administrator is not claimed. `EXACT_CXL_INTERNAL_CONDITION = UNPROVEN` remains unchanged.

## OFFICIAL SAMPLE BUILD FIDELITY

The installed official sample was inspected at:

```text
D:\apps\AMDuProf\Examples\CollectAllCounters\CollectAllCounters.cpp
D:\apps\AMDuProf\Examples\CollectAllCounters\CollectAllCounters.sln
D:\apps\AMDuProf\Examples\CollectAllCounters\CollectAllCounters.vcxproj
```

The source is the vendor sample and visibly contains the documented public sequence: initialize, `AMDTPwrGetSupportedCounters`, enable counters, set a 1000 ms period, start, read, stop, and close. The project selects x64 and `v140`, links `AMDPowerProfileAPI.lib`, and the built artifact is x64 with a direct `AMDPowerProfileAPI.dll` import and the expected public symbols (`AMDTPwrProfileInitialize`, enumeration, enable, start/read/stop, and close).

The fidelity result is:

```text
OFFICIAL_SAMPLE_BUILD_FIDELITY = INCONCLUSIVE
```

The source semantics, x64 architecture, and public API import are confirmed. Exact compiler command, import-library provenance, and build environment were not retained. The installed project file still names the default `C:\Program Files\AMD\AMDuProf` include/bin paths while the actual installation is `D:\apps\AMDuProf`; the prior record says the x64 sample was built with the already-installed `C:\BuildTools` toolchain against the installed include/library, but there is no saved build log. This is a reproducibility bookkeeping limitation, not evidence that the sample used a different API.

The Administrator C sample evidence is absent and remains `INCONCLUSIVE`; it was not rerun during this evidence-consumption task.

## ROOT CAUSE REFINEMENT

```text
PRIMARY_CLASSIFICATION: DEPENDENCY_LOAD_FAILURE
ROOT_CAUSE: DEPENDENCY_LOAD_FAILURE
SUBCAUSE: CXLBASETOOLS_LOAD_PATH_FATAL_EXIT
EXACT_CXL_INTERNAL_CONDITION: UNPROVEN
CXL_FATAL_EXIT: CONTEXT_DEPENDENT
```

Confidence is high for the dependency boundary and high for the public-API path relationship. The exact CXL condition is medium/low confidence because no private symbol, event-level load order, or internal diagnostic explains why the same installed CXL image terminates the minimal child but not the CLI process.

Decisive evidence:

- Direct CXL load aborts under both non-admin and Administrator tokens.
- Direct API load and init-only probe abort before API initialization through the CXL dependency path.
- `AMDuProfCLI.exe` has non-delay direct imports of the API and CXL, and the successful CLI process therefore loads them.
- Prior saved CLI debugger evidence reached `AMDTPwrProfileInitialize(0)`; Administrator CLI then completed a real power timechart.

Disproven or unsupported primary hypotheses:

- `PROBE_IMPLEMENTATION_DEFECT`: not supported as the primary cause; the direct CXL child reproduces the boundary independently of Resource Timeline API calls.
- `VC_RUNTIME_MISSING_OR_BROKEN`: not supported; Microsoft CRT/system dependencies loaded normally and VC runtime status was previously `OK`.
- `APPLICATION_CONTROL_BLOCK`: no blocking security evidence was found in the prior audit.
- Elevation as a direct CXL remediation: disproven by ADMIN-A, ADMIN-B, and ADMIN-E.
- Hyper-V/VBS/HVCI as the established cause: not proven; all remain unchanged.
- “CLI avoids the public API/CXL path”: disproven by the static graph and saved CLI initialization observation.

Remaining hypotheses:

- A private CXL initialization condition that depends on the larger CLI module/process context or initialization order.
- A configuration, environment, parent-process, or loader-search difference not captured in the existing D2 evidence.
- Public API/driver component-coherence issues, still unproven and not tested by installation mutation.

The primary classification is not changed to `VENDOR_DLL_INITIALIZATION_ABORT` because the requested root-cause taxonomy treats the proven first failing boundary as the dependency-load failure; the vendor `FatalExit` is its observed mechanism.

## PRODUCT IMPLICATIONS

No product architecture was changed. Future candidates, not decisions, are:

- documented `AMDPowerProfileAPI`;
- external `AMDuProfCLI` execution;
- another documented AMD interface;
- deferring the AMD source.

The CLI's successful short run does not approve an external CLI provider. A future design would separately need to qualify process lifetime, startup/steady-state overhead, cadence, output parsing, privilege, busy behavior, failure isolation, install dependency, and license terms. None of that is part of this task.

The current metric decisions remain:

| Metric | Decision |
|---|---|
| `CPU_PACKAGE_POWER` | `DEFER`; CLI values are control evidence only, not Resource Timeline API qualification |
| `AMD_CORE_EFFECTIVE_FREQUENCY` | `DEFER`; CLI lists Frequency on Thread, but no direct API descriptor/sample evidence exists |
| `CPU_EFFECTIVE_FREQUENCY` | `DEFER_AGGREGATION_CONTRACT` |
| `CPU_PACKAGE_TEMPERATURE` | `DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD`; the CLI list's lack of Temperature is not treated as proof of public API unsupported status |

Distribution remains `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`; the only candidate deployment model remains `EXTERNAL_INSTALLED_DEPENDENCY`.

## DEFERRED QUESTIONS

- Exact runtime load timestamps/order for the successful Administrator CLI, including any child/helper process modules.
- The minimum CLI-established state that makes CXL loading non-fatal.
- Whether the public API can initialize under Administrator once the direct load boundary is safe.
- Administrator official-sample behavior; the existing ADMIN-C control artifact is missing.
- User-mode/driver version-coherence explanation for the prior `5.3.521` versus `5.3.481.0` metadata split.
- Package-power, per-identity frequency, temperature, cadence, lifecycle, busy, and overhead qualification through Resource Timeline's own API path.

Resolving loader order would require a separately authorized, manually launched Administrator runtime trace. No such trace was run or prepared here because the supplied static/non-delay import evidence answers the primary public-path question.

## EVIDENCE-HARNESS BOOKKEEPING

Two historical capture defects were preserved and classified:

- `PREVIOUS_API_SHA_MISMATCH = HARNESS_METADATA_DEFECT`: the raw inventory's expected API hash omitted the final `7` (`...A427`, 63 characters). The installed file's actual authoritative hash is the full `...A4277` value above, and it matches the requested artifact identity. Historical JSON was not rewritten.
- `ADMIN-C_EVIDENCE = INCONCLUSIVE`: the summary contains a `null` C entry and no C JSON/stdout/stderr file. The saved evidence cannot distinguish command omission, invocation/capture loss, path/call failure, or serialization loss. No vendor result is inferred and no ADMIN-C rerun was performed. No repository harness source for that one-off Administrator script exists in this branch, so no unsubstantiated harness patch was made. A future runner must fail closed when a required test ID or capture file is missing.

## VALIDATION

Only measurement documentation was changed for this record; `tools/metric-probe` and `src-tauri` source were not modified. Validation performed for this evidence-consumption change:

- `git diff --check`: `PASS` after the final documentation patch.
- Required-section and artifact-reference review: performed against the supplied evidence and installed files.
- No Rust suite was repeated because no Rust source changed.

## DELIVERY

- No AMD DLL, driver, installer, header, PDF, sample source/binary, session dump, or sensitive process artifact was committed.
- No AMD/Windows service, driver, registry, PATH, security policy, Hyper-V, VBS, HVCI, or installation mutation was performed.
- The delivery commit and push are recorded in the final report after the documentation change.
- Draft PR visibility remains `BLOCKED_TOOLING` if `gh` is unavailable; this does not change the technical result.

## NEXT STEP

`AUTHORIZED_CLI_VS_DIRECT_API_LOADER_CONTEXT_TRACE` is the narrow next diagnostic if the exact context difference is required. It may use one manually launched Administrator runtime/module trace, without loader snaps, injection, hooks, preload experiments, or system mutation.

Do not start `CPU-SENSOR-AMD-PROVIDER-DESIGN`. Only after the direct API load/init boundary is made safe by evidence may the original ordered live qualification be reconsidered.
