# Native release qualification harness

`release-soak` is a development/qualification-only observer for the native Tauri process. It attaches to an already-running executable, samples process and SQLite state at a configurable cadence, and writes sanitized JSONL evidence. It never launches the product, reads process command lines, reads application content, or mutates the database during normal observation.

## Build

```powershell
cargo build --release --manifest-path tools/release-soak/Cargo.toml
```

## Commands

```powershell
# Check an isolated qualification database without opening it for writes.
release-soak.exe schema --db <qualification-db>

# Attach to a native release process. Duration is configurable; 24h is not implicit.
release-soak.exe run --pid <pid> --db <qualification-db> --duration 10m `
  --output <raw.jsonl> --events <events.jsonl> --cadence 30s --warmup 60s `
  --app-version 0.3.2 --git-commit <commit> --build-type release `
  --executable-name resource-timeline.exe

# Derive a sanitized summary from raw evidence.
release-soak.exe summary --input <raw.jsonl> --events <events.jsonl> --output <summary.json>

# Append an allow-listed operator event.
release-soak.exe mark --output <events.jsonl> --kind dynamic_disable_gpu

# Hold a bounded SQLite write lock and roll it back; no data/schema mutation is issued.
release-soak.exe database-busy --db <qualification-db> --duration 3s --events <events.jsonl>
```

The formal command uses `--duration 24h`. A shorter run, unit test, or summary projection is never a 24-hour soak.

## Evidence boundary

Evidence records UTC and epoch timestamps, monotonic elapsed time, process CPU using whole-machine percentage, working set/private memory where available, threads, handles, SQLite main/WAL/SHM sizes, schema/integrity metadata, committed frame counts, writer delay, persisted provider health, collection configuration, and sanitized machine metadata. It omits absolute paths, usernames, window titles, process command lines, document/application content, raw crash data, serials, and UUIDs.

Collector queue/drop counters and FrameWriter queue/drop counters are not exported by the frozen production API. The summary records them as unavailable and uses committed-frame continuity, SQLite metadata, and persisted writer delay instead of inventing a metric.

## Sleep/wake and clock validation

The harness does not force sleep, wake, reboot, or host clock changes. For a manual sleep/wake run:

1. Start the soak and note the exact timestamp.
2. Manually put Windows to sleep and wake it later.
3. Let the harness continue and inspect wall-clock versus monotonic gaps and post-wake provider health.

System-time-change behavior is covered through the product's deterministic clock seams and local-calendar tests. Destructive host OS clock manipulation is intentionally not run.
