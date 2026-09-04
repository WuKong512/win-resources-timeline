# CPU-SENSOR-AMD CXL EXECUTABLE-DIRECTORY ROOT-CAUSE CLOSURE

This record closes the static/evidence-consumption audit of the visible QE-1
operand. No AMD executable or DLL was run for this closure. The result is
based on the accepted static control-flow audit, read-only registry evidence,
and already-authoritative runtime records.

~~~text
RESULT = PASS_WITH_UNRESOLVED_VENDOR_INTERNAL
STATIC_ANALYSIS_AND_EVIDENCE_CONSUMPTION_ONLY = true
PRODUCTION_INTEGRATION = false
~~~

## BASELINE

~~~text
REPOSITORY = WuKong512/win-resources-timeline
BRANCH = spike/cpu-sensor-amd-uprof-live-qualification
START_HEAD = fca781fa6cc325cd473a4145401fe85c7eae1616
ORIGIN_MAIN = 0c470c48b4e60bd94dfe720ec8981f919db8b1c1
WORKING_TREE_AT_ENTRY = CLEAN
ENTRY_GATE = PASS
DUPLICATE_TASK_GATE = PASS
~~~

No branch reconciliation was performed. origin/main had no relevant drift from
the accepted baseline.

## AUTHORITATIVE EVIDENCE

The lifetime discriminator evidence root was:

~~~text
C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-static-hold-20260901T084518601Z
~~~

The hold fixture record reports process start, approximately 63.2 ms runtime,
timeout false, signed exit -1 / 0xFFFFFFFF, zero-byte persisted stdout and
stderr, capture complete true, and harness failure false. Neither durable main
marker appeared. This remains STARTUP_FAILURE_SUPPORTED with stage
BEFORE_DURABLE_MAIN_MARKER; Rust main entry itself is not proven absent.

The separate vendor no-op record reports AMDuProf.exe alive after approximately
3048 ms, application bootstrap reached, and AMDProfilerService.exe observed as
a child. This confirms vendor executable survival divergence, without by itself
proving which vendor property causes it.

## REGISTRY AND DERIVED DIRECTORIES

The same key/value was read through PowerShell registry access and reg.exe;
neither operation modified the registry.

~~~text
REGISTRY_HIVE = HKLM
REGISTRY_KEY = SOFTWARE\WOW6432Node\AMD\AMDProfiler
REGISTRY_VALUE = InstallationPath
REGISTRY_INSTALLATION_PATH = D:\apps\AMDuProf\
REGISTRY_INSTALL_ROOT_MATCH = MATCH
~~~

The statically recovered concatenation produces:

~~~text
CXL_ALLOWED_DIRECTORY_1 = D:\apps\AMDuProf\bin
CXL_ALLOWED_DIRECTORY_2 = D:\apps\AMDuProf\bin\AMDPerf
~~~

Both derived directories exist on the machine. The alternate candidate
install-root plus AMDPerf without bin was not used because it is not the
recovered append order.

## M1 COUNTERFACTUAL EVALUATION

The authoritative hold-fixture path is:

~~~text
M1_EXE_PATH = F:\File\codex\codex-worktrees\08bd\resource-timeline\tools\amd-uprof-public-api-ab\target\release\amd-uprof-static-api-hold-fixture.exe
M1_EXE_DIRECTORY = F:\File\codex\codex-worktrees\08bd\resource-timeline\tools\amd-uprof-public-api-ab\target\release
M1_SHA256 = B680E7761FC3E64193E7140B57326154A64AB702C62763C7693EA97234DC1676
~~~

Using a transparent Windows-compatible, case-insensitive equality model
corresponding to _wcsicmp, without calling AMD code:

| Comparison | Result |
|---|---|
| _wcsicmp(D:\apps\AMDuProf\bin, F:\File\codex\codex-worktrees\08bd\resource-timeline\tools\amd-uprof-public-api-ab\target\release) | NONZERO |
| _wcsicmp(D:\apps\AMDuProf\bin\AMDPerf, F:\File\codex\codex-worktrees\08bd\resource-timeline\tools\amd-uprof-public-api-ab\target\release) | NONZERO |

