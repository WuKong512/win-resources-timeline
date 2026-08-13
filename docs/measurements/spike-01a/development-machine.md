# Spike-01A Windows Basic Metrics Probe

- Facts below are generated from the final-default-v4 and final-disabled-v4 JSON reports.
- Scope: current development machine result only; not generalizable to other hardware.

## Machine

- OS: Windows 25H2 build 26200
- CPU: AMD Ryzen 7 9700X 8-Core Processor
- Architecture: x64; logical processors: 16
- Physical memory: 31.66 GiB

## Runs

| Run | Duration | Core interval | Process interval | Categories | Core frames | Process frames | Core coverage | Process coverage | Permission scope |
|---|---:|---:|---:|---|---:|---:|---:|---:|---|
| default-v4 | 60 s | 2000 ms | 5000 ms | process=on, disk=on, network=on, power=on | 30/30 | 12/12 | 57986 ms | 54995 ms | non-administrator process |
| disabled-categories-v4 | 60 s | 2000 ms | 5000 ms | process=off, disk=off, network=off, power=off | 30/30 | 0/0 | 57987 ms | n/a | non-administrator process |

All v4 runs executed as a non-administrator process. The administrator comparison is pending; no UAC elevation was triggered.

## Probe Resources

| Run | Resource | Unit | Samples | Average | P50 | P95 | Max | Peak |
|---|---|---|---:|---:|---:|---:|---:|---:|
| default-v4 | probe_cpu_share_percent | percent | 29 | 0.099363562438547 | 0.097705102551276 | 0.097705102551276 | 0.146484375 | 0.146484375 |
| default-v4 | working_set_bytes | bytes | 30 | 13.00 MiB | 13.00 MiB | 13.12 MiB | 13.12 MiB | 13.12 MiB |
| default-v4 | thread_count | count | 30 | 6.86666666666667 | 7 | 8 | 8 | 8 |
| default-v4 | handle_count | count | 30 | 201.5 | 201.5 | 202 | 202 | 202 |
| disabled-categories-v4 | probe_cpu_share_percent | percent | 29 | 0.099361877867813 | 0.09765625 | 0.097705102551276 | 0.146484375 | 0.146484375 |
| disabled-categories-v4 | working_set_bytes | bytes | 30 | 6.18 MiB | 6.19 MiB | 6.20 MiB | 6.20 MiB | 6.20 MiB |
| disabled-categories-v4 | thread_count | count | 30 | 2.6 | 4 | 4 | 4 | 4 |
| disabled-categories-v4 | handle_count | count | 30 | 90 | 90 | 90 | 90 | 90 |

## Metrics

| Run | Metric | Status | Samples | Failed | Coverage | Call P50 | Call P95 | Call Max | Latest |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| default-v4 | `cpu.usage_percent` | supported | 29 | 0 | 57986 ms | 0.0297 ms | 0.09328 ms | 0.25 ms | 5.76171875 |
| default-v4 | `memory.used_bytes` | supported | 30 | 0 | 57986 ms | 0.0036 ms | 0.003855 ms | 0.0039 ms | 14438449152 |
| default-v4 | `memory.available_bytes` | supported | 30 | 0 | 57986 ms | 0.0036 ms | 0.003855 ms | 0.0039 ms | 19558268928 |
| default-v4 | `memory.usage_percent` | supported | 30 | 0 | 57986 ms | 0.0036 ms | 0.003855 ms | 0.0039 ms | 42.4701264340396 |
| default-v4 | `probe.cpu_time_100ns` | supported | 30 | 0 | 57986 ms | 30.5333 ms | 32.64418 ms | 37.0872 ms | 12968750 |
| default-v4 | `probe.working_set_bytes` | supported | 30 | 0 | 57986 ms | 30.5333 ms | 32.64418 ms | 37.0872 ms | 13758464 |
| default-v4 | `probe.thread_count` | supported | 30 | 0 | 57986 ms | 30.5333 ms | 32.64418 ms | 37.0872 ms | 8 |
| default-v4 | `probe.handle_count` | supported | 30 | 0 | 57986 ms | 30.5333 ms | 32.64418 ms | 37.0872 ms | 202 |
| default-v4 | `process.enumerated_count` | supported | 12 | 0 | 54995 ms | 0.402 ms | 0.630545 ms | 0.7857 ms | 308 |
| default-v4 | `process.accessible_count` | supported | 12 | 0 | 54995 ms | 0.402 ms | 0.630545 ms | 0.7857 ms | 130 |
| default-v4 | `process.restricted_count` | supported | 12 | 0 | 54995 ms | 0.402 ms | 0.630545 ms | 0.7857 ms | 178 |
| default-v4 | `process.enumeration_elapsed_ms` | supported | 12 | 0 | 54995 ms | 0.402 ms | 0.630545 ms | 0.7857 ms | 0.4196 |
| default-v4 | `process.detail_cpu_time_readable_count` | supported | 12 | 0 | 54995 ms | 1.58575 ms | 3.14302 ms | 4.7092 ms | 130 |
| default-v4 | `process.detail_working_set_readable_count` | supported | 12 | 0 | 54995 ms | 1.58575 ms | 3.14302 ms | 4.7092 ms | 130 |
| default-v4 | `process.detail_private_memory_readable_count` | supported | 12 | 0 | 54995 ms | 1.58575 ms | 3.14302 ms | 4.7092 ms | 130 |
| default-v4 | `process.detail_io_readable_count` | supported | 12 | 0 | 54995 ms | 1.58575 ms | 3.14302 ms | 4.7092 ms | 130 |
| default-v4 | `process.detail_permission_denied_count` | supported | 12 | 0 | 54995 ms | 1.58575 ms | 3.14302 ms | 4.7092 ms | 178 |
| default-v4 | `process.detail_probe_failed_count` | supported | 12 | 0 | 54995 ms | 1.58575 ms | 3.14302 ms | 4.7092 ms | 0 |
| default-v4 | `process.detail_raced_count` | supported | 12 | 0 | 54995 ms | 1.58575 ms | 3.14302 ms | 4.7092 ms | 0 |
| default-v4 | `system.uptime_ms` | supported | 30 | 0 | 57986 ms | 0.0003 ms | 0.000455 ms | 0.0012 ms | 1973781 |
| disabled-categories-v4 | `cpu.usage_percent` | supported | 29 | 0 | 57987 ms | 0.0202 ms | 0.0922 ms | 0.1629 ms | 2.10681038706516 |
| disabled-categories-v4 | `memory.used_bytes` | supported | 30 | 0 | 57987 ms | 0.0036 ms | 0.005055 ms | 0.0052 ms | 14466830336 |
| disabled-categories-v4 | `memory.available_bytes` | supported | 30 | 0 | 57987 ms | 0.0036 ms | 0.005055 ms | 0.0052 ms | 19529887744 |
| disabled-categories-v4 | `memory.usage_percent` | supported | 30 | 0 | 57987 ms | 0.0036 ms | 0.005055 ms | 0.0052 ms | 42.5536085629122 |
| disabled-categories-v4 | `probe.cpu_time_100ns` | supported | 30 | 0 | 57987 ms | 29.2887 ms | 29.928075 ms | 30.9475 ms | 9218750 |
| disabled-categories-v4 | `probe.working_set_bytes` | supported | 30 | 0 | 57987 ms | 29.2887 ms | 29.928075 ms | 30.9475 ms | 6492160 |
| disabled-categories-v4 | `probe.thread_count` | supported | 30 | 0 | 57987 ms | 29.2887 ms | 29.928075 ms | 30.9475 ms | 1 |
| disabled-categories-v4 | `probe.handle_count` | supported | 30 | 0 | 57987 ms | 29.2887 ms | 29.928075 ms | 30.9475 ms | 90 |
| disabled-categories-v4 | `system.uptime_ms` | supported | 30 | 0 | 57987 ms | 0.0003 ms | 0.0005 ms | 0.0008 ms | 2048203 |

