# CPU-SENSOR-SOURCE-Q1 — AMD uProf / AMDPowerProfileAPI Qualification

**Result:** `PASS_WITH_DEFERRED_LIVE`

This is a source qualification artifact, not a production implementation. It records the current AMD uProf 5.3 evidence, the deployment and security boundary, and the live-probe prerequisite. Q1 does not modify the production collector, `ProviderHost`, `CollectionPlan`, `MetricCatalog`, schema, migrations, Dashboard, Metric Explorer, or the existing generic CPU probe.

## Decision summary

AMD uProf / `AMDPowerProfileAPI` is a technically promising optional AMD source for the Ryzen 9000 family:

- AMD's current 5.3 release notes mark Live Power Profiling as supported for AMD Ryzen 9000 series.
- The current metrics documentation gives useful semantics for package temperature, package power, and core effective frequency.
- The local CPU identity, `Family 1Ah / Model 44h`, is inside the documented `Family 1Ah Model 40h–4Fh` counter range.

The source is not deployment-qualified in Q1. The machine has no AMD uProf installation, so no API contract probe, live values, lifecycle run, busy-session exercise, or overhead measurement was performed. The currently enabled Microsoft Hypervisor independently makes the temperature counter unavailable under AMD's documented limitation; VBS and HVCI states are recorded as platform context. Exact API signatures, client privilege behavior, installed driver/service lifecycle, and the version-specific redistribution terms remain gates for the next qualification pass.

**Source decision:** `DEFER`

The source is worth a controlled follow-up, but it is not admitted as a production Provider and is not yet cleared for bundling or unattended installation.

## Baseline and entry gate

- Repository: `WuKong512/win-resources-timeline`
- Remote: `https://github.com/WuKong512/win-resources-timeline.git`
- Authoritative `origin/main` at entry: `4228142409431c517c75cd3e92f5a7cba75e02d8`
- Branch baseline / `START_HEAD`: `4228142409431c517c75cd3e92f5a7cba75e02d8`
- Branch: `spike/cpu-sensor-amd-uprof-qualification`
- PR #18: confirmed merged by the merge commit in local history (`Merge pull request #18 ... [CPU-SENSOR-SPIKE] Hardware sensor feasibility`).
- Entry working tree: clean.
- Duplicate-task gate: no competing AMD uProf qualification, `AMDPowerProfileAPI` integration, or optional CPU Provider branch was present in the fetched remote heads or local branch list. GitHub CLI was unavailable in the environment, so this gate is based on the fetched remote refs and repository history rather than a complete GitHub API listing.
- Read before qualification: `docs/measurements/cpu-sensor-feasibility.md`, `docs/upgrade/execution-plan.md`, the current ProviderHost/CollectionPlan/MetricCatalog contracts, and `tools/metric-probe`'s existing CPU sensor probe.

The historical PR #18 conclusion remains unchanged: generic CPU sensor feasibility does not admit package temperature, package power, or effective frequency to production. Q1 adds a source-specific qualification record only.

## Official AMD evidence

The reviewed AMD sources are the current public 5.3 materials:

