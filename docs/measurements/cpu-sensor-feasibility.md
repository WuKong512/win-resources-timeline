# CPU-SENSOR-SPIKE HARDWARE SENSOR FEASIBILITY

**Date:** 2026-08-28
**Scope:** Windows CPU hardware-sensor feasibility, evidence, and implementation admission only. This artifact does not implement or enable a production CPU sensor Provider.

**RESULT: `PASS_WITH_DEFERRED_METRICS`**

The independent probe and current-machine evidence are complete. No requested CPU hardware metric is admitted to production yet. The Windows built-ins are useful for OS CPU state/utility, but they do not establish package temperature, package power, or effective-average frequency. The technically capable third-party routes found in this spike still have unresolved driver, distribution, legal, cross-hardware, or long-run lifecycle gates.

## BASELINE

| Field | Value |
| --- | --- |
| `BASE_COMMIT` | `9a53af7fc3a35a4971a8bb6c40c93fe332e3e74c` |
| `START_HEAD` | `9a53af7fc3a35a4971a8bb6c40c93fe332e3e74c` |
| `FINAL_HEAD` | Delivery HEAD is reported in the final delivery record |
| `origin/main` | `9a53af7fc3a35a4971a8bb6c40c93fe332e3e74c` |
| `BRANCH` | `spike/cpu-sensor-feasibility` |
| `PR` | Draft PR URL is reported in the final delivery record |
| `WORKING_TREE` | Clean at delivery; ignored local probe artifacts are not committed |
| `DUPLICATE_TASK_GATE` | `PASS` |

Entry-gate evidence:

- `git fetch origin --prune` completed successfully.
- The remote is `https://github.com/WuKong512/win-resources-timeline.git`.
- PR #17, `[PR-09] Add dashboard metric explorer`, is closed and merged at the baseline commit.
- No open PR or remote branch for an equivalent CPU sensor task was found. The local `agent/spike-01*` names refer to prior generic/GPU spike work; no CPU production implementation or `Spike-01 CPU` task was found in the duplicate search.
- Current worktrees, local/remote branches, and the detached starting checkout were inspected before creating this branch from `origin/main`.

## CURRENT PRODUCTION TRUTH

The current production collector remains unchanged:

- `src-tauri/src/collector/system_metrics.rs` samples CPU usage through the existing system sampler. `SystemSample` contains `cpu_percent`, memory, disk, GPU samples, and process snapshot state, but no CPU temperature, package power, or CPU frequency/effective-clock fields.
- The production `WindowsBaselineProvider` in `src-tauri/src/collector/provider.rs` exposes CPU, memory, disk, and process capabilities only. The CPU sensor probe primitives in this branch are not called by it.
- `src-tauri/src/models.rs` has `MetricCategory::Cpu` and the existing distinction between supported, unsupported, permission-denied, provider-missing, probe-failed, and failed states. A failure is never represented as a synthetic zero.
- Temperature/power/frequency descriptors currently present in the MetricCatalog are GPU descriptors. PR-09 has the generic status/unit-family/catalog shape needed for future CPU descriptors, but it does not make CPU sensors available.
- Therefore the following production metrics are currently absent: `cpu.temperature_celsius`, `cpu.package_temperature_celsius`, `cpu.package_power_watts`, and `cpu.effective_clock_mhz`.

The spike intentionally records OS signals under separate names (`cpu.os_reported_*`, `cpu.processor_*`) and reference values under `reference.*`; it does not relabel either route as a CPU package sensor.

## HARDWARE PROFILE

| Field | Observed value |
| --- | --- |
| `WINDOWS` | Windows 11 Pro, version `10.0.26200`, DisplayVersion `25H2`, build `26200`, x64 |
| `CPU_VENDOR` | AuthenticAMD |
| `CPU_MODEL` | AMD Ryzen 7 9700X 8-Core Processor |
| `CORES_THREADS` | 8 cores / 16 logical processors |
| `MOTHERBOARD` | ASUSTeK COMPUTER INC. TUF GAMING B650M-E WIFI |
| `POWER_MODE` | High performance (`8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c`) |
| `PRIVILEGE` | Probe process reported non-administrator (`elevated=false`); the profile CIM query was read-only diagnostic work |
| `REFERENCE_MONITOR` | Existing MSI Afterburner 4.6.6 with RTSS; no HWiNFO, LibreHardwareMonitor, OpenHardwareMonitor, Ryzen Master, AIDA64, Core Temp, or OCCT installation was found |
| `SENSITIVE_IDENTIFIERS` | Serial numbers, usernames, executable paths, and process identities were not written to reports |

