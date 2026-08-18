# Spike-01B NVIDIA NVML Implementation Admission

- `BASE_COMMIT`: `0b4ed4a551107ce4cb7fe63cbe8063ba3bd3ea67`
- `START_HEAD`: `0b4ed4a551107ce4cb7fe63cbe8063ba3bd3ea67`
- Branch: `agent/spike-01b-admission-completion`
- Scope: implementation admission on the current development machine only
- Production NVIDIA Provider: not added

This report adds evidence alongside the existing 60-second development-machine report. It does not overwrite or invalidate the older evidence. It is not a claim that NVIDIA GPUs, GeForce GPUs, or NVML are supported across a product line.

## Test Machine And Permission

- OS: Windows 25H2 build 26200, x64
- CPU: AMD Ryzen 7 9700X 8-Core Processor
- Physical memory: 33,996,718,080 bytes
- GPU: NVIDIA GeForce RTX 5070 Ti
- NVIDIA driver: 610.88
- Existing long and deterministic runs: non-administrator process, `elevated=false`
- Administrator comparison: independent PowerShell role check returned `IsInRole(Administrator) = True`; the probe read `machine.elevated = true`
- Power scope: `gpu_board`; the power metric is not whole-system or wall power

## Evidence Matrix

| Evidence | Result |
|---|---|
| 60 s non-admin enabled | PASS |
| 60 s disabled control | PASS |
| Administrator comparison | PASS: valid elevated token, stable sampling, and no material short-run regression |
| 30 min idle | PASS |
| 30 min representative load | PASS for the observed ordinary Chrome/desktop window; not a stress test |
| Enable-disable-re-enable | PASS |
| Shutdown / cleanup | PASS for exercised sessions |
| Missing DLL handling | PASS: deterministic probe-only loader failure injection |
| Partial unsupported metrics | PASS: deterministic probe-only NVML return injection |
| Transient metric failure isolation | PASS: deterministic timeout injection and recovery |
| Init-time fatal runtime failure | PASS: deterministic GPU-lost initialization injection |
| Sampling-stage fatal runtime failure | PASS: deterministic GPU-lost sampling injection |
| Sleep / wake | PASS: real Windows Sleep -> Wake with bounded dropped samples and post-wake recovery |
| Low-power state | N/A / not exercised |
| 24 h soak | DEFERRED |
| Database-growth soak | DEFERRED |
| Cross-hardware NVIDIA | DEFERRED |
| AMD | DEFERRED |
| Intel | DEFERRED |

## Existing 60-Second Evidence

The committed `development-machine.md` and `development-machine.json` remain unchanged. The prior non-admin enabled and disabled controls recorded 30/30 core samples, zero drops and zero late wakeups. The enabled run read all eight requested NVML metrics; the disabled run emitted no GPU samples. The prior result is development-machine evidence only.

## 30-Minute Idle

Release run, non-admin, from `2026-08-18T04:47:46Z` through `2026-08-18T05:17:46Z` UTC:

- Configuration: 1800 seconds, 2000 ms core/GPU interval, process/disk/network/power probes off, GPU probe on.
- Core samples: 900 expected, 900 executed, 0 dropped.
- GPU metric samples: 900 per metric for all eight metrics; 0 failed samples.
- Late wakeups: 0.
- Average probe CPU share: 0.099%; P50 0.098%; P95 0.098%; max 0.147%.
- Working set: 30.77 MiB average, 31.27 MiB peak, 1.47 MiB end-minus-start.
- Threads: peak 4, end 1. Handles: 124 at start, average, peak and end.
- NVML call latency P50/P95/max stayed below 0.018/0.024/0.907 ms for the eight metrics; the temperature maximum was the largest observed single-call outlier.
- Input responsiveness: the probe did not instrument keyboard or mouse latency; no stutter observation was recorded during this isolated run, and no causal conclusion is made.

This is a clean idle window as observed by the execution environment. No GPU benchmark or intentional GPU load was started.

## 30-Minute Representative Load

Release run, non-admin, from `2026-08-18T05:22:25Z` through `2026-08-18T05:52:25Z` UTC. The window overlapped ordinary Chrome/desktop activity already present on the machine. No artificial maximum-power stress test or benchmark was used.