Therefore:

~~~text
M1_COMPARE_BIN = NONZERO
M1_COMPARE_AMDPERF = NONZERO
M1_QE1_VISIBLE_PREDICATE = TRUE
~~~

## VENDOR CONTROL EVALUATION

The accepted surviving control is:

~~~text
VENDOR_EXE_PATH = D:\apps\AMDuProf\bin\AMDuProf.exe
VENDOR_EXE_DIRECTORY = D:\apps\AMDuProf\bin
VENDOR_SHA256 = 8F1195F9900C0FF6A5E18AA33127C83DF74622CA7BB68C0C532C3776F6FFD762
~~~

| Comparison | Result |
|---|---|
| _wcsicmp(D:\apps\AMDuProf\bin, D:\apps\AMDuProf\bin) | ZERO |
| _wcsicmp(D:\apps\AMDuProf\bin\AMDPerf, D:\apps\AMDuProf\bin) | NONZERO |

~~~text
VENDOR_COMPARE_BIN = ZERO
VENDOR_COMPARE_AMDPERF = NONZERO
VENDOR_QE1_VISIBLE_PREDICATE = FALSE
~~~

This is a static counterfactual evaluation. AMDuProf.exe was not run again.

## QE-1 AND TERMINATION WORDING

The visible QE-1 path is:

~~~text
exe_dir = directory(GetModuleFileNameW(GetModuleHandleW(NULL)))
path_bin = InstallationPath + bin
path_amdperf = path_bin + \AMDPerf
if (_wcsicmp(path_bin, exe_dir) != 0 &&
    _wcsicmp(path_amdperf, exe_dir) != 0) {
    quick_exit(0xFFFFFFFF)
}
~~~

CXL does not directly import or call KERNEL32!FatalExit. It invokes CRT
quick_exit(0xFFFFFFFF) on the visible candidate path. The historical
KERNEL32!FatalExit(0xFFFFFFFF) stack correlates strongly with QE-1, but the
CRT-to-Kernel32 transition remains unproven statically.

~~~text
FATAL_CONDITION_FAMILY = MODULE_IDENTITY_FAILURE
ROOT_CAUSE = CXL_PROCESS_EXECUTABLE_DIRECTORY_POLICY_MISMATCH
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
CRT_TO_KERNEL32_FATAL_EXIT_TRANSITION = UNPROVEN_STATICALLY_BUT_STRONGLY_RUNTIME_CORRELATED
~~~

## ROOT-CAUSE RECONCILIATION

The M1 directory differs from both visible allowed directories, while the
surviving vendor executable directory equals the first allowed directory.
The subsequent byte-identical directory counterfactual moved only the hold
fixture's executable directory to the first allowed directory and changed the
outcome from an early `0xFFFFFFFF` termination to two durable main markers and
normal exit. This closes the recovered directory predicate as the causal
explanation for this incident. It does not prove that every other vendor build
or unrelated CXL policy has the same behavior.

~~~text
STATIC_VS_DYNAMIC_LOAD_HYPOTHESIS = CLOSED_AS_PRIMARY_CAUSE
VENDOR_EXECUTABLE_SPECIFIC_CONTEXT = REFINED_TO_EXECUTABLE_DIRECTORY_POLICY
VENDOR_IMPORT_TOPOLOGY_HYPOTHESIS = CLOSED_AS_PRIMARY_CAUSE
PROCESS_IDENTITY_HYPOTHESIS = REFINED_TO_PROCESS_EXECUTABLE_DIRECTORY_POLICY
SIGNATURE_HYPOTHESIS = CLOSED_NO_SUPPORT
SHUTDOWN_OR_DETACH_HYPOTHESIS = CLOSED_AS_PRIMARY_CAUSE
~~~

## BASENAME HYPOTHESIS

The recovered predicate compares the executable directory, not the executable
basename. Therefore:

~~~text
PROCESS_BASENAME_HYPOTHESIS = NOT_SUPPORTED_BY_VISIBLE_FATAL_PREDICATE
PROCESS_BASENAME_ONLY_CONTROL = CANCELLED_NO_DISCRIMINATING_POWER
~~~