## PROBE IMPLEMENTATION BOUNDARY

The branch extends `tools/metric-probe` only:

- `cpu-sensors` uses one reusable PDH Processor Information query, the existing `CallNtPowerInformation(ProcessorInformation)` helper, and a read-only adapter for the documented MSI Afterburner MAHM shared-memory mapping when an already-running reference monitor publishes it.
- `cpu-sensor-lifecycle` exercises enable → disable → re-enable → disable. `enable()` owns source handles; `disable()` drops the PDH query and mapped view. The probe never starts Afterburner, installs a driver, starts a service, writes a mapping, changes system settings, or writes the application database.
- `reference.*` keys are deliberately not production metric keys. The Afterburner mapping is evidence/reference input only.
- Reports are atomic, sanitized JSON/Markdown under ignored `artifacts/metric-probe/` directories.

Representative commands used:

```powershell
tools/metric-probe/target/release/metric-probe.exe cpu-sensors --duration-seconds 300 --poll-interval-ms 1000 --output-dir artifacts/metric-probe/cpu-sensors-idle-5m
tools/metric-probe/target/release/metric-probe.exe cpu-sensors --duration-seconds 300 --poll-interval-ms 1000 --output-dir artifacts/metric-probe/cpu-sensors-load-5m
tools/metric-probe/target/release/metric-probe.exe cpu-sensor-lifecycle --enabled-duration-ms 5000 --disabled-duration-ms 5000 --output-dir artifacts/metric-probe/cpu-sensor-lifecycle-verified
```

## CANDIDATE SOURCES

“Temperature/power/frequency” in the matrix means the source can expose something in that family; it does not mean the value has the requested package/effective semantics.

| `SOURCE` | `TEMPERATURE` | `POWER` | `FREQUENCY` | `ADMIN_REQUIRED` | `DRIVER` | `OVERHEAD` | `LICENSE` | `VERDICT` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Windows PDH Processor Information | No package sensor | No | Processor Frequency, `% Processor Performance`, `% Processor Utility`, `% of Maximum Frequency` | No for query | No | Measured low user-mode query cost; 500 ms mostly repeats | Windows API | Admit only as OS auxiliary signal; not a package sensor |
| `CallNtPowerInformation(ProcessorInformation)` | No | No | `CurrentMhz`, `MaxMhz`, throttle/policy state | No in this probe | No | Measured as part of probe; cheap | Windows API | Admit only as OS current/policy signal; not effective average |
| WMI/CIM `MSAcpi_ThermalZoneTemperature` | Thermal-zone value only; no package guarantee | No | No | Access varies | Firmware/ACPI implementation | Low query cost, semantics unavailable | Windows/firmware | `NOT ADMISSIBLE AS CPU PACKAGE TEMPERATURE` |
| Windows thermal sensor IOCTL / ACPI thermal framework | Zone/component dependent | No | No | Driver/device dependent | Sensor driver or firmware | Not measured here | Windows/firmware | Defer; must identify the zone and component |
| Windows Energy Meter Interface (EMI) | No | Energy accumulator can yield average power for the metered device/rail | No | Device dependent | A compliant device driver is required | Not measured; no device verified | Windows API/driver contract | Defer; no current CPU-package device evidence |
| AMD uProf / AMDPowerProfileAPI | Package temperature referenced to Tctl in the documented metric set | Estimated average core/package power | Core effective frequency over sampling period | Mode/family dependent; not installed | Vendor profiler/library components; exact resident path needs validation | Not measured on this machine | Redistribution/SDK terms not confirmed for this app | Technical candidate; `BLOCKED_LEGAL_DISTRIBUTION_REVIEW` until terms/security/soak clear |
| AMD Ryzen Master | CPU temperature and CPU power gauges; CPU power is distinct from SOC/PPT | VDDCR CPU power; SOC and PPT are separate values | GUI telemetry, but effective-average contract for this app not established | Yes in normal use | Vendor tuning service/driver components | Not measured | Proprietary application; no redistributable production API established | Reference-only; `BLOCKED_LEGAL_DISTRIBUTION_REVIEW` |
| LibreHardwareMonitor | CPU package/Tctl/Tdie/CCD sensors depending on hardware | CPU package/energy-derived sensors depending on hardware | Average/effective clock sensors depending on hardware | Some sensors require elevation | PawnIO/kernel device path; MSR/SMN/SMU access | Not measured; no installation permitted in this spike | MPL-2.0 plus third-party notices, including LGPL-2.1 PawnIO component | Technical candidate; `BLOCKED_LEGAL_DISTRIBUTION_REVIEW` |
| OpenHardwareMonitor lineage | CPU temperature where supported | Not a stable package-power contract across hardware | Clocks/load, but no uniform effective-average contract | Often | WinRing0-style kernel driver | Not measured; security/AV behavior is a concern | Exact redistribution set must be audited | Reject for this admission; driver/security/legal gate not met |
| HWiNFO shared memory / SDK | Rich monitor values when external HWiNFO is running | Rich monitor values when external HWiNFO is running | Rich monitor values, semantics depend on selected sensor | External process may be elevated | HWiNFO owns the hardware path | Not measured as a production dependency | Official terms limit non-Pro shared memory to 12 hours; commercial/embedding restrictions apply | Reference-only; `BLOCKED_LEGAL_DISTRIBUTION_REVIEW` |
| MSI Afterburner MAHM shared memory | CPUHAL-labeled reference value observed | CPUHAL-labeled reference value observed | CPU clock reference observed | Existing monitor/driver route requires external setup | External monitor/driver | Probe read cost measured; monitor cost excluded | Proprietary external dependency; not redistributed | Reference-only; not a production source |
| Intel Power Gadget API | Package temperature | Average package power | Current package frequency | Installer/admin setup | DLL connects to its driver | Official docs support 1–1000 ms sampling; not measured here | Intel distribution terms/driver not cleared | Intel-only and unvalidated on AMD; `BLOCKED_LEGAL_DISTRIBUTION_REVIEW` |
| Vendor-specific AMD/Intel interfaces | Potentially precise | Potentially precise | Potentially effective | Varies | Usually vendor component/driver | Unknown until a concrete SDK is selected | Must be reviewed per SDK | Architecture route, not an admission |

