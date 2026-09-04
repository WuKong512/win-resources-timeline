# CPU-SENSOR-AMD CLI VS DIRECT LOADER CONTEXT TRACE

This record consumes the already-generated V6 CDB evidence. It does not rerun
AMDuProfCLI, `metric-probe`, CDB, profiling, or sampling, and it does not alter
AMD or Windows state. The raw CDB logs are authoritative; the generated V6
summary is treated as a convenience index only.

```text
RESULT: DEBUGGER_PERTURBED_SUCCESS_PATH
CLI_DEBUGGER_PERTURBATION: CONFIRMED
CLI_CXL_LOAD_SEMANTICS_OBSERVED: STATIC_PROCESS_START
PROBE_CXL_IMAGE_MAPPING: CONFIRMED
PROBE_ENTRYPOINT_BREAKPOINT_VALIDITY: INVALID_HARNESS_BREAKPOINT_EXPRESSION
PROBE_CXL_PROCESS_ATTACH: NOT_OBSERVED
STATIC_VS_DYNAMIC_LOAD_HYPOTHESIS: STILL_PLAUSIBLE_BUT_UNQUALIFIED
CXL_LOAD_MODE_CORRELATION: UNPROVEN
EXACT_DEBUGGER_PERTURBATION_MECHANISM: UNPROVEN
CXL_CONTEXT_CLASS: INCONCLUSIVE
CXL_FATAL_STAGE: UNKNOWN
CLI_FATAL_EXIT_HIT: true
```

The result means that the V6 run cannot be used as a successful debugger
control: the historically successful non-debugger Administrator CLI path was
perturbed by CDB. It is not a declaration that the AMD source works, nor is it
permission to begin `CPU-SENSOR-AMD-PROVIDER-DESIGN`.

## BASELINE

