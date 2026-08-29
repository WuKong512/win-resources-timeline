# CPU-SENSOR-AMD DLL LOAD ABORT ROOT-CAUSE QUALIFICATION

Qualification date: 2026-08-28 (Asia/Shanghai). This is a follow-up to the historical `CPU-SENSOR-AMD-LIVE-QUALIFICATION` record. It is a qualification-only investigation for the current Windows machine, not production integration and not permission to redistribute AMD software.

## Result

`RESULT: BLOCKED`

`ROOT_CAUSE: DEPENDENCY_LOAD_FAILURE`

`LOAD_ABORT_RESOLVED: false`

The original zero-sample blocked result is preserved in [`cpu-sensor-amd-uprof-live-qualification.md`](cpu-sensor-amd-uprof-live-qualification.md). This follow-up reproduced the boundary in a smaller loader-only child and captured the termination: `KERNEL32!FatalExit` was reached with `0xFFFFFFFF` while the vendor dependency chain was being loaded. The first component proven to trigger that termination was `CXLBaseTools.dll`, transitively loaded by `AMDSysUtils.dll`. The exact internal vendor condition that causes `CXLBaseTools.dll` to call `FatalExit` is not proven.

The official AMD CLI provided a separate control: in its full installed process context the API DLL and companion module graph loaded, `AMDTPwrProfileInitialize(0)` was reached, and it returned `0x80070005` (`AMDT_ERROR_ACCESSDENIED`) under the current non-admin token. It did not enumerate counters. This is useful privilege evidence, but it does not provide live metric values and does not resolve the Resource Timeline-owned minimal-load abort.

## Baseline and entry gate

- Repository: `WuKong512/win-resources-timeline`
- `BASE_COMMIT`: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`
- `START_HEAD`: `17fdab93922afbf9cdbc3e7bcb574a7bead99175`
- `origin/main`: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`
- Branch: `spike/cpu-sensor-amd-uprof-live-qualification`
- Worktree: this worktree already owned the expected qualification branch and was clean before the follow-up. `main` remained checked out in another worktree; no force checkout or branch theft was performed.
- Entry checks: `git fetch origin --prune` passed; repository identity passed; the historical head exists; the Q1 merge commit is an ancestor of `origin/main`; local qualification head was the expected existing branch head.
- `DUPLICATE_TASK_GATE`: `PASS` for local/remote branches and commits. `gh` was unavailable, so open-PR visibility could not be queried through GitHub tooling.
- Drift: none relative to the authoritative baseline; `origin/main` still points to the Q1 merge commit. The previous qualification head is retained as history and was not reset, rebased, merged, cherry-picked, or amended.
- Historical previous blocker: `AMD_UPROF_LIBRARY_LOAD_ABORTS_PROBE`, with no AMD API samples. It remains historical evidence and is not rewritten.
- Specification/plan state: Q1 static source qualification is merged; this follow-up remains blocked before Resource Timeline-owned API initialization.

## AMD uProf installation and artifact audit