## WINDOWS BUILT-IN API AUDIT

### Performance counters / PDH

The probe queried:

- `\Processor Information(_Total)\Processor Frequency`
- `\Processor Information(_Total)\% Processor Performance`
- `\Processor Information(_Total)\% Processor Utility`
- `\Processor Information(_Total)\% of Maximum Frequency`

On this machine, the one-shot values were available without administrator elevation. During the 5-minute runs, `Processor Frequency` remained `3800 MHz` for 299 successful samples in both idle and load, while `Processor Utility` moved from an idle mean of `16.516%` to a load mean of `83.886%`; `% of Maximum Frequency` moved from a mean of `90.261%` to `99.833%`. These signals are useful evidence of OS scheduling/performance state and load, not package hardware temperature or power.

Microsoft’s PerfLib example identifies `ProcessorFrequency`, `PercentMaximumFrequency`, `ProcessorPerformance`, and `ProcessorUtility` as separate counter fields. Microsoft also documents that Processor Utility accounts for performance state and Turbo Boost to represent work, whereas a frequency field is not the same thing as effective work frequency. The probe therefore keeps `cpu.processor_utility_percent` separate from any future `cpu.effective_clock_mhz`.

### `CallNtPowerInformation(ProcessorInformation)`

The current production-adjacent helper returns one `PROCESSOR_POWER_INFORMATION` per logical processor. Microsoft defines `MaxMhz` as the maximum specified clock and `CurrentMhz` as that maximum multiplied by current processor throttle. This is not an average of useful work over the collection interval. The probe observed current/max `3800/3800 MHz`; it does not promote those values to effective frequency.

### WMI/CIM and ACPI thermal zones

The class `MSAcpi_ThermalZoneTemperature` exists in `root/wmi`, but the read-only instance query on this machine returned `不支持` (“not supported”). More importantly, Windows’ thermal framework models a thermal zone as an abstract firmware-defined region. A zone can represent a component, SoC, chassis/skin region, or another platform-defined area; `_TMP` can be a direct or extrapolated value. Nothing in this API establishes CPU package scope.

**Result: `NOT ADMISSIBLE AS CPU PACKAGE TEMPERATURE`.** The project must not create `cpu.package_temperature_celsius` from this class without a platform-specific proof of the zone identity and comparison against a package reference.

### Windows thermal IOCTLs

The thermal sensor interface is a device/driver contract. It can report a temperature in tenths of a degree Kelvin for the associated thermal zone, but the association and semantics are supplied by platform firmware/driver configuration. It is not a cross-vendor CPU package API and was not used as a package sensor.

### Energy Meter Interface

Windows EMI is a promising architecture for `average power` only when an EMI-compliant metering device is present. The device periodically measures voltage/current on a rail and accumulates absolute energy; the client subtracts two readings and divides by elapsed time. The result’s scope is the metered device/rail described by the device metadata. No current-machine enumeration proving a CPU package EMI channel was obtained, so EMI cannot be used as `cpu.package_power_watts` in this spike.

