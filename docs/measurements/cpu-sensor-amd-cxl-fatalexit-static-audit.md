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
START_HEAD = b9045d304bf36adc988fda6a839b7385d020ee13
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

## INDIRECT STATUS SLOT RESOLUTION

The two predecessor calls use `RVA 0x12378`. This address is not an opaque
vendor-global function pointer: it is an import-address-table slot.

```text
INDIRECT_STATUS_SLOT_RVA = 0x12378
INDIRECT_STATUS_SLOT_SECTION = .rdata
SECTION_CHARACTERISTICS = 0x40000040 (Initialized Data, Read Only)
FILE_OFFSET = 0x11578
INDIRECT_STATUS_SLOT_KIND = IAT
INDIRECT_STATUS_IMPORTED_DLL = api-ms-win-crt-string-l1-1-0.dll
INDIRECT_STATUS_IMPORTED_SYMBOL = _wcsicmp
IMPORT_DIRECTORY_OVERLAP = false
IAT_DIRECTORY_OVERLAP = true (RVA 0x12000, size 0x3A8)
BASE_RELOCATION_AT_RVA_0x12378 = not observed
RELOCATION_STATUS = no base-relocation entry observed for this IAT slot
```

Two independent static checks support this resolution. `dumpbin /imports`
reports the string-runtime IAT at `RVA 0x12318` with the ordered imports
including `_wcsicmp` at zero-based slot 12; `0x12318 + 12 * 8 = 0x12378`.
The raw slot bytes at file offset `0x11578` are `58 C5 01 00 00 00 00 00`,
which is the on-disk import-name RVA `0x1C558`; the corresponding
`IMAGE_IMPORT_BY_NAME` bytes contain the `_wcsicmp` name. The loader resolves
that slot to the CRT string function at runtime.

The independent disassembly/raw-reference check found exactly two code reads,
both indirect calls, at `RVA 0x1A5A` and `RVA 0x1A6F`. No code writer to this
IAT slot or static initializer writer was observed in the image; its value is
initialized by normal PE import resolution.

```text
READ_XREFS = 2
CALL_XREFS = 2
WRITE_XREFS = 0 observed in image code
INITIALIZER_XREFS = loader-resolved IAT, not an in-image value initializer
```

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
0x1A5A  _wcsicmp(RCX=RBX, RDX=R14), test EAX
0x1A6F  _wcsicmp(RCX=[rsp+38], RDX=R14), test EAX
0x1A79  [0x1D942] = 0 only if both tests are nonzero
0x1A80  ECX = EDI
0x1A82  quick_exit(0xFFFFFFFF)
```

Both calls are to the imported `api-ms-win-crt-string-l1-1-0.dll!_wcsicmp`.
The first uses the string data held by the path object at `[rsp+30]` after the
`bin` operation. After that call, `RBX` is reloaded from `[rsp+38]`; the
second path object has been copied and extended with the `\\AMDPerf` literal.
`R14` is the wide-character output buffer populated by the preceding
`MultiByteToWideChar` call from the module-directory buffer. If either
comparison returns zero (case-insensitive equality), control branches to
`0x1A89`, sets `[0x1D942] = 1`, and follows the normal return/cleanup path.
Only when both `_wcsicmp` results are nonzero does the code clear
`[0x1D942]` and reach QE-1. The exact private string-object ownership and
separator normalization are not symbolized, but the operands, comparison
polarity, and literals are directly visible.

```text
QE1_CHECK_1:
  target = api-ms-win-crt-string-l1-1-0.dll!_wcsicmp
  arguments = RCX=RBX ([rsp+30] path after "bin"), RDX=R14 (module-directory buffer)
  return = EAX (signed comparison result)
  comparison = TEST EAX,EAX; JE 0x1A89
  fatal polarity = nonzero mismatch permits check 2; zero equality is normal
