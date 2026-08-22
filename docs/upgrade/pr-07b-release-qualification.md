# PR-07B Native Release Qualification

## Current phase

**PR-07B PRE-SOAK QUALIFICATION**

`RESULT: READY_FOR_24H`

The 24-hour native run has not been completed. The 10-minute run below validates the harness and native attachment path only; it is not a release-gate soak and is not a 24-hour PASS.

## Identity and entry gate

- `BASE_COMMIT`: `89a9cc821771bd65b80c730bf922b37ff7777075`
- `START_HEAD`: `89a9cc821771bd65b80c730bf922b37ff7777075`
- Branch: `agent/pr-07b-native-release-qualification`
- Main at entry: `origin/main` matched `89a9cc821771bd65b80c730bf922b37ff7777075`
- Static schema target: `PRAGMA user_version = 8`
- Qualification database: isolated Tauri identifier/data root; the existing user database was not opened for migration or writes
- Feature seam: `qualification` changes only the test build's mutex name and disables autostart side effects; normal product builds retain the existing behavior

The current installed/user database was observed separately as schema 7 and was not used for qualification. The isolated qualification database passed `user_version=8`, `quick_check=ok`, and foreign-key check count `0`.

## Observability audit

| Area | Result |
| --- | --- |
| CPU, working set, private memory, threads, handles | Externally observable from the native PID using Windows APIs |
| SQLite main/WAL/SHM sizes, schema, sessions, committed frames, writer delay | Read-only harness observation |
| Provider persisted status, enabled/failed/unsupported metric counts | Read-only SQLite observation; GUI lifecycle state also observed |
| Collection configuration, sample counts, runtime duration, retention holds | Read-only SQLite observation and evidence header/footer |
| Collector drops, writer queue depth, writer drops | Missing from frozen external production contract; recorded as unavailable rather than adding a production debug API |
| Crash retention holds | Read-only `retention_hold` count |
| Sleep/wake | Monotonic/wall-clock gap observation plus manual operator procedure |
| System-time changes | Existing deterministic clock seams/local-calendar tests; no host clock mutation |

## Native build and environment

- App version: `0.3.2`
- Build type: optimized Tauri release build with `qualification` feature
- Executable identity: `resource-timeline.exe`
- Collection configuration at dry-run start: CPU, memory, disk, process; GPU was then enabled/disabled/re-enabled during GUI smoke
- System interval: 5 seconds
- Foreground poll interval: 1 second
- Windows: `25H2 build 26200`, x64
- CPU: `AMD Ryzen 7 9700X 8-Core Processor`, 16 logical processors
- Physical memory: approximately 32 GiB
- GPU: `NVIDIA GeForce RTX 5070 Ti`
- Battery: not present

## Harness

- Location: `tools/release-soak/`
- Evidence schema: `pr-07b-native-evidence/v1`
- Harness version: `0.1.0`
- Observation cadence: 30 seconds for soak/dry-run; 10 seconds for the short busy-recovery observation
- CPU basis: whole-machine percentage, not single-core normalized
- Raw format: JSONL; summary format: sanitized JSON
- Privacy boundary: no absolute paths, usernames, window titles, process command lines, application content, raw crash data, serials, or UUIDs
- Summary command: `release-soak summary --input <raw.jsonl> --events <events.jsonl> --output <summary.json>`

## Dry-run

Command shape:

```powershell
release-soak.exe run --pid <qualification-pid> --db <qualification-db> --duration 10m `
  --output dry-run.jsonl --events dry-run-events.jsonl --cadence 30s --warmup 60s `
  --app-version 0.3.2 --git-commit 89a9cc821771bd65b80c730bf922b37ff7777075 `
  --build-type release --executable-name resource-timeline.exe
