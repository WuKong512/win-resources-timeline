# PR-07B Multi-Session Native Release Qualification

## Verdict

**RESULT: `BLOCKED — TARGETED QUALIFICATION REQUIRED`**

The three independent native duration sessions satisfy the current multi-session
duration gate. The release gate remains blocked only by targeted scenario coverage
that is not present in the existing evidence: real Windows sleep/wake and a
complete product-process clean shutdown/reopen sequence. No additional 10-hour
soak is required for those scenarios.

This report does not concatenate Run A, Run B, or Run C. Each session is evaluated
independently and each raw evidence file remains local and unchanged.

## Contract and identity

The qualification contract used for this closeout is:

- at least 3 independent native sessions;
- every counted session is at least 10 hours;
- at least one counted session is at least 12 hours;
- aggregate valid native runtime is at least 32 hours.

The former continuous-24-hour target is not used as the current session-counting
contract. Run A and Run B retain their historical `INCOMPLETE` status only with
respect to that former target; they are independently eligible under the current
multi-session contract when their measured elapsed windows are evaluated below.

- Repository: `https://github.com/WuKong512/win-resources-timeline.git`
- `BASE_COMMIT`: `89a9cc821771bd65b80c730bf922b37ff7777075`
- `START_HEAD`: `89a9cc821771bd65b80c730bf922b37ff7777075`
- Test/qualification commit: `27a3a6f1c860ed902ce2aae5438accc5a089393c`
- Branch: `agent/pr-07b-native-release-qualification`
- Draft PR: [#15](https://github.com/WuKong512/win-resources-timeline/pull/15)
- `origin/main` at closeout: `89a9cc821771bd65b80c730bf922b37ff7777075`
- Working tree at audit: clean

### Native build identity

All three formal sessions use the same production-like optimized Tauri release
binary with the qualification feature enabled:

- executable name: `resource-timeline.exe`
- local artifact: `src-tauri/target/release/resource-timeline.exe`
- executable SHA-256: `AA93A64A936C22F4377CB03D0E356A18AA4CE99B7F225B828A7DF115C2FDF266`
- executable size: `12,944,384` bytes
- executable mtime UTC: `2026-08-22T02:43:13.6550064Z`
- app version: `0.3.2`
- build type: `releasequalification`
- feature proof: source qualification guard `Local\\ResourceTimelineQualificationGuard`,
  qualification identifier `com.local.resource-timeline.qualification`, and the
  corresponding marker were present in the validated binary

The observation harness was also unchanged across the three sessions:

- harness: `tools/release-soak/target/release/release-soak.exe`
- harness version: `0.1.0`
- harness SHA-256: `C324457DA123AEAB7F00E918E3715F29CDF2355306CF7E864429DDF93BF69BDB`
- harness mtime UTC: `2026-08-22T02:41:41.6216981Z`
- raw cadence: 30 seconds
- warm-up: 5 minutes; CPU distribution excludes warm-up, full memory series is retained
- CPU semantics: whole-machine percentage, not single-core-normalized percentage
- collection configuration: `memory`, `disk`, `process`, `gpu`, `cpu`; system interval 5 seconds;
  foreground interval 1 second; GPU provider enabled at session boundaries

The database was the isolated qualification data root identified by
`com.local.resource-timeline.qualification`, not the real user database or an
existing installation state. The evidence records the database filename only by
design. A post-run schema audit against the isolated qualification database
returned `user_version=8`, `quick_check=ok`, and zero foreign-key errors.

The machine was Windows 25H2 build 26200, x64, AMD Ryzen 7 9700X (16 logical
processors), NVIDIA GeForce RTX 5070 Ti, approximately 32 GiB RAM, and no battery.

## Evidence index and integrity

Raw evidence is retained in the local qualification temporary directory and is not
committed. The directory path, usernames, executable absolute paths, command lines,
and application content are intentionally omitted from this report.

| Run | Raw evidence file | Raw SHA-256 | Derived summary | Summary SHA-256 |
| --- | --- | --- | --- | --- |
| A | `formal-24h-27a3a6f1c860.jsonl` | `0EADB708AA7E43A578D3B04B24382BA0718D50567EDFF7F62F4FE8387C9F13F8` | `formal-24h-27a3a6f1c860-interrupted-summary.json` | `CF50AA97DFE472A12F1121270FC3E63EB7AFDCD33185DDFC156825F6D87D6403` |
| B | `formal-24h-rerun-20260823-27a3a6f1c860.jsonl` | `AE9F7C7EDF983B17B4A2EB9038FD26F4EC8489C207896A7F92BFBDE67039863B` | `formal-24h-rerun-20260823-27a3a6f1c860-summary.json` | `348CE975F810C318A08069CFFDAE766406BEC3407F6BDC8FB9E9F1396EE12702` |
| C | `formal-run-c-20260824-27a3a6f1c860.jsonl` | `EAF763CC5CE0A7D7BE0926D05816C884BDC326808EA804433C2DE237242A03B3` | `formal-run-c-20260824-27a3a6f1c860-derived-summary.json` | `5A60FB18AFE8B87A225EB76A9D1D46EF73986B84C3983762BDE9AA11C0B122CC` |

Run C operator-event evidence is separate from its raw time series:

- file: `formal-run-c-20260824-27a3a6f1c860-events.jsonl`
- SHA-256: `08C6A764920FE3F2A9FC63528234CD0F36237068258D366F0086E31755C877FF`
- recorded events: GUI smoke start, GPU disable, GPU enable, DB-busy start/end,
  and reopen start

All three raw files were readable after completion and their hashes were recomputed
at closeout. No raw Run A/B/C file was appended to, rewritten, or merged.

## Session gate

| Requirement | Run A | Run B | Run C | Aggregate |
| --- | ---: | ---: | ---: | ---: |
| Start UTC | `2026-08-22T02:57:49.495Z` | `2026-08-23T03:42:14.324Z` | `2026-08-24T01:13:29.899Z` | — |
| End UTC | `2026-08-22T15:35:49.495Z` | `2026-08-23T16:11:42.906Z` | `2026-08-24T11:13:29.899Z` | — |
| Local window (+08:00) | `10:57:49.495 → 23:35:49.495` | `11:42:14.324 → 00:11:42.906` | `09:13:29.899 → 19:13:29.899` | — |
| Valid elapsed runtime | `45,480,000 ms` / `12.633333 h` | `44,970,000 ms` / `12.491667 h` | `36,000,000 ms` / `10.000000 h` | `126,450,000 ms` / `35.125000 h` |
| At least 10 hours | PASS | PASS | PASS | 3 sessions |
| At least 12 hours | PASS | PASS | not required | PASS |
| Independent session | PASS: PID 29188, session 3 | PASS: PID 31612, session 5 | PASS: PID 33768, session 6 | PASS |
| Continuity evidence | 0 gaps, 0 restart markers | 0 gaps, 0 restart markers | 0 gaps, 0 restart markers | PASS |
| Evidence integrity | SHA verified; no footer in historical file | SHA verified; no footer in historical file | SHA verified; `COMPLETE`, duration reached | PASS |

Run A and Run B have no footer because they were stopped before the former
continuous-24-hour target. Their measured windows contain no observation gap or
restart marker and are counted independently, as required by the current contract.
Run C has a complete footer with `status=COMPLETE`, `terminationReason=duration_reached`,
and 1,200 observations.

The current duration gate therefore passes: 3 valid sessions, every counted session
at least 10 hours, two sessions at least 12 hours, aggregate 35.125 hours, and
longest session 12.633333 hours.

## Per-run resource analysis

### CPU

CPU is the process observation normalized to whole-machine percentage. The reported
distribution excludes the first 5-minute warm-up and does not represent a
single-core percentage.

| Run | Samples | Average | P50 | P95 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| A | 1,507 | `0.041531%` | `0.039063%` | `0.068360%` | `0.110677%` |
| B | 1,490 | `0.049311%` | `0.045573%` | `0.078125%` | `0.110677%` |
| C | 1,190 | `0.065049%` | `0.058593%` | `0.094402%` | `0.833327%` |

Run C's maximum is a bounded whole-machine observation; it did not produce a
sustained increase in average or P95.

### Memory

Working set and private memory retain the complete observation series. The
post-warm-up steady-state start is listed separately; the slope is a linear
engineering description of the observed window, not a leak diagnosis.

| Run | Working set start → end (B) | Steady start after 5m (B) | Max (B) | Delta (B) | Slope (B/h) | Private start → end (B) | Trend |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| A | `82,616,320 → 55,959,552` | `82,653,184` | `83,202,048` | `-26,656,768` | `-2,439,616` | `41,320,448 → 41,541,632` (`+221,184`) | working set decreasing; private nearly flat |
| B | `75,227,136 → 45,690,880` | `75,268,096` | `75,796,480` | `-29,536,256` | `-894,335` | `44,265,472 → 45,924,352` (`+1,658,880`) | working set decreasing; no sustained working-set growth |
| C | `85,708,800 → 53,780,480` | `86,470,656` | `87,339,008` | `-31,928,320` | `-3,572,596` | `45,936,640 → 41,230,336` (`-4,706,304`) | decreasing observed |

The post-warm-up working-set maxima were 83,152,896 B, 75,796,480 B, and
87,339,008 B for A/B/C respectively. The higher Run C startup reflects the
already-used isolated qualification session and GUI activity; it fell throughout
the run. No sustained unexplained memory growth is indicated by these sessions.

### Threads and handles

| Run | Threads start → end | Thread max | Handles start → end | Handle max | Interpretation |
| --- | ---: | ---: | ---: | ---: | --- |
| A | `47 → 44` | `47` | `828 → 848` | `926` | no sustained trend observed |
| B | `52 → 52` | `54` | `773 → 746` | `802` | stable/decreasing |
| C | `61 → 45` | `61` | `864 → 706` | `864` | decreasing after startup |

The Run A handle delta of +20 is a bounded run-to-run fluctuation, not a
monotonic cross-session increase. No thread or handle leak pattern is visible.

### Database footprint

The three components are reported separately. The qualification database is
persistent across sessions, so the increasing main-file starting point is not a
clean-database-per-run comparison.

| Run | Main DB start → end / delta (B) | WAL start → end / delta (B) | SHM start → end / delta (B) | Total start → end / delta (B) |
| --- | ---: | ---: | ---: | ---: |
| A | `3,129,344 → 17,367,040` / `+14,237,696` | `4,251,872 → 4,297,192` / `+45,320` | `32,768 → 32,768` / `0` | `7,413,984 → 21,697,000` / `+14,283,016` |
| B | `17,694,720 → 31,129,600` / `+13,434,880` | `4,297,192 → 4,317,792` / `+20,600` | `32,768 → 32,768` / `0` | `22,024,680 → 35,480,160` / `+13,455,480` |
| C | `31,170,560 → 42,606,592` / `+11,436,032` | `4,317,792 → 4,317,792` / `0` | `32,768 → 32,768` / `0` | `35,521,120 → 46,957,152` / `+11,436,032` |

After Run C, a read-only schema/integrity audit observed the isolated database at
`44,494,848 B` main, `4,321,912 B` WAL, and `32,768 B` SHM. The difference between
the last Run C observation and this post-run state is consistent with SQLite
checkpoint/maintenance behavior; it is not treated as a new time-series sample.

## Reliability and provider health

| Run | Expected / observed | Observation gaps | Wall-gap total / max | Drops | Writer delay avg / max | Provider failures / recoveries | Unexpected exit |
| --- | ---: | ---: | ---: | --- | ---: | ---: | --- |
| A | `1516 / 1517` | `0` | `341 ms / 30,011 ms` | not externally observable | `17.326 / 228 ms` | `0 / 0` | historical stop before 24h; no restart marker |
| B | `1499 / 1500` | `0` | `2 ms / 30,001 ms` | not externally observable | `12.334 / 44 ms` | `0 / 0` | historical stop before 24h; no restart marker |
| C | `1200 / 1200` | `0` | `305 ms / 30,008 ms` | not externally observable | `6.484 / 34 ms` | `0 / 0` | `false`; duration footer complete |

The frozen external production contract does not export collector drop counters,
writer drop counters, or writer queue depth. The harness records this as
`NOT RECORDED` rather than adding a production debug API. Committed-frame counts,
zero observation gaps, process-alive state, and writer delay are retained as the
available non-invasive evidence. No unrecovered provider failure loop was seen;
GPU persisted status was supported with 8 enabled metrics and zero failed or
unsupported metrics at session boundaries.

Schema was `8 → 8` in every formal session. The post-run isolated database check
passed quick-check and foreign-key validation. No user database was opened,
migrated, deleted, or corrupted.

## Scenario coverage

The short controlled DB-busy run used a 3-second `BEGIN IMMEDIATE` lock followed
by rollback against the isolated database, with no schema/data mutation. Its local
raw evidence SHA-256 is
`BA9EBF8696FBA311884DD38A70A092E35356B7850E2ED3FE8E0FF67FC767150B`.
Run C also recorded `db_busy_start` and `db_busy_end` at
`2026-08-24T01:25:07.744Z` and `2026-08-24T01:25:10.747Z`.

Dynamic provider evidence in Run C:

- GPU disable: `2026-08-24T01:14:51.818Z`
- GPU re-enable: `2026-08-24T01:21:15.268Z`
- provider was restored to supported/enabled state; no duplicate lifecycle or
  growing retry loop was observed

The local-midnight path is covered by deterministic local-calendar/clock-seam
tests, and Run B's measured wall-clock interval crossed local midnight. The host
system clock was not changed.

### Authoritative matrix

`PASS` in a run column means the evidence is present for that session. `N/A`
means the per-run condition is not required for that row. `PARTIAL` is not a
release PASS.

| Requirement | Run A | Run B | Run C | Final |
| --- | --- | --- | --- | --- |
| ≥10h | PASS | PASS | PASS | PASS |
| ≥12h | PASS | PASS | N/A | PASS |
| Normal interactive workload | N/A | N/A | PASS: GUI smoke | PASS |
| Long idle/background | PASS | PASS | PASS | PASS |
| Local-midnight rollover | N/A | PASS: interval crossed midnight + seam tests | N/A | PASS |
| Real sleep/wake | NOT COVERED | NOT COVERED | NOT COVERED; sleep count 0 | **BLOCKED** |
| Provider/category disable-enable | N/A | N/A | PASS: GPU disable/wait/enable | PASS |
| DB busy/recovery | N/A | N/A | PASS: bounded lock/recovery | PASS |
| Clean shutdown | NOT RECORDED | NOT RECORDED | NOT RECORDED | **BLOCKED** |
| Clean reopen | N/A | N/A | PARTIAL: reopen start only; prior window reopen was verified | **BLOCKED** |
| Provider health/recovery | PASS: healthy endpoints | PASS: healthy endpoints | PASS: healthy endpoints and toggle restore | PASS |
| No sustained memory leak | PASS | PASS | PASS | PASS |
| No sustained thread leak | PASS | PASS | PASS | PASS |
| No sustained handle leak | PASS | PASS | PASS | PASS |
| DB/WAL bounded behavior | PASS | PASS | PASS | PASS |
| Schema/integrity | PASS: 8→8 | PASS: 8→8 | PASS: 8→8; post-run check OK | PASS |
| GUI/native smoke | N/A | N/A | PARTIAL: app/UI smoke complete; process close/reopen pending | **BLOCKED** |

## GUI/native smoke

The existing interactive Windows desktop evidence covers application launch,
Timeline, Usage, Crashes, Settings, visible collection status, pause/resume,
1d/7d/30d views, historical-date selection against collected data, return to the
current date, settings save, GPU/category control, and no-reload navigation. The
GPU disable/wait/enable sequence was repeated during Run C. The earlier smoke
also verified management-window close/reopen continuity; that path keeps the
background process alive and is not equivalent to product-process shutdown.

No UI redesign or screenshots were added. The missing release evidence is the
system-tray/product-process `Stop collection and exit` path followed by a clean
reopen and continuity check.

## Cross-run stability

- Memory: working-set starts were 82.6 MB, 75.2 MB, and 85.7 MB; ends were 56.0
  MB, 45.7 MB, and 53.8 MB. There is no monotonic startup or steady-state rise.
  Private-memory deltas were +0.22 MB, +1.66 MB, and -4.71 MB.
- Threads: starts 47/52/61; ends 44/52/45; maxima 47/54/61. Run C's higher
  startup count fell rather than accumulating.
- Handles: starts 828/773/864; ends 848/746/706; maxima 926/802/864. There is
  no increasing baseline across restarts.
- DB/WAL: the persistent qualification main DB grows as samples are retained;
  per-session total growth declines from 14.28 MB to 13.46 MB to 11.44 MB.
  WAL remains approximately 4.25–4.32 MB and SHM remains 32,768 B. This is
  bounded/explainable SQLite behavior, not evidence of a persistent WAL runaway.
- Provider lifecycle: each session begins and ends with the same supported GPU
  provider state; Run C's explicit toggle restored the state. No duplicate
  lifecycle or retry loop was observed.
- Writer/queue/drop: no direct queue/drop counters are exposed. Writer delay and
  committed-frame continuity show recovery and no stuck writer, but this remains
  a measurement limitation.
- Shutdown/reopen: no product-process shutdown duration or complete process
  reopen marker was recorded. This is a targeted coverage blocker, not a claim of
  failure.

## Performance and storage

### Budget

For the tested AMD/NVIDIA default-background profile, the documented average CPU
target is `<0.5%` whole-machine CPU and steady working-set target is `<80 MB`.

- CPU budget: **PASS for the tested profile**. All averages were 0.0415–0.0650%
  and all P95 values were 0.0684–0.0944% whole-machine CPU.
- Memory budget: **PASS for observed steady state**. Working-set trends decreased
  in all three sessions and ended below 80 MB; the warm-up peak is retained above
  that line in Run C and is not misrepresented as a steady leak.
- Storage behavior: **PASS for bounded/explainable behavior**. Main DB growth is
  measurable, WAL/SHM are stable, and the observed rate is consistent across runs.

The performance document also calls for Intel/AMD CPU, multiple GPU vendors, and
battery-device measurements. This closeout machine supplies AMD CPU, NVIDIA GPU,
and no-battery coverage only; that cross-device matrix is a limitation of this
qualification evidence and is not substituted with an unsupported claim.

### Storage projection

These are linear engineering estimates from each measured window, not an actual
7-day soak. They include main DB + WAL + SHM.

| Run | Observed total growth rate (B/h) | Projected 24h (B) | Projected 7d (B) |
| --- | ---: | ---: | ---: |
| A | `1,130,581.74` | `27,133,961.79` | `189,937,732.56` |
| B | `1,077,156.50` | `25,851,756.10` | `180,962,292.73` |
| C | `1,143,603.20` | `27,446,476.80` | `192,125,337.60` |
| Median representative | `1,130,581.74` | `27,133,961.79` | `189,937,732.56` |
| Worst observed | `1,143,603.20` | `27,446,476.80` | `192,125,337.60` |

Retention holds were present during the formal sessions. The main file grew as
data was retained; WAL and SHM were approximately flat, and the observed summaries
did not show a changed slope. Checkpoint/cleanup can change a future slope, so the
projections are estimates rather than guaranteed future disk use.

## System-time and sleep/wake method

System-time validation uses injectable/deterministic clock seams and local-calendar
tests for local midnight and forward/backward wall-clock movement. Monotonic elapsed
time is used for duration accounting. Real host clock modification is **NOT RUN BY
DESIGN**.

The harness does not force sleep, reboot, or power-state changes. None of the three
formal raw files contains a sleep marker or a non-zero sleep interval count. The
missing targeted test must be user-triggered: start a short native observation,
record `sleep_start`, manually sleep Windows, wake after a controlled interval,
record `sleep_end`, and verify wall-clock gap separation, provider recovery, and no
false high-frequency drop conclusion.

## Targeted qualification required

Do not start another 10-hour soak by default. The smallest safe completion is:

1. **Real sleep/wake:** use the same validated qualification binary, isolated
   qualification database, and short harness observation; the user manually puts
   Windows to sleep and wakes it. Do not issue a power command from automation.
2. **Process clean shutdown/reopen:** use the product's normal tray `Stop
   collection and exit` action, record bounded exit, relaunch the same validated
   qualification artifact, record a completed reopen marker, then verify schema 8,
   SQLite integrity, provider state, and collection continuity. Do not use
   `Stop-Process` as the clean-shutdown evidence and do not touch the real user
   database or installed application state.

These targeted checks do not need to be 10 hours and must not be concatenated with
Run A/B/C. After both are evidenced, update this report's matrix and verdict.

## Limitations

- Collector drop counters, writer queue depth, and writer drop counters are not
  externally observable in the frozen production contract; no production API was
  added for qualification.
- Raw evidence intentionally omits absolute paths, usernames, window titles,
  command lines, document content, crash payloads, serials, and UUIDs.
- The three formal sessions use one AMD/NVIDIA non-battery Windows machine.
- The projected 24-hour/7-day storage figures are linear estimates, not a 7-day
  soak; retention, rollup, and checkpoint behavior can change the slope.
- Real host sleep/wake and real host clock changes are not automated. Clock-change
  coverage is deterministic; sleep/wake remains pending targeted manual evidence.

## Closeout status

- Duration gate: **PASS**
- Resource trend gate: **PASS for observed metrics**
- Schema/integrity: **PASS**
- Provider lifecycle and DB-busy recovery: **PASS**
- GUI smoke: **PARTIAL** pending process-level close/reopen
- Real sleep/wake: **NOT COVERED**
- Final PR-07B verdict: **BLOCKED — TARGETED QUALIFICATION REQUIRED**

No PR-08 work is included in this closeout.