QE1_CHECK_2:
  target = api-ms-win-crt-string-l1-1-0.dll!_wcsicmp
  arguments = RCX=[rsp+38] (copied path after "\\AMDPerf"), RDX=R14
  return = EAX (signed comparison result)
  comparison = TEST EAX,EAX; JE 0x1A89
  fatal polarity = nonzero mismatch reaches QE-1; zero equality is normal
```

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

## MODULE AND PROCESS PATH OPERAND

The module-path helper at `RVA 0x1670` does not ask for the CXL module. At
`RVA 0x16B1` it clears `ECX` and calls the KERNEL32 IAT slot resolved as
`GetModuleHandleW`; this is `GetModuleHandleW(NULL)`, whose documented target
is the calling process executable. At `RVA 0x16C7` it moves that return value
into `RCX` for `GetModuleFileNameW` at `RVA 0x16CA`.

```text
GETMODULEHANDLE_TARGET = NULL_CURRENT_EXE
GETMODULEHANDLE_ARGUMENT = RCX = 0
GETMODULEFILENAME_HMODULE = return value of GetModuleHandleW(NULL)
MODULE_FILENAME_SOURCE = PROCESS_EXE_PATH
```

The bounded transformation is visible in the same helper:

```text
wide_module_path[0x104] = zeroed buffer
GetModuleFileNameW(GetModuleHandleW(NULL), wide_module_path, 0x104)
wcstombs(multibyte_buffer[0x105], wide_module_path, 0x105)
scan multibyte_buffer for the last '\\'
copy the prefix before that separator to the caller's output
write a NUL at the separator position
```

Thus the resulting operand is the directory of the process executable, not
`CXLBaseTools.dll`'s own path. No parent-directory traversal or
`GetCurrentDirectoryW` call is visible in this bounded helper. The conversion
and last-separator scan are visible; the private string-object ownership is
not symbolized.

## REGISTRY OPERAND

The helper at `RVA 0x1780` uses:

```text
REGISTRY_API_OPEN = ADVAPI32!RegOpenKeyW at RVA 0x17CC
REGISTRY_API_QUERY = ADVAPI32!RegQueryValueExW at RVA 0x17FC
REGISTRY_HIVE = HKLM (RCX = 0xFFFFFFFF80000002)
REGISTRY_KEY = SOFTWARE\WOW6432Node\AMD\AMDProfiler
REGISTRY_VALUE = InstallationPath
REGISTRY_ACCESS_FLAGS = none explicit (RegOpenKeyW, default read access)
WOW64_SELECTION = literal WOW6432Node in the key; no KEY_WOW64_* flag observed
REGISTRY_BUFFER = 0x800-byte data buffer; lpType is NULL; size returned separately
```

The read-only machine query was performed twice, with PowerShell registry
access and `reg.exe query`, without modification:

```text
REGISTRY_VALUE_PRESENT = true
REGISTRY_INSTALLATION_PATH = D:\apps\AMDuProf\
ACTUAL_AMD_INSTALL_ROOT = D:\apps\AMDuProf
REGISTRY_INSTALL_ROOT_MATCH = MATCH (case-insensitive, trailing separator normalized)
REGISTRY_INSTALL_ROOT_EXISTS = true
REGISTRY_VALUE_TYPE = REG_SZ (observed by reg.exe; the DLL does not request lpType)
```

The relevant read-only filesystem checks were:

| Derived/candidate path | Exists |
|---|---|
| `D:\apps\AMDuProf` | yes |
| `D:\apps\AMDuProf\bin` (`InstallationPath` + `bin`) | yes |
| `D:\apps\AMDuProf\bin\AMDPerf` (the preceding path + `\AMDPerf`) | yes |
| hold fixture executable directory (the `GetModuleFileNameW` directory operand) | yes |
| `D:\apps\AMDuProf\AMDPerf` | no; not the observed append order |

The registry root therefore does not currently show a stale/mismatched install
root. This does not prove that every vendor bootstrap state is valid.

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

The bounded fatal predecessor also calls `KERNEL32!MultiByteToWideChar` at
`RVA 0x1A45` before the two `_wcsicmp` checks. The exact arguments are
`RCX=0`, `RDX=0`, `R8=&[rsp+60]` (the multibyte module-directory buffer),
`R9=0xFFFFFFFF`, `stack+0x20=R14` (the wide output buffer), and
`stack+0x28=0x104` (capacity). The return value is not used as the
`_wcsicmp` result; `R14` is subsequently passed as the right-hand comparison
operand.

### DATA-SOURCE CLASSIFICATION

```text
PROCESS_IMAGE_OR_PATH = RESOLVED_AS_PROCESS_EXE_PATH_DIRECTORY
MODULE_PATH = RESOLVED_AS_PROCESS_EXE_PATH_DIRECTORY,
  GetModuleHandleW(NULL) -> GetModuleFileNameW(hExe, ...)
