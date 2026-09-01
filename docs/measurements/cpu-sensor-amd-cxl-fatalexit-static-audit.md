# CPU-SENSOR-AMD CXL FATALEXIT STATIC CONTROL-FLOW AUDIT

This is a read-only audit of the exact installed `CXLBaseTools.dll`. No AMD
DLL, executable, service, driver, debugger, or profiler was run for this
audit. The purpose is to distinguish a direct `KERNEL32!FatalExit` call from
the statically visible CRT termination path and to narrow, without guessing,
the data used immediately before the `0xFFFFFFFF` termination candidates.

```text
RESULT = PASS_WITH_UNRESOLVED_VENDOR_INTERNAL
STATIC_ANALYSIS_ONLY = true
PRODUCTION_INTEGRATION = false
```

## BASELINE

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = 4c03a31956797d82c5f1e8f34d635cd564387799
ORIGIN_MAIN = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
WORKING_TREE_AT_ENTRY = CLEAN
ENTRY_GATE = PASS
DUPLICATE_TASK_GATE = PASS (no competing branch; gh PR visibility unavailable)
```

The preceding lifetime discriminator is preserved in
[`cpu-sensor-amd-static-fixture-lifetime-discriminator.md`](cpu-sensor-amd-static-fixture-lifetime-discriminator.md).
Its one Administrator run recorded the hold fixture terminating after about
`63.1963 ms` with signed `-1` / `0xFFFFFFFF`, zero-byte persisted stdout and
stderr, complete capture, and neither synchronous marker. The supported
classification is:

```text
RESULT = STARTUP_FAILURE_SUPPORTED
M1_FAILURE_FAMILY = STARTUP
STATIC_FAILURE_STAGE = BEFORE_DURABLE_MAIN_MARKER
DURABLE_MAIN_MARKER_REACHED = false
STABLE_MAIN_WINDOW_REACHED = false
BEFORE_RETURN_MARKER_REACHED = false
SHUTDOWN_OR_DETACH_HYPOTHESIS = DOWNGRADED
RUST_MAIN_ENTRY_ITSELF = UNPROVEN
```

The accepted vendor no-op control remains separate evidence: the signed
`AMDuProf.exe` survived the native 3,000 ms observation and launched an
`AMDProfilerService.exe` child. This establishes runtime survival divergence,
not the cause of either path.

## TARGET IDENTITY

```text
PATH = D:\apps\AMDuProf\bin\CXLBaseTools.dll
SIZE = 134040 bytes
SHA256 = 4815D4631BCA9C051DC4293538DF8D402BD848E705228F497DF718EDCA1F8931
MACHINE = 0x8664 / x64
PE_FORMAT = PE32+
SUBSYSTEM = Windows GUI (2)
FILE_VERSION = 5.3.521.0
PRODUCT_VERSION = 5.3.521.0
AUTHENTICODE = Valid
SIGNER = CN=Advanced Micro Devices, O=Advanced Micro Devices, S=California, C=US
PE_TIMESTAMP = 0x6A2AE738 (2026-06-12 00:50:00, as reported by dumpbin)
IMAGE_BASE = 0x180000000
ENTRY_RVA = 0x10FC4
```

The identity was checked with `Get-FileHash`, `Get-AuthenticodeSignature`,
and the installed Microsoft `dumpbin.exe` (`14.44.35228.0`). The file was
only read; it was not loaded into a process.

## STATIC TOOLING AND CROSS-CHECK

The primary PE report was generated with the installed `dumpbin.exe` headers,
imports, exports, and disassembly output. A separate raw-byte PE scan then
resolved the `.text` `FF 15 rel32` calls against the `quick_exit` IAT VA. The
two methods agree:

```text
TEXT_RVA = 0x1000
TEXT_RAW_OFFSET = 0x400
QUICK_EXIT_IAT_RVA = 0x122D8
QUICK_EXIT_IAT_VA_AT_PREFERRED_BASE = 0x1800122D8
QUICK_EXIT_CALLS_FROM_RAW_SCAN = 0x1A82, 0x1B64
FATALEXIT_LITERAL_SCAN = none in ASCII or UTF-16 data
```

Address/file-offset sanity checks agree as well:

| RVA | preferred VA | file offset | raw bytes |
|---|---:|---:|---|
| `0x10FC4` | `0x180010FC4` | `0x103C4` | `48 89 5C 24 08 ...` |
| `0x1A82` | `0x180001A82` | `0xE82` | `FF 15 50 08 01 00` |
| `0x1B64` | `0x180001B64` | `0xF64` | `FF 15 6E 07 01 00` |

## FATALEXIT IMPORT

```text
FATALEXIT_IMPORT_PRESENT = false
FATALEXIT_IAT_RVA = NOT_APPLICABLE
STATIC_FATALEXIT_XREF_COUNT = 0 (direct KERNEL32!FatalExit references)
```

`CXLBaseTools.dll` has no normal or delay import for `KERNEL32!FatalExit`, and
the raw file contains no `FatalExit` ASCII or UTF-16 literal. The relevant
normal import is instead:

```text
IMPORT = api-ms-win-crt-runtime-l1-1-0.dll!quick_exit
QUICK_EXIT_IAT_RVA = 0x122D8
QUICK_EXIT_IAT_VA_AT_PREFERRED_BASE = 0x1800122D8
DELAY_IMPORT_DIRECTORY = absent (RVA 0, size 0)
```

The DLL also imports `TerminateProcess` at IAT RVA `0x120E0`, but the static
`.text` scan found no direct call to that IAT slot. Its presence is therefore
not evidence that it is the observed termination path. The CRT
implementation reached by `quick_exit` is outside this DLL's import/call
graph, so a subsequent CRT-to-`FatalExit` edge cannot be proven statically
from this artifact.

## STATICAL IDENTIFIABLE TERMINATION CANDIDATES

The two real code callsites are CRT `quick_exit` callsites, not direct
`FatalExit` callsites. Both are inside the same `.pdata` runtime-function
boundary `[RVA 0x18A0, RVA 0x1B81)` with unwind metadata at `RVA 0x1699C`.
Private AMD symbols are unavailable; the nearby export is used only as a
locator.

| ID | RVA / file offset | operation | argument origin | bounded function locator |
|---|---:|---|---|---|
| `QE-1` | `0x1A82` / `0xE82` | indirect call through `quick_exit` IAT | `EDI = 0xFFFFFFFF` at `0x1968`, then `ECX = EDI` at `0x1A80` | nearest exported-symbol locator `?asWideString@gtString@@...` at `RVA 0x1630`; `0x1A88 - 0x1630 = 0x458` |
| `QE-2` | `0x1B64` / `0xF64` | indirect call through `quick_exit` IAT | explicit `ECX = 0xFFFFFFFF` at `0x1B5F` | same local runtime-function region; no private function identity claimed |

Therefore:

```text
FATALEXIT_0XFFFFFFFF_CALLSITE = not proven as a direct FatalExit call;
  static quick_exit(-1) candidates are QE-1 and QE-2