## AMD-SPECIFIC SEMANTICS

The current machine is AMD Ryzen 7 9700X. The official AMD uProf metric documentation distinguishes:

- CPU Core Effective Frequency: effective frequency for the sampling period, not a nominal/base clock.
- Power: estimated average power, with core/package choices depending on the processor and profile.
- Temperature: average Celsius value referenced to Tctl for the supported package metric.

AMD Ryzen Master also distinguishes CPU/VDDCR CPU power from SOC telemetry and PPT. These names must not be collapsed into a generic “CPU temperature” or “CPU power”. On AMD platforms, `Tctl`, `Tdie`, CCD temperatures, socket/motherboard sensors, and package/SoC power have different meanings. If a future source exposes only `Tdie` or `CCD0`, the descriptor must say so (for example `cpu.tdie_temperature_celsius` or `cpu.ccd0_temperature_celsius`) rather than silently publishing `cpu.package_temperature_celsius` or averaging unlike sensors.

The LHM source was reviewed for architecture evidence: its AMD CPU implementation contains package power, Tctl/Tdie, and average/effective-clock sensor concepts, and its PawnIO path accesses privileged device interfaces. That proves technical precedent, not current-machine validation or production redistribution permission.

## INTEL ARCHITECTURE RESEARCH

Intel Power Gadget is an official Intel route that exposes package temperature, average package power/energy, and current package frequency through `EnergyLib*.dll`. Its setup connects to a driver, requires administrator installation, and is documented for second-generation-and-later Intel Core processors. It is Intel-only and was not run on this AMD machine. Its current clock/frequency API is also not automatically an effective-average frequency contract for Resource Timeline.

The Intel route is therefore architecture-level research only:

- AMD-only: AMD uProf/AMDPowerProfileAPI and AMD SMU/SMN-specific paths.
- Intel-only: Intel Power Gadget MSR/energy-counter path.
- Cross-vendor: PDH/PerfLib, `CallNtPowerInformation`, WMI/ACPI, and Windows EMI; none of these cross-vendor APIs proves the requested CPU package semantics by itself.
- Current Intel support status: **unvalidated**. No Intel hardware, driver, or reference comparison was available on this machine.

## METRIC SEMANTICS CONTRACT

The future product descriptors should use these meanings:

| Future key | Required meaning | Explicit non-meanings |
| --- | --- | --- |
| `cpu.package_temperature_celsius` | One package-level thermal reading with a source-declared package/Tctl-equivalent scope | Not an arbitrary ACPI zone, motherboard/socket sensor, undocumented per-core maximum, or averaged CCD values |
| `cpu.package_power_watts` | Sampled instantaneous or source-declared average package power, with `power_scope=cpu_package`, source timestamp, and sample/energy interval | Not TDP, package power limit, core-only power, SOC-only power, PPT percentage, or wall/PSU power |
| `cpu.effective_clock_mhz` | Average effective CPU work frequency over the collection interval, with the source’s aggregation (core/package) recorded | Not nominal/base/max clock, OS `CurrentMhz`, `% Processor Utility`, or an instantaneous boost clock |

For power, an energy accumulator should be preferred. If only instantaneous power exists, policy may trapezoid-integrate adjacent valid samples and preserve coverage gaps. For frequency, the production key must not be populated until the source’s effective/average window is proved. No values are averaged across unlike temperature sensors.

## REFERENCE COMPARISON

The only suitable installed comparison tool was MSI Afterburner 4.6.6. The probe read its documented `MAHMSharedMemory` mapping read-only. It did not start or control the monitor and did not use the monitor as a dependency. The local SDK header identifies CPU temperature, CPU clock, and CPU power entries; the monitor’s own CPUHAL labels remain external reference semantics.

### Idle — 5 minutes

- Configuration: 300 seconds, 1 second poll, 300/300 scheduled samples, 0 dropped, 300 source polls.
- The Afterburner source timestamp advanced at approximately 1 Hz (300 distinct source seconds).
- Reference temperature: all samples `53.875..91.625 °C`; trailing four minutes `53.875..72.375 °C`, mean `59.598 °C`. The first 30-s mean was `66.425 °C`; the last 30-s mean was `56.604 °C`, so the run included an initial transient and was not perfectly flat idle.
- Reference power: all samples `40.617..131.992 W`; trailing four minutes `40.617..85.803 W`, mean `51.341 W`. First/last 30-s means were `54.762/43.918 W`.
- Reference clock: `3600..5450 MHz`; it is an instantaneous/reference clock reading, not effective-average frequency.
- OS `Processor Utility`: mean `16.516%`; OS `Processor Frequency`: constant `3800 MHz`.