```

The first dry-run exposed only a summary expected-sample off-by-one (`21` expected vs `20` observed with zero observation gaps). The summary calculation was fixed and the dry-run was rerun before proceeding.

Fixed-harness run: `2026-08-22T02:11:58.913Z` to `2026-08-22T02:21:58.913Z`, exactly 600 seconds, status `COMPLETE`.

- CPU post-warm-up: 18 samples; average `0.0532%`, P50 `0.0472%`, P95 `0.0856%`, max `0.0911%` whole-machine
- Memory working set: start `88.7 MB`, end `88.6 MB`, max `88.7 MB`, observed growth `-65,536 B`; linear slope `-0.37 MB/hour`; marked `decreasing_observed`, not declared a leak from this short run
- Private memory: start `45.7 MB`, end `45.7 MB`, growth `-53,248 B`
- Threads: `57 → 55`, max `58`
- Handles: `961 → 932`, max `968`
- SQLite main: `2,080,768 → 2,269,184 B`, growth `188,416 B`
- WAL: `4,243,632 → 4,243,632 B`, growth `0 B`
- SHM: `32,768 → 32,768 B`, growth `0 B`
- Total main+WAL+SHM: `6,357,168 → 6,545,584 B`, growth `188,416 B`
- Linear engineering estimate from this window: `1,130,496 B/hour`; projected 24h `27,131,904 B`; projected 7d `189,923,328 B`. This is not a 7-day soak.
- Reliability: expected `20`, observed `20`, observation gaps `0`, process restarts `0`, provider failure transitions `0`
- Wall-clock gap total: `7 ms`; no sleep/scheduler gap observed
- End committed frames: `225`; writer delay average `8.74 ms`, max `99 ms`
- Schema: user version `8` at start and end; retention holds `0` at start and end
- Collector and writer drop deltas: unavailable by design because the frozen external contract does not export them

The fixed dry-run was a harness PASS, not a release-gate PASS. Its starting working set reflects the already-used GUI session; the run itself was background-only and stable. The formal soak remains the gate measurement.

## Database-busy validation

A separate 60-second native observation ran with a 3-second `BEGIN IMMEDIATE` transaction against the isolated database, followed by `ROLLBACK`. No schema/data mutation, deletion, or corruption injection was used.

- Status: `COMPLETE`
- Expected/observed harness observations: `6 / 6`
- Observation gaps: `0`
- Process restarts/unexpected termination: `0 / false`
- Writer delay average/max: `155.35 / 1,986 ms`
- Committed frames at end: `20`
- SQLite sizes were unchanged during the short window
- Provider failures/recoveries: `0 / 0`

The writer remained recoverable after lock release; the elevated bounded writer delay is retained as evidence.

Local raw evidence is intentionally not committed. Representative SHA-256 values:

- fixed dry-run raw: `E0D232C2F2ED49F7165549C0D9A24A454C5425E0B1EACDA734DC0810964F8C1C`
- fixed dry-run summary: `CC33D3059F4A8EC67C2FC11E4A48989A76DA468F5002AE0A7C34792534799391`
- busy raw: `BA9EBF8696FBA311884DD38A70A092E35356B7850E2ED3FE8E0FF67FC767150B`
- busy summary: `8CCD9973C89043BA92CC060ACC8753266586A0E0E27C8DD0747DD151BFAA589C`

## Dynamic enable/disable

GUI smoke recorded operator timestamps for:

- GPU enable → provider state `gpu: Enabled`
- GPU disable → provider state `gpu: Disabled by you`
- GPU re-enable → provider state `gpu: Enabled`
- Baseline CPU disable → provider state `cpu: Disabled by you`
- Baseline CPU re-enable → GUI showed `CPU Enabled`, and the provider controls later showed `cpu: Enabled`

No duplicate lifecycle or growing retry loop was observed in the available native evidence. The final formal run must begin with the intended stable configuration.

## GUI smoke

Observed in the interactive Windows desktop session:

- App launch and Timeline load
- Usage and Settings load
- Collection status visible
- Pause then Resume
- Navigation across Timeline, Usage, Crashes, Settings
- Usage Day/7 days/30 days switching
- Date picker opened; only the current date had collected data in the fresh isolated database. An older date was attempted but remained unavailable. No synthetic historical data was created.
- Settings collection plan saved
- GPU and baseline category toggles saved and lifecycle status observed
- No reload was required and no obvious UI/runtime error was visible

Reopen continuity was verified: closing the management window and reopening from the same qualification artifact preserved the current Timeline date and collected GPU/system history. Tray “Stop collection and exit” bounded-shutdown smoke remains to be completed; the window-close path was verified as background continuation.

## Sleep/wake

Not forced by automation. Manual procedure for the formal run:

1. Start the 24-hour harness.
2. Record `sleep_start` before manually sleeping Windows.
3. Wake Windows after a controlled interval and record `sleep_end`.
4. Confirm wall-clock gap is separated from monotonic elapsed time, no false high-frequency drop is inferred, and providers/Timeline recover.

## System-time-change method

Deterministic coverage uses the existing injectable clock/local-calendar seams for local midnight and forward/backward wall-clock movement. Monotonic elapsed accounting is checked through those tests. Real host clock modification is **NOT RUN BY DESIGN**.

## 24-hour run

- Status: `READY`
- Exact command: start after final GUI reopen/continuity smoke with the freshly verified qualification PID, using `--duration 24h --cadence 30s --warmup 5m`
- Expected raw evidence: local temporary evidence directory only; do not commit raw JSONL
- Restart policy: any app/harness restart interrupts the run; do not concatenate runs
- Interruption policy: mark `INTERRUPTED`, preserve the partial evidence and SHA-256, then decide whether to rerun

## Verdict

`READY_FOR_24H`. The harness, isolated schema, native attachment, short dry-run, busy recovery, and most GUI/dynamic coverage are ready. There is no 24-hour PASS yet. The formal verdict remains pending and must use the actual 24-hour data; if the memory trend or any other budget exceeds the documented gate, report `BLOCKED` rather than changing the budget.