DIRECT_FATALEXIT_CALLSITE_UNIQUENESS = NOT_APPLICABLE (no direct import)
FATALEXIT_CALLSITE_UNIQUENESS = INCONCLUSIVE_FOR_KERNEL32
INDIRECT_QUICK_EXIT_CALLSITE_UNIQUENESS = NOT_UNIQUE (two callsites)
```

`INCONCLUSIVE_FOR_KERNEL32` is deliberate rather than a count of zero:
there are no statically visible direct `KERNEL32!FatalExit` callsites in this
DLL, while the CRT implementation behind the two visible `quick_exit` calls
is outside the artifact. The two visible `quick_exit(-1)` candidates are
therefore real and non-unique, but they cannot be promoted to direct
`FatalExit` callsites.

## PREDECESSOR CONTROL FLOW

### QE-1: RVA 0x1A82

The bounded region begins at `RVA 0x18A0`. It first checks a global byte at
`RVA 0x1D941`, sets it to `1`, obtains an internal object, clears a local
wide buffer, and constructs/inspects path-related string state. The relevant
sequence is:

```text
0x18CE  compare [0x1D941] with 0
0x18D5  nonzero -> 0x1B56 (state-dependent alternate path)
0x18DB  [0x1D941] = 1
0x18E2  internal object acquisition
0x190F  clear local buffer
0x193B  string construction using "InstallationPath"
0x1957  call path/registry helper at 0x1780
0x19D9  use "bin"
0x1A0E  use "\\AMDPerf"
0x1A45  call imported MultiByteToWideChar
0x1A5A  indirect call through [0x180012378], test EAX
0x1A6F  same indirect call, test EAX
0x1A79  [0x1D942] = 0 only if both tests are nonzero
0x1A80  ECX = EDI
0x1A82  quick_exit(0xFFFFFFFF)
```

If either of the two indirect calls returns zero, control branches to
`0x1A89`, sets `[0x1D942] = 1`, and follows the normal return/cleanup path.
The indirect target at `[0x180012378]` is not a PE import resolved by this
audit; its semantic identity is unknown. Thus the static evidence identifies
the boolean gate but not what the vendor routine checks.

### QE-2: RVA 0x1B64

The second candidate is a state fallback in the same runtime function:

```text
0x1B56  compare [0x1D942] with 0
0x1B5D  nonzero -> 0x1B28 (normal return)
0x1B5F  ECX = 0xFFFFFFFF
0x1B64  quick_exit(0xFFFFFFFF)
```

QE-2 is reached from the earlier `[0x1D941] != 0` path when the second state
byte is still zero. It is not independently attributable to a particular
file, registry value, or process property from static evidence.

## LOCAL DATA AND API SOURCES

The following strings are referenced by the bounded region or its directly
called path/registry helper:

| RVA | representation | value | evidence role |
|---:|---|---|---|
| `0x14BA0` | UTF-16 | `SOFTWARE\\WOW6432Node\\AMD\\AMDProfiler` | registry subkey passed at `0x17BE` |
| `0x14BF0` | UTF-16 | `InstallationPath` | value/string input passed at `0x192F` |
| `0x14C18` | UTF-16 | `bin` | path component used at `0x19D9` |
| `0x14C20` | UTF-16 | `\\AMDPerf` | path component used at `0x1A0E` |
| `0x14CB0` | UTF-16 | `Assertion failure (false)` | present in the local read-only string area; no direct causal edge claimed |

The immediately reachable path/registry helper at `RVA 0x1780` calls:

```text
0x17CC -> ADVAPI32!RegOpenKeyW
0x17FC -> ADVAPI32!RegQueryValueExW
```

The module-path helper at `RVA 0x1670`, called before QE-1, calls:

```text
0x16B3 -> KERNEL32!GetModuleHandleW(NULL)
0x16CA -> KERNEL32!GetModuleFileNameW(...)
```

The bounded fatal predecessor also calls `KERNEL32!MultiByteToWideChar` at
`RVA 0x1A45` before the two unknown indirect checks. These calls prove that
module-path, registry/install-root, and string/encoding state are touched on
the path leading to the candidate block. They do not prove which returned
value makes either indirect check nonzero.

### DATA-SOURCE CLASSIFICATION

```text
PROCESS_IMAGE_OR_PATH = SUPPORTED_AS_REACHABLE_INPUT_SURFACE,
  exact fatal comparison unproven