- Repository: `WuKong512/win-resources-timeline`.
- Base commit: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- Start head: `31d6cd2d0c444a6a0391e21e4e85fab466aca847`.
- `origin/main`: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`.
- Branch: `spike/cpu-sensor-amd-uprof-live-qualification`.
- The qualification branch already existed and was used; no competing branch
  was created. No open-PR inspection tool was available (`gh` was not
  available).
- Entry working tree: clean. Existing measurement records were preserved:
  the load-abort follow-up, the CLI/direct-API divergence record, and the
  loader-context differential record.
- V6 evidence root (local, not committed as an artifact):
  `C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-loader-context-trace-v6-20260831T014752671Z`.

The historical control remains the successful, non-debugger Administrator
command recorded by the preceding divergence investigation. The V6 CLI run is
a separate instrumented attempt and is not silently substituted for that
historical result.

## DEBUGGER IDENTITY

The same Microsoft CDB was used for both controls:

| Field | Value |
| --- | --- |
| Path | `C:\Program Files (x86)\Windows Kits\10\Debuggers\x64\cdb.exe` |
| Version | `10.0.14321.1024` |
| SHA-256 | `5E265CD6C071CA970541AFF8C4FABF212D96C88897744D7B3C71475DCA128A9C` |
| Architecture | x64 |
| Signature | Valid Microsoft signature |
| Debugger switches | `-o -G -hd`; no `-g` |

The V6 commands retained the initial breakpoint, child-process observation,
standard-heap mode, automatic termination evidence, and the requested timeout
headroom. CLI watchdog: 45,000 ms. Direct-probe watchdog: 20,000 ms.

## TRACE VALIDITY

The JSON files and raw stdout/stderr files parse and are present. Empty stderr
files are valid captures, not failures. The decisive validity fields are:

| Control | Capture | Target outcome | Watchdog | Valid successful control? |
| --- | --- | --- | --- | --- |
| CLI | complete | root `CLI_FATAL_EXIT_HIT`; root termination not captured | timeout | No |
| Direct probe | complete | CXL mapped; no valid entrypoint or termination marker | timeout | No |

`CLI-WRAPPER-RESULT.json` records root CDB PID `8172`, signed wrapper exit
`-1`, unsigned `0xFFFFFFFF`, and `timeout=true`. It also records that the
root CLI success classifier was false. `PROBE-WRAPPER-RESULT.json` records
CDB PID `20092`, the same signed/hex wrapper result, and `timeout=true`.

Both wrappers attempted owned-tree termination. The tree-kill overload was
unavailable in the installed PowerShell/.NET surface, so
`kill_tree_succeeded=false` with the recorded `MethodException`; the narrow
fallback `Kill()` then succeeded. This is wrapper cleanup evidence, not a
target-process exit status and not a broad process-name kill.

The CLI command did create non-empty diagnostic output before the trace was
invalidated: `timechart.csv` was 1,442 bytes and `session.uprof` was 10,884
bytes. Their observed SHA-256 values were respectively
`C44A3A8BCD19A920BC040DA0E8F46BE3C346AA2CCBBA8613E565633D0B1F4272` and
`FFA3540C42E0EA6ED9C07255281C2329FE3D6BC7319DBEE23198F1F95CD9E311`.
The CSV contains four package-power records (48.69, 42.50, 46.42, and
41.57 W), but those artifacts do not override the root FatalExit and timeout.
They are diagnostic evidence only, not Resource Timeline metric qualification.

## COMMON PROCESS CONTEXT

The two CDB invocations were launched from the same manually elevated
PowerShell context:

- Administrator proof: username `ODETOMOUNTASEAS\Hello`, PowerShell PID
  `34572`, x64, `whoami.exe /groups` exit 0, Administrator membership true,
  and `S-1-16-12288` present. No self-elevation occurred.
- Debuggee working directory for both controls:
  `D:\apps\AMDuProf\bin`.
- Environment mode: inherited unchanged. The captured context contained
  `AMDPROFILERPATH=D:\apps\AMDuProf\bin`, an inherited PATH already containing
  that directory, and the same TEMP/TMP and x64 process context.
- No persistent PATH, current-directory, registry, service, driver, security,
  hypervisor, or installation mutation was performed.
- Sensitive values from the captured environment are intentionally not
  reproduced in this repository document.

The root debuggee identities were distinct and were taken from `|.` / `~.`
output, not inferred from the CDB prompt:

| Control | Process | PID | Thread | TID |
| --- | --- | --- | --- | --- |
| CLI | `AMDuProfCLI.exe` | `0x2128` | root thread | `0x3240` |
| Probe | `metric_probe.exe` | `0x1010` | root thread | `0x86C4` |

No separate CLI helper process identity was observed in the V6 trace. The
`-o` switch was nevertheless retained so that a helper would have been
visible if created.

## PE TLS / ENTRYPOINT AUDIT

The V6 static initialization table reported no statically observable TLS
callbacks for the targeted modules. The observed PE entrypoint RVAs were:

| Module | AddressOfEntryPoint RVA | TLS callbacks | Relevant role |
| --- | ---: | --- | --- |
| `CXLBaseTools.dll` | `0x10FC4` | none observed | failing direct dependency |
| `AMDSysUtils.dll` | `0x1AF8C` | none observed | AMD utility dependency |
| `AMDPowerProfileAPI.dll` | `0x2BC7C` | none observed | public power API |
| `AMDProfileCommon.dll` | `0xFF5F0` | none observed | CLI profiling layer |
| `AMDBackendUtils.dll` | `0xAD4DC` | none observed | CLI backend layer |

No conclusion about vendor-private initialization semantics is drawn from the
absence of TLS callbacks. The trace only establishes the PE audit result and
the entrypoint addresses used by the V6 harness.

## CLI INITIAL BREAK

`CLI_INITIAL_BREAK` appears at `CLI-TRACE.log` line 80. At that breakpoint,
`|.` identified root process `0x2128` as `AMDuProfCLI.exe` and `~.` identified
thread `0x2128.3240`. The PEB reported the AMD bin current directory and the
exact timechart command line.

The initial `lm` already contained the statically imported AMD graph, including
`CXLBaseTools`, `AMDSysUtils`, `AMDPowerProfileAPI`, `AMDProfileCommon`,
`AMDBackendUtils`, `AMDBaseUtils`, `AMDSharedUtils`, `CXLOSWrappers`,
`AMDCpuPerfEventUtils`, `AMDProfileDataAccessor`, `AMDProfilerDAL`, and the
other CLI profiling modules. This is a mapped-module observation at the
initial breakpoint; it is not proof that their initialization routines have
completed.

## CLI PRE-CXL CONTEXT

The first CXL process-attach marker is at `CLI-TRACE.log` line 318. At that
point CXL was already mapped as part of the CLI's process-start/static import
graph, and the relevant AMD modules were present in the module list. The
entrypoint register capture was:

```text
RCX = 00007ff968200000
RDX = 0000000000000001
R8  = 000000395aeff6d0
RIP = 00007ff968210fc4
RSP = 000000395aefece8
```

Because `RDX & 0xffffffff == 1` and `R8 != 0`, the only supported label is:

```text
CLI_CXL_LOAD_SEMANTICS_OBSERVED = STATIC_PROCESS_START
```

This is a loader-semantics observation only. The V6 CLI did not survive to a
valid successful completion, so it cannot establish `STATIC_LOAD_SURVIVES` or
confirm a static-versus-dynamic causal relationship.

## CLI AMD INITIALIZATION SEQUENCE

Filtering the raw markers to PE entry events whose `RDX` low 32 bits equal 1
produces this process-attach sequence:

| Sequence | Marker | Module | `R8` classification |
| ---: | --- | --- | --- |
| 1 | `CLI_CXL_IMAGE_ENTRY` (line 318) | `CXLBaseTools.dll` | `STATIC_PROCESS_START` |
| 2 | `CLI_AMDSYSUTILS_IMAGE_ENTRYPOINT` (line 433) | `AMDSysUtils.dll` | static process attach |
| 3 | `CLI_AMDPOWERPROFILEAPI_IMAGE_ENTRYPOINT` (line 550) | `AMDPowerProfileAPI.dll` | static process attach |
| 4 | `CLI_AMDBACKENDUTILS_IMAGE_ENTRYPOINT` (line 668) | `AMDBackendUtils.dll` | static process attach |
| 5 | `CLI_AMDPROFILECOMMON_IMAGE_ENTRYPOINT` (line 771) | `AMDProfileCommon.dll` | static process attach |

The raw trace separately contains 20 thread-attach events (`RDX=2`) and five
detach events (`RDX=3`). They are excluded from the process-attach ordering
used here. In particular, the final CXL entry marker at line 4033 is a thread
detach, not a CXL process attach.

## CLI API INITIALIZE CALL

`CLI_POWER_API_INITIALIZE_CALL` was reached twice, at lines 2720 and 2770.
Both events belong to the root CLI process/thread. The captured x64 first
argument was `RCX=0` and the instruction address was
`AMDPowerProfileAPI+0x1AA20`.

The first captured call stack identifies:

```text
AMDPowerProfileAPI!AMDTPwrProfileInitialize
AMDBackendUtils!AMDTProfileMetadata::InitializePowerProfiler+0x64
AMDBackendUtils!AMDTProfileMetadata::Initialize+0x84
AMDBackendUtils!AMDTimeLineEventParser::ConfigureSupportedCounters+0x39
AMDBackendUtils!AMDTProfileMetadata::GetSelectedCounterList+0x6f
AMDBackendUtils!AMDTProfileMetadata::GetLiveCounterIds+0x126
AMDuProfCLI+...
```

The second call was reached through `AMDProfileCommon!AMDTProfTimeLineCollect::Initialize+0xa7`.
The breakpoint captured call-site reachability, not the API return value.

## DIRECT PROBE INITIAL BREAK

`PROBE_INITIAL_BREAK` appears at `PROBE-TRACE.log` line 34. The root process
was `metric_probe.exe` PID `0x1010`, TID `0x86C4`, x64. Its initial module set
contained the probe and Windows/runtime modules, but not the AMD CXL graph.
The PEB confirmed the AMD bin current directory and the exact direct-load
command.

## DIRECT CXL LOAD

The probe's direct load path emitted `BEFORE_LOAD` and then
`PROBE_CXL_MAPPED` at `PROBE-TRACE.log` lines 197 and 203. The mapped image was
the expected x64 `D:\apps\AMDuProf\bin\CXLBaseTools.dll` (displayed by CDB
with the `\\?\` prefix), version 5.3.521.0, with the preflight SHA
`4815D4631BCA9C051DC4293538DF8D402BD848E705228F497DF718EDCA1F8931`.

Therefore:

```text
PROBE_CXL_IMAGE_MAPPING = CONFIRMED
```

The mapping marker is not an entrypoint marker and does not prove that CXL
initialization completed.

## DIRECT CXL PRE-INIT CONTEXT

V6 did not capture a valid CXL entrypoint or process-attach boundary for the
probe. The intended commands used deferred expressions such as:

```text
bu CXLBaseTools+0x10FC4
```

and equivalent expressions for the other AMD modules. CDB reported at
`PROBE-TRACE.log` lines 175-201 that these expressions contained symbols not
qualified with a module name and could not be resolved, including after CXL
was mapped.

Consequently:

```text
PROBE_ENTRYPOINT_BREAKPOINT_VALIDITY = INVALID_HARNESS_BREAKPOINT_EXPRESSION
PROBE_CXL_PROCESS_ATTACH = NOT_OBSERVED
PROBE_CXL_PRE_INIT_CONTEXT = NOT_CAPTURED
```

`NOT_OBSERVED` means that the required breakpoint was not valid; it does not
mean that CXL did not execute its process-attach routine.

## FATAL EXIT

### CLI

`CLI_FATAL_EXIT_HIT` appears at `CLI-TRACE.log` line 4174. The event is
associated with the root CLI process `0x2128` and thread `0x2128.3240`.
The explicit register output is:

```text
RCX = 0000000000000000
RIP = 00007ff9b24414b0   (KERNEL32!FatalExit breakpoint)
RSP = 000000395aeffab8
```

The expected post-breakpoint `kb 60`, `.lastevent`, and later termination
markers are absent: the log stops during the module-list output and the
wrapper watchdog then kills CDB. No `CLI_NT_TERMINATE_PROCESS` marker was
captured, so the root CLI termination status is unknown. The wrapper's
`0xFFFFFFFF` is the wrapper/watchdog result; it is not the value captured in
the `FatalExit` `RCX` register.

The preceding CXL marker at line 4033 is a `DLL_THREAD_DETACH` event
(`RDX=3`) and its shutdown stack must not be reused as the FatalExit stack.
The V6 log therefore contains no reliable FatalExit caller stack or caller
module+offset. Presence of `CXLBaseTools`, `AMDPowerProfileAPI`,
`AMDProfileCommon`, or `AMDBackendUtils` on the FatalExit stack is
`NOT_CAPTURED`, not yes.

```text
CLI_FATAL_EXIT_STAGE = UNKNOWN
CLI_CXL_PROCESS_ATTACH_FATAL = NOT_ESTABLISHED
```

The fact that the process-attach sequence and API call markers occurred before
the FatalExit proves trace progress, but it does not prove which initialization
or runtime operation caused the termination.

### Probe

No `PROBE_CXL_FATAL_EXIT` and no `PROBE_NT_TERMINATE_PROCESS` marker appears
in the V6 probe log. The wrapper timed out and fallback-killed CDB after the
CXL mapping record. Thus:

```text
PROBE_FATAL_EXIT_BREAKPOINT_HIT = false
EXACT_PROBE_TERMINATION_PATH = UNPROVEN
PROBE_EXIT_0XFFFFFFFF = INCONCLUSIVE
```

The earlier, separate direct-CXL investigation remains the historical evidence
for a direct-load `FatalExit(0xFFFFFFFF)` path. It is not merged into V6 as if
V6 captured that stack.

## PRE-INIT MODULE SET DIFF

At CLI initial break, the static import graph was mapped before initialization.
At probe initial break, no AMD module was mapped; the probe later mapped only
CXL through `LoadLibraryExW`. This establishes a difference in observed
mapping context, but not a causal prerequisite.

An equivalent probe CXL pre-initialization module set was not captured because
the entrypoint breakpoints were invalid. Therefore:

```text
CXL_PRE_INIT_MODULE_SET_DIFF = INCONCLUSIVE
CXL_CONTEXT_CLASS = INCONCLUSIVE
```

No CLI-only AMD module is promoted to a required preload candidate. Mapped
versus initialized state cannot be compared reliably on the two sides from V6.

## INITIALIZATION ORDER DIFF

The CLI process-attach order is known from the five valid `RDX=1` events above.
The probe process-attach order is unknown because none of its intended
entrypoint breakpoints existed. The only probe sequence evidence is the CXL
image-mapping event and the loader stack printed for that mapping event.

```text
CLI process-attach order: CXL -> AMDSysUtils -> AMDPowerProfileAPI -> AMDBackendUtils -> AMDProfileCommon
Probe process-attach order: NOT_OBSERVED
AMD_INITIALIZATION_ORDER_DIFF: INCONCLUSIVE
INITIALIZED_BEFORE_CXL_CANDIDATES: NONE_ESTABLISHED
```

No random DLL preloading or permutation test is authorized by this result.

## ROOT CAUSE REFINEMENT

The inherited direct-load evidence remains the working root-cause record. It
must not be read as though V6 independently proved the probe's termination
path:

```text
CURRENT_WORKING_ROOT_CAUSE = DEPENDENCY_LOAD_FAILURE
INHERITED_SUBCAUSE_FROM_PRIOR_DIRECT_LOAD_EVIDENCE = CXLBASETOOLS_LOAD_PATH_FATAL_EXIT
EXACT_CXL_INTERNAL_CONDITION = UNPROVEN
V6_TRACE_RESULT = DEBUGGER_PERTURBED_SUCCESS_PATH
V6_PROBE_TERMINATION_PATH = UNPROVEN
```

For this instrumented comparison, the primary trace classification is:

```text
RESULT = DEBUGGER_PERTURBED_SUCCESS_PATH
CLI_DEBUGGER_PERTURBATION = CONFIRMED
EXACT_DEBUGGER_PERTURBATION_MECHANISM = UNPROVEN
```

The historical non-debugger CLI was successful, whereas the V6 CDB-launched
CLI hit `FatalExit` and was watchdog-terminated. That is sufficient to confirm
debugger perturbation of the success control, but not to identify whether the
mechanism is debugger presence detection, debugger timing, breakpoint effects,
or another debugger-specific process-state change.

The static/dynamic hypothesis remains:

```text
STATIC_VS_DYNAMIC_LOAD_HYPOTHESIS = STILL_PLAUSIBLE_BUT_UNQUALIFIED
CXL_LOAD_MODE_CORRELATION = UNPROVEN
```

The V6 CLI observation is only `STATIC_PROCESS_START`; it is not evidence that
static loading survives. The probe's dynamic process-attach `R8` was not
captured. The exact CXL FatalExit caller was not captured on either V6 side.

### Disproven or unsupported claims

- `STATIC_LOAD_SURVIVES=true` — unsupported because the CLI was not successful
  under CDB.
- `STATIC_VS_DYNAMIC_DLL_LOAD_SEMANTICS=CONFIRMED` — unsupported because the
  direct probe process-attach arguments were not captured.
- `PROBE_CXL_PROCESS_ATTACH` did not execute — not established; its breakpoint
  was invalid.
- V6 probe target itself exited `0xFFFFFFFF` — not established; V6 recorded a
  CDB watchdog/fallback result.
- CXL was on the CLI FatalExit stack — not captured.
- A specific anti-debug, timing, or vendor branch caused the CLI failure — not
  established.

## PRODUCT IMPLICATION

No production code, Provider registration, CollectionPlan contract,
MetricCatalog, schema, UI, or metric admission changed. No AMD metric is
qualified by this trace. The current metric disposition remains deferred:

- `CPU_PACKAGE_POWER = DEFER`.
- `AMD_CORE_EFFECTIVE_FREQUENCY = DEFER`.
- `CPU_EFFECTIVE_FREQUENCY = DEFER_AGGREGATION_CONTRACT`.
- `CPU_PACKAGE_TEMPERATURE = DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD`.

The historical CLI samples are not a substitute for a safe, independently
validated Resource Timeline source. Future source options remain only
unimplemented candidates: documented `AMDPowerProfileAPI`, an external CLI
source, another documented AMD interface, or deferral.

## DEFERRED QUESTIONS

The next experiment is design-only:

```text
CPU-SENSOR-AMD STATIC-IMPORT VS DYNAMIC-LOAD MINIMAL A/B
```

Before implementing it, inspect the installed SDK/sample package for an
authoritative import library or other supported link surface that can create a
genuine static-import executable using the existing CXL artifact. Do not
fabricate vendor linkage or redistribute AMD material. If no supported import
surface exists, use a separately designed non-invasive successful-CLI
image-load observation plus the direct-load trace rather than forcing an
unsupported static test.

Still unresolved:

- the direct probe CXL process-attach `R8` value and exact dynamic semantics;
- the exact CXL FatalExit caller and stage on a valid probe trace;
- whether a static-import minimal executable survives without the full CLI;
- whether debugger presence, timing, breakpoints, or another CDB-visible state
  causes the CLI perturbation;
- whether the CLI's internal use of the public API is semantically identical to
  a direct client call.

No reinstall/repair is recommended from this evidence alone. No provider
design should start.

## VALIDATION

- Read-only cross-check completed for all V6 JSON records, raw CLI/probe logs,
  wrapper output, artifact preflight, Administrator proof, common context, and
  CLI output files.
- Artifact preflight: all expected SHA-256 values and x64 checks passed; the
  repository probe is hash/architecture locked and is not required to be
  Authenticode-signed. AMD binaries and CDB had required valid signatures.
- Raw marker and line-reference review completed. The V6 summary's false
  `timechart_csv_non_empty`/`session_uprof_non_empty` booleans were not allowed
  to override the actual non-empty files, and neither were they used to mark
  the debugger control successful.
- Only this measurement document was changed. No Rust source was changed, so
  the full Rust test suites were not rerun for this documentation-only
  evidence-consumption task.
- `git diff --check`: PASS.

## DELIVERY

- Documentation change: this file.
- Commit: recorded in the final delivery report for this change.
- Push: existing qualification branch only; no force push.
- Draft PR: not created/updated because PR tooling was unavailable; this is
  separate from the technical evidence classification.
- Working tree: required to be clean after commit/push.
- System mutations: none. In particular, no AMD installation/service/driver,
  registry, PATH, security policy, Hyper-V, VBS, HVCI, or boot configuration
  changed.

## NEXT STEP

`CPU-SENSOR-AMD STATIC-IMPORT VS DYNAMIC-LOAD MINIMAL A/B` — design and
authoritative-link-surface inspection only. Do not begin
`CPU-SENSOR-AMD-PROVIDER-DESIGN`.