The `system.uptime_ms` latency is measured around the direct uptime API call. Unsupported, permission-denied, and probe-failed states are represented as statuses and are not converted into numeric zero samples.

## Independent Process Detail

The four readable counts are independent child-read outcomes. `process.detail_permission_denied_count` means the number of processes with at least one denied child read; it is not the sum of denied API calls.

| Run | CPU time readable | Working set readable | Private memory readable | I/O readable | Permission denied processes | Probe failed processes | Exited/raced processes |
|---|---:|---:|---:|---:|---:|---:|---:|
| default-v4 | 130 | 130 | 130 | 130 | 178 | 0 | 0 |
| disabled-categories-v4 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |

## v3 Comparison

| Metric | Final v3 | Final v4 | Observed delta |
|---|---:|---:|---:|
| Enumerated processes | 346 | 308 | -38 |
| Limited-access processes | 153 | 130 | -23 |
| CPU time readable processes | 127 | 130 | +3 |
| I/O readable processes | 127 | 130 | +3 |

The v3 and v4 runs enumerated different process populations, so the absolute 127-to-130 CPU-time and I/O differences are observed cross-run deltas only and cannot prove causal improvement. The primary within-run evidence is that v3 had 153 limited-access processes and 127 CPU/I/O-readable processes, while v4 had 130 limited-access processes and 130 readable processes for each of CPU time, working set, private memory, and I/O. Thus, in this v4 run, every process with a successfully opened `PROCESS_QUERY_LIMITED_INFORMATION` handle also produced all four detail reads successfully. This is an observation from the current development machine, not a general Windows 10/11 estimate.

## Permission Tests

| Integrity level | Status | Detail |
|---|---|---|
| non-administrator | completed | non-administrator process |
| administrator | pending | Not executed; no UAC elevation was triggered. |

## Resource Budget

- Default v4 average probe CPU share: 0.099363562438547% of whole-machine CPU; under 0.5% budget: True.
- Default v4 peak probe working set: 13.12 MiB; under 80 MiB budget: True.
- Disabled-categories v4 average probe CPU share: 0.099361877867813%; peak working set: 6.20 MiB.
- Observed default-minus-disabled difference: 0.000001684570734 percentage points CPU and 6.92 MiB.
- The measured default configuration is within the stated CPU and memory budgets on this machine; cross-hardware validation remains deferred.

## Privacy And Deferred Work

- Sanitized: True.
- Process detail retention: in memory only; aggregate counts only in report.
- No user name, computer name, full path, MAC address, IP address, security identifier, process name, or command line is included.

| Item | Status | Reason |
|---|---|---|
| CPU/GPU temperature or power | deferred | Not implemented in this spike |
| GPU utilization, frequency, and VRAM | deferred | Vendor APIs are explicitly out of scope |
| Battery charge/discharge power, health, and cycles | deferred | Windows baseline status only |
| SMART/NVMe temperature | deferred | No storage health API is called |
| Fan or pump telemetry | deferred | No hardware sensor provider is called |
| Windows crash judgment | deferred | No crash inference is implemented |
| Event Log event classification | deferred | No Event Log subscription is implemented |
| Formal sleep/wake event subscription | deferred | No event subscription is implemented |
| Process GPU/VRAM attribution | deferred | No GPU process attribution is implemented |
| Cross-hardware validation | deferred | This result is limited to the current machine |
| Administrator comparison | pending | The other Windows integrity level was not run automatically |

Source reports: `artifacts/metric-probe/final-default-v4/report.json` and `artifacts/metric-probe/final-disabled-v4/report.json`.