- Core samples: 900/900, 0 dropped; late wakeups: 0.
- GPU metric samples: 900 per metric, 0 failures.
- Average probe CPU share: 0.099%; peak observed share 0.195%.
- Working set: 31.15 MiB peak; handles remained 124; peak threads 4 and end threads 1.
- Observed GPU ranges: utilization 1-21%; temperature 48-50 C; board power 39.610-57.464 W; graphics clock 2527-2745 MHz; memory clock 16001 MHz; memory-controller utilization 0-6%; VRAM used 3.20-4.39 GiB; VRAM total 17,094,934,528 bytes.
- The load was ordinary desktop/browser activity during this particular window. It must not be generalized to games, compute workloads, or all user workloads.
- Call latency remained in the same sub-millisecond range as idle for every NVML metric; no failure or drop increase was observed.
- Input responsiveness was not instrumented. No automated evidence of degradation was available and no causal claim is made.

## Administrator Comparison

A valid same-configuration 60-second Release run was executed from a PowerShell session independently confirmed as Administrator:

`[Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) = True`

The probe itself read the actual process token as `machine.elevated = true`.

- Configuration: 60 seconds, 2000 ms interval, non-GPU probe categories disabled.
- Core samples: 30 expected, 30 executed, 0 dropped.
- Late wakeups: 0.
- All eight NVIDIA NVML metrics: `supported`, 30 successful samples and 0 failed samples each.
- Average probe CPU share: approximately 0.101042%; peak approximately 0.146484%.
- Working set: 30,695,424 bytes start, 30,740,480 bytes peak, 30,711,808 bytes end; end-minus-start +16,384 bytes.
- Threads: 4 start, 4 peak, 1 end.
- Handles: 122 start, average, peak and end; delta 0.

No material capability, sampling-stability, or resource-behavior regression was observed relative to the non-administrator short run on this development machine. This result is limited to this machine and does not establish uniform permission behavior for all NVIDIA devices. An earlier elevation attempt that reported `elevated=false` is historical only and is superseded by this valid token-confirmed comparison.

## Lifecycle And Cleanup

The deterministic lifecycle report exercised:

`load library -> nvmlInit_v2 -> sample -> nvmlShutdown -> release library -> disabled interval -> load again -> nvmlInit_v2 -> sample -> final shutdown -> final library release`

Observed in the current Release run:

- Enabled phase: 10 samples, 60 GPU metric calls, 0 failures, shutdown `ok`, library released and session resources released.
- Disabled phase: 0 samples, 0 GPU calls, no NVML metric calls, no new session.
- Re-enabled phase: 10 samples, 60 GPU metric calls, 0 failures, shutdown `ok`, library released and session resources released.
- Final cleanup completed: `true` while the process was still alive before exit.
- Threads stayed at 4 in the short lifecycle snapshots; handles were stable after the initial load. The initial short-run delta was +34 handles and +24,252,416 bytes working set. The two 30-minute runs showed handles stable at 124 and only about 1.5 MiB end-minus-start working-set change after initialization.

The short lifecycle resource delta is recorded rather than treated as a 24-hour leak result. Formal long-duration leak and database-growth gates remain deferred.

## Deterministic Failure And Capability Scenarios

All scenarios are probe-only. They do not change the production loader search path and do not copy an implementation into Tauri.

### Missing DLL: deterministic loader failure

The probe-only injected loader was exercised through the same loader result path used by native initialization. It returned `provider_missing` with reason `nvml_runtime_missing`; the provider was not established, one load attempt was recorded, no library was acquired, and it emitted zero GPU calls and zero GPU samples. It did not retry indefinitely, did not crash, and completed cleanup safely. The scenario also checks that the CPU and memory probe statuses remain supported before and after the injection.

### Partial Unsupported

The injected dispatch returned the official unsupported-equivalent result for power and memory clock only. Utilization, temperature, graphics clock and VRAM continued sampling. Unsupported metrics retained explicit `unsupported` status and `nvml_not_supported` reason, with no numeric zero. The GPU device remained usable and cleanup completed.

### Transient Failure

The injected power call returned `nvml_timeout` once, then succeeded. Other metrics continued sampling, one failed sample was counted, the failure reason was retained, a later successful sample recovered the metric, and cleanup completed without a retry loop.

### Provider Initialization Runtime Failure