- `DOWNLOADED_INSTALLER`: `NO` in the searched user download scope; no installer was executed.
- `INSTALLED`: `YES`
- `INSTALLED_VERSION`: `5.3.521` / file and product version `5.3.521.0` for the user-mode uProf chain.
- `INSTALL_ROOT`: `D:\apps\AMDuProf\`; the standard `C:\Program Files\AMD\AMDuProf\` root was absent.
- `API_LIBRARY`: `D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll`
- API SHA-256: `9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A427`
- API architecture: x64; PE32+; subsystem Windows GUI; 58 named exports; no delay-import directory was observed by the available PE parser.
- API signature: Authenticode `Valid`; signer `CN=Advanced Micro Devices, O=Advanced Micro Devices, S=California, C=US`.
- `HEADER`: `D:\apps\AMDuProf\include\AMDTPowerProfileApi.h` and `AMDTPowerProfileDataTypes.h`, read as the installed contract.
- `API_PDF`: `D:\apps\AMDuProf\Help\AMDPowerProfilerAPI.pdf`, present. Where the PDF conflicted with the installed header, the installed header remained authoritative.
- `OFFICIAL_SAMPLE`: `D:\apps\AMDuProf\Examples\CollectAllCounters\CollectAllCounters.cpp`, present and read. No vendor source was copied into production code or committed.
- Installed version markers: `D:\apps\AMDuProf\bin\AMDPerf\metadata\version.txt` and `D:\apps\AMDuProf\bin\Data\Config\Version.txt` both identify `5.3.521.0`.
- No AMD installer log was found in the bounded installed-root search. The installed API, companion DLLs, drivers, headers, PDF, and sample were inspected in place only.

## Load-abort classification

### Minimal loader reproduction

The new diagnostic command is `amd-uprof-load-only-child`. It accepts only an absolute path, canonicalizes it, emits `BEFORE_LOAD`, invokes exactly `LoadLibraryExW`, and does not resolve or call an AMD export. The parent captures the child's stdout/stderr, signed exit code, unsigned hexadecimal representation, and timeout state. The parent survives and fails closed.

Observed reproduction:

```text
BEFORE_LOAD path=\\?\D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll flags=0x00000900 process_architecture=x64
signed exit code: -1
unsigned exit code: 0xFFFFFFFF
stdout after BEFORE_LOAD: none
stderr: empty
```

The child did not return from the loader call. A parent probe run recorded `isolated_load_exit_code=-1`, `isolated_load_exit_code_hex=0xFFFFFFFF`, `isolated_load_timed_out=false`, and `API_not_called`; the session/event count stayed empty. This is not described as an ordinary `LoadLibrary` error.

### Debugger evidence

An already-installed x64 CDB was attached to the loader-only child. The decisive breakpoint was:

- `KERNEL32!FatalExit`, with `rcx=00000000ffffffff`.
- The return path was in UCRT; the stack included `CXLBaseTools!gtString::asWideString+0x458` and `CXLBaseTools!gtStringTokenizer::getNextToken+0x2f96`, followed by `ntdll!LdrLoadDll`, `KERNELBASE!LoadLibraryExW`, and the probe's load-only function.
- The same `FatalExit(0xFFFFFFFF)` boundary occurred when loading `CXLBaseTools.dll` directly in its own short-lived child.
- No target access-violation breakpoint, fail-fast, UCRT `abort`, invalid-parameter handler, or exception fault code was observed. The debugger's initial break-in exception was normal debugger startup behavior, not the target termination.

`LOAD_ABORT_CLASSIFICATION`: `EXPLICIT_FATAL_EXIT_DURING_VENDOR_DEPENDENCY_LOAD`; no Windows exception/WER fault classification was established. There was no matching Application Error, WER, SideBySide, Code Integrity, AppLocker, Defender, or AMD service/driver event in the reproduction window. WER archive inspection was partially inaccessible to the non-admin token, so absence of an archived report is not treated as proof that no hidden report exists.

This proves a process termination at the vendor dependency-load boundary. It does not prove the private `CXLBaseTools` code path or its internal reason for calling `FatalExit`.

## Probe implementation audit

- Process architecture: x64, matching the installed API DLL.
- Path: verified absolute path, canonicalized before the child is spawned; the child receives the exact API path, not a directory or bare DLL name.
- Load flags: `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32` (`0x00000900`).
- Search behavior: no CWD lookup, arbitrary PATH fallback, or unknown side-by-side fallback in the Resource Timeline probe.
- FFI: UTF-16 NUL-terminated path, `PCWSTR`, default module handle, and the Windows loader call are used with the installed Rust Windows bindings.
- Lifetime: the UTF-16 buffer remains alive for the call; the child is isolated and short-lived. It does not explicitly unload a successfully mapped diagnostic module.
- API ordering: no AMD export is resolved or invoked in the load-only child. The parent only enters API qualification after a normal child exit.
- Failure handling: the parent records abnormal termination and does not synthesize metric values. The child has no `unwrap`/panic path in the loader operation.
- Regression coverage: the probe test preserves signed `-1` and hexadecimal `0xFFFFFFFF` separately and verifies that the observation is not treated as success.

The loader-only child reproduces the abort independently of the higher-level sampling path. The official sample result below provides an additional non-Rust control.

## PE and dependency analysis

`AMDPowerProfileAPI.dll` directly imports `AMDSysUtils.dll`, the Microsoft runtime, and ordinary Windows system/API-set libraries. `AMDSysUtils.dll` directly imports `CXLBaseTools.dll` and the same runtime family. `CXLBaseTools.dll` imports the Microsoft runtime and ordinary Windows libraries. The API's required exported names match the installed header, including initialize, supported-counter enumeration, enable, timer period, start, read, stop, and close.

The bounded search found the AMD/local chain only under `D:\apps\AMDuProf\bin`; no second uProf copy was found under the checked Program Files AMD root or other bounded uProf roots. `D:\apps\AMDuProf\bin` is on PATH, but the Resource Timeline loader does not use arbitrary PATH selection. The official CLI itself resolved its AMD modules from the same D: installation and Microsoft runtime modules from System32.

### Dependency resolution table

| DLL | Resolved/checked path | Version | Architecture | Signature | Resolution observation |
|---|---|---:|---|---|---|
| `AMDPowerProfileAPI.dll` | `D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll` | `5.3.521.0` | x64 | Valid AMD | PE map succeeds; normal load child terminates via vendor dependency path |
| `AMDSysUtils.dll` | `D:\apps\AMDuProf\bin\AMDSysUtils.dll` | `5.3.521.0` | x64 | Valid AMD | Direct normal load reaches transitive `CXLBaseTools.dll` abort |
| `CXLBaseTools.dll` | `D:\apps\AMDuProf\bin\CXLBaseTools.dll` | `5.3.521.0` | x64 | Valid AMD | Direct normal load triggers `FatalExit(0xFFFFFFFF)` |
| `MSVCP140.dll` | `C:\Windows\System32\MSVCP140.dll` | `14.50.35719.0` | x64 | Valid Microsoft | Loader-only child returns normally |
| `VCRUNTIME140.dll` | `C:\Windows\System32\VCRUNTIME140.dll` | `14.50.35719.0` | x64 | Valid Microsoft | Loader-only child returns normally |
| `VCRUNTIME140_1.dll` | `C:\Windows\System32\VCRUNTIME140_1.dll` | `14.50.35719.0` | x64 | Valid Microsoft | Loader-only child returns normally |
| `api-ms-win-crt-*` | `C:\Windows\System32\downlevel\` implementations | `10.0.26100.1` | x64 | Valid Microsoft | Checked runtime API-set implementations load normally |
| `ADVAPI32.dll`, `SHLWAPI.dll`, `USER32.dll` | `C:\Windows\System32\` | OS version | x64 | System-signed | Loader-only child returns normally |
| `KERNEL32.dll` | System-resident core module | OS version | x64 | System-signed | Explicit standalone path probing is not a meaningful missing-DLL test; normal process module resolution is present |

No external dependency was proven missing. A map-only (`DONT_RESOLVE_DLL_REFERENCES`) diagnostic mapped the API, `AMDSysUtils.dll`, and `CXLBaseTools.dll` successfully. That separates PE image mapping from normal import resolution/initialization; it is not treated as a safe API-load result.

### Dependency load matrix

Each row used the same isolated, loader-only child and did not resolve or call exports.

| DLL | Resolved path | Version | Load result | Exit | Loader error/event |
|---|---|---:|---|---|---|
| `AMDPowerProfileAPI.dll` | `D:\apps\AMDuProf\bin\` | `5.3.521.0` | Abort during dependency load | `-1` / `0xFFFFFFFF` | `FatalExit`; no matching Windows event |
| `AMDSysUtils.dll` | `D:\apps\AMDuProf\bin\` | `5.3.521.0` | Abort through transitive CXL load | `-1` / `0xFFFFFFFF` | Same `CXLBaseTools` stack boundary |
| `CXLBaseTools.dll` | `D:\apps\AMDuProf\bin\` | `5.3.521.0` | Abort | `-1` / `0xFFFFFFFF` | `KERNEL32!FatalExit(0xFFFFFFFF)` |
| Microsoft CRT/API-set/UI libraries | System32/downlevel | OS/runtime versions | Normal child completion | `0` | No loader error |

The matrix identifies `CXLBaseTools.dll` as the first proven aborting component in the tested vendor chain. It does not establish whether the trigger is a particular initialization ordering, process context, or another private vendor condition.

## VC runtime

`VC_RUNTIME_STATUS = OK` for presence and ordinary resolution evidence:

- x64 registry metadata: `HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64`, `Installed=1`, version `v14.50.35719.00`.
- System32 `MSVCP140.dll`, `VCRUNTIME140.dll`, and `VCRUNTIME140_1.dll` are present, Microsoft-signed, and version `14.50.35719.0`.
- `ucrtbase.dll` is present and Microsoft-signed; required CRT API-set implementation files are present under System32/downlevel.
- The relevant runtime files load normally in the isolated dependency matrix.

This rules out a simple missing x64 VC++ runtime based on available evidence. It does not rule out an undocumented vendor/runtime interaction inside the CXL code.

## AMD component version coherence

`AMD_COMPONENT_COHERENCE = INCONCLUSIVE`.

The user-mode API and directly related uProf DLLs are a coherent `5.3.521.0` set and are AMD-signed. `AMDCpuProfiler.sys` reports file version `4.4.1.0` and product version `5.3.481.0`; `AMDPowerProfiler.sys` reports file version `10.6.3.0` and product version `5.3.481.0`. Both are valid AMD-signed drivers. The relevant services point to those exact files and were running without mutation. No installer history was available to establish whether the driver product metadata is a legitimate package relationship or a stale component. Version strings alone are therefore not called a mixed install.

## Official AMD controls

### CLI

`AMDuProfCLI.exe --version` and `--help` completed normally and identified `5.3.521.0`. The documented `timechart` path was inspected from the installed executable. A bounded `timechart --list` and a one-second `timechart --event power --interval 1000 --duration 1` invocation printed `ERROR: There is no counters avialable` and did not emit numeric samples. CDB showed the CLI loaded the complete AMD companion graph, reached `AMDTPwrProfileInitialize(0)`, returned `0x80070005` (`AMDT_ERROR_ACCESSDENIED`), and did not call `AMDTPwrGetSupportedCounters`.

The redirected `timechart -h` invocation waited at the CLI's `Press any key...` prompt and was terminated after the bounded timeout; only that owned CLI process was terminated. It was not treated as a vendor crash.

`AMD_OFFICIAL_CLI_CONTROL = PERMISSION_BLOCKED` for the useful power-profile control. The version/help subcommands are `PASS`, but they do not prove the power API works.

### CollectAllCounters sample

The exact installed `CollectAllCounters.cpp` was compiled with the already-installed x64 MSVC toolchain (`C:\BuildTools`) against the installed include/lib directories. The sample semantics were not changed. The isolated executable ran from `D:\apps\AMDuProf\bin` under the same non-admin token, emitted no sample/status output, and exited `-1` / `0xFFFFFFFF` before it could report initialization. A separate debugger trace was not attached to the sample, so its precise internal abort frame is not claimed beyond consistency with the loader-only result.

`OFFICIAL_SAMPLE_CONTROL = ABORT`. This independently weighs against the custom Rust loader being the primary cause of the minimal API-load termination, while the official CLI demonstrates that a larger AMD process context can load the graph far enough to return a permission status.

## Security and platform state

`APPLICATION_CONTROL_STATUS = CLEAR` within observable evidence: no matching Code Integrity, AppLocker, Defender, SideBySide, WER, or Application Error block was found for the reproduction window; the checked AMD chain is Authenticode-valid. This is scoped to observable logs and policy state and is not a claim that every third-party security control is absent.

- `MICROSOFT_HYPERVISOR`: `ENABLED`
- `VBS`: `ENABLED`
- `HVCI`: `ENABLED`
- `PLATFORM_MUTATIONS`: `NO`

No direct evidence connects Hyper-V, VBS, or HVCI to the `FatalExit`. They remain platform context, not the selected root cause.

## Root cause decision gate

Exactly one primary classification is selected:

`ROOT_CAUSE = DEPENDENCY_LOAD_FAILURE`

- Confidence: `HIGH` for the observed abort boundary and first proven component; `MEDIUM` for the underlying vendor-internal trigger, which remains unknown.
- Decisive evidence: loader-only child reproduces `-1` / `0xFFFFFFFF`; CDB catches `KERNEL32!FatalExit` with `0xFFFFFFFF`; direct `CXLBaseTools.dll` load reproduces the same boundary; PE map-only and Microsoft runtime loads succeed.
- Disproven or unsupported as primary causes: PE mapping failure; simple missing VC runtime; API export/signature invocation; ordinary Rust parent failure; generic application-control block; Hyper-V/VBS/HVCI causation.
- Remaining hypotheses: an undocumented condition in or immediately triggered by the vendor `CXLBaseTools` load/init path; companion-module/process-context differences; and unresolved version coherence between user-mode `5.3.521` and driver product metadata `5.3.481.0`. None is silently selected as the exact internal cause.

This is classified as dependency-load failure rather than simply “LoadLibrary failed.” The mechanism is an explicit vendor-triggered process termination during the dependency path. `VENDOR_DLL_INITIALIZATION_ABORT` describes the observed mechanism inside that boundary but is not a second primary classification.

## User authorization required and remediation boundary

`USER_AUTHORIZATION_REQUIRED = YES` for any administrator comparison. The current non-admin run must remain the first and only automatically performed privilege level.

- Exact proposed action: after explicit user authorization, the user launches the already-built qualification probe and the installed AMD CLI from an Administrator console for a bounded initialization/control comparison, with Hyper-V/VBS/HVCI, services, drivers, PATH, registry, and installation unchanged. No self-elevation or elevated helper is created by the probe.
- Why: the official CLI reached `AMDTPwrProfileInitialize` and returned `AMDT_ERROR_ACCESSDENIED`; an admin comparison is needed to distinguish a privilege-only blocker from the independent minimal-child dependency abort.
- Expected effect: either obtain a vendor init/enumeration status under admin or reproduce the same dependency abort without changing the platform. It is not expected to repair the vendor installation.
- Rollback: none for the test; close the owned processes normally. No service, driver, registry, boot, security-policy, or installation rollback is needed.
- Admin/reboot: Administrator authorization is required; reboot is not required for this comparison. The user must launch the process; Codex did not request UAC elevation.
- Evidence to collect afterward: signed/hex child exit, complete bounded stdout/stderr, exact AMD status, counter enumeration count only if the API actually returns one, and whether the same CXL boundary is observed.

No installer repair, runtime installation, driver change, service change, security-policy change, boot change, or platform mutation is proposed or authorized by this task. If a future vendor-supported remediation is considered after the above evidence, it requires a separate explicit user decision and must be reviewed for admin/reboot, rollback, and legal implications first.

## Targeted live requalification gate

The ordered gate stopped at step 1:

1. Load DLL: `BLOCKED`; Resource Timeline-owned minimal child terminates before return.
2. Resolve symbols: `NOT_REACHED`.
3. Initialize once: `NOT_REACHED` in the Resource Timeline probe. Official CLI control separately returned `0x80070005`.
4. Enumerate counters: `NOT_REACHED`; no count was returned.
5. Minimal counter/sample, cadence, idle/load/recovery, busy, lifecycle, and performance: `NOT_RUN`.

The probe emitted no synthetic CPU power, temperature, or frequency numbers. There was no API session, no Resource Timeline sampler, and no vendor counter sample in this follow-up.

## Package power

- `STATUS`: `NOT_REACHED; DEFER`
- Candidate descriptor contract: package scope; watts; average over the vendor sampling period; `ESTIMATED`; not instantaneous and not external wall power.
- `SANITY`: `NOT_RUN`
- `IDLE`: `NOT_RUN`
- `LOAD`: `NOT_RUN`
- `RECOVERY`: `NOT_RUN`
- `SOURCE_DECISION`: `DEFER`

No numeric zero is used as a failure value. The historical zero-sample result means no API sample existed, not that package power was zero.

## Effective frequency

- `STATUS`: `NOT_REACHED; DEFER`
- `IDENTITY_SEMANTICS`: `NOT_REACHED`; no live descriptor was enumerated, so physical-core versus compute-unit versus logical-processor identity is not inferred.
- `UNIT`: `NOT_REACHED` (the live descriptor was not returned).
- `WINDOW`: `NOT_REACHED`.
- `IDENTITY_COUNT`: `NOT_REACHED`; no count was returned.
- `IDLE`: `NOT_RUN`
- `LOAD`: `NOT_RUN`
- `MISSING_IDENTITY_BEHAVIOR`: `NOT_REACHED`
- `SOURCE_DECISION`: `DEFER`
- `PRODUCT_AGGREGATION_DECISION`: `DEFER_AGGREGATION_CONTRACT`; no package headline or `cpu.effective_clock_mhz` was created.

## Package temperature

- `STATUS`: `DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD`
- `MICROSOFT_HYPERVISOR_EFFECT`: `NOT_ESTABLISHED`; Hyper-V remained enabled, but no direct evidence ties it to the abort.
- `API_BEHAVIOR`: `NOT_OBSERVED`; no temperature counter was enumerated, and `UNSUPPORTED` was not inferred.
- `SOURCE_DECISION`: `DEFER`

## Cadence, concurrency, and lifecycle

### Cadence

No cadence run was allowed after the load gate failed. “Samples: 0” below means no API run, not a numeric metric value.

| Requested | Actual | Samples | Late | Dropped | API average latency | API p95 latency | API max latency |
|---:|---|---|---|---|---|---|---|
| 500 ms | N/A; not reached | no API run | N/A | N/A | N/A | N/A | N/A |
| 1000 ms | N/A; not reached | no API run | N/A | N/A | N/A | N/A | N/A |
| 2000 ms | N/A; not reached | no API run | N/A | N/A | N/A | N/A | N/A |

`PRODUCT_CANDIDATE_INTERVAL = NOT_SELECTED`.

### Concurrency

- `SINGLE_SESSION_RULE`: installed contract/Q1 evidence records one power-profile session at a time; not exercised in this follow-up.
- `SECOND_SESSION_RESULT`: `NOT_RUN`.
- `VENDOR_STATUS`: `N/A`; no second session was started.
- `PROVIDER_BUSY_MAPPING`: `NOT_VALIDATED`; do not map the unobserved status to `provider_busy`.
- `FIRST_SESSION_SURVIVED`: `N/A`.
- `RETRY_BEHAVIOR`: no retry storm; one bounded load attempt per diagnostic run.
- `RYZEN_MASTER_INTERACTION`: `NOT_ESTABLISHED`; no other application was killed or modified.

### Lifecycle

- `ENABLE`: `NOT_RUN`; no API session or poller was created.
- `DISABLE`: `NOT_RUN` as a provider lifecycle; the parent failed closed and owned child processes were reaped.
- `QUIESCENCE`: load child ended; no Resource Timeline polling remained.
- `RE_ENABLE`: `NOT_RUN`.
- `FINAL_DISABLE`: `NOT_RUN` as a provider lifecycle.
- `PROCESS_EXIT`: parent probe exited normally after recording the child failure.
- `POLLS_AFTER_DISABLE`: `0` because no poller started; this is not a lifecycle pass.
- `RESOURCE_TIMELINE_OWNED_LEAKS`: none observed in the bounded diagnostic path; full API lifecycle remains unqualified.
- `AMD_EXTERNAL_DRIVER_RESIDENCY`: drivers/services remained resident and unchanged, as expected; no attempt was made to unload them.

## Failure isolation and performance

### Failure isolation

- `MISSING_LIBRARY`: guarded by absolute/canonical installation verification; no installed vendor file was removed or altered.
- `UNSAFE_PATH`: fail-closed; relative paths and unverified roots are rejected, with no random DLL fallback.
- `UNSUPPORTED_COUNTER`: `NOT_REACHED`.
- `PERMISSION`: official CLI returned exact `0x80070005`; Resource Timeline non-admin API permission result was not reached.
- `BUSY`: `NOT_RUN`.
- `INVALID_VALUE`: `NOT_REACHED`; no numeric value was emitted.
- `TIMEOUT/LATE`: the five-second child guard was not triggered for the reproduced abort; no sample cadence was started.
- `ZERO_SYNTHESIS`: `PASS`; failure is represented by status/error fields and no fake numeric zero.

### Performance

- `AVG_CPU`: `N/A`; no source polling run.
- `P95_CPU`: `N/A`.
- `PEAK_WORKING_SET`: `N/A` for source qualification.
- `HANDLE_COUNT`: `N/A` for source qualification.
- `THREAD_COUNT`: `N/A` for source qualification.
- `API_AVG_LATENCY`: `N/A`; API was not entered by the Resource Timeline probe.
- `API_P95_LATENCY`: `N/A`.
- `API_MAX_LATENCY`: `N/A`.
- `AMD_EXTERNAL_OVERHEAD`: `NOT_ATTRIBUTABLE`.

No long soak was run. The short loader diagnostic is not represented as all-day collector overhead evidence.

## External reference

- `AFTERBURNER_RTSS_STATE`: naturally running (`MSIAfterburner`, `RTSS`, and `RTSSHooksLoader64` were observable).
- `MODIFIED`: `NO`.
- `COMPARISON_PERFORMED`: `NO`; no read-only comparison was necessary or safe once the source gate failed.
- `COMPARISON_INTERPRETATION`: N/A. Afterburner/RTSS was not treated as ground truth, and no polling/settings/injection change was made.

## Metric and production decisions

- `CPU_PACKAGE_POWER`: `DEFER`; no live package-power values or cadence/lifecycle evidence.
- `AMD_CORE_EFFECTIVE_FREQUENCY`: `DEFER`; no live descriptor identity or repeated values.
- `CPU_EFFECTIVE_FREQUENCY`: `DEFER_AGGREGATION_CONTRACT`; aggregation was not evaluated or added.
- `CPU_PACKAGE_TEMPERATURE`: `DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD`; not labeled unsupported by Hyper-V.

`SOURCE_DECISION = DEFER`. The source cannot enter optional-provider design from this follow-up because the Resource Timeline-owned load gate is unresolved and the only successful API entry evidence is a non-admin permission denial with no counter enumeration.

Production boundary remains unchanged:

- `PRODUCTION_INTEGRATION`: `NO`
- `PROVIDER_ADDED`: `NO`
- `METRIC_CATALOG_CHANGED`: `NO`
- `SCHEMA_CHANGED`: `NO`
- `UI_CHANGED`: `NO`
- `DISTRIBUTION`: `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`
- Deployment model, if ever revisited: `EXTERNAL_INSTALLED_DEPENDENCY` only; no AMD DLL, driver, header, PDF, sample, installer, or license was copied or committed.

## Validation and delivery

The follow-up changes are limited to the qualification probe's loader-only diagnostic/capture path and this documentation/execution-plan status. No production collector, ProviderHost behavior, CollectionPlan contract, MetricCatalog, schema, migration, dashboard, or metric explorer was modified.

- `METRIC_PROBE_FMT`: `PASS` (`cargo fmt --manifest-path tools/metric-probe/Cargo.toml -- --check`).
- `METRIC_PROBE_TESTS`: `PASS` — `65 passed; 0 failed; 0 ignored; 0 measured`.
- `METRIC_PROBE_RELEASE_BUILD`: `PASS`.
- `SRC_TAURI_FMT`: `PASS` (`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`).
- `SRC_TAURI_CHECK`: `PASS`.
- `SRC_TAURI_TESTS`: `PASS` — `225 passed; 0 failed; 2 ignored; 0 measured`; the binary and doctest targets also completed with zero tests.
- `DIFF_CHECK`: `PASS` (`git diff --check`).
- `DELIVERY`: the historical qualification commit was not rewritten. The follow-up delivery commit, push state, and final clean worktree are recorded with the final report.
- Vendor artifacts: not committed; temporary sample build outputs remain outside tracked files/under ignored diagnostic output and were cleaned where created at repository root.

## ADMIN PRIVILEGE COMPARISON

This is a subsequent evidence-consumption record for the controlled administrator comparison. The historical non-admin evidence above is preserved; the original zero-sample blocked result is not rewritten. No AMD test was rerun during evidence consumption.

### Evidence directory and capture integrity

- Evidence directory: `C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-admin-comparison-v2-20260829T031818104Z`
- Summary: `ADMIN-COMPARISON-SUMMARY.json`, captured `2026-08-29T03:18:26.8395122Z`.
- `ADMIN-00`: passed. The manually launched process was x64, `WindowsPrincipal.IsInRole(Administrator)=true`, integrity SID `S-1-16-12288`, and `whoami /all` exited `0` / `0x00000000`. The recorded user was `ODETOMOUNTASEAS\\Hello`; the initial current directory was `C:\Users\Hello`.
- A, B, D0, D1, D2, and E each have an independent JSON, stdout, and stderr file. Each capture is complete, has `timed_out=false`, and has no tree-kill or fallback-kill attempt. Their stderr captures are empty; their exact stdout is preserved beside each JSON.
- C has no `ADMIN-C-OFFICIAL-COLLECTALLCOUNTERS.json`, stdout, or stderr file. The summary has a `null` entry at the C position. `ADMIN-C = INCONCLUSIVE`; no sample result, main-entry result, or initialization result is inferred.
- D2 generated a non-empty output tree containing `timechart.csv` (1,349 bytes, SHA-256 `D18CCC886AFC1AB40EACF7A8183BCFD3DA1D14EFA8ED21050909069191EF8BC5`) and `session.uprof` (10,883 bytes, SHA-256 `82A785C4A0BC18AFDFBAF1D89BA18229017CB7276A02FE064E20FBA017E5877F`). The `.config`, `.error`, `.log`, and `metadata` directories were present but empty.

### Artifact identity

The inventory found the expected paths and x64 PE architecture. The probe, CXL DLL, official sample, and CLI hashes matched their expected values. The API DLL file itself is the authoritative 64-character hash `9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277` and is AMD Authenticode-valid. The inventory's `expected_sha256` field for that one item was malformed and omitted the final `7` (`...A427`, 63 characters), so its generated `sha256_matches=false` flag is a capture-harness metadata defect, not an API file mismatch.

The direct A/B controls used the same new x64 `metric-probe.exe` (`69D551BB8423823BC092605DA5C42CB21A45CACA618838B8437B080F2D659687`), exact argument vectors, explicit working directory `C:\Users\Hello\.codex\worktrees\08bd\resource-timeline`, and the same AMD DLL identities as their corresponding new administrator commands. E used the same new binary, arguments, working directory, and API DLL identity as `NONADMIN-E-RESOURCE-TIMELINE-INIT-ONLY`; elevation was the experimental variable. The historical pre-fix A/B/C probe bytes necessarily predate the required init-only child addition, so the old and new probe hashes are not claimed identical.

### Non-admin versus administrator matrix

| Test | Non-admin | Administrator | Changed? |
|---|---|---|---|
| Direct CXL load | `-1` / `0xFFFFFFFF`; debugger evidence: `KERNEL32!FatalExit(0xFFFFFFFF)` | `-1` / `0xFFFFFFFF`; `BEFORE_LOAD` only; no timeout | No |
| Direct API DLL load | `-1` / `0xFFFFFFFF`; transitive CXL abort | `-1` / `0xFFFFFFFF`; `BEFORE_LOAD` only; no timeout | No |
| Official `CollectAllCounters` sample | `-1` / `0xFFFFFFFF` historical result | Missing evidence files and `null` summary entry | Inconclusive |
| Official CLI `timechart --list` | `ERROR: There is no counters avialable` in the prior non-admin control | `0` / `0x00000000`; Socket `[0]`, Core `[0-7]`, Thread `[0-15]`; Power on Socket/Core; Frequency and P-State on Thread | Yes |
| Official CLI power | `0x80070005` / `AMDT_ERROR_ACCESSDENIED` during prior non-admin initialization; no samples | `0` / `0x00000000`; profiling started and finished; output generated | Yes |
| Resource Timeline init-only | `-1` / `0xFFFFFFFF`; stopped before load returned and before initialize | `-1` / `0xFFFFFFFF`; `INIT_ONLY_BEFORE_LOAD` only | No |

The explicit A/B working directory and arguments are consistent. The direct-load rows use the same AMD DLL paths and hashes. The missing C artifact prevents claiming byte-level or behavioral administrator comparison for that sample.

### ADMIN-A — direct CXL load

Command identity:

```text
C:\Users\Hello\.codex\worktrees\08bd\resource-timeline\tools\metric-probe\target\release\metric-probe.exe amd-uprof-load-only-child --path D:\apps\AMDuProf\bin\CXLBaseTools.dll
```

The child started and exited without timeout at `2026-08-29T03:18:19.4303247Z`–`03:18:19.5043896Z`. It emitted only:

```text
BEFORE_LOAD path=\\?\D:\apps\AMDuProf\bin\CXLBaseTools.dll flags=0x00000900 process_architecture=x64
```

Exit was signed `-1`, unsigned `0xFFFFFFFF`; stdout/stderr capture was complete; no loader error was returned to the parent. Administrator elevation did not resolve the direct CXL termination.

`DEPENDENCY_LOAD_ABORT = PERSISTS`.

### ADMIN-B — direct API DLL load

Command identity:

```text
C:\Users\Hello\.codex\worktrees\08bd\resource-timeline\tools\metric-probe\target\release\metric-probe.exe amd-uprof-load-only-child --path D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll
```

The child started and exited without timeout at `2026-08-29T03:18:19.5359820Z`–`2026-08-29T03:18:19.5771077Z`. It emitted only:

```text
BEFORE_LOAD path=\\?\D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll flags=0x00000900 process_architecture=x64
```

Exit was signed `-1`, unsigned `0xFFFFFFFF`; stdout/stderr capture was complete. This is consistent with the transitive `AMDPowerProfileAPI.dll -> AMDSysUtils.dll -> CXLBaseTools.dll` load-abort path. It is not an API initialization result.

`ADMIN_API_DLL_LOAD = ABORT_VIA_TRANSITIVE_CXL_PATH`.

### ADMIN-C — official CollectAllCounters sample

The expected independent evidence files are absent, and the summary has no result object for this test. It is therefore:

`OFFICIAL_SAMPLE_CONTROL = INCONCLUSIVE`

No claim is made about whether the administrator sample reached `main`, `AMDTPwrProfileInitialize`, enumeration, or a clean close.

### ADMIN-D0/D1/D2 — official CLI

`ADMIN-D1-OFFICIAL-CLI-VERSION` completed at `0` / `0x00000000` and printed `AMDuProfCLI.exe Version 5.3.521.0`.

`ADMIN-D0-OFFICIAL-CLI-TIMECHART-LIST` completed at `0` / `0x00000000`, with complete capture and no stderr. It reported:

- Devices: Socket `[0]`, Core `[0-7]`, Thread `[0-15]`.
- Categories: Power on Socket/Core; Frequency and P-State on Thread.
- No temperature category was reported. This does not prove temperature is unsupported by the public API under this platform configuration.

`ADMIN-D2-OFFICIAL-CLI-TIMECHART-POWER` used exactly:

```text
D:\apps\AMDuProf\bin\AMDuProfCLI.exe timechart --event power --interval 1000 --duration 5 --output-dir C:\Users\Hello\AppData\Local\Temp\resource-timeline-amd-admin-comparison-v2-20260829T031818104Z\ADMIN-D2-power-output
```

It completed normally in `5,446.921 ms`, with exit `0` / `0x00000000`, no timeout, complete stdout/stderr capture, and no process-tree kill. The CLI reported `Profiling started...`, elapsed marks at approximately `0`, `1013`, `2023`, `3026`, and `4032` ms, then `Profile finished`.

The generated CSV identifies:

- sampling interval: `1000 milli-seconds`;
- profile duration: `5 seconds`;
- package descriptor: `socket0-package-power`, counter ID `48`, unit `W`, description `Socket0-Average Package Power reported in Watts`;
- core descriptors: `core0-power` through `core7-power`, counter IDs `50` through `64`, unit `W`;
- three records at `11:18:22:646`, `11:18:23:646`, and `11:18:24:650`;
- package values `49.44`, `42.26`, and `40.29 W`.

These are real short-run vendor CLI records, not Resource Timeline API samples and not an acceptance of a production metric. No direct public API initialization status was emitted by D2, and no frequency or temperature sample was collected.

`ADMIN_OFFICIAL_CLI_POWER = PASS_FOR_SHORT_VENDOR_CLI_CONTROL`.

### ADMIN-E — Resource Timeline init-only

The exact command was:

```text
C:\Users\Hello\.codex\worktrees\08bd\resource-timeline\tools\metric-probe\target\release\metric-probe.exe amd-uprof-init-only-child --install-root D:\apps\AMDuProf
```

It used the strict contract:

```text
load -> resolve init/enumeration/close -> initialize once -> enumerate once if initialized -> close
```

The child emitted only:

```text
INIT_ONLY_BEFORE_LOAD path=\\?\D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll flags=0x00000900 process_architecture=x64
```

It exited without timeout at `2026-08-29T03:18:26.7856829Z`–`2026-08-29T03:18:26.8094478Z` with signed `-1` / `0xFFFFFFFF`. Capture was complete and stderr was empty. The highest reached gate was `BEFORE_LOAD`; load did not return, symbol resolution did not occur, initialize was not called, enumeration count is unavailable, and close was not called by the child. No AMD status code was returned.

This was not a sampling test. No enable, timer, start, read, workload, cadence, or busy operation was entered.

`ADMIN_RESOURCE_TIMELINE_INIT = ABORT_BEFORE_LOAD`.

### Privilege and direct-API/CLI classification

`DEPENDENCY_LOAD_ABORT = PERSISTS`.

Administrator elevation did not change the direct CXL, direct API, or init-only result. The non-admin `AMDT_ERROR_ACCESSDENIED` boundary is nevertheless crossed by the official CLI's short power path: administrator CLI profiling starts and emits records. Because direct API initialization under Administrator was never reached, the permission conclusion is limited:

`PERMISSION_BOUNDARY = PARTIALLY_RESOLVED` — resolved for the official CLI path, unresolved for the direct API path, which still aborts during dependency loading.

`DIRECT_API_VS_VENDOR_CLI = DIVERGENT`.

The evidence does not prove that the CLI uses the same public `AMDPowerProfileAPI.dll` load path. The next investigation must compare process environment, working directory, loaded modules, DLL search behavior, and the CLI's internal initialization path. No reinstall or repair is justified or performed by this record.

### Revised root cause

`ROOT_CAUSE = DEPENDENCY_LOAD_FAILURE`

`SUBCAUSE = CXLBASETOOLS_LOAD_PATH_FATAL_EXIT`

Confidence remains high for the observed boundary: direct administrator CXL loading still terminates at signed `-1` / `0xFFFFFFFF`, while the earlier debugger evidence proved `KERNEL32!FatalExit(0xFFFFFFFF)` in the CXL dependency path. The administrator comparison disproves elevation as a fix for that direct load abort and establishes a separate vendor-CLI/direct-API divergence. `EXACT_CXL_INTERNAL_CONDITION = UNPROVEN` remains unchanged. The missing C sample evidence prevents using that control to strengthen or weaken the conclusion.

### Metric decisions after administrator comparison

- `CPU_PACKAGE_POWER = DEFER`: the CLI produced short-run package-power records, but the Resource Timeline-owned API did not load and no API cadence/lifecycle/failure-isolation qualification was performed.
- `AMD_CORE_EFFECTIVE_FREQUENCY = DEFER`: D0 reports Frequency on Thread identities, but no direct API descriptor semantics or repeated Resource Timeline samples were obtained.
- `CPU_EFFECTIVE_FREQUENCY = DEFER_AGGREGATION_CONTRACT`: no product aggregation was selected or added.
- `CPU_PACKAGE_TEMPERATURE = DEFER_UNREACHED_DUE_TO_LIBRARY_LOAD`: no temperature counter was enumerated; Hyper-V/VBS/HVCI remain unchanged and are not blamed.

`SOURCE_DECISION = DEFER`.

No cadence, sampling expansion, idle/load/recovery, busy/concurrency, lifecycle, overhead, or soak qualification was run after this comparison. No numeric value was synthesized for any failed or unreached API path.

### Platform, mutation, and legal state

- Microsoft Hypervisor: `ENABLED`; VBS: `ENABLED`; HVCI: `ENABLED`.
- Platform, AMD installation, services, drivers, registry, PATH, boot configuration, security policy, and Afterburner/RTSS state: unchanged by this task.
- `USER_AUTHORIZATION_REQUIRED`: none for consuming the already-authorized Administrator evidence; any installation repair/reinstall, driver/service change, security-policy change, or platform change remains a separate authorization gate.
- Distribution remains `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`; no AMD DLL, driver, installer, header, PDF, sample, or license was committed.

## Next step

`CPU-SENSOR-AMD-CLI-VS-DIRECT-API-DIVERGENCE-INVESTIGATION`

Do not start `CPU-SENSOR-AMD-PROVIDER-DESIGN`. First investigate why the installed official CLI can complete a short power profile while direct `AMDPowerProfileAPI.dll`/`CXLBaseTools.dll` loading still aborts, preserving the current platform and installation. Only if that boundary becomes safe should the original ordered live qualification resume.
