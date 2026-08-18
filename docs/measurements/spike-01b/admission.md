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
- Long and deterministic runs: non-administrator process, `elevated=false`
- Power scope: `gpu_board`; the power metric is not whole-system or wall power

## Evidence Matrix

| Evidence | Result |
|---|---|
| 60 s non-admin enabled | PASS |
| 60 s disabled control | PASS |
| Administrator comparison | PENDING: attempted normal UAC launch still reported `elevated=false` |
| 30 min idle | PASS on this machine |
| 30 min representative load | PASS for the observed ordinary Chrome/desktop window; not a stress test |
| Enable-disable-re-enable | PASS |
| Shutdown / cleanup | PASS for exercised sessions |
| Missing DLL handling | PASS: deterministic probe-only loader failure injection |
| Partial unsupported metrics | PASS: deterministic probe-only NVML return injection |
| Transient metric failure isolation | PASS: deterministic timeout injection and recovery |
| Init-time fatal runtime failure | PASS: deterministic GPU-lost initialization injection |
| Sampling-stage fatal runtime failure | PASS: deterministic GPU-lost sampling injection |
| Sleep / wake | PENDING: manual evidence required; not safely automated here |
| Low-power state | Not applicable / not exercised on this desktop |
| 24 h soak | DEFERRED to PR-07 release validation |
| Database-growth soak | DEFERRED to PR-07 release validation |
| Cross-hardware NVIDIA | DEFERRED; this is one RTX 5070 Ti |
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

A same-configuration 60-second run was launched through the normal Windows `Start-Process -Verb RunAs` path from `2026-08-18T05:19:04Z` through `2026-08-18T05:20:04Z` UTC. Its report recorded `elevated=false`, so it is not a valid administrator comparison and is not marked PASS. It nevertheless showed the same device discovery, eight supported metrics, 30/30 samples, zero drops, zero failures and similar resource counts as the non-admin short run. This observation is correlation only, not evidence that elevation has no effect.

Manual administrator procedure:

```powershell
Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-Command','tools\metric-probe\target\release\metric-probe.exe run --duration-seconds 60 --core-interval-ms 2000 --output-dir artifacts/metric-probe/spike-01b-admin-manual --no-process-probe --no-disk-probe --no-network-probe --no-power-probe'
$r = Get-Content artifacts/metric-probe/spike-01b-admin-manual/report.json | ConvertFrom-Json
$r.machine.elevated
```

The result is valid only when the final command prints `True` and the same configuration is used.

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

## Sleep / Wake Manual Evidence

This Codex execution environment did not safely initiate and observe Windows sleep/wake, so this item is explicitly `PENDING`, not PASS. No previous GPU value should be filled across the sleep interval.

Manual procedure:

1. Start the Release probe for at least 900 seconds with the same GPU-only configuration.
2. After the first several samples, use the normal Windows Start > Power > Sleep action.
3. Wake the machine, allow the probe to continue, and let it finish.
4. Run the following result command against the raw report:

```powershell
$r = Get-Content artifacts/metric-probe/spike-01b-sleep-wake-manual/report.json | ConvertFrom-Json; $m = $r.metrics | Where-Object { $_.provider -eq 'nvidia-nvml' -and $_.metric_key -eq 'gpu.utilization_percent' }; $p = @($m.samples | Sort-Object timestamp_ms); $gaps = for ($i = 1; $i -lt $p.Count; $i++) { [pscustomobject]@{ gap_ms = $p[$i].timestamp_ms - $p[$i-1].timestamp_ms; previous_ms = $p[$i-1].timestamp_ms; current_ms = $p[$i].timestamp_ms } }; $gaps | Sort-Object gap_ms -Descending | Select-Object -First 5; $r.sampling; $m.support_status; $m.reason_code
```

The manual result must record the last pre-sleep timestamp, first post-wake timestamp, gap, first post-wake NVML result, whether re-init was necessary, absence of fabricated duplicate samples, and final cleanup. A scheduling gap alone must not be labeled as sleep.

## Admission Decision

**PR-04 NVIDIA implementation admission: PARTIAL**

Most short-term implementation-admission evidence is complete on this development machine, including the 30-minute idle/load windows, lifecycle release/re-enable behavior, loader failure handling, partial unsupported metrics, and transient/sampling-stage failure classification. The admission remains `PARTIAL` because two genuinely manual entry items remain: a valid administrator comparison and sleep/wake observation. Until both are completed, this report does not declare the PR-04 production NVIDIA Provider entry gate satisfied. No production NVML Provider was added in this Spike.

The evidence remains limited to this development machine and does not say that NVIDIA GPUs are supported, that all GeForce GPUs are production ready, or that NVML is proven low overhead on all machines. Default-enable policy, support-matrix coverage, release hardware gates, and PR-04 runtime integration tests remain later work. PR-04 must preserve unsupported, permission, missing-provider, runtime-failure and legal-zero semantics.

## Deferred Scope

- 24-hour soak and database-growth soak: PR-07 release/stability validation.
- Broad NVIDIA hardware and driver matrix: later support-matrix evidence.
- AMD and Intel GPU validation: later provider work.
- Production Tauri Provider, storage changes, schema changes, UI, CPU sensors, crash analysis and PR-05: not part of this Spike.

## Privacy And Artifacts

The committed report is sanitized. It retains only the development-machine hardware identity needed to interpret the evidence, metric status, units, ranges and aggregate timing/resource figures. Raw JSON/Markdown artifacts remain under ignored `artifacts/metric-probe/` and are not committed. No account identity, host identity, absolute path, process command line, window caption, security identifier or database contents are included here.