The injected initialization returned `runtime_failed` with reason `nvml_gpu_lost`. No GPU sample was emitted, no GPU metric call was made, the process remained alive, and cleanup was safe. This is distinct from unsupported and ordinary probe failure. It is initialization-failure evidence, not sampling-stage failure evidence.

### Sampling-Stage Fatal Runtime Failure

The provider initialized successfully and completed one normal sample. The next injected utilization call returned `NVML_ERROR_GPU_IS_LOST`, which mapped to `runtime_failed` / `nvml_gpu_lost`; the failed sample had no numeric value, other metric calls continued for that bounded sample, the exact expected metric-call count showed no synchronous retry loop, and shutdown/library release completed. This is deterministic probe-only boundary evidence, not evidence that the physical GPU was actually lost.

## Real Windows Sleep / Wake

A 900-second non-administrator Release probe was run across a real Windows Sleep -> Wake cycle using the normal Windows power action. No simulated thread sleep or timestamp injection was used.

Scheduler evidence:

- Wall duration: 900,000 ms.
- Core samples: 450 expected, 386 executed, 64 dropped; the accounting closes exactly: `450 = 386 + 64`.
- Late wakeups: 1.
- Maximum timestamp gap: 133,866 ms (133.866 seconds).
- Last pre-sleep utilization sample: `2026-08-18 20:49:48 +08:00`.
- First post-wake utilization sample: `2026-08-18 20:52:02 +08:00`.
- Normal surrounding interval: approximately 2 seconds.

The approximately 133.9-second gap is consistent with the manually performed Windows sleep window. A timestamp gap alone would not prove sleep; this result is accepted as Sleep/Wake evidence because the user actually executed the Windows Sleep -> Wake action. The probe did not backfill or fabricate continuous 2-second samples across the sleep interval. Missed periods were recorded as dropped samples.

Wake recovery evidence:

- The first post-wake utilization sample was a valid numeric value of 5%, followed by 4%, 9%, 1%, 1%, and 1% at the normal cadence.
- Utilization had 384 successful samples and 384 unique timestamps; no duplicate timestamp or sleep-gap fabricated sample was observed.
- Temperature: 386 successful samples, 0 failed.
- VRAM used: 386 successful samples, 0 failed.
- VRAM total: 386 successful samples, 0 failed.
- Utilization, memory-controller utilization, power, graphics clock, and memory clock: 384 successful samples and 2 failed samples each, followed by continued successful sampling.
- All eight metrics remained `supported`.
- The five affected metrics ended with reason `partial_sampling_failures`; this records bounded sampling failures within the run, not loss of support.
- The available evidence shows the existing probe session resumed successful NVML sampling after wake; no explicit reinitialization event was observed in the available evidence. This does not infer driver-internal behavior.

The exact native failure reason for the bounded failures was not surfaced in the inspected report projection, so this report does not label them as timeout, GPU lost, or driver reset.

## Admission Decision

**PR-04 NVIDIA implementation admission: PASS on this development machine**

The short-term implementation-admission gate is satisfied for the current development machine. The valid administrator comparison and real Windows Sleep/Wake observation completed the remaining manual evidence items. This is a machine-scoped implementation-admission result; it does not claim NVIDIA product-family support, all GeForce support, default enablement, production readiness, broad driver compatibility, a complete release support matrix, long-term leak safety, a completed 24-hour soak, or a completed database-growth gate.

The next permitted stage is to begin a scoped PR-04 production NVIDIA Provider implementation behind capability detection and the existing provider/lifecycle contracts. This does not mean the NVIDIA feature is done. PR-04 must preserve unsupported, permission, missing-provider, runtime-failure and legal-zero semantics and must perform its own runtime integration tests. No production NVML Provider was added in this Spike.

## Deferred Scope

- 24-hour soak and database-growth soak: PR-07 release/stability validation.
- Broad NVIDIA hardware and driver matrix: later support-matrix evidence.
- AMD and Intel GPU validation: later provider work.
- Production Tauri Provider, storage changes, schema changes, UI, CPU sensors, crash analysis and PR-05: not part of this Spike.

## Privacy And Artifacts

The committed report is sanitized. It retains only the development-machine hardware identity needed to interpret the evidence, metric status, units, ranges and aggregate timing/resource figures. Raw JSON/Markdown artifacts remain under ignored `artifacts/metric-probe/` and are not committed. No account identity, host identity, absolute path, process command line, window caption, security identifier or database contents are included here.
