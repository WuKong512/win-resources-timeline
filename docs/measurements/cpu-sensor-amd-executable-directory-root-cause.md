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
ROOT_CAUSE_CONFIDENCE = HIGH
CRT_TO_KERNEL32_FATAL_EXIT_TRANSITION = UNPROVEN_STATICALLY_BUT_STRONGLY_RUNTIME_CORRELATED
~~~

## ROOT-CAUSE RECONCILIATION

The M1 directory differs from both visible allowed directories, while the
surviving vendor executable directory equals the first allowed directory.
Together with the accepted M1 startup failure and vendor survival evidence,
this explains the observed divergence in substantial part. It does not prove
that the directory predicate is the only vendor bootstrap requirement, nor
that all processes placed in an allowed directory will survive.

~~~text
STATIC_VS_DYNAMIC_LOAD_HYPOTHESIS = DEPRIORITIZED
VENDOR_EXECUTABLE_SPECIFIC_CONTEXT = EXPLAINED_IN_SUBSTANTIAL_PART_BY_EXECUTABLE_DIRECTORY
VENDOR_IMPORT_TOPOLOGY_HYPOTHESIS = DOWNGRADED_AS_PRIMARY_CAUSE
PROCESS_IDENTITY_HYPOTHESIS = REFINED_TO_PROCESS_EXECUTABLE_DIRECTORY_POLICY
SIGNATURE_HYPOTHESIS = NO_SUPPORT
~~~

## BASENAME HYPOTHESIS

The recovered predicate compares the executable directory, not the executable
basename. Therefore:

~~~text
PROCESS_BASENAME_HYPOTHESIS = NOT_SUPPORTED_BY_VISIBLE_FATAL_PREDICATE
PROCESS_BASENAME_ONLY_CONTROL = CANCELLED_NO_DISCRIMINATING_POWER
~~~

No basename experiment was run.

## OPTIONAL DIRECTORY-ONLY CONFIRMATION

A future byte-identical hold-fixture run from an allowed CXL directory could
provide runtime confirmation, but it would require placing a diagnostic file
under the AMD installation tree. That is a separate deliberate mutation and
was not performed here.

~~~text
PROCESS_DIRECTORY_HYPOTHESIS = STRONGLY_SUPPORTED
PROCESS_DIRECTORY_RUNTIME_CONFIRMATION = OPTIONAL_NOT_REQUIRED_FOR_PRIMARY_ROOT_CAUSE
OPTIONAL_DIRECTORY_ONLY_CONFIRMATION = DESIGN_ONLY / NOT_RUN
USER_AUTHORIZATION_REQUIRED = REQUIRED_BEFORE_ANY_INSTALL-TREE_COPY
~~~

## ARCHITECTURAL CONSTRAINT

~~~text
DIRECT_IN_PROCESS_USE_OF_CXL_TRANSITIVE_CHAIN
FROM_ARBITRARY_APPLICATION_DIRECTORY = INCOMPATIBLE_WITH_OBSERVED_CXL_DIRECTORY_POLICY
PROCESS_GLOBAL_CWD_MUTATION = NOT_ACCEPTABLE_AS_DEFAULT_WORKAROUND
~~~

This is an evidence-backed constraint, not a production provider decision.
No registry spoofing, PATH change, DLL replacement, patch, or bypass is
recommended.

## REMAINING UNCERTAINTY

Only these residuals remain:

1. CRT_TO_KERNEL32_FATAL_EXIT_TRANSITION.
2. PRIVATE_IMPLEMENTATION_DETAILS_AROUND_QE1, including string-object
   ownership/normalization and the runtime branch result.
3. QE2_EXACT_ROLE.
4. ANY_OTHER_INDEPENDENT_VENDOR_BOOTSTRAP_REQUIREMENTS.

## VALIDATION AND DELIVERY

~~~text
REGISTRY_EVIDENCE_CROSS_CHECK = PASS
PATH_CONSTRUCTION_CROSS_CHECK = PASS
M1_PATH_CROSS_CHECK = PASS
VENDOR_PATH_CROSS_CHECK = PASS
WINDOWS_CASE_INSENSITIVE_COMPARISON_SANITY = PASS
QE1_PREDICATE_CONSISTENCY = PASS
HISTORICAL_RUNTIME_EVIDENCE_CROSS_CHECK = PASS
AMD_RUNTIME_EXECUTED_FOR_THIS_CLOSURE = false
SYSTEM_MUTATIONS = none
~~~

Updated the existing CXL audit and execution plan; this closure record is new.
No AMD binary or temporary machine-specific artifact was committed.
Documentation-only closure commit: this audit's delivery commit.

## NEXT STEP

No basename control is justified. If additional confirmation is required,
separately authorize one native directory-only confirmation with an explicit
install-tree copy/cleanup protocol. Do not run B1 or start
CPU-SENSOR-AMD-PROVIDER-DESIGN.