MODULE_PATH = SUPPORTED_AS_REACHABLE_INPUT_SURFACE,
  GetModuleHandleW/GetModuleFileNameW are in the called helper
CURRENT_DIRECTORY = NO_DIRECT_SUPPORT_IN_BOUNDED_PATH
ENVIRONMENT = NO_DIRECT_SUPPORT_OBSERVED
REGISTRY = SUPPORTED_AS_REACHABLE_INPUT_SURFACE,
  RegOpenKeyW/RegQueryValueExW precede the candidate block
VERSION = NO_DIRECT_SUPPORT_IN_BOUNDED_FATAL_REGION
INSTALL_ROOT = SUPPORTED_AS_REACHABLE_INPUT_SURFACE,
  InstallationPath + AMDProfiler key + bin/AMDPerf construction
COMMAND_LINE = NO_DIRECT_SUPPORT_OBSERVED
SIGNATURE_OR_TRUST = NO_DIRECT_SUPPORT_OBSERVED
CONFIG_FILE = NO_DIRECT_SUPPORT_OBSERVED
STRING_TOKENIZATION = PLAUSIBLE, not a proven private semantic identity
LOADER_STATE = SUPPORTED, via process-attach dispatcher and D941/D942 state
UNKNOWN = the indirect target at [0x180012378] and CRT quick_exit behavior
```

The `SUPPORTED_AS_REACHABLE_INPUT_SURFACE` wording means the API/string is
on the bounded static path; it does not mean that the path's exact branch
condition has been reconstructed.

## ENTRYPOINT AND PROCESS-ATTACH REACHABILITY

The PE has a TLS directory (`RVA 0x15F00`, size `0x28`), but its callback
array is empty. No TLS callback RVA is therefore available:

```text
TLS_DIRECTORY = PRESENT
TLS_CALLBACKS = NONE
CXL_TLS_FATAL_STAGE = NOT_SUPPORTED_BY_STATIC_EVIDENCE
```

The PE `AddressOfEntryPoint` is `RVA 0x10FC4`. Its code preserves the three
entry arguments, conditionally performs the CRT/security-cookie setup for
`reason == 1`, and transfers to a dispatcher at `RVA 0x10E9C`. In that
dispatcher, a process-attach (`EDX == 1`) path can call the runtime function
at `RVA 0x18A0` at `RVA 0x10F21`; a second call with `EDX == 0` is also
reachable at `RVA 0x10F3D` after a state-dependent result.

Accordingly:

```text
FATALEXIT_REACHABLE_FROM_PROCESS_ATTACH_PATH = SUPPORTED
```

This is a limited static statement: the process-attach entry/dispatcher has a
credible edge to the function containing QE-1/QE-2. It is not proof that the
vendor branch executed on the observed machine, and it is not proof that the
CRT `quick_exit` implementation calls `KERNEL32!FatalExit`.

```text
CXL_FATAL_STAGE_FROM_THIS_STATIC_AUDIT = PROCESS_ATTACH_PATH_TO_CRT_CANDIDATE
DIRECT_KERNEL32_FATAL_EXIT_STAGE = UNPROVEN
```

## HISTORICAL RUNTIME CORRELATION

The accepted prior debugger evidence recorded:

```text
KERNEL32!FatalExit(0xFFFFFFFF)
CXLBaseTools!gtString::asWideString+0x458
CXLBaseTools!gtStringTokenizer::getNextToken+0x2f96
```

The first frame correlates strongly with QE-1: the return address immediately
after the six-byte call at `RVA 0x1A82` is `RVA 0x1A88`, and the nearby
exported-symbol locator at `RVA 0x1630` gives exactly `0x458`. This supports:

```text
STATIC_RUNTIME_CORRELATION = STRONG
HISTORICAL_FATAL_EXIT_CORRELATED_SITE = QE-1 quick_exit(-1) candidate
```

The correlation is to the CXL CRT callsite immediately preceding the
historically observed termination, not to a statically visible direct
`FatalExit` call. `gtStringTokenizer::getNextToken+0x2f96` is retained only
as a prior debugger label; it is not accepted as a private function identity.

## CONTROL-FLOW CONCLUSIONS

```text
FATAL_CONDITION_FAMILY = INITIALIZATION_STATE_FAILURE
  (supported by D941/D942 state gates and status tests)
