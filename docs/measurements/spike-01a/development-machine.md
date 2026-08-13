# Spike-01A Windows Basic Metrics Probe

- Scope: current development machine result only; not generalizable to other hardware.
- Source schema: `spike-01a/v1`; public measurement document: `spike-01a/public-measurement/v1`.
- Facts below are generated from the final default-v3 and disabled-categories-v3 JSON reports.

## Machine

| Field | Value |
|---|---|
| OS | Windows 25H2 build 26200 |
| Architecture | x64 |
| CPU | AMD Ryzen 7 9700X 8-Core Processor |
| Logical processors | 16 |
| Physical memory | 31.66 GiB |

## Run Comparison

| Run | Duration | Core interval | Process interval | Process | Disk | Network | Power | Core frames | Process frames | Core coverage | Process coverage | Avg probe CPU | Peak working set | Permission |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| default-v3 | 60 s | 2000 ms | 5000 ms | True | True | True | True | 30/30 | 12/12 | 57986 ms | 54997 ms | 0.146519% | 13.43 MiB | non-administrator process |
| disabled-categories-v3 | 60 s | 2000 ms | 5000 ms | False | False | False | False | 30/30 | 0/0 | 57986 ms | n/a ms | 0.141465% | 6.74 MiB | non-administrator process |

The default run completed 30/30 core frames and 12/12 process frames. The disabled-categories run completed 30/30 core frames and 0/0 process frames; disabled category metrics are absent from that report.

## Probe Resources

| Run | Resource | Unit | Samples | Average | P50 | P95 | Max | Peak |
|---|---|---|---:|---:|---:|---:|---:|---:|
| default-v3 | probe_cpu_share_percent | percent | 29 | 0.146519 | 0.146484 | 0.175811 | 0.19541 | 0.19541 |
| default-v3 | working_set_bytes | bytes | 30 | 13.28 MiB | 13.27 MiB | 13.43 MiB | 13.43 MiB | 13.43 MiB |
| default-v3 | thread_count | count | 30 | 6 | 6 | 7 | 7 | 7 |
| default-v3 | handle_count | count | 30 | 201.5 | 201.5 | 202 | 202 | 202 |
| disabled-categories-v3 | probe_cpu_share_percent | percent | 29 | 0.141465 | 0.146484 | 0.175811 | 0.19541 | 0.19541 |
| disabled-categories-v3 | working_set_bytes | bytes | 30 | 6.73 MiB | 6.73 MiB | 6.74 MiB | 6.74 MiB | 6.74 MiB |
| disabled-categories-v3 | thread_count | count | 30 | 2.5 | 2.5 | 4 | 4 | 4 |
| disabled-categories-v3 | handle_count | count | 30 | 90 | 90 | 90 | 90 | 90 |

## Metric Coverage And Call Cost