### Representative load — 5 minutes

- Workload: eight BelowNormal PowerShell workers running a bounded numeric loop for the probe duration. They were started only for this run, stopped in `finally`, and verified absent afterward. This was a representative load, not an extreme stress test.
- Configuration: 300 seconds, 1 second poll, 300/300 scheduled samples, 0 dropped, 300 source polls.
- The Afterburner source timestamp produced 299 distinct source seconds.
- Reference temperature: all samples `63.625..96 °C`; trailing four minutes `95.75..96 °C`, mean `95.777 °C`. First/last 30-s means were `93.350/95.750 °C`.
- Reference power: all samples `76.227..143.169 W`; trailing four minutes `130.255..143.169 W`, mean `133.868 W`. First/last 30-s means were `123.161/134.695 W`.
- Reference clock: `5220..5450 MHz`, trailing mean `5286.875 MHz`.
- OS `Processor Utility`: mean `83.886%`; `% of Maximum Frequency`: mean `99.833%`; OS `Processor Frequency`: constant `3800 MHz`.

The idle/load trend is physically plausible for the installed reference monitor: load raises both reference temperature and reference power, and the OS utility signal rises. It is not an independent validation that the Afterburner labels are package scope; it only supplies the requested reference trend and highlights the semantic mismatch between a boost clock and effective average frequency.

## UPDATE CADENCE

The four requested poll intervals were each run for 30 seconds with the same release probe. “Repeat ratio” is repeated samples divided by successful samples; it is evidence of the value stream’s granularity, not a claim that a source has no internal work.

| Poll | Expected/executed/dropped | Reference temp repeat | Reference power repeat | Reference clock repeat | Probe avg CPU | Probe P95 CPU |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 500 ms | 60 / 59 / 1 | 49.2% | 49.2% | 96.6% | 0.3875% | 0.3909% |
| 1 s | 30 / 30 / 0 | 56.7% | 0.0% | 93.3% | 0.1954% | 0.1954% |
| 2.5 s | 12 / 12 / 0 | 33.3% | 0.0% | 83.3% | 0.0828% | 0.1040% |
| 5 s | 6 / 6 / 0 | 16.7% | 0.0% | 83.3% | 0.0595% | 0.0620% |

The installed reference monitor’s timestamp advances at about one second. Polling at 500 ms therefore reads repeated values frequently, especially for the clock. Polling faster than the source does not add semantic resolution. A future production Provider should start with the existing balanced 2-second core plan or a 1-second detailed plan, then validate the selected hardware API’s native cadence. A 100-ms esports-style loop is not justified by this evidence.

## PERFORMANCE

All measurements below are probe-own process measurements. The already-running Afterburner process/driver is excluded. The eight load workers are excluded. The probe does not write SQLite, so database growth is not applicable to this spike.

| Run | Duration | Poll | Avg CPU | P95 CPU | Peak working set | Peak handles | Peak threads |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Existing probe baseline | 300 s | 1 s | 0.1970% | 0.1955% | 7,495,680 B | 90 | 4 |
| CPU sensor probe, idle | 300 s | 1 s | 0.2853% | 0.2931% | 11,939,840 B | 113 | 4 |
| CPU sensor probe, load | 300 s | 1 s | 0.2203% | 0.2930% | 11,894,784 B | 117 | 4 |

Against the same-process baseline, the idle sensor probe added approximately `0.0883` percentage points average CPU and `4,444,160 B` peak working set, with no peak-thread increase. The 500-ms cadence stayed below the project’s `0.5%` probe CPU budget on this run but roughly doubled the 1-second own CPU share. These numbers are current-machine evidence for the small OS/MAHM adapter only; they are not an overhead claim for LHM, AMD uProf, HWiNFO, or a kernel driver.

The measured direct call latency was also small for this adapter: in the 5-minute idle run, PDH frequency read P50/P95/max was `0.1151/0.3754/0.503 ms`; the Afterburner temperature mapping read was `0.0013/0.0017/0.0187 ms`. Load values were `0.3909/0.6530/0.8407 ms` and `0.0015/0.0020/0.0207 ms`. Initialization, driver work, and external monitor cost are not included in those per-read numbers.

## PLAUSIBILITY TESTS

