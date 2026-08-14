# Spike-01B NVIDIA NVML GPU Metrics And Background-Cost Probe

- Measurement document: `spike-01b/public-measurement/v1`
- Probe schema: `spike-01b/v1`
- `STARTING_HEAD`: `7c5f03debcc258d5b4e48fb71ec8edcbb0466c06`
- `BASE_COMMIT`: `7c5f03debcc258d5b4e48fb71ec8edcbb0466c06`
- Branch: `agent/spike-01b-nvidia-nvml-probe`
- Scope: current development machine only; not generalizable to other hardware or the NVIDIA product line.

## Machine

- OS: Windows 25H2 build 26200, x64
- CPU: AMD Ryzen 7 9700X 8-Core Processor
- Logical processors: 16
- Physical memory: 33,996,718,080 bytes
- GPU: NVIDIA GeForce RTX 5070 Ti
- NVIDIA driver: 610.88
- Permission scope: non-administrator process, `elevated=false`
- Administrator comparison: pending; no UAC elevation was triggered.

## Provider

- Provider: `nvidia-nvml`
- Loading: `LoadLibraryExW` with `LOAD_LIBRARY_SEARCH_SYSTEM32` for `nvml.dll`, then the official `ProgramW6432` NVIDIA Corporation NVSMI location. No recursive disk scan is used.
- Lifecycle: `nvmlInit_v2` after dynamic loading; `nvmlShutdown` and library release on normal and error cleanup paths.
- Dependencies: no new Rust crate and no third-party binary. The existing `windows 0.58` dependency supplies Win32 loader APIs.
- Boundary: independent `tools/metric-probe` CLI only. No formal production Provider is implemented.

## Measurement Runs

Both runs were sequential Release executions with the same configuration except for the GPU switch. No UAC prompt, administrator switch, GPU stress test, game, benchmark, driver change, or GPU configuration change was performed. No MSI Afterburner or HWiNFO process was started, stopped, or modified.

| Run | UTC window | Duration | Core/GPU interval | Process | Disk | Network | Power baseline | GPU | Core samples | GPU samples | GPU coverage | Permission |
|---|---|---:|---:|---|---|---|---|---|---:|---:|---:|---|
| `nvml-enabled` | 2026-08-14 03:48:44Z to 03:49:44Z | 60 s | 2000 ms | off | off | off | off | on | 30/30 | 30/30 | 57,985 ms / 96.642% | non-admin |
| `nvml-disabled` | 2026-08-14 03:49:53Z to 03:50:53Z | 60 s | 2000 ms | off | off | off | off | off | 30/30 | 0/0 | 0 ms / 0% | non-admin |

Both runs had zero dropped core samples, zero GPU drops, and zero late wakeups. The disabled run created no GPU device or metric records.

## Probe Resources

| Run | Average probe CPU | CPU P50 | CPU P95 | CPU max | Working-set average | Working-set peak | Peak threads | Peak handles |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `nvml-enabled` | 0.0976806763% | 0.0977051026% | 0.0977051026% | 0.146484375% | 28.665 MiB | 28.691 MiB | 4 | 117 |
| `nvml-disabled` | 0.1027352308% | 0.0977051026% | 0.1465576538% | 0.1465576538% | 5.916 MiB | 5.949 MiB | 4 | 83 |

The enabled-minus-disabled observed delta was **-0.0050545545 percentage points CPU**, **+23,846,912 bytes / 22.742 MiB peak working set**, **0 peak threads**, and **+34 peak handles**. These are observed values from two sequential 60-second runs on this machine and do not establish causal overhead.

The enabled run is below the current probe reference targets of average CPU below 0.5% and working set below 80 MiB on this machine. This is not a full-application or long-duration acceptance result.

## NVIDIA Metrics

All eight requested metrics were supported on `gpu:nvidia:index-0`, with 30 samples, zero failures, and 57,985 ms covered duration. `call_latency_ms` is the per-metric NVML call latency distribution.

| Metric | Status | Reason | Unit | Range | Power scope | Samples | Failed | Coverage | Call P50 | Call P95 | Call max | Latest |
|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| `gpu.utilization_percent` | supported | `ready` | percent | 0..100 | n/a | 30 | 0 | 96.642% | 0.00715 ms | 0.009265 ms | 0.501 ms | 6 |
| `gpu.memory_controller_utilization_percent` | supported | `ready` | percent | 0..100 | n/a | 30 | 0 | 96.642% | 0.00715 ms | 0.009265 ms | 0.501 ms | 1 |
| `gpu.temperature_celsius` | supported | `ready` | C | driver-defined non-negative Celsius value | n/a | 30 | 0 | 96.642% | 0.0014 ms | 0.002155 ms | 0.0679 ms | 47 |
| `gpu.power_watts` | supported | `ready` | W | driver-defined non-negative board power | `gpu_board` | 30 | 0 | 96.642% | 0.0022 ms | 0.0033 ms | 0.1686 ms | 41.776 |
| `gpu.graphics_clock_mhz` | supported | `ready` | MHz | driver-defined non-negative clock | n/a | 30 | 0 | 96.642% | 0.0018 ms | 0.00273 ms | 0.0594 ms | 2535 |
| `gpu.memory_clock_mhz` | supported | `ready` | MHz | driver-defined non-negative clock | n/a | 30 | 0 | 96.642% | 0.00025 ms | 0.0005 ms | 0.0006 ms | 16001 |
| `gpu.vram_used_bytes` | supported | `ready` | bytes | 0..`gpu.vram_total_bytes` | n/a | 30 | 0 | 96.642% | 0.01395 ms | 0.020895 ms | 0.0219 ms | 3,527,151,616 |
| `gpu.vram_total_bytes` | supported | `ready` | bytes | non-negative device memory capacity | n/a | 30 | 0 | 96.642% | 0.01395 ms | 0.020895 ms | 0.0219 ms | 17,094,934,528 |

Power is explicitly `unit=W` and `power_scope=gpu_board`. It is board power, not whole-system power, socket input, wall power, or total system power. Unsupported, permission-denied, and failed states are represented by status and reason code and never become numeric zero samples.

## Reference Tool

A single read-only `nvidia-smi` snapshot reported the same GPU and driver, with 16,303 MiB total memory, 3,057 MiB used, and 12,939 MiB free. `nvidia-smi` also uses NVML, so it is not an independent source or implementation. It was used only for same-backend reasonableness checking and does not replace an independent MSI Afterburner or HWiNFO time-window comparison; no independent comparison was run.

## Privacy

- Sanitized: `true`
- Privacy scan: `passed`
- Public data retains only product name, driver version, stable per-run device index, metric status, units, aggregates, and timing distributions.
- Omitted: account identity, host identity, hardware unique identifiers, network address identifiers, absolute paths, process identity/invocation text, window caption text, security identifiers, and database contents.
- Raw measurements remain under ignored `artifacts/metric-probe/` and are not part of the public report.

## Status And Limits

| Item | Status |
|---|---|
| Administrator comparison | pending; no UAC elevation was triggered |
| 30-minute idle window | deferred |
| 30-minute representative-load window | deferred |
| 24-hour soak and storage-growth validation | deferred |
| Cross-hardware NVIDIA/AMD/Intel validation | deferred |
| Formal production GPU Provider | deferred |
| Schema v7, database writes, Tauri runtime, and UI | not modified |

The current machine result supports continuing the isolated NVML feasibility work. It does not authorize a formal production Provider or imply support across the NVIDIA GeForce product line.

Source raw reports: `artifacts/metric-probe/spike-01b-enabled-60s/report.json` and `artifacts/metric-probe/spike-01b-disabled-60s/report.json`.