PATH_REGISTRY_STRING_PROCESSING = REACHABLE_PREDECESSOR_SURFACE
  (supported; exact branch dependency unproven)
EXACT_CXL_INTERNAL_CONDITION = UNPROVEN
EXACT_CRT_QUICK_EXIT_TO_KERNEL32_FATAL_EXIT = UNPROVEN
```

The static audit cannot say whether QE-1 or QE-2 was the particular source of
the historical `FatalExit` without runtime evidence that does not alter the
successful vendor control. It also cannot identify the semantic target of
the indirect pointer at `[0x180012378]`. No assertion is made that
`R8/lpvReserved`, the current directory, the process name, signing, or a
specific registry value directly causes the branch.

## UPDATED HYPOTHESIS RANKING

Only three primary candidate families remain:

1. **CXL initialization-state failure** — high confidence as the immediate
   static control-flow family. The function gates on global state and has two
   explicit `quick_exit(-1)` paths, but the vendor state predicate is unknown.
2. **Path/registry/string bootstrap prerequisite** — medium confidence as an
   input family. `GetModuleFileNameW`, the AMDProfiler registry key,
   `InstallationPath`, `bin`, `\\AMDPerf`, and string conversion are all on
   the bounded predecessor path; their influence on the final indirect checks
   is not proven.
3. **Vendor executable/bootstrap topology** — low-to-medium inherited
   hypothesis. The native vendor executable survives while the minimal static
   fixture fails, and earlier import audits show different parent topology, but
   this static DLL alone does not establish which vendor bootstrap difference
   changes the state.

The earlier static-vs-dynamic load theory remains low priority: static import
alone failed in the hold fixture, while the vendor executable has additional
context. The lifetime evidence downgrades shutdown/detach as the primary
family.

## RECOMMENDED NEXT EXPERIMENT

```text
RECOMMENDED_NEXT_EXPERIMENT = NATIVE_PROCESS_IMAGE_PATH_ONLY_CONTROL
WHY_THIS_ONE = The bounded predecessor demonstrably queries the current
  module image path before the candidate checks, while prior cwd and child-PATH
  controls were negative. Use byte-identical diagnostic-fixture content at a
  second executable path/name, with the same Administrator token, CWD,
  inherited environment, AMD DLLs, and no debugger; compare only the durable
  startup marker and exit. This changes one supported process-context surface
  without changing AMD installation state.
