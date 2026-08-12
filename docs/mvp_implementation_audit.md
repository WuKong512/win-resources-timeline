# MVP Implementation Audit

Audit date: 2026-07-13

## Baseline

- Platform: Windows 10/11 x64, current-user desktop process, no administrator requirement.
- Stack: Tauri 2, Rust 2021, React 18, TypeScript 5, Tailwind 3, SQLite through rusqlite 0.32.
- Package locks: `package-lock.json` and `src-tauri/Cargo.lock` are present.
- Lifecycle: the Rust collector and tray stay alive when the WebView closes. `--background` destroys the initial WebView. A second launch or tray left-click recreates/focuses it. Tray Quit flushes and exits.
- Database: `app_local_data_dir/resource-timeline.sqlite3`, WAL mode, `user_version = 4`, forward migration from v1/v2/v3, settings and collected data preserved during migration.
- Default sampling: foreground 1 second, system resources 5 seconds, idle threshold 5 minutes, raw system retention 7 days. The management window can select slower sampling and 1-30 day retention.
- Privacy: local only; no telemetry, account, network sync, titles, URLs, keystrokes, clipboard, pointer coordinates, or file-access history.

## Capability Matrix

| Capability | Status | Evidence or limitation |
|---|---|---|
| Collector survives WebView close | Verified | Windows smoke test; samples continued with the window closed. |
| `--background` without persistent WebView | Verified | Windows process/runtime test. |
| Tray open, pause/resume, quit/flush | Verified | Compiled and Windows lifecycle smoke tested; tray left-click open added in v0.1.0. |
| Single instance | Verified | Two launches retained one collector; second launch opens the window. |
| Foreground interval merging | Verified | Rust interval-engine tests and Windows app-switch test. |
| Active/idle | Verified | Engine tests and native idle source; configurable threshold. |
| Lock/unlock | Implemented, partially verified | Native session observer is present; basic Windows test completed, broader hardware coverage remains. |
| Suspend/resume | Implemented, partially verified | Native power notifications and gap reset are present; broader hardware coverage remains. |
| Long scheduler gaps | Verified | Rust test prevents attribution across long gaps. |
| Crash recovery | Verified | Open interval recovers to its last checkpoint; database integration test. |
| UTC milliseconds and range clipping | Verified | Database integration test and frontend time tests. |
| System CPU/memory/disk | Verified | Windows samples include PDH total disk throughput; missing metrics remain null. |
| Clickable app resource snapshots and per-app history | Implemented, compile/test verified | Each system tick stores foreground-tracked executable groups plus the top five by CPU, resident memory, and process I/O. Windows runtime overhead and multi-day database growth still require remeasurement. |
| SQLite WAL, migration, retention | Verified | Migration and settings round-trip tests; daily idempotent pruning. |
| Hidden apps | Verified | Hiding changes query/display behavior without deleting history. |
| Today, Timeline, Resources, Settings | Verified | Production frontend build and Windows smoke test. |
| Pause/resume and runtime frequency control | Verified | Commands, persisted settings, validation, and manager hot reload compile and test. |
| Background-only autostart | Verified | Autostart registers `--background`; management toggle is available. |
| Installer and portable executable | Verified | Release EXE, MSI, and NSIS bundles build on Windows x64. |
| Automated checks | Verified | 9 Rust tests, 4 frontend tests, TypeScript lint, production build, strict Clippy. |
| Low-resource baseline | Verified | Earlier 10-minute Release background run: 0.0029% average CPU, 46.46 MB average working set, 46.50 MB peak. The final repeat was waived by user direction. |

## Risks And Gaps

- P0: no known data-loss, duplicate-collector, privacy, or false-time-attribution defect remains in the audited paths.
- P1: installer upgrade-in-place, multiple Windows display scales, and a longer multi-day soak are not yet verified.
- P1: the frontend production bundle is about 1.23 MB before gzip and can be split later; it does not affect collector-only runtime because the WebView is closed in background mode.
- P2: long-term one-minute aggregation, week/month views, full-fidelity/high-resolution attribution, and anomaly events are deferred.
- The earlier low-resource baseline predates process snapshot collection and must not be treated as performance evidence for the schema-v3 build.
- The repository was supplied without Git metadata, so uncommitted-change history could not be audited.

## Current Vertical Slice

The selected slice was local operational control: make the management window discoverable and let the user pause collection, lower collection frequency, change raw retention, and control background-only autostart without weakening privacy or migration safety.

Exit criteria: v1 data survives v2 migration; invalid faster-than-baseline rates are rejected; settings persist and apply without restart; Windows bundles build; the portable launch path opens the management window; prior collector-only resource use remains under the reference targets.