CURRENT_DIRECTORY = NO_DIRECT_SUPPORT_IN_BOUNDED_PATH
ENVIRONMENT = NO_DIRECT_SUPPORT_OBSERVED
REGISTRY = SUPPORTED_AS_REACHABLE_INPUT_SURFACE,
  RegOpenKeyW/RegQueryValueExW precede the candidate block; actual root matches
VERSION = NO_DIRECT_SUPPORT_IN_BOUNDED_FATAL_REGION
INSTALL_ROOT = SUPPORTED_AS_REACHABLE_INPUT_SURFACE,
  InstallationPath + AMDProfiler key + bin/AMDPerf construction
COMMAND_LINE = NO_DIRECT_SUPPORT_OBSERVED
SIGNATURE_OR_TRUST = NO_DIRECT_SUPPORT_OBSERVED
CONFIG_FILE = NO_DIRECT_SUPPORT_OBSERVED
STRING_TOKENIZATION = NOT_REQUIRED_FOR_THE_RESOLVED_QE1_COMPARISONS
LOADER_STATE = SUPPORTED, via process-attach dispatcher and D941/D942 state
UNKNOWN = private string-object semantics, indirect CRT quick_exit transition,
  and the earlier state-setting function's broader purpose
```

The `SUPPORTED_AS_REACHABLE_INPUT_SURFACE` wording means the API/string is
on the bounded static path. `PROCESS_IMAGE_OR_PATH` is now narrowed because
the operand is resolved to the process executable's directory; it is not a
claim that an executable basename comparison was observed. The two QE-1
comparisons are resolved to this directory versus two registry/install-derived
path variants; only private helper naming and the downstream CRT transition
remain opaque.

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

## QE-1 BOUNDED PSEUDOCODE

The narrowest defensible reconstruction is:

```text
install_path = Registry[HKLM\SOFTWARE\WOW6432Node\AMD\AMDProfiler]
               .InstallationPath
path_bin = internal_path_object_after_registry_assignment(install_path)
path_bin = append(path_bin, "bin")

exe_path = GetModuleFileNameW(GetModuleHandleW(NULL))
exe_dir = prefix_before_last_backslash(
              wcstombs(exe_path)
          )

path_amdperf = copy(path_bin)
path_amdperf = append(path_amdperf, "\\AMDPerf")

if (_wcsicmp(path_bin, exe_dir) != 0) {
    if (_wcsicmp(path_amdperf, exe_dir) != 0) {
        [0x1D942] = 0
        quick_exit(0xFFFFFFFF)       // QE-1
    }
}
```

This pseudocode names no private vendor function. The two comparison operands,
the `TEST EAX,EAX` / `JE 0x1A89` polarity, the registry strings, and the
process-executable path source are directly supported by the disassembly.
The exact separator normalization and internal object-copy operations remain
unresolved. On the observed machine the normalized candidate directories are
`D:\apps\AMDuProf\bin` and
`D:\apps\AMDuProf\bin\AMDPerf`; the hold fixture's executable directory
is the repository release directory, so it matches neither candidate by
static path comparison.

## CONTROL-FLOW CONCLUSIONS

```text
FATAL_CONDITION_FAMILY = MODULE_IDENTITY_FAILURE
  (process executable directory matches neither install_path\bin nor
   install_path\bin\AMDPerf)
