# CPU-SENSOR-AMD EXECUTABLE-DIRECTORY FINAL RUNTIME CONFIRMATION

This record prepares one manual, native runtime counterfactual. It does not
run the fixture, load an AMD DLL, start an AMD executable, or change the AMD
installation. The runtime result remains pending human authorization.

## BASELINE

```text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = 060cd82f2aff0bac0d3a1e93d76fc6b2f73633dd
ORIGIN_MAIN = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
WORKING_TREE_AT_ENTRY = CLEAN
RESULT = ADMIN_DIRECTORY_COUNTERFACTUAL_REQUIRED
```

The accepted static closure identifies a visible CXL process-executable
directory predicate. `D:\apps\AMDuProf\bin` and
`D:\apps\AMDuProf\bin\AMDPerf` are the accepted directory candidates from
the installed `InstallationPath`; the original repository build directory is
neither candidate.

## CURRENT ROOT-CAUSE HYPOTHESIS

```text
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = HIGH_FROM_STATIC_ANALYSIS_AND_PRIOR_RUNTIME_DIVERGENCE
RUNTIME_COUNTERFACTUAL_STATUS = NOT_RUN
```

The directory-only run is a confirmation experiment, not a replacement for
the accepted static/evidence-based closure. It must not be described as
completed until the copied fixture produces real runtime evidence.

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

## AUTHORIZATION STATUS

```text
ADMIN_DIRECTORY_COUNTERFACTUAL = NOT_EXECUTED
USER_AUTHORIZATION_REQUIRED = true
SYSTEM_MUTATIONS_PERFORMED_BY_CODEX = none
AMD_RUNTIME_EXECUTED_BY_CODEX = false
```

The only authorized future mutation is the exact temporary diagnostic EXE
copy described above, performed from a manually launched Administrator x64
PowerShell and cleaned after durable evidence is written. No automatic UAC,
elevation, retry, profiling, sampling, or provider integration is permitted.

## NEXT STEP

Run the one prepared Administrator command block exactly once. Consume the
resulting raw stdout/stderr, qualification JSON, cleanup record, and vendor
DLL re-hashes in a separate evidence-closure task. Do not run B1 or begin
`CPU-SENSOR-AMD-PROVIDER-DESIGN`.

```text
RESULT = ADMIN_DIRECTORY_COUNTERFACTUAL_REQUIRED
ADMIN_DIRECTORY_COUNTERFACTUAL_COMMANDS_READY
```