No basename experiment was run.

## DIRECTORY-ONLY RUNTIME CONFIRMATION

The previously planned byte-identical hold-fixture run was subsequently
performed by the user under the separately authorized installation-tree
mutation protocol. It used the exact source binary from the failing directory,
copied it without modification to `D:\apps\AMDuProf\bin`, and removed only that
exact diagnostic copy after qualification had been persisted. The complete
evidence is recorded in
[`cpu-sensor-amd-executable-directory-runtime-confirmation.md`](cpu-sensor-amd-executable-directory-runtime-confirmation.md).

~~~text
PROCESS_DIRECTORY_HYPOTHESIS = STRONGLY_SUPPORTED
PROCESS_DIRECTORY_RUNTIME_CONFIRMATION = PASS
BYTE_IDENTICAL_DIRECTORY_COUNTERFACTUAL = CONFIRMED
CXL_EXECUTABLE_DIRECTORY_POLICY_CAUSALITY = RUNTIME_CONFIRMED
ROOT_CAUSE_CONFIDENCE = CONFIRMED_BY_STATIC_AND_RUNTIME_COUNTERFACTUAL
TEMPORARY_DIAGNOSTIC_COPY_CLEANUP = PASS
VERIFIED_VENDOR_DLL_INTEGRITY = UNCHANGED
~~~

## ARCHITECTURAL CONSTRAINT

~~~text
DIRECT_IN_PROCESS_USE_OF_CXL_TRANSITIVE_CHAIN
FROM_ARBITRARY_APPLICATION_DIRECTORY = INCOMPATIBLE_WITH_OBSERVED_CXL_DIRECTORY_POLICY
PROCESS_GLOBAL_CWD_MUTATION = NOT_ACCEPTABLE_AS_DEFAULT_WORKAROUND
~~~

This is an evidence-backed constraint, not a production provider decision.
No registry spoofing, PATH change, DLL replacement, patch, or bypass is
recommended. The exact vendor executable-directory requirement is now closed
for this incident; the CRT-to-`KERNEL32!FatalExit` transition and QE2 private
role remain implementation details that are not needed before architecture
work.

## REMAINING UNCERTAINTY

Only these residuals remain, and none is required before architecture work:

1. `CRT_TO_KERNEL32_FATAL_EXIT_TRANSITION` — unproven statically but strongly
   runtime correlated.
2. `QE2_EXACT_PRIVATE_ROLE` — unresolved.
3. Other private vendor internals — not required for the closed root cause.

## VALIDATION AND DELIVERY

~~~text
REGISTRY_EVIDENCE_CROSS_CHECK = PASS
PATH_CONSTRUCTION_CROSS_CHECK = PASS
M1_PATH_CROSS_CHECK = PASS
VENDOR_PATH_CROSS_CHECK = PASS
WINDOWS_CASE_INSENSITIVE_COMPARISON_SANITY = PASS
QE1_PREDICATE_CONSISTENCY = PASS
HISTORICAL_RUNTIME_EVIDENCE_CROSS_CHECK = PASS
AMD_RUNTIME_EXECUTED_BY_CODEX_FOR_THIS_STATIC_CLOSURE = false
FINAL_USER_RUNTIME_EVIDENCE_CONSUMED = true
TEMPORARY_DIRECTORY_COPY_MUTATION = AUTHORIZED_AND_REMOVED
CHECKED_VENDOR_DLLS = UNCHANGED
~~~

Updated the existing CXL audit and execution plan; this closure record is new.
No AMD binary or temporary machine-specific artifact was committed.
Documentation-only closure commit: this audit's delivery commit.

## NEXT STEP

The directory-only root-cause confirmation is complete. The next decision is
the separate architecture record
[`cpu-sensor-amd-provider-architecture.md`](../architecture/cpu-sensor-amd-provider-architecture.md).
No basename control, B1 run, or process-global CWD workaround is justified.
Provider implementation remains a separate, explicitly gated task.
