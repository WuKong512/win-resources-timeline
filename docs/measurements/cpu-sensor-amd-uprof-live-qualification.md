# CPU-SENSOR-AMD-LIVE-QUALIFICATION

Qualification date: 2026-08-28 (Asia/Shanghai). This is a Windows live-source qualification record for the current machine only. It is not production integration and does not grant permission to redistribute AMD software.

## Result

`RESULT: BLOCKED`

`BLOCKER: AMD_UPROF_LIBRARY_LOAD_ABORTS_PROBE`

The installed API path and architecture checks passed, but a Resource Timeline-owned non-admin child process exited with code `-1` while executing the explicit `LoadLibraryExW` call for `AMDPowerProfileAPI.dll`. The parent probe failed closed and did not call `AMDTPwrProfileInitialize`. No API version query, counter enumeration, sample, stop, close, lifecycle, or busy evidence was obtained. The result is therefore not a source PASS and is recorded as `DEFER`, rather than inferring that the vendor API is reliable or unsupported from static artifacts.

## Baseline

- Repository: `WuKong512/win-resources-timeline`
- Q1 merge commit: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`
- Q1 merge-base check: passed; the Q1 commit is an ancestor of `origin/main`.
- Start head: `0c470c48b4e60bd94dfe720ec8981f919db8b1c1`
- Base branch state: local `origin/main` resolved to the same commit at task start.
- Qualification branch: `spike/cpu-sensor-amd-uprof-live-qualification`
- Branch provenance: created from the verified latest `origin/main`, not from the old Q1 branch.
- Worktree note: the repository's `main` branch was already checked out in another worktree, so Git refused a second `git switch main`; the qualification branch was created directly from the verified `origin/main` ref. No content drift was introduced by this environmental limitation.
- Duplicate task gate: `PASS`; no matching live-qualification branch or commit was found. `gh` was unavailable, so a complete open-PR API search could not be performed.
- Specification state: Q1 static source qualification is merged; this live task is blocked before API entry.

## AMD uProf installation

The read-only audit distinguished the installed product from a downloaded installer.

- `DOWNLOADED_INSTALLER`: `NO`; no `AMDuProf-5.3*.exe` was found in the searched user download scope.
- `INSTALLER_PATH`: `N/A`
- `INSTALLER_VERSION`: `N/A`
- `INSTALLER_SHA256`: `N/A`
- `INSTALLER_SIGNATURE`: `N/A`
- `INSTALLED`: `YES`
- `INSTALLED_VERSION`: `5.3.521` (`AMDuProf.exe`, `AMDuProfCLI.exe`, and API DLL file/product version `5.3.521.0`)
- `INSTALL_ROOT`: `D:\apps\AMDuProf\` (the standard `C:\Program Files\AMD\AMDuProf\` root was absent; the actual root came from the installed-product registry entry/PATH and was verified by its artifact tree)
- `API_LIBRARY`: `D:\apps\AMDuProf\bin\AMDPowerProfileAPI.dll`
- API library size: `280,984` bytes
- API library SHA-256: `9634020BCAF3F2E639E0EEA2D64433E3F369A80A1FC54B9220CA732F830A4277`
- API library architecture: `x64`, matching the probe process.
- API library signature: `Valid`; signer `CN=Advanced Micro Devices, O=Advanced Micro Devices, S=California, C=US`.
- `HEADER`: `D:\apps\AMDuProf\include\AMDTPowerProfileApi.h` and `AMDTPowerProfileDataTypes.h`, present and read as the authoritative installed contract.
- `API_PDF`: `D:\apps\AMDuProf\Help\AMDPowerProfilerAPI.pdf`, present and read. It is release v1.2 material and contains an older/inconsistent counter-value description; it was not allowed to override the installed header.
- `OFFICIAL_SAMPLE`: `D:\apps\AMDuProf\Examples\CollectAllCounters\CollectAllCounters.cpp`, present and read. The sample confirms the initialize/enumerate/enable/timer/start/read/stop/close sequence.
- The installed `.lib` and vendor artifacts were used only in place; none were copied into the repository.
- Library load policy in the qualification probe: verified absolute API path with `LoadLibraryExW` and `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32`. There is no bare DLL-name, CWD, or arbitrary-PATH fallback.
- Failure observed: the isolated load-check child exited `-1` before returning from the loader call. The parent emitted `unsafe_library` state with no numeric value and did not attempt a direct load afterward.

## Driver / service

All observations were read-only. No driver, service, registry, or startup mutation was performed.

- `DRIVER`: `C:\Windows\System32\drivers\AMDCpuProfiler.sys`; file version `4.4.1.0`, product version `5.3.481.0`, Authenticode `Valid`, AMD signer, owner `NT AUTHORITY\SYSTEM`.
- `DRIVER`: `C:\Windows\System32\drivers\AMDPowerProfiler.sys`; file version `10.6.3.0`, product version `5.3.481.0`, Authenticode `Valid`, AMD signer, owner `NT AUTHORITY\SYSTEM`.
- `DRIVER_VERSION`: the installed user-space/API version is `5.3.521.0`, while both profiler driver product versions report `5.3.481.0`; this version drift was recorded and not silently assumed compatible.
- `DRIVER_SIGNATURE`: both driver files were Authenticode-valid AMD-signed files.
- `SERVICE`: `AMDPowerProfiler`, kernel driver, `Running`, `Manual`, registry `Type=1`, `Start=3`, image `\??\C:\WINDOWS\system32\drivers\AMDPowerProfiler.sys`.
- `SERVICE`: `AMDCpuProfiler`, kernel driver, `Running`, `Manual`, registry `Type=1`, `Start=3`, image `\??\C:\WINDOWS\system32\drivers\AMDCpuProfiler.sys`.
- `SERVICE`: `AMDProfilerLoadService`, Win32 own process, `Running`, `Automatic`, registry `Type=16`, `Start=2`, `ObjectName=LocalSystem`, image `D:\apps\AMDuProf\bin\AMDProfilerLoadService.exe`.
- `SERVICE`: `AMDProfilerService`, not registered/found in the read-only service audit.
- `MUTATIONS_PERFORMED`: `NO`; no service stop/start, driver reset/unload, registry edit, or installer execution.

## Privilege

- `INITIAL_PROCESS_ELEVATED`: `FALSE`.
- `NON_ADMIN_SAMPLING`: `BLOCKED_BEFORE_API_LOAD`; the non-admin process reached the verified install gate, but the isolated DLL load child exited before API initialization.
- `PERMISSION_RESULT`: `UNREACHED`; no vendor permission status was obtained, so this must not be mislabeled as `PermissionDenied`.
- `ADMIN_COMPARISON`: `NOT_AUTHORIZED` and not attempted.
- `AUTO_ELEVATION_PERFORMED`: `NO`.

## Platform

- `MICROSOFT_HYPERVISOR`: `ENABLED`, based on the Q1 platform evidence; it was not disabled for this task.
- `VBS`: `ENABLED` platform context; the read-only registry value `EnableVirtualizationBasedSecurity=1` was observed.
- `HVCI`: `ENABLED` platform context; the read-only scenario value `HypervisorEnforcedCodeIntegrity\Enabled=1` was observed.
- `PLATFORM_MUTATIONS`: `NO`; no boot configuration, Hyper-V, VBS, HVCI, or Memory Integrity change.

## API contract extracted from installed artifacts

The installed `AMDTPowerProfileApi.h` and `AMDTPowerProfileDataTypes.h` were authoritative. The PDF and official sample were used only as corroborating evidence. No API signature was guessed from exports or an older web example.

The installed C ABI uses `AMDTResult` as an unsigned 32-bit result and exposes these operations with the following parameter shapes:

1. `AMDTPwrProfileInitialize(AMDTPwrProfileMode)`
2. `AMDTPwrGetSupportedCounters(AMDTUInt32*, AMDTPwrCounterDesc**)`
3. `AMDTPwrGetCounterId(AMDTCounter, AMDTUInt32*)`
4. `AMDTPwrGetCounterDesc(AMDTUInt32, AMDTPwrCounterDesc*)`
5. `AMDTPwrEnableCounter(AMDTUInt32)`
6. `AMDTPwrSetTimerSamplingPeriod(AMDTUInt32)`
7. `AMDTPwrStartProfiling()`
8. `AMDTPwrReadAllEnabledCounters(AMDTUInt32*, AMDTPwrSample**)`
9. `AMDTPwrStopProfiling()`
10. `AMDTPwrProfileClose()`

- `API_VERSION`: no API version-query function is declared in the installed header; no query was invented.
- `INIT`: must precede enumeration and profile work; the installed header documents driver-unavailable, platform-unsupported, driver-version-mismatch, previous-session-not-closed, and internal-failure results.
- `ENUMERATION`: `GetSupportedCounters` returns descriptors valid until `ProfileClose`; each descriptor carries counter ID, device ID/type/instance, category, aggregation, unit, range, name, and description.
- `CONFIGURE`: selected counters are enabled before the timer and start calls.
- `START`: start follows initialization, enumeration, enable, and timer configuration.
- `SAMPLE`: `ReadAllEnabledCounters` returns vendor-owned sample memory; the installed header states that returned sample memory is valid until the next read. `AMDT_ERROR_PROFILE_DATA_NOT_AVAILABLE` is a no-data state, not a zero.
- `STOP`: normal profile stop.
- `CLOSE`: closes/releases the profile session after stop.
- `STATUS_MODEL`: vendor result is retained as a symbolic name plus hexadecimal code, with stable qualification state kept separate.
- `BUSY_STATUS`: `AMDTPwrStartProfiling` documents `AMDT_ERROR_ACCESSDENIED (0x80070005)` as profiler busy/not accessible. Other relevant installed status constants include `AMDT_ERROR_DRIVER_ALREADY_INITIALIZED (0x80080001)`, `AMDT_ERROR_PROFILE_SESSION_EXISTS (0x80080017)`, and `AMDT_ERROR_PREVIOUS_SESSION_NOT_CLOSED (0x80080020)`.
- `UNSUPPORTED_STATUS`: `AMDT_ERROR_NOTSUPPORTED`, `AMDT_ERROR_PLATFORM_NOT_SUPPORTED`, `AMDT_ERROR_HYPERVISOR_NOT_SUPPORTED`, and `AMDT_ERROR_COUNTER_NOT_ACCESSIBLE` are distinct from no-data and failure.
- `SAMPLING_CONSTRAINT`: the installed PDF/sample uses a 100 ms example; the installed header does not expose a minimum/maximum constant. This probe deliberately targets 500/1000/2000 ms product-candidate cadences, not maximum throughput.
- `SIMULTANEOUS_COUNTER_SUPPORT`: contract-level `YES` for enabling multiple descriptors before one start; the official collect-all sample enables multiple counters in one session. Current-machine live qualification: `NOT_CONFIRMED`, because the API library could not be safely loaded.
- Timestamp note: the installed `AMDTPwrSystemTime` field named `m_microSecond` is documented in the installed header as milliseconds. A future implementation must preserve that vendor meaning; it must not assume microseconds from the field name.

## Package power

Target reference key: `reference.amd_uprof.package_power_watts`.

- `STATUS`: `NOT_REACHED_UNSAFE_LIBRARY_LOAD`.
- `SCOPE`: intended package scope from the descriptor contract; not live-confirmed.
- `UNIT`: intended `W` (`AMDT_PWR_UNIT_WATT`); no value returned.
- `WINDOW`: intended vendor sample-period average; not instantaneous and not external wall power.
- `ESTIMATED_QUALIFIER`: `ESTIMATED` per Q1 semantic decision; not live-confirmed.
- `SANITY`: not run; no 30-second numeric sample series.
- `IDLE`: not run; no 120-second idle distribution.
- `LOAD`: not run; no bounded workload was started because library loading was blocked.
- `RECOVERY`: not run.
- `SOURCE_DECISION`: `DEFER`.

There are no numeric package-power values and no synthesized zeros in any report.

## Core effective frequency

Target reference key: `reference.amd_uprof.core_effective_frequency_mhz`.

- `STATUS`: `NOT_REACHED_UNSAFE_LIBRARY_LOAD`.
- `IDENTITY_SEMANTICS`: the probe is prepared to preserve the installed descriptor's `CPU_COMPUTE_UNIT`, `CPU_CORE`, `PHYSICAL_CORE`, or `THREAD` identity. A `THREAD` descriptor would be reported as logical-processor/thread identity, not relabeled as physical core.
- `UNIT`: intended `MHz` (`AMDT_PWR_UNIT_MEGA_HERTZ`); no value returned.
- `WINDOW`: vendor sample interval; not live-confirmed.
- `IDENTITY_COUNT`: no descriptors enumerated; expected machine logical processor count is 16 from the Q1 baseline, but this is not evidence that the API would return 16 identities.
- `IDLE`: not run.
- `LOAD`: not run.
- `MISSING_IDENTITY_BEHAVIOR`: implementation emits a status without a value for a missing returned identity; no zero synthesis. Not live exercised.
- `SOURCE_DECISION`: `DEFER`.
- `PRODUCT_AGGREGATION_DECISION`: `CPU_EFFECTIVE_FREQUENCY = DEFER_AGGREGATION_CONTRACT`; no package headline or aggregate was produced.

## Package temperature

- `STATUS`: `DEFER_CURRENT_PLATFORM_CONFIGURATION`.
- `MICROSOFT_HYPERVISOR_EFFECT`: Q1 identified the enabled Microsoft Hypervisor as the current platform blocker for package temperature. VBS/HVCI remain context and were not changed.
- `API_BEHAVIOR`: not live-observed because the DLL load aborted before initialization; therefore this task does not claim that the API enumerates temperature, returns `unsupported`, or returns another error on this platform.
- `SOURCE_DECISION`: `DEFER_CURRENT_PLATFORM_CONFIGURATION`.
- A valid temperature unexpectedly returned under the current platform would require a separate Q1 document-conflict review; no such value was observed.

## Cadence

Each cadence run performed the verified-root and isolated-load gate. None reached the AMD timer or read API.

| Requested | Actual | Samples | Late | Dropped | API avg latency | API p95 latency | API max latency |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 500 ms | N/A | 0 | 0 | 0 | N/A | N/A | N/A |
| 1000 ms | N/A | 0 | 0 | 0 | N/A | N/A | N/A |
| 2000 ms | N/A | 0 | 0 | 0 | N/A | N/A | N/A |

- `PRODUCT_CANDIDATE_INTERVAL`: `NOT_SELECTED`; the expected 1000 ms preference cannot be supported without live evidence.
- The no-sample cadence results mean “API not entered”, not “API returned zero” and not a performance pass.

## Concurrency / busy

- `SINGLE_SESSION_RULE`: installed header/sample lifecycle is single-client session oriented; normal use is initialize, profile, stop, close before a new session.
- `SECOND_SESSION_RESULT`: `NOT_RUN`; the first session could not be started safely.
- `VENDOR_STATUS`: `NOT_OBTAINED`.
- `PROVIDER_BUSY_MAPPING`: code preserves vendor status before mapping; no mapping was made without a live status.
- `FIRST_SESSION_SURVIVED`: `NOT_APPLICABLE`.
- `RETRY_BEHAVIOR`: no retry storm; one isolated load check per explicitly requested short test, then fail closed.
- `RYZEN_MASTER_INTERACTION`: `NOT_ESTABLISHED`; no Ryzen Master process was acted on and no conflict was inferred.

## Lifecycle

The qualification-only command contains explicit lifecycle and generation instrumentation, but the loader gate prevented execution.

- `ENABLE`: `NOT_RUN`.
- `DISABLE`: `NOT_RUN`.
- `QUIESCENCE`: `NOT_RUN`.
- `RE_ENABLE`: `NOT_RUN`.
- `FINAL_DISABLE`: `NOT_RUN`.
- `PROCESS_EXIT`: parent probe exited normally after writing the blocked report; the isolated load child exited `-1`.
- `POLLS_AFTER_DISABLE`: `0 (no session was enabled; not a lifecycle pass)`.
- `RESOURCE_TIMELINE_OWNED_LEAKS`: no parent probe-owned handles/workers were left resident by the blocked run; no AMD profile handle was created. The AMD external driver/service remains resident as it did before the test.
- `AMD_EXTERNAL_DRIVER_RESIDENCY`: expected and unchanged; driver residency is not evidence that a Provider remained active.

## Failure isolation

- `MISSING_LIBRARY`: invalid-root simulation produced a report and no numeric value.
- `UNSAFE_PATH`: absolute-root, canonical containment, required installed-artifact markers, PE architecture, and explicit safe-load checks fail closed.
- `UNSUPPORTED_COUNTER`: not live reached; selection/reporting preserves missing-counter status without a value.
- `PERMISSION`: no natural permission result was observed; ACLs were not modified.
- `BUSY`: not exercised because no first session could start.
- `INVALID_VALUE`: unit coverage rejects non-finite values and negative power/frequency values; rejected values never enter distributions.
- `TIMEOUT/LATE`: no API call was reached; no forced timeout or cancellation was used. Scheduler fields remain zero because polling did not begin.
- `ZERO_SYNTHESIS`: `PASS`; failure, missing, no-data, unsupported, and unsafe-load states contain no numeric zero.

## Performance

- `AVG_CPU`: N/A for AMD polling; no read call occurred.
- `P95_CPU`: N/A.
- `PEAK_WORKING_SET`: N/A for AMD polling.
- `HANDLE_COUNT`: no AMD session handle; parent process resource accounting was not treated as an API performance sample.
- `THREAD_COUNT`: no AMD polling worker was started.
- `API_AVG_LATENCY`: N/A.
- `API_P95_LATENCY`: N/A.
- `API_MAX_LATENCY`: N/A.
- `SAMPLE_COUNT`: 0.
- `LATE_SAMPLES`: 0.
- `DROPPED_SAMPLES`: 0.
- `RESOURCE_TIMELINE_PROBE_OVERHEAD`: only the parent's install verification/report work was observed; it is not an AMD source-overhead measurement.
- `AMD_EXTERNAL_OVERHEAD`: `NOT_ATTRIBUTABLE`; no aggregate was fabricated for resident AMD services/drivers.
- No new 10-hour-or-longer soak was run. PR-07B historical long-run evidence remains separate and was not reused as AMD-source overhead evidence.

## External reference

- `AFTERBURNER_RTSS_STATE`: MSI Afterburner, RTSS, and RTSSHooksLoader64 were naturally present during read-only process inspection.
- `MODIFIED`: `NO`; no stop, restart, polling change, settings change, injection, hook, or reset.
- `COMPARISON_PERFORMED`: `NO`; no reference read was needed because the AMD API did not load.
- `COMPARISON_INTERPRETATION`: N/A. Afterburner/RTSS would not be treated as ground truth; sampling window, Tctl semantics, estimated power semantics, timing offset, and filtering would require interpretation.

## Legal / distribution

- `EXTERNAL_INSTALLED_DEPENDENCY`: candidate deployment model only.
- `DLL_REDISTRIBUTION`: `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`.
- `DRIVER_REDISTRIBUTION`: `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`.
- `LEGAL_REVIEW`: required before any product distribution decision; live qualification would not grant redistribution rights.
- `VENDOR_ARTIFACTS_COMMITTED`: `NO`; no AMD DLL, driver, header, PDF, sample, license, or proprietary package was committed.

## Metric decisions

- `CPU_PACKAGE_POWER`: `DEFER`; no live API value or lifecycle/overhead evidence.
- `AMD_CORE_EFFECTIVE_FREQUENCY`: `DEFER`; no live descriptor identity or sample evidence.
- `CPU_EFFECTIVE_FREQUENCY`: `DEFER_AGGREGATION_CONTRACT`; no aggregation decision was made.
- `CPU_PACKAGE_TEMPERATURE`: `DEFER_CURRENT_PLATFORM_CONFIGURATION`; current Hypervisor context remains unchanged and API behavior was not reached.

## Source decision

`DEFER`

The installed artifact contract is sufficiently specific to keep a minimal qualification probe safe, but the current non-admin process cannot load the signed API DLL without an abnormal child exit. Package power and per-identity frequency therefore cannot be called `PASS_SOURCE`; temperature remains independently deferred. This is insufficient evidence to enter optional Provider design.

## Production boundary

- `PRODUCTION_INTEGRATION`: `NO`.
- `PROVIDER_ADDED`: `NO`.
- `METRIC_CATALOG_CHANGED`: `NO`.
- `SCHEMA_CHANGED`: `NO`.
- `UI_CHANGED`: `NO`.

Only the independent `tools/metric-probe` command surface and this measurement record are in scope. ProviderHost, CollectionPlan, production collector behavior, MetricCatalog, schema/migrations, Dashboard, and Metric Explorer were not changed.

## Validation

The qualification probe was formatted, unit-tested, and release-built before live attempts. Final repository validation is recorded in the delivery commit:

- `METRIC_PROBE_FMT`: `PASS` (`cargo fmt --manifest-path tools/metric-probe/Cargo.toml -- --check`)
- `METRIC_PROBE_TESTS`: `PASS`, 63 tests (`cargo test --manifest-path tools/metric-probe/Cargo.toml`)
- `METRIC_PROBE_RELEASE_BUILD`: `PASS` (`cargo build --release --manifest-path tools/metric-probe/Cargo.toml`)
- `SRC_TAURI_FMT`: `PASS` (`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`)
- `SRC_TAURI_CHECK`: `PASS` (`cargo check --manifest-path src-tauri/Cargo.toml`)
- `SRC_TAURI_TESTS`: `PASS` (`cargo test --manifest-path src-tauri/Cargo.toml`)
- `DIFF_CHECK`: `PASS` (`git diff --check`)

## Delivery

- `COMMIT`: recorded after final validation.
- `PUSH`: recorded after final validation.
- `DRAFT_PR`: `BLOCKED_TOOLING`; `gh` was not installed/available for draft PR creation.
- `PR_URL`: `N/A`.
- `WORKING_TREE`: recorded after delivery.

## Next step

Do not start `CPU-SENSOR-AMD-PROVIDER-DESIGN`. First resolve the isolated `AMDPowerProfileAPI.dll` load abort in an authorized, unchanged Windows environment (or obtain vendor-supported loader guidance), then rerun `CPU-SENSOR-AMD-LIVE-QUALIFICATION`. No admin comparison is authorized by this task.