EXECUTION_STATUS = DESIGN_ONLY / NOT_RUN
```

This is not a claim that image identity is causal. If a future run is
authorized, it must preserve the original fixture and evidence, avoid copying
or modifying vendor files, and stop after the one native control. No random
preloads, debugger instrumentation, profiling, sampling, or provider design
is justified by this audit.

## UNRESOLVED POINTS

```text
1. Which CRT implementation is reached by quick_exit on this installation?
2. Which target does [0x180012378] resolve to at runtime?
3. Which value(s) make the two indirect checks in QE-1 return nonzero?
4. Whether QE-1 or QE-2 produced the historical FatalExit event.
5. Which vendor executable/bootstrap state supplies the successful predicate.
```

These are deliberately not answered by guessing from export names, module
names, or the fact that the AMD CLI survives.

## VALIDATION

```text
STATIC_TARGET_SHA256_CROSS_CHECK = PASS
AUTHENTICODE_VERSION_ARCHITECTURE_CHECK = PASS
DUMPBIN_IMPORT_EXPORT_HEADER_CHECK = PASS
RAW_BYTE_XREF_CROSS_CHECK = PASS
RVA_FILE_OFFSET_SANITY = PASS
FATALEXIT_LITERAL_SCAN = PASS (none found)
AMD_RUNTIME_EXECUTED_FOR_THIS_AUDIT = false
SYSTEM_MUTATIONS = none
GIT_DIFF_CHECK = pending delivery check
```

No repository source or production code was changed for the analysis. The
measurement records are the only intended changes.

## DELIVERY

```text
PRODUCTION_PROVIDER_CHANGED = false
METRIC_CATALOG_CHANGED = false
SCHEMA_CHANGED = false
UI_CHANGED = false
AMD_ARTIFACT_COMMITTED = false
```

This document and the lifetime-discriminator closure are committed on the
existing qualification branch only. No AMD binary, dump, or machine-specific
temporary analysis artifact is committed.

## NEXT STEP

`NATIVE_PROCESS_IMAGE_PATH_ONLY_CONTROL` remains design-only. It must be
separately authorized before execution. Do not start
`CPU-SENSOR-AMD-PROVIDER-DESIGN`, B1, profiling, sampling, or a broad vendor
context matrix.