| Family | Result | Evidence and limitation |
| --- | --- | --- |
| Temperature | Trend `PASS`; package admission `DEFER` | Reference Celsius values remained plausible, rose under load, and settled near 96 °C; no Windows built-in package identity was proved |
| Power | Trend `PASS`; package admission `DEFER` | Reference values were non-negative and rose from trailing idle mean `51.341 W` to load mean `133.868 W`; source scope/average window is not independently established |
| Frequency | OS signal `PASS`; effective-frequency admission `DEFER` | OS utility and performance changed with load, but OS frequency stayed at 3800 MHz and reference clock is a different instantaneous/boost semantic |

TDP, nominal frequency, base frequency, and maximum frequency were not used as live sensor values.

## LIFECYCLE

The repaired release lifecycle report uses schema `cpu-sensor-spike-lifecycle/v2` and four 5-second phases at a 500-ms scheduler interval. `sample_attempt_count` means an enabled session poll was attempted; `logical_source_poll_count_delta` means the session actually entered its source-poll path; `successful_source_read_count` and `failed_source_read_count` count source results, not session objects returned by `sample()`.

The targeted repair output is `artifacts/metric-probe/cpu-sensor-lifecycle-review-repair/lifecycle.json` and its Markdown rendering; these local artifacts remain ignored and are not committed.

| Phase | Source generation | Scheduler ticks | Sample attempts | Logical source polls | Successful source reads | Failed source reads | Handles released at start | No source polling while disabled |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| enabled-1 | 1 | 10 | 10 | 10 | 29 | 1 | false | false |
| disabled-1 | 1 | 10 | 0 | 0 | 0 | 0 | true | true |
| re-enabled-1 | 2 | 10 | 10 | 10 | 29 | 1 | false | false |
| disabled-2 | 2 | 10 | 0 | 0 | 0 | 0 | true | true |

Per-source results were `nt_power=10/10/0`, `pdh=10/9/1`, and `afterburner=10/10/0` in each enabled phase (attempted/successful/failed). The first PDH result is the expected warm-up failure. The baseline successful source set was `nt_power`, `pdh`, `afterburner`, and the same set succeeded after re-enable. An absent optional source would have zero attempts and `provider_missing`; it is not required for recovery.

`ENABLE_DISABLE_REENABLE: PASS` because both enabled phases performed source polling, both disabled phases had scheduler ticks but zero sample attempts/source polls/source results, every initially successful source recovered, source generations changed from `1` to `2`, and disable released the source handles. The final report recorded `cleanup_completed=true`, thread delta `0`, handle delta `-3`, and working-set delta `57,344 B`. No worker thread is created by the probe. This does not prove a future vendor driver can resume after sleep or tolerate a driver reset.

`CLEANUP: PASS` for the independent adapter. `enable()` reconstructs the PDH query and mapped view after the prior session drops them; it does not reuse stale handles.

## FAILURE ISOLATION

The independent probe keeps each metric/source status separate:

- `ProviderMissing`: a missing MAHM mapping is represented without a value; a unit test exercises a non-existent mapping name.
- `PermissionDenied`: `OpenFileMappingW(ERROR_ACCESS_DENIED)` is mapped to `permission_denied`; no ACL was changed on the development machine, so this path was not forced in a live run.
- `Unsupported`: missing individual sensor fields and the unsupported ACPI thermal WMI instance query retain `unsupported` rather than zero.
- `ProbeFailed`/`Failed`: malformed MAHM headers, non-committed/no-access/guard pages, allocation-base changes, uncovered ranges, invalidated mappings, layout changes, PDH initialization/read failures, NaN/Inf, and out-of-range temperature/power values are represented as failures. Unit coverage rejects NaN, infinity, `FLT_MAX` sentinel values, malformed layout arithmetic, unsafe memory-region states/protection, and changed header/entry metadata.
- Warm-up: the first PDH derived-counter sample is recorded as a warm-up skip/failure reason, not as a fake zero.
- Timeout: **not exercised** in this standalone probe. The future production Provider must run source calls behind the existing ProviderHost per-operation deadline/cancellation boundary; a new adapter must not make an unbounded hardware call on the collector thread.
- Reference absent: covered by the missing-mapping test; the current 5-minute runs had the already-running Afterburner reference available.

`FAILURE_ISOLATION: PASS` for probe-level independent source/metric handling and for the fact that this branch is not on the production collector path. A malformed or changed MAHM mapping returns `ReadStatus::Failed` without a value; it is never converted to synthetic zero. Production impact is intentionally not claimed because no production Provider was implemented.

## SLEEP / RESUME

`SLEEP_RESUME: NOT EXERCISED`.

