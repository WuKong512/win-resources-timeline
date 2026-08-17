# PR-04A GPU Storage Contract Evidence

## Status

- Scope: schema/runtime storage only; no production vendor Provider.
- BASE_COMMIT: `bc4491e8623388e10cb3e50c71a6e2efff3dbdb5`
- Storage version: v8, forward-only from v7.
- NVIDIA NVML production admission: **pending**.

PR-04A separates storage readiness from hardware Provider admission. The current Spike-01B report is still a single-development-machine feasibility report for an NVIDIA GeForce RTX 5070 Ti. It does not authorize a production NVML Provider or support across the NVIDIA product line.

## Storage Admission Matrix

| Metric | Storage mapping | Unit | Scope | Null/zero behavior | Provider admission |
| --- | --- | --- | --- | --- | --- |
| `gpu.utilization_percent` | `gpu_sample.usage_pct` | percent | per device | NULL means absent; `0` is legal | pending Spike-01B completion |
| `gpu.memory_controller_utilization_percent` | `gpu_sample.memory_controller_usage_pct` | percent | per device | NULL means absent; `0` is legal | pending Spike-01B completion |
| `gpu.temperature_celsius` | `gpu_sample.temp_c` | Celsius | per device | NULL means absent; `0` is a numeric value if source reports it | pending Spike-01B completion |
| `gpu.power_watts` | `gpu_sample.board_power_w` + `power_scope` | W | `gpu_board` only | NULL means absent; never whole-system power | pending Spike-01B completion |
| `gpu.graphics_clock_mhz` | `gpu_sample.core_clock_mhz` | MHz | per device | NULL means absent; `0` is a numeric value | pending Spike-01B completion |
| `gpu.memory_clock_mhz` | `gpu_sample.memory_clock_mhz` | MHz | per device | NULL means absent; `0` is a numeric value | pending Spike-01B completion |
| `gpu.vram_used_bytes` | `gpu_sample.vram_used_bytes` | bytes | per device | NULL means absent; `0` is legal | pending Spike-01B completion |
| `gpu.vram_total_bytes` | `gpu_sample.vram_total_bytes` | bytes | per device | NULL means absent; `0` is a numeric value only if source reports it | pending Spike-01B completion |

Unsupported, disabled, permission-denied and failed states are not encoded as numeric zero. They remain in `collection_session_metric.support_status/enabled`, with `collection_session_metric.provider_id -> provider.id` (and the provider's `kind`/`name`/`version`) plus `interval_ms` available as a storage-contract capability for historical traceability. PR-04A does not contain a production Provider that automatically maintains this metadata truth; that behavior is part of PR-04 admission.

Every GPU row also carries provider-neutral `quality_mask`, including `0`. It is independent from nullable metric columns: `Some(0)` remains a legal numeric zero, while `None` remains SQL `NULL`.

## Runtime Evidence

- `ResourceSnapshot.system.gpus` supports zero, one or multiple devices.
- `hardware_device.stable_key` routes samples; the writer does not use array position as database identity.
- GPU rows are written in the same transaction as `sample_frame`, CPU, memory, disk and process rows.
- Invalid board-power scope aborts the frame transaction, leaving no partial frame/device/sample rows; a corrected retry commits cleanly.
- Query service restores nested GPU samples in `SystemSample` and supports a time-range query with an optional stable device key.
- `get_gpu_samples` requires `maxPoints` in the inclusive range 500..10000. The limit is per device, so two devices may each return up to the requested number of points; SQL-side per-device selection prevents the command from exposing an unlimited raw-row contract. `system_samples` selects its bounded frame set before loading GPU rows.
- v7 GPU rows are preserved by migration; existing non-NULL `board_power_w` rows receive `power_scope = 'gpu_board'`.

## Remaining PR-04 Work

Before a production NVIDIA Provider can be admitted, Spike-01B must add the short-term evidence still marked pending: administrator comparison, 30-minute idle, 30-minute representative load, enable/disable/re-enable, shutdown/cleanup, missing DLL, partial unsupported/failure behavior and feasible sleep/wake or low-power lifecycle checks.

24-hour soak, database-growth soak, broad NVIDIA hardware coverage, AMD/Intel validation and the complete release hardware matrix remain later default-enable, support-matrix or release/stability gates. They are not PR-04A storage entry blockers, and PR-04A does not claim they are complete.