- [AMD uProf product and download page](https://www.amd.com/en/developer/uprof.html) — current Windows package and feature matrix.
- [uProf 5.3 supported processors](https://docs.amd.com/r/en-US/63856-uProf-release-notes/2.-Supported-Processors) — processor-family support table.
- [Live Power Profile metrics](https://docs.amd.com/r/en-US/57368-uProf-user-guide/12.2.-Metrics?contentId=QOsLN9WVGoJ7Dw3wbzO6xw) — metric units, scope, and sampling-window semantics.
- [Live Power Profile features](https://docs.amd.com/r/en-US/57368-uProf-user-guide/12.1.-Features?contentId=fI1baX8ZlewYY4svWOshSQ) — logical-core and package-level metric placement.
- [AMDPowerProfileAPI library](https://docs.amd.com/r/en-US/57368-uProf-user-guide/12.4.2.-AMDPowerProfileAPI-Library?contentId=_b16YwYI4vYCojehBnPIYQ), [API usage](https://docs.amd.com/r/en-US/57368-uProf-user-guide/12.4.2.1.-Using-the-APIs), and [Windows API sample page](https://docs.amd.com/r/en-US/57368-uProf-user-guide/12.4.2.1.1.-Windows?contentId=IEmkwhzmHRtkfrDrR3yqUA) — API purpose, driver prerequisite, and official `CollectAllCounters` sample location.
- [uProf 5.3 limitations](https://docs.amd.com/r/en-US/57368-uProf-user-guide/12.5.-Limitations?contentId=XbSKrRmNaMvfHjL5qiOu2A) — one-session, sampling, and Hyper-V limitations.
- [Windows installation](https://docs.amd.com/r/en-US/57368-uProf-user-guide/3.1.-Installing-Using-Windows?contentId=ULIgv7c2GeY9YTmyjsIr7Q) — documented install root.
- [uProf 5.3 EULA gate](https://www.amd.com/en/developer/uprof/uprof-eula/uprof-5-3-eula.html) — version-specific terms are gated behind acceptance.
- [EPM troubleshooting](https://docs.amd.com/r/en-US/57368-uProf-user-guide/14.3.3.-Endpoint-Privilege-Management-EPM-Software-Blocking-AMD-uProf) — the documented Windows driver filename example and service/driver installation boundary.

The AMD web pages establish the high-level API and metric contract. They do not expose the complete C/C++ header, counter IDs, signatures, status codes, or the exact Windows service key. Q1 does not reconstruct those details from a binary or from an older blog.

## AMD uProf version and local installation audit

### Version

The current AMD download page lists `AMDuProf-5.3.521.exe`, release date `2026-06-17`, as a new build of the AMD uProf 5.3 release. The qualification target is therefore **AMD uProf 5.3.521 / 5.3.x**, subject to the exact installed package being verified during a future live pass.

### Local installation result

The current machine was inspected read-only as a non-admin account. No matching uProf installation was found:

- standard AMD/uProf installation locations: no uProf files;
- uninstall metadata in 64-bit, 32-bit, and current-user uninstall hives: no uProf entry;
- `AMDuProf`, `AMDuProfCLI`, `AMDPowerProfileAPI`, and the documented API help artifact: not present/on PATH;
- matching uProf service keys: none observed;
- running uProf or Ryzen Master process: none observed;
- no installer was downloaded, no EULA was accepted, no elevation was requested, and no service or driver configuration was changed.

The standard install root documented by AMD is `C:\Program Files\AMD\AMDuProf\`. That location is also the explicit root a future adapter must resolve; it is not a license to redistribute the contents.

**Installed:** `NO`

**Live gate:** `BLOCKED_LIVE_PROVIDER_NOT_INSTALLED`

The existing MSI Afterburner and RTSS processes were left untouched. Because no AMD uProf session could be opened, no Q1 source-to-MAHM comparison was run; the external reference result is `NOT_AVAILABLE` for this qualification.

## Local platform and support mapping

The following values were re-probed without collecting or recording serial numbers, machine identifiers, usernames, private paths, MAC addresses, or IP addresses:

| Field | Observed value | Evidence / interpretation |
| --- | --- | --- |
| Vendor | `AuthenticAMD` | Windows processor registry identity |
| Processor | `AMD Ryzen 7 9700X 8-Core Processor` | Windows processor registry identity |
| Family / model / stepping | `Family 26 / Model 68 / Stepping 0` = `1Ah / 44h / 0` | In-memory CPU topology probe plus processor identifier |
| Physical cores | `8` | `GetLogicalProcessorInformationEx(RelationProcessorCore)` |
| Logical processors | `16` | Windows processor topology registry and topology probe |
| Windows | Windows 11 Pro, `25H2`, build `26200.9168` | Display version/build registry values; the compatibility `ProductName` field still says Windows 10 Pro |
| Hypervisor | `PRESENT` | `IsProcessorFeaturePresent(PF_HYPERVISOR_PRESENT)` returned true |
| VBS | `ENABLED` | Device Guard `EnableVirtualizationBasedSecurity=1` |
| HVCI / memory integrity | `ENABLED` | Device Guard HVCI `Enabled=1` |
| Credential Guard | `NOT_OBSERVED` | No value was used in the source decision |

Support mapping is explicit rather than inferred from the product name alone:

```text
local CPUID identifier: Family 26 / Model 68
        -> hexadecimal: Family 1Ah / Model 44h
        -> AMD metrics page: Family 1Ah Model 40h–4Fh is documented
        -> AMD release notes: Ryzen 9000 series Live Power Profiling = Yes
```

The local 9700X is therefore inside the current documented family/model counter range and the current Ryzen 9000 Live Power Profiling family row. This is static support evidence, not live support evidence.

AMD lists Windows 11 through 26H1 in the current uProf page; the local Windows 11 25H2 build is inside that stated OS range. The Hypervisor result is decisive for temperature: AMD's current limitations page says that when Microsoft Hypervisor is enabled on the host, the temperature counter is not supported. Temperature absence on this machine must therefore be reported as `UNSUPPORTED_BY_CURRENT_PLATFORM_CONFIGURATION`, not as an API implementation failure.

## Metric semantics and reference keys

All keys in this section are qualification-only reference keys. They must not be added to the production MetricCatalog or schema in Q1.

### Package temperature

- Reference key: `reference.amd_uprof.package_temperature_celsius`
- Source: AMD Live Power Profile / `AMDPowerProfileAPI` Temperature category.
- Scope: **Package**.
- Unit: **Celsius**.
- Window: **Average temperature for the sampling period**.
- Reference: AMD documents the value with reference to **Tctl**.
- Static verdict: `SOURCE_CANDIDATE` on a supported, non-Hyper-V configuration.
- Current-machine verdict: `DEFER_CURRENT_PLATFORM_CONFIGURATION` because Microsoft Hypervisor is present.
- Live value: not available; uProf is not installed.

This must not be described as a die-local sensor reading independent of Tctl, nor as an instantaneous temperature sample.

### Package power

- Reference key: `reference.amd_uprof.package_power_watts`
- Source: AMD Live Power Profile / `AMDPowerProfileAPI` Power category.
- Scope: **Package**; AMD also documents Core power, but this Q1 key is package-only.
- Unit: **Watts**.
- Window: **Average power for the sampling period**.
- Qualification qualifier: **Estimated** consumption based on platform activity levels.
- Static verdict: `SOURCE_CANDIDATE`.
- Live value: not available; uProf is not installed.

The word `estimated` is mandatory. The value must never be presented as exact instantaneous package power or as a laboratory-grade external power-meter measurement.

### Core effective frequency

- Reference key: `reference.amd_uprof.core_effective_frequency_mhz`.
- Source: AMD Live Power Profile / `AMDPowerProfileAPI` Frequency category.
- Scope: **Core-level**, presented by AMD's feature page under logical-core metrics.
- Unit: **MHz**.
- Window: **CPU Core Effective Frequency for the sampling period**.
- Static verdict: `SOURCE_CANDIDATE` as a per-core source.
- Live value: not available; uProf is not installed.

The source is Core Effective Frequency. It is not a documented package/socket effective-frequency counter. A single per-core counter must not be renamed to `cpu.effective_clock_mhz`, and a probe must retain core identity instead of silently averaging it.

## Effective-frequency aggregation gate

No reviewed AMD 5.3 Live Power Profile page establishes a package or socket aggregate effective-frequency counter. The product aggregate therefore remains deferred.

| Candidate | Benefit | Unresolved problem |
| --- | --- | --- |
| Simple mean across available cores | Easy to explain and compute | Idle/offline/missing cores have equal influence; the result moves when the available-core set changes |
| Active-core-only mean | Avoids treating idle cores as active work | Requires an explicit active threshold and has discontinuities as cores cross it |
| Utilization-weighted mean | Can track where work is actually running | Requires a compatible utilization window, denominator policy, and treatment of zero-utilization/missing cores; AMD's source semantics do not define this product aggregation |
| Per-core presentation only | Faithful to the documented source and preserves identity | More complex user presentation; no single CPU headline value |

Until one of these contracts is supported by source semantics and product review:

- `AMD_CORE_EFFECTIVE_FREQUENCY = SOURCE_CANDIDATE`
- `CPU_EFFECTIVE_FREQUENCY = DEFER_AGGREGATION_CONTRACT`

## Driver, service, privilege, and security audit

### What is established

- AMD's 5.3 Windows API documentation says the `CollectAllCounters` sample must be linked with the AMDPowerProfileAPI library and that the power profiling driver must be installed and running.
- AMD's current Windows MSR documentation says PMC MSRs are accessed through an ioctl call to the `AMDPowerProfiler` driver on Windows.
- AMD's current EPM troubleshooting page names `AMDPowerProfiler.sys` (or similar) as a kernel-mode driver example and says uProf can install kernel-mode drivers and system services.
- AMD documents the user-mode installation root as `C:\Program Files\AMD\AMDuProf\`.

### What is not established on this machine

| Audit item | Q1 result |
| --- | --- |
| Driver name | `AMDPowerProfiler.sys` is the official 5.3 filename example; exact installed package/service key is not verified because uProf is absent |
| Service name | `NOT_VERIFIED`; `AMDProfilerService` in the guide is a separate user-mode remote-profile application server, not evidence of the local power-profile service name |
| Driver binary location category | Windows-managed kernel-driver location owned by the AMD installation; exact path not observed |
| Signing status | `NOT_VERIFIED`; no uProf binary was present to Authenticode-check |
| Startup type | `NOT_APPLICABLE` on this machine; no matching uProf driver/service was installed |
| Running state | `NOT_RUNNING / NOT_INSTALLED` for the uProf profiling component |
| Installation ownership | External AMD uProf installation, not Resource Timeline |
| API client administrator requirement | `NOT_ESTABLISHED` by the reviewed public 5.3 Windows API pages |
| Driver installation administrator requirement | `NOT_ESTABLISHED` by the reviewed public 5.3 pages; kernel-driver installation is an external administrative deployment prerequisite until verified from the installer/package |
| Non-admin sampling | `NOT_VERIFIED`; the next live pass must start non-admin and surface permission failure without elevation |
| Driver resident after session close | `NOT_ESTABLISHED`; do not claim that the driver unloads when a profile stops |
| External component overhead | `NOT_ATTRIBUTABLE` without an installed, controlled run |

The future Provider must own and stop its own work: API session, sample calls, timers, worker activity, handles, and retry state. Disabling the Provider must stop Resource Timeline polling and close/release the API session. It must not unload an AMD-owned driver, reset another profiler, stop an external service, or claim that external driver residency equals an enabled Resource Timeline Provider. An AMD driver may remain installed and resident while the Provider is disabled; that distinction must be visible in health/diagnostic reasoning.

The external kernel component is a security and operational boundary: it introduces vendor-owned privileged code, installation policy, signing requirements, and possible EPM/endpoint-control interactions. Those concerns are part of deployment review, not something the Resource Timeline collector should silently repair.

## API contract audit

AMD's public 5.3 guide points to the installed `AMDPowerProfilerAPI.pdf` and the installed `CollectAllCounters` sample. Neither is available on this machine. The following contract is deliberately marked unknown where the official artifact was not available:

| Contract item | Q1 evidence status | Implementation rule |
| --- | --- | --- |
| Initialization API/signature | `NOT_ESTABLISHED` | Obtain the exact 5.3 header/sample; do not infer from a binary or older release |
| Version API/signature | `NOT_ESTABLISHED` | Verify from the official header and use it for compatibility gating |
| Counter enumeration | Category names are documented; exact enumeration API is `NOT_ESTABLISHED` | Preserve unknown counters and fail closed on missing required counters |
| Counter IDs/categories | Frequency, Power, Temperature categories are documented; exact IDs are `NOT_ESTABLISHED` | Map only documented IDs from the installed official artifact |
| Enable/configure | `NOT_ESTABLISHED` | Validate the returned status and requested counter set before start |
| Start | `NOT_ESTABLISHED` | One explicit session generation per start |
| Read/sample | Metric semantics are documented; function and result layout are `NOT_ESTABLISHED` | Bound each call and reject invalid/non-finite fields |
| Stop/close | `NOT_ESTABLISHED` | Stop and close are separate tracked operations; close failure is observable |
| Error/status model | `NOT_ESTABLISHED` | Preserve vendor status for diagnostics; map to existing Provider error/health states without inventing values |
| Sampling interval constraints | CLI minimum documented as 100 ms; API-specific constraint unknown | Provisional Resource Timeline request: 1000 ms |
| Concurrent sessions | One power profile session at a time is documented | Return `provider_busy`/equivalent unavailable state and back off; never steal the session |
| One session for temperature + package power + all-core frequency | `NOT_ESTABLISHED` | Verify from `CollectAllCounters`/PDF; do not assume category switching or simultaneous availability |

The official sample name is evidence that a collect-all workflow exists, but it is not enough to prove that the exact three requested metrics can be configured in one session. That question is a mandatory live/API-artifact gate.

## Distribution, licensing, and loading boundary

AMD's current uProf page labels the download **Download with EULA**, and the current [uProf 5.3 EULA page](https://www.amd.com/en/developer/uprof/uprof-eula/uprof-5-3-eula.html) requires acceptance before the material can be downloaded. Q1 did not accept the EULA or download the package. The exact version-specific clauses for the API library, driver, headers, samples, commercial use, modification, and redistribution were therefore not treated as established permission.

| Question | Q1 answer |
| --- | --- |
| A. User installs AMD uProf and Resource Timeline dynamically calls the local library | Technically plausible as an external-installed dependency, subject to exact installed-version API compatibility, non-admin/permission behavior, and legal review. This is the only deployment shape worth carrying forward without bundling. |
| B. Resource Timeline redistributes `AMDPowerProfileAPI.dll` | No authorization established. Do not redistribute. `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`. |
| C. Resource Timeline redistributes or installs the AMD profiling driver | No authorization established. Do not redistribute or install automatically; the kernel-driver and elevation boundary makes this a separate legal/security review. `BLOCKED_LEGAL_DISTRIBUTION_REVIEW`. |
| D. Redistribution authorization | No redistribution authorization has been established. Bundling or redistribution remains blocked until the exact version-specific license terms are reviewed and either explicitly grant the intended redistribution rights or separate AMD permission covering the intended distribution model is obtained. A developer API page is not an implicit redistribution grant. |

No AMD proprietary DLL, header, driver, sample, or license file is committed. Q1 does not make a legal conclusion. The distribution verdict is **`BLOCKED_LEGAL_DISTRIBUTION_REVIEW`** until the exact 5.3 license artifacts are reviewed and either grant the intended redistribution rights or are supplemented by separate AMD permission covering the intended distribution model.

If a future implementation is approved for an external-installed dependency, it must use runtime dynamic loading from an explicit, verified installation path. The documented install root may be used as the first candidate, but the exact library subpath must come from the installed package/manifest. The adapter must not use a bare `LoadLibrary("AMDPowerProfileAPI.dll")`, current-working-directory lookup, or random PATH search. Use a safe Windows DLL-loading strategy with an explicit absolute path, verify the selected AMD binary, and return `ProviderMissing` or an equivalent unavailable state when the path cannot be resolved safely.

## Concurrency and busy behavior

AMD documents: **only one power profile session can run at a time**. This creates a potential conflict with AMD uProf or any other application that holds the same power-profile facility. Ryzen Master interaction is **`NOT_ESTABLISHED`** in Q1 and must be verified, if relevant, during a future controlled live qualification.

Q1 did not open a second session because uProf is not installed. Busy behavior is therefore `DOCUMENTED_NOT_LIVE_EXERCISED`.

The future Provider contract is:

1. Attempt one bounded initialization/start.
2. If the facility is occupied, report `provider_busy` (or the existing equivalent health/status) and no metric value.
3. Do not take over, reset, force-close, kill, or reconfigure another profiler.
4. Use the existing ProviderHost bounded retry/backoff and generation reconciliation; do not retry continuously or create a CPU loop.
5. Re-attempt only after backoff and a fresh generation. After the other profiler releases the facility, a later start may acquire it normally.

The same fail-closed behavior applies when AMD uProf itself is active. Missing, busy, denied, unsupported, and failed are availability states, never zero-valued temperature, power, or frequency.

## Optional live probe status

The live-probe entry gate was not satisfied: uProf, the official headers/API PDF, and the library are absent. No independent `cpu-sensor-amd-uprof` command was added to `tools/metric-probe`; the existing `cpu-sensors` and `cpu-sensor-lifecycle` commands remain unchanged. Production `ProviderHost` was never called.

**Live result:** `BLOCKED_LIVE_PROVIDER_NOT_INSTALLED`

| Run | Result |
| --- | --- |
| Non-admin sanity, 30 s, 1000 ms | Not run — prerequisite absent |
| Idle, 120 s, 1000 ms | Not run — prerequisite absent |
| Bounded representative load, 120 s, 1000 ms | Not run — prerequisite absent |
| Cadence, 500/1000/2000 ms | Not run — prerequisite absent |
| Existing MSI Afterburner external reference | Not run for Q1 source comparison; Afterburner/RTSS were not started or changed |
| Enable → disable → re-enable → final disable | Not run — prerequisite absent |
| Two-session busy test | `DOCUMENTED_NOT_LIVE_EXERCISED` |

If a future pass satisfies the gate, it must use a non-admin process first, a provisional 1000 ms interval, bounded idle/load runs, the three cadence runs, and the lifecycle sequence above. It must emit only the three `reference.amd_uprof.*` keys, preserve per-core identity, record timestamps and source status, and clean up every session/handle/thread before exit. It must not silently fall back to a different source or to a production metric.

## Lifecycle contract for the next implementation task

No lifecycle evidence is claimed in Q1. The required shape for a future adapter is:

```text
AmdCpuSensorBackend
  probe()
  start(plan)
  sample()
  stop()
```

This is an adapter seam, not a second Provider framework. A future implementation must continue to reuse:

- `ProviderHost` for bounded calls, health, retries/backoff, lifecycle outcomes, and generation reconciliation;
- `CollectionPlan` for requested categories, intervals, and disabled-provider truth;
- capability truth and `ProviderMissing`/permission/unsupported/failed states;
- `MetricProvider` metadata and the existing provider adapter seam;
- `MetricCatalog` only when a later product admission explicitly adds production metadata.

The expected lifecycle evidence is:

- **enable:** a new API session generation starts and real AMD polls are counted;
- **disable:** source poll count stops at zero, the session is stopped/closed/released, and no retry loop remains;
- **re-enable:** a new session generation is initialized; stale handles/state are not reused;
- **final disable:** cleanup is complete, including failed-stop visibility;
- **failure:** a stop/close failure is surfaced as health/error evidence and is not hidden by zero values.

## Failure isolation matrix

The adapter must preserve the distinction between unavailable data and a real numeric zero:

| Failure | Required result |
| --- | --- |
| Library missing or unsafe path | `ProviderMissing` / unavailable; no load from CWD or PATH |
| Driver/service missing | `ProviderMissing` or explicit unavailable reason; no installation attempt |
| Unsupported CPU family/model | `Unsupported` |
| Unsupported counter/category | Per-field unsupported/unavailable; do not invent a substitute |
| Permission denied | `PermissionDenied`; no automatic elevation |
| Profiling session busy | `provider_busy`; bounded backoff, no takeover |
| Initialization failure | `StartupFailed`/unavailable with vendor status preserved |
| Source becomes unavailable during polling | `SampleFailed`/health degradation, bounded retry/backoff, generation-safe recovery |
| Invalid or non-finite value | Discard the field and report probe/source failure; never convert to zero |
| Stop/close failure | `StopFailed`/health failure with cleanup status |
| Microsoft Hypervisor temperature limitation | `UNSUPPORTED_BY_CURRENT_PLATFORM_CONFIGURATION`, not an API bug |

The only legal zero is a real zero explicitly returned by the source with a valid status and timestamp.

## Performance status

No AMD uProf component was installed, so Q1 has no source-specific measurements:

- poll interval / duration: `NOT_AVAILABLE`;
- average probe CPU / P95 probe CPU: `NOT_AVAILABLE`;
- peak working set: `NOT_AVAILABLE`;
- handle count / thread count: `NOT_AVAILABLE`;
- per-call API latency: `NOT_AVAILABLE`;
- AMD driver/service CPU: `NOT_ATTRIBUTABLE`.

The 1000 ms interval is a deployment recommendation, not a measurement. AMD's public limitation sets a 100 ms minimum for the CLI and recommends a larger period to reduce sampling/rendering overhead; the API-specific interval constraint still requires the official API artifact. PR #18 generic Windows/MAHM measurements are not reused as AMD uProf overhead evidence.

## Per-metric and deployment decisions

- `CPU_PACKAGE_TEMPERATURE`: `DEFER_CURRENT_PLATFORM_CONFIGURATION` on this machine; otherwise `SOURCE_CANDIDATE` pending live/API/privilege evidence. Keep package scope, Celsius, Tctl reference, and average sampling-window semantics.
- `CPU_PACKAGE_POWER`: `SOURCE_CANDIDATE` pending live/API/privilege evidence. Keep package scope, Watts, average sampling-window semantics, and the `estimated` qualifier.
- `AMD_CORE_EFFECTIVE_FREQUENCY`: `SOURCE_CANDIDATE` as per-core reference data. Preserve core identity and MHz/window semantics.
- `CPU_EFFECTIVE_FREQUENCY`: `DEFER_AGGREGATION_CONTRACT`; no package aggregate is established.

**Deployment verdict:** `DEFER`

**Production status:** `NO_PRODUCTION_INTEGRATION_IN_Q1`

The source is not rejected: its static metric semantics and Ryzen 9000 support are useful. It is not qualified for bundled distribution, and it is not yet qualified as an external-installed dependency because live/API/privilege evidence is missing and the legal distribution gate is unresolved.

## Validation and delivery record

Q1 makes documentation-only changes. The final repository validation was run on this branch after the review-repair edits:

| Check | Final result |
| --- | --- |
| `cargo fmt --manifest-path tools/metric-probe/Cargo.toml -- --check` | `PASS` (exit 0; Cargo reported only the environment warning `could not canonicalize path C:\Users\Hello`) |
| `cargo test --manifest-path tools/metric-probe/Cargo.toml` | `PASS` — 57 passed; 0 failed; 0 ignored |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | `PASS` (exit 0; same environment warning) |
| `cargo check --manifest-path src-tauri/Cargo.toml` | `PASS` |
| `cargo test --manifest-path src-tauri/Cargo.toml` | `PASS` — 225 passed; 0 failed; 2 ignored; the binary/doc-test targets also reported 0 tests |
| `git diff --check` | `PASS` |
| `tools/metric-probe` release build | `NOT_REQUIRED` — probe source unchanged |

No 10-hour soak or new long-run qualification was run. The live AMD uProf runs remain separately blocked by the absent installation and are not represented as validation passes.

## Next step

`CPU-SENSOR-AMD-LIVE-QUALIFICATION`

That next task should begin only after a user/owner has legally installed the exact AMD uProf 5.3 package and made the official API header/PDF/sample available without Q1 downloading, accepting, elevating, or changing the platform.