The probe did not trigger system sleep or wake and did not mutate power settings. A future optional Provider must re-probe/recreate vendor handles after resume and preserve the existing stopped/failed/unsupported distinctions. The current evidence is not a sleep/resume correctness PASS.

## LICENSE / SECURITY

The Windows API and PDH route adds no third-party redistribution obligation and does not require a kernel driver, but it cannot supply the requested package semantics. The remaining technically capable routes have material gates:

- LibreHardwareMonitor is MPL-2.0 and its repository includes separate third-party notices; the PawnIO component/embedded modules introduce additional license review and a privileged device/driver path. The app cannot bundle it until MPL/LGPL obligations, driver redistribution/signing, security review, and uninstall behavior are approved.
- OpenHardwareMonitor lineage uses a WinRing0-style kernel-access route. Its driver/admin/AV and crash-surface risks are not compatible with an implicit always-on dependency for a local timeline without an explicit security and distribution decision.
- HWiNFO’s official license pages describe a 12-hour limit for non-Pro shared memory and restrict commercial use/embedding; the SDK is a paid integration route. It is not a production dependency or a silently started sidecar.
- MSI Afterburner is used only as an existing user-selected reference. Its process/driver is outside the probe’s own overhead and is not redistributed.
- AMD uProf and Ryzen Master provide useful vendor semantics, but this spike did not install their components or verify a redistributable, long-running, non-invasive Windows integration contract. Treating an installed profiling/tuning component as an implicit Resource Timeline dependency would fail the distribution/security gate.
- Intel Power Gadget is a DLL/driver/installer route for Intel processors and remains unvalidated here. It cannot be used to claim Intel support.

Any future Ring0/privileged route must document driver signing, admin consent, device access, service lifetime, crash/recovery risk, sleep/resume behavior, uninstall cleanup, and whether the driver remains resident while the Provider is disabled. “Can read a value” is insufficient for a long-running background application.

## ARCHITECTURE DECISION

**Current decision: `NO_PRODUCTION_ADMISSION`.**

If a later source clears the gates, it should be implemented as an **`OPTIONAL_CPU_SENSOR_PROVIDER`**, not as an extension of `windows-baseline`:

- ordinary CPU usage is core baseline functionality with no hardware-driver dependency;
- CPU temperature/power/effective-clock access has different permissions, failure modes, vendor coverage, distribution terms, and security risk;
- failure of the optional Provider must never stop CPU usage, memory, disk, GPU, or process collection;
- the future Provider must reuse `CollectionPlan`, capability/health metadata, bounded deadline/retry/backoff, real disable/stop semantics, and `MetricCatalog`; it must not create a second Provider framework;
- each source descriptor must expose scope and semantic qualifiers, and unavailable/failed fields must remain absent rather than zero.

## DATA MODEL

| Field | Result |
| --- | --- |
| `SCHEMA_CHANGE_REQUIRED` | `NO` |
| `DTO_CHANGE_REQUIRED` | `YES`, later and additive only |
| `METRIC_CATALOG_COMPATIBLE` | `YES` |

Schema v8 already contains `cpu_sample.temp_c`, `cpu_sample.package_power_w`, and `cpu_sample.effective_clock_mhz`, plus quality/source columns. `system_rollup_1m` is keyed by generic `metric_key`/`device_id`, and `collection_session_metric` already stores enabled/support status/provider/interval. No migration is needed for this spike and no migration was made.

The current `SystemSample` DTO still contains only `cpu_percent` for CPU. A future production integration will need additive DTO/writer/query/provider metadata work to carry the three fields, source scope, quality, coverage, and status correctly. That work is deliberately outside this branch.

PR-09’s Metric Explorer can accept future CPU descriptors naturally: each descriptor can carry a CPU category, unit family (Celsius, watts, or MHz), provider/source identity, and capability status. This compatibility is an architectural observation, not a request to modify Dashboard or PR-09.

## IMPLEMENTATION ADMISSION

| Metric | Verdict | Reason |
| --- | --- | --- |
| `CPU_PACKAGE_TEMPERATURE` | `DEFER` | ACPI/WMI does not prove package scope; AMD-capable third-party routes need driver/security/legal/long-run validation. The reference trend is plausible but not an independent package proof. |
| `CPU_PACKAGE_POWER` | `DEFER` | Windows EMI is device/rail dependent and no package channel was verified; Afterburner/Ryzen Master/uProf semantics and redistribution path are not cleared. TDP is not used. |
| `CPU_EFFECTIVE_FREQUENCY` | `DEFER` | OS `CurrentMhz`/PDH frequency is not interval-effective work frequency; AMD uProf/LHM technical routes require unresolved distribution and privileged-access validation. |

