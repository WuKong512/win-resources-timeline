# CPU-SENSOR-AMD EXECUTABLE-DIRECTORY FINAL RUNTIME CONFIRMATION

This record originally prepared one manual, native runtime counterfactual. The
subsequent user-supplied evidence is now consumed below; Codex did not execute
the AMD runtime. The experiment remains a diagnostic counterfactual, not a
production-provider approval.

## BASELINE

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = 060cd82f2aff0bac0d3a1e93d76fc6b2f73633dd
ORIGIN_MAIN = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
WORKING_TREE_AT_ENTRY = CLEAN
RESULT = PROCESS_DIRECTORY_RUNTIME_CONFIRMATION_PASS
PREPARATION_RESULT = ADMIN_DIRECTORY_COUNTERFACTUAL_REQUIRED
```

The accepted static closure identifies a visible CXL process-executable
directory predicate. `D:\apps\AMDuProf\bin` and
`D:\apps\AMDuProf\bin\AMDPerf` are the accepted directory candidates from
the installed `InstallationPath`; the original repository build directory is
neither candidate.

## CURRENT ROOT-CAUSE HYPOTHESIS

```text
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
RUNTIME_COUNTERFACTUAL_STATUS = CONFIRMED
```

The directory-only run was a confirmation experiment, not a replacement for
the accepted static/evidence-based closure. Its result is recorded in the
final evidence section below.

## WHY THE BASENAME CONTROL WAS CANCELLED

The recovered predicate obtains the host executable directory with
`GetModuleFileNameW(GetModuleHandleW(NULL))`, then compares that directory
against the two installed candidates. No basename operand was recovered.
Consequently a basename-only experiment has no supported discriminating
power and is not part of this run.

## SOURCE FIXTURE

The byte-locked source artifact is:

```text
tools/amd-uprof-public-api-ab/target/release/amd-uprof-static-api-hold-fixture.exe
SHA256 = B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676
ARCHITECTURE = x64
DIRECT_IMPORT_AMDPOWERPROFILEAPI = true
DIRECT_IMPORT_CXLBASETOOLS = false
AMD_API_CALL_FROM_MAIN = false
```

The static preflight performed during preparation re-read the PE import table
and confirmed the direct public API import assertion. The vendor dependency
chain remains:

```text
fixture -> AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll
```

The source binary must not be rebuilt for the authorized run. The historical
source build directory is the known failing directory; the destination is the
only intended variable in the native counterfactual, subject to the normal
Windows executable/module search caveat.

## DESTINATION AND MUTATION BOUNDARY

```text
DESTINATION = D:\apps\AMDuProf\bin\resource-timeline-amd-static-hold-confirm.exe
DESTINATION_DIRECTORY = D:\apps\AMDuProf\bin
ARGUMENTS = none
WORKING_DIRECTORY = D:\apps\AMDuProf\bin
TIMEOUT_MS = 10000
FIXTURE_HOLD_WINDOW_MS = 3000
```

The destination basename intentionally differs from AMD binaries. The
recovered predicate is directory-based, so this does not change the intended
counterfactual. The future manual run may perform exactly one temporary
installation-tree mutation: copy the byte-locked source to the exact
destination after proving the destination is absent. It must hash the copy
before launch, persist qualification before cleanup, remove only that exact
path, and verify its absence. No DLL, registry value, service, driver, PATH,
environment, security setting, or AMD binary may be changed.

Codex performed no installation-tree copy and no runtime cleanup.

## INDEPENDENT VARIABLE

```text
HISTORICAL_EXE_DIRECTORY = repository build directory
COUNTERFACTUAL_EXE_DIRECTORY = D:\apps\AMDuProf\bin
INDEPENDENT_VARIABLE = PROCESS_EXECUTABLE_DIRECTORY
```

The fixture bytes, arguments, inherited environment, manually elevated token,
PowerShell architecture, vendor DLLs, and working directory are intended to
remain the same in the two observations. Executable placement naturally also
participates in Windows loader/search context; because the required vendor
DLLs already reside in the destination directory and no PATH or DLL is
changed, this is the closest native counterfactual for the recovered CXL
policy, not a claim of laboratory-perfect isolation.

## FIXTURE CONTRACT

The copied fixture is the existing hold fixture, unchanged:

1. retain the official `AMDPowerProfileAPI.lib` import pointer;
2. do not invoke an AMD export;
3. synchronously checked-write `HOLD_FIXTURE_MAIN_REACHED=true`;
4. sleep for 3,000 ms;
5. synchronously checked-write `HOLD_FIXTURE_BEFORE_RETURN=true`;
6. return normally and allow normal process shutdown.

It does not call `FreeLibrary`, `ExitProcess`, AMD initialization, profiling,
sampling, or a workload.

## WRAPPER IMPLEMENTATION

The small preparation wrapper is:

`tools/amd-uprof-public-api-ab/run-admin-static-api-hold-in-amd-bin.ps1`

The normal path performs, in order:

1. read-only Administrator/x64 proof without self-elevation;
2. source/API/CXL hash, architecture, signature, and source PE-import
   preflight;
3. exact destination-not-present gate;
4. one non-overwriting byte copy and immediate destination hash check;
5. one native launch with no arguments from the AMD `bin` directory;
6. raw stdout/stderr persistence, signed and hexadecimal exit capture, marker
   parsing, and qualification JSON persistence;
7. exact copied-file cleanup and post-cleanup verification;
8. post-run API/CXL hash checks and a separate cleanup record.

The qualification file is written before the copied file can be removed:

```text
STATIC-HOLD-IN-AMD-BIN.qualification-before-cleanup.json
```

Raw output is persisted before marker classification. Negative target exits
are data, not wrapper exceptions. Timeout cleanup, if ever needed, is limited
to the owned target process tree and is separate from the qualification.

## CLASSIFICATION CONTRACT

| Observation | Classification |
| --- | --- |
| Both durable markers, approximately 3 s hold, exit 0 | `PROCESS_DIRECTORY_RUNTIME_CONFIRMATION = PASS`; directory policy causality confirmed by the byte-identical counterfactual |
| Both markers, then `0xFFFFFFFF` | Startup gate confirmation plus separate shutdown/other failure |
| Main marker only | Startup gate bypassed; additional runtime failure |
| No main marker, `0xFFFFFFFF`, complete capture | Directory change insufficient; another startup prerequisite remains |
| Timeout or incomplete harness evidence | Harness/timeout result; no causal conclusion |

The runtime result must not be inferred from process start alone, and no
second run is authorized automatically.

## CLEANUP GUARANTEE

Cleanup is attempted only after the qualification JSON has been written. The
wrapper verifies the current destination hash before deleting the exact
diagnostic path; a changed hash is left in place and reported rather than
deleting an unknown file. The cleanup record separately reports whether the
destination remains and whether the two checked vendor DLL hashes stayed at
their pre-run values.

## SYNTHETIC VALIDATION

Preparation used `-SyntheticTest`, which launches only Windows PowerShell,
`whoami.exe`, and the temporary synthetic files; it does not load an AMD DLL
or start an AMD executable. The regression summary was:

```text
SYNTHETIC_REGRESSION = PASS
T1 source/destination hash equality = PASS
T2 preexisting destination blocks overwrite = PASS
T3 exact marker parsing = PASS
T4 approximately 3-second successful process = PASS
T5 early signed -1 exit = PASS
T6 raw stdout/stderr including empty streams = PASS
T7 qualification-before-cleanup persistence = PASS
T8 exact-file cleanup = PASS
T9 cleanup verification = PASS
T10 parser failure preserves raw evidence = PASS
T11 timeout behavior = PASS
EXIT_CODE_HEX(-1) = 0xFFFFFFFF
AMD_RUNTIME_EXECUTED = false
```

The `-StaticPreflightOnly` check also passed for the byte-locked source and
the installed vendor API/CXL artifacts, including x64 and PE import assertions.
The repository-built source is intentionally allowed to be unsigned; its hash
and x64/import identity are locked. Vendor DLL signatures remain required.

## PREPARATION-ERA AUTHORIZATION STATUS

```text
ADMIN_DIRECTORY_COUNTERFACTUAL = NOT_EXECUTED
USER_AUTHORIZATION_REQUIRED = true
SYSTEM_MUTATIONS_PERFORMED_BY_CODEX = none
AMD_RUNTIME_EXECUTED_BY_CODEX = false
```

At preparation time, the only authorized future mutation was the exact
temporary diagnostic EXE copy described above, performed from a manually
launched Administrator x64 PowerShell and cleaned after durable evidence was
written. No automatic UAC, elevation, retry, profiling, sampling, or provider
integration was permitted.

## PREPARATION-ERA NEXT STEP (SUPERSEDED)

The prepared command block was later run by the user. The resulting raw
stdout/stderr, qualification JSON, cleanup record, and vendor DLL re-hashes
are consumed in the final section below. Do not run B1 or begin
`CPU-SENSOR-AMD-PROVIDER-DESIGN`.

```text
PREPARATION_RESULT = ADMIN_DIRECTORY_COUNTERFACTUAL_REQUIRED
ADMIN_DIRECTORY_COUNTERFACTUAL_COMMANDS_READY
```

## FINAL RUNTIME EVIDENCE CLOSURE

The authoritative user-generated evidence root is:

```text
C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-directory-confirmation-20260902T072548689Z
```

The run was valid: `ADMIN-00` recorded an x64 PowerShell with Administrator
membership, `whoami /groups` exit `0`, and integrity SID `S-1-16-12288`;
preflight passed for every checked artifact. The source fixture was not
rebuilt, and the destination copy was absent before creation and byte-identical
after creation.

```text
SOURCE_SHA256 = B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676
DESTINATION_SHA256 = B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676
BYTE_IDENTICAL_COPY = true
SOURCE_EXE_DIRECTORY = F:\File\codex\codex-worktrees\08bd\resource-timeline\tools\amd-uprof-public-api-ab\target\release
DESTINATION_EXE_DIRECTORY = D:\apps\AMDuProf\bin
TARGET_PATH = D:\apps\AMDuProf\bin\resource-timeline-amd-static-hold-confirm.exe
PROCESS_STARTED = true
TARGET_PID = 36188
STARTED_AT_UTC = 2026-09-02T07:25:49.0589398Z
FINISHED_AT_UTC = 2026-09-02T07:25:52.1480808Z
DURATION_MS = 3089.141
HOLD_WINDOW_MS = 3000
TIMEOUT_MS = 10000
TIMEOUT = false
TARGET_EXIT_SIGNED = 0
TARGET_EXIT_HEX = 0x00000000
STDOUT_BYTES = 65
STDERR_BYTES = 0
CAPTURE_COMPLETE = true
HARNESS_FAILED = false
TARGET_PROCESS_FAILED = false
QUALIFICATION = PROCESS_DIRECTORY_RUNTIME_CONFIRMATION_PASS
```

The persisted raw stdout was:

```text
HOLD_FIXTURE_MAIN_REACHED=true
HOLD_FIXTURE_BEFORE_RETURN=true
```

This is the requested byte-identical directory counterfactual. The historical
run used the same fixture bytes from the repository build directory and ended
after approximately `63.2 ms` with `-1 / 0xFFFFFFFF`, without either durable
marker. Its working directory was already `D:\apps\AMDuProf\bin`, so the
contrast is not attributable to current working directory alone. The closest
native independent variable is the process executable directory; placement
also naturally participates in Windows loader/search context, so this is not a
claim of laboratory-perfect isolation.

```text
PROCESS_DIRECTORY_RUNTIME_CONFIRMATION = PASS
BYTE_IDENTICAL_DIRECTORY_COUNTERFACTUAL = CONFIRMED
CXL_EXECUTABLE_DIRECTORY_POLICY_CAUSALITY = RUNTIME_CONFIRMED
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
```

Cleanup was evaluated only after qualification had been persisted. The exact
temporary destination was removed and verified absent. The API and CXL vendor
DLLs retained their pre-run hashes:

```text
TEMPORARY_DIAGNOSTIC_COPY_CLEANUP = PASS
CLEANUP_STATUS = REMOVED
DESTINATION_FILE_EXISTS_AFTER_CLEANUP = false
VERIFIED_VENDOR_DLL_INTEGRITY = UNCHANGED
AMDPowerProfileAPI_SHA_BEFORE_AFTER = 9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277
CXLBaseTools_SHA_BEFORE_AFTER = 4815D4631BCA9C051DC4293538DF8D402BD848E705228F497DF718EDCA1F8931
```

`system_mutations_performed = true` in the summary refers to the explicitly
authorized temporary diagnostic copy. It does not establish that the entire
AMD installation tree was unchanged; only the exact copied file and the two
checked vendor DLL identities were verified.

## FINAL ROOT-CAUSE STATUS

The static predicate and runtime counterfactual now agree:

```text
allowed_1 = D:\apps\AMDuProf\bin
allowed_2 = D:\apps\AMDuProf\bin\AMDPerf
exe_dir = directory(GetModuleFileNameW(GetModuleHandleW(NULL)))
if (_wcsicmp(allowed_1, exe_dir) != 0 &&
    _wcsicmp(allowed_2, exe_dir) != 0) {
    quick_exit(0xFFFFFFFF)
}
```

The remaining private details are not required for root-cause closure:

```text
STATIC_VS_DYNAMIC_LOAD_HYPOTHESIS = CLOSED_AS_PRIMARY_CAUSE
PROCESS_BASENAME_HYPOTHESIS = CLOSED_NO_SUPPORT
SIGNATURE_HYPOTHESIS = CLOSED_NO_SUPPORT
VENDOR_IMPORT_TOPOLOGY_HYPOTHESIS = CLOSED_AS_PRIMARY_CAUSE
SHUTDOWN_OR_DETACH_HYPOTHESIS = CLOSED_AS_PRIMARY_CAUSE
VENDOR_EXECUTABLE_SPECIFIC_CONTEXT = REFINED_TO_EXECUTABLE_DIRECTORY_POLICY
CRT_TO_KERNEL32_FATAL_EXIT_TRANSITION = UNPROVEN_STATICALLY_BUT_STRONGLY_RUNTIME_CORRELATED
QE2_EXACT_PRIVATE_ROLE = UNRESOLVED
OTHER_PRIVATE_VENDOR_INTERNALS = NOT_REQUIRED_FOR_ROOT_CAUSE
```

No claim is made that these closed factors can never matter in another vendor
build or process context; they are closed only as the primary explanation for
this incident. No provider implementation, elevation path, B1 run, or new AMD
profiling was performed by Codex.