EXACT_CXL_INTERNAL_CONDITION = SUPPORTED_VISIBLE_PREDICATE:
  _wcsicmp(install_path + "bin", process_executable_directory) != 0
  AND _wcsicmp(install_path + "bin" + "\AMDPerf",
               process_executable_directory) != 0
PATH_REGISTRY_STRING_PROCESSING = REACHABLE_AND_OPERAND_RESOLVED
REGISTRY_INSTALL_ROOT_MISMATCH = NOT_SUPPORTED (observed value matches)
PROCESS_IMAGE_PATH_EXPERIMENT_RELEVANCE = HIGH
EXACT_CRT_QUICK_EXIT_TO_KERNEL32_FATAL_EXIT = UNPROVEN
```

The static audit cannot say whether QE-1 or QE-2 was the particular source of
the historical `FatalExit` without runtime evidence that does not alter the
successful vendor control. The formerly unresolved pointer is now resolved to
`_wcsicmp`, and the visible QE-1 predicate is resolved to two path-equality
checks. The remaining uncertainty is the private string-object plumbing, the
runtime branch actually taken, and the CRT transition after `quick_exit`.

## UPDATED HYPOTHESIS RANKING

Only three primary candidate families remain:

1. **Process executable-directory identity mismatch** — high confidence as the
   visible QE-1 fatal predicate. The process directory is compared
   case-insensitively with `InstallationPath\\bin` and then
   `InstallationPath\\bin\\AMDPerf`; the hold fixture directory matches
   neither, while the installed vendor `bin` directory matches the first.
2. **Path/registry/string bootstrap construction** — medium confidence as a
   residual implementation risk. The registry value is present and matches
   the known install root, but private string-object copying and separator
   handling are not symbolized.
3. **Remaining CXL initialization/CRT state** — low-to-medium confidence as
   the unresolved residual. QE-2, global state bytes, the exact runtime branch,
   and the CRT transition remain outside the statically resolved predicate.

The earlier static-vs-dynamic load theory remains low priority: static import
alone failed in the hold fixture, while the vendor executable has additional
context. The lifetime evidence downgrades shutdown/detach as the primary
family.

## RECOMMENDED NEXT EXPERIMENT

```text
RECOMMENDED_NEXT_EXPERIMENT = PROCESS_BASENAME_ONLY_CONTROL
WHY_THIS_ONE = The bounded predecessor demonstrably resolves the process
  executable path and strips to its directory before the visible comparisons.
  A byte-identical fixture at a second basename in the same directory would
  isolate any residual hidden basename dependence without changing the visible
  directory predicate, registry, AMD installation, token, CWD, environment,
  or debugger state. This is design-only; do not change both filename and
  directory in one experiment.
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
2. Which exact private string-object operations normalize the two path variants?
3. What runtime `_wcsicmp` results were returned for the two comparisons?
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
INDIRECT_STATUS_SLOT_RESOLUTION = PASS (_wcsicmp)
REGISTRY_READ_ONLY_CROSS_CHECK = PASS (PowerShell + reg.exe)
PATH_EXISTENCE_CHECK = PASS
AMD_RUNTIME_EXECUTED_FOR_THIS_AUDIT = false
SYSTEM_MUTATIONS = none
GIT_DIFF_CHECK = PASS
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

`PROCESS_BASENAME_ONLY_CONTROL` remains design-only. It must be
separately authorized before execution. Do not start
`CPU-SENSOR-AMD-PROVIDER-DESIGN`, B1, profiling, sampling, or a broad vendor
context matrix.