No metric is `ADMIT` in this task. The correct next step is a separately approved optional CPU-sensor Provider spike against one explicitly selected, legally redistributable source; until then, production remains CPU-usage-only for the CPU family.

## SOURCES

Primary documentation reviewed:

- [Microsoft `PROCESSOR_POWER_INFORMATION`](https://learn.microsoft.com/en-us/windows/win32/power/processor-power-information-str): `CurrentMhz` is maximum specified frequency multiplied by current throttle.
- [Microsoft thermal management Design Guide](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/design-guide): thermal zones are abstract platform-defined regions and may use direct or extrapolated values.
- [Microsoft Energy Meter Interface](https://learn.microsoft.com/en-us/windows-hardware/drivers/powermeter/energy-meter-interface): energy meters expose accumulated rail/device energy from which clients can calculate average power.
- [Microsoft PerfLib Processor Information consumer example](https://learn.microsoft.com/en-us/windows/win32/perfctrs/using-the-perflib-functions-to-consume-counter-data): Processor Frequency, Processor Performance, Processor Utility, and related fields are distinct counters.
- [Microsoft CPU Utility explanation](https://learn.microsoft.com/en-us/troubleshoot/windows-client/performance/cpu-usage-exceeds-100): Processor Utility is a work/utilization signal that accounts for performance state and Turbo Boost.
- [AMD uProf metrics](https://docs.amd.com/r/en-US/57368-uProf-user-guide/12.2.-Metrics): AMD effective-frequency, estimated-power, and Tctl-referenced temperature metric semantics.
- [AMD Ryzen Master gauges](https://docs.amd.com/r/en-US/68886-ryzen-master-user-guide/Gauges): CPU/VDDCR CPU, SOC, and PPT telemetry distinctions.
- [Intel Power Gadget API on Windows](https://www.intel.com/content/www/us/en/developer/articles/training/using-the-intel-power-gadget-30-api-on-windows.html): Intel-only package power/temperature/frequency API, DLL/driver setup, and sampling notes.
- [LibreHardwareMonitor](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor), [MPL-2.0 license](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/master/LICENSE), [third-party notices](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/master/THIRD-PARTY-NOTICES.txt), and [PawnIO adapter](https://github.com/LibreHardwareMonitor/LibreHardwareMonitor/blob/master/LibreHardwareMonitorLib/PawnIo/PawnIo.cs).
- [OpenHardwareMonitor](https://github.com/HardwareMonitor/openhardwaremonitor) and [WinRing0 source notice](https://github.com/HardwareMonitor/openhardwaremonitor/blob/master/External/WinRing0/OpenLibSys.h).
- [HWiNFO shared-memory terms](https://www.hwinfo.com/forum/threads/shared-memory-support.18/), [HWiNFO SDK](https://www.hwinfo.com/sdk/), and [HWiNFO licenses](https://www.hwinfo.com/licenses/).

## VALIDATION RECORD

- `cargo build --release --manifest-path tools/metric-probe/Cargo.toml`: passed.
- `cargo test --manifest-path tools/metric-probe/Cargo.toml`: 54 passed, 0 failed, including focused lifecycle and MAHM validation tests.
- `cargo fmt --manifest-path tools/metric-probe/Cargo.toml -- --check`: passed.
- 30-second cadence runs at 500 ms, 1 s, 2.5 s, and 5 s: passed.
- 5-minute idle run: passed, 300/300 scheduled samples.
- 5-minute representative-load run: passed, 300/300 scheduled samples; workload PIDs cleaned.
- repaired enable/disable/re-enable lifecycle run: passed; enabled phases recorded `29` successful source reads each, disabled phases recorded zero attempts/source polls/source results, and all initially successful sources recovered.
- Main `src-tauri` production collector was not modified and was not invoked by the probe.

## DELIVERY

| Field | Value |
| --- | --- |
| `ARTIFACT` | `docs/measurements/cpu-sensor-feasibility.md` plus independent `tools/metric-probe` commands |
| `COMMIT` | Delivery commit is reported in the final delivery record |
| `PUSH` | Branch push result is reported in the final delivery record |
| `DRAFT_PR` | Draft PR URL is reported in the final delivery record |
| `WORKING_TREE` | Final status is reported in the final delivery record |

## NEXT STEP

Only a metric whose source separately clears the semantic, license/distribution, security, lifecycle, cross-hardware, and long-run overhead gates may enter a later production Provider task. This spike does not authorize production CPU sensor integration.