| Run | Metric | Status | Samples | Failed | Coverage | Call P50 | Call P95 | Call Max | Latest |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| default-v3 | `cpu.usage_percent` | supported | 29 | 0 | 57986 ms | 0.0534 ms | 0.22944 ms | 0.317 ms | 3.857422 |
| default-v3 | `memory.used_bytes` | supported | 30 | 0 | 57986 ms | 0.0043 ms | 0.006455 ms | 0.0083 ms | 17.21 GiB |
| default-v3 | `memory.available_bytes` | supported | 30 | 0 | 57986 ms | 0.0043 ms | 0.006455 ms | 0.0083 ms | 14.45 GiB |
| default-v3 | `memory.usage_percent` | supported | 30 | 0 | 57986 ms | 0.0043 ms | 0.006455 ms | 0.0083 ms | 54.369709 |
| default-v3 | `probe.cpu_time_100ns` | supported | 30 | 0 | 57986 ms | 34.9574 ms | 36.32346 ms | 37.6342 ms | 17343750 |
| default-v3 | `probe.working_set_bytes` | supported | 30 | 0 | 57986 ms | 34.9574 ms | 36.32346 ms | 37.6342 ms | 13.43 MiB |
| default-v3 | `probe.thread_count` | supported | 30 | 0 | 57986 ms | 34.9574 ms | 36.32346 ms | 37.6342 ms | 7 |
| default-v3 | `probe.handle_count` | supported | 30 | 0 | 57986 ms | 34.9574 ms | 36.32346 ms | 37.6342 ms | 202 |
| default-v3 | `process.enumerated_count` | supported | 12 | 0 | 54997 ms | 0.67335 ms | 0.86961 ms | 0.8761 ms | 346 |
| default-v3 | `process.accessible_count` | supported | 12 | 0 | 54997 ms | 0.67335 ms | 0.86961 ms | 0.8761 ms | 153 |
| default-v3 | `process.restricted_count` | supported | 12 | 0 | 54997 ms | 0.67335 ms | 0.86961 ms | 0.8761 ms | 193 |
| default-v3 | `process.enumeration_elapsed_ms` | supported | 12 | 0 | 54997 ms | 0.67335 ms | 0.86961 ms | 0.8761 ms | 0.873 |
| default-v3 | `process.detail_cpu_time_readable_count` | supported | 12 | 0 | 54997 ms | 2.28925 ms | 2.62896 ms | 2.6745 ms | 127 |
| default-v3 | `process.detail_working_set_readable_count` | supported | 12 | 0 | 54997 ms | 2.28925 ms | 2.62896 ms | 2.6745 ms | 153 |
| default-v3 | `process.detail_private_memory_readable_count` | supported | 12 | 0 | 54997 ms | 2.28925 ms | 2.62896 ms | 2.6745 ms | 153 |
| default-v3 | `process.detail_io_readable_count` | supported | 12 | 0 | 54997 ms | 2.28925 ms | 2.62896 ms | 2.6745 ms | 127 |
| default-v3 | `process.detail_permission_denied_count` | supported | 12 | 0 | 54997 ms | 2.28925 ms | 2.62896 ms | 2.6745 ms | 219 |
| default-v3 | `process.detail_probe_failed_count` | supported | 12 | 0 | 54997 ms | 2.28925 ms | 2.62896 ms | 2.6745 ms | 0 |
| default-v3 | `process.detail_raced_count` | supported | 12 | 0 | 54997 ms | 2.28925 ms | 2.62896 ms | 2.6745 ms | 0 |
| default-v3 | `system.uptime_ms` | supported | 30 | 0 | 57986 ms | 0.0004 ms | 0.0006 ms | 0.0011 ms | 34835734 |
| disabled-categories-v3 | `cpu.usage_percent` | supported | 29 | 0 | 57986 ms | 0.0238 ms | 0.11222 ms | 0.1317 ms | 2.681619 |
| disabled-categories-v3 | `memory.used_bytes` | supported | 30 | 0 | 57986 ms | 0.00405 ms | 0.005755 ms | 0.0061 ms | 17.21 GiB |
| disabled-categories-v3 | `memory.available_bytes` | supported | 30 | 0 | 57986 ms | 0.00405 ms | 0.005755 ms | 0.0061 ms | 14.46 GiB |
| disabled-categories-v3 | `memory.usage_percent` | supported | 30 | 0 | 57986 ms | 0.00405 ms | 0.005755 ms | 0.0061 ms | 54.345432 |
| disabled-categories-v3 | `probe.cpu_time_100ns` | supported | 30 | 0 | 57986 ms | 34.05195 ms | 35.17837 ms | 35.3874 ms | 13125000 |
| disabled-categories-v3 | `probe.working_set_bytes` | supported | 30 | 0 | 57986 ms | 34.05195 ms | 35.17837 ms | 35.3874 ms | 6.73 MiB |
| disabled-categories-v3 | `probe.thread_count` | supported | 30 | 0 | 57986 ms | 34.05195 ms | 35.17837 ms | 35.3874 ms | 1 |
| disabled-categories-v3 | `probe.handle_count` | supported | 30 | 0 | 57986 ms | 34.05195 ms | 35.17837 ms | 35.3874 ms | 90 |
| disabled-categories-v3 | `system.uptime_ms` | supported | 30 | 0 | 57986 ms | 0.0004 ms | 0.000655 ms | 0.001 ms | 34835359 |

The uptime latency row is measured from the direct uptime API call. Unsupported, permission-denied, and probe-failed states are represented as statuses and do not become numeric zero samples.

## Independent Process Detail Readability

| Run | CPU time readable | Working set readable | Private memory readable | I/O readable | Permission denied | Probe failed | Exited/raced |
|---|---:|---:|---:|---:|---:|---:|---:|
| default-v3 | 127 | 153 | 153 | 127 | 219 | 0 | 0 |
| disabled-categories-v3 | n/a | n/a | n/a | n/a | n/a | n/a | n/a |

These are independent final-sample aggregate counts. Working set and private memory share one underlying API call, so equal counts are expected when that call succeeds; CPU time and I/O are not coupled to memory success.

## Permission Tests

| Integrity level | Status | Detail |
|---|---|---|
| non-administrator | completed | non-administrator process |
| administrator | pending | Not executed; no UAC elevation was triggered. |

No UAC elevation was triggered. The administrator comparison remains pending.

## Resource Conclusion

- Default v3 average probe CPU share: 0.146519% of whole machine CPU; budget check under 0.5%: True.
- Default v3 peak probe working set: 13.43 MiB; budget check under 80 MiB: True.
- Disabled-categories v3 average probe CPU share: 0.141465%; peak working set: 6.74 MiB.
- Observed default-minus-disabled difference: 0.005054 percentage points CPU and 6.69 MiB working set.
- These are current-machine observations, not product-wide performance claims.

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
