# Resource Timeline

Resource Timeline is a local-only Windows 10/11 x64 desktop MVP that records which executable owns the foreground window, whether the current user session is active or idle, sampled system CPU/memory/disk totals, and compact top-app resource snapshots. The React WebView is disposable: closing it leaves the Rust collector and tray process running.

CPU, memory, and disk curves are system totals. A chart point can be selected to inspect the separately sampled leading apps at that moment; the app rows are not inferred from the foreground timeline and do not necessarily add up to the system total.

## Current capabilities

- Polls the foreground window at the configured 1/5/10-second interval using `GetForegroundWindow`, `GetWindowThreadProcessId`, `OpenProcess`, and `QueryFullProcessImageNameW`.
- Never calls `GetWindowText` and does not store window titles or title hashes.
- Uses executable path plus process name as the persistent app identity; PID is never a persistent identity.
- Splits intervals on app and active/idle transitions, allows a short no-foreground grace period, and leaves gaps across delayed scheduler ticks.
- Stores UTC Unix epoch milliseconds and queries half-open local-day ranges supplied by the frontend.
- Uses SQLite WAL, schema migrations through `PRAGMA user_version` (currently v8), one open interval with checkpoints, and recovery to `last_seen_time_ms` after an unclean exit. The v5 migration repairs the regular false gaps created by older builds when foreground polling was slower than one second; v6 persists the default-on Windows startup preference; v7/v8 provide the frame, process, provider, and per-device GPU storage contracts.
- Samples CPU and memory every 5 seconds after a warm-up refresh and reads system-wide disk throughput from Windows PDH `_Total` counters. Missing metrics remain `NULL` and chart as gaps.
- At each retained system sample, selects raw process instances before logical-app aggregation: the union of the top five by CPU, working-set memory, and I/O plus the resolvable foreground process. PID, creation time, executable identity, selection reasons, NULLs, and quality masks are retained in `process_sample`; bounded 1-minute, 1-hour, and daily app-resource rollups provide long-term totals.
- Provides a per-app resource-history selector for sampled CPU, resident memory, and process I/O. Disk tooltips and axes use human-readable byte units.
- Receives lock/unlock, suspend/resume, and shutdown notifications through a hidden native Win32 window without creating a WebView.
- Runs collection through a bounded control channel and exposes health, pause/resume, clear-data, app filtering, and autostart commands.
- Asynchronously reads normalized Windows System Event Log facts, classifies supported crash/restart boundaries, creates idempotent cases and retention holds, and exposes objective evidence summaries/status DTOs. Crash evidence does not infer cause, severity, blame, probability, or remediation.
- Supports a tray menu, `--background` startup, on-demand WebView creation, and single-instance focus behavior.
- Provides Today, Timeline, Resources, and Settings views with loading, error, and empty states.
- Timeline uses a data-aware calendar: dates without foreground samples are dimmed and cannot be selected. Idle visibility remains available at zero and shows the recorded duration; Today and the current-day timeline refresh automatically.
- Supports persistent English and Simplified Chinese UI selection in Settings.
- Uses the Windows GUI subsystem for Release builds, so normal launches do not open a console window.

## Prerequisites

- Windows 10 or 11 x64
- Rust stable using the MSVC toolchain
- Visual Studio Build Tools with Desktop development with C++ and a Windows SDK
- Node.js 20 or newer and npm
- WebView2 Runtime

Install Rust with the official rustup installer, then select MSVC:

```powershell
rustup default stable-x86_64-pc-windows-msvc
```

## Develop, test, and build

```powershell
npm install
npm run lint
npm run test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run tauri:dev
npm run tauri:build
```

Generated dependencies and build outputs are intentionally excluded from source control: `node_modules/`, `.vite/`, `dist/`, and `src-tauri/target/`. The installer and portable binaries under `release/` are also ignored and should be attached to GitHub Releases; `release/README.zh-CN.md` remains part of the source repository.

Normal startup creates the collector, tray, database writer, and management WebView. Startup with `--background` creates only the collector and tray, without showing a management window. Closing or hiding the main window does not exit the process. A second launch focuses or recreates the window. Only **Stop collection and exit** in the tray menu terminates the collector.

## Running the app and opening management

MSI installation is not required. The portable Release executable opens the management window directly and stores data in the same current-user local-data location. Open or reopen management in any of these ways:

- launch the portable executable or the installed Start menu entry;
- launch Resource Timeline a second time while the collector is already running;
- left-click the tray icon or choose **Open Resource Timeline** from its menu.

Autostart is enabled by default and uses `--background`, so Windows starts only the collector and tray. Each Release launch reconciles the current-user Run entry with the current stable executable path, allowing the versionless portable file to be replaced in place.

The Settings page can pause/resume collection, enable background-only autostart, and change foreground polling, system sampling, idle threshold, and raw-system-sample retention. It explains the accuracy, transient-process coverage, overhead, and storage tradeoffs for each sampling interval. Available choices never run faster than the original 1-second foreground and 5-second system defaults.

## Local data and privacy

The database is stored under Tauri's current-user `app_local_data_dir` as `resource-timeline.sqlite3`. The exact path and current size are shown in Settings. Raw system samples are retained for 7 days by default and can be configured from 1 to 30 days; foreground intervals and app identities remain until the user clears collected data. Settings survive a clear operation.

No analytics, telemetry, crash upload, account, or network sync is included. The app does not collect keyboard content, pointer coordinates, clipboard data, browser URLs, file access paths, or window title text. Executable paths stay local but may include the Windows user directory name.

## Sampling semantics and limitations

- Foreground polling resolution follows the configured interval, so faster switches may be missed. Gap detection scales with that interval instead of assuming a fixed one-second poll.
- The idle threshold defaults to 300 seconds. Idle foreground intervals remain available and are visually muted.
- UWP, protected, elevated, and system processes may resolve as a shared `unresolved` identity.
- Sleep, lock, process failure, and delayed scheduling appear as gaps; the collector never fills missing time from the previous app. Scheduler-gap detection remains a fallback if native notifications are missed.
- CPU and memory peaks are peaks among 5-second samples, not continuous maxima.
- App resource details exist only for samples captured by schema-v3-or-newer builds. Older system samples remain visible but have no app snapshot.
- Raw process instances are selected independently by CPU, memory, and I/O with a fixed Top-N of five per dimension; repeated selection is one row with an OR-ed reason mask. Logical-app aggregation happens only in rollups/queries. Missing process metrics stay NULL and are not converted to measured zero.
- On Windows, per-process I/O includes all read/write I/O reported for that process, not only physical-disk traffic. The system disk curve remains the PDH physical-disk total.
- The collector's own small resource cost is included in system totals.
- Disk throughput uses `\\PhysicalDisk(_Total)\\Disk Read Bytes/sec` and `Disk Write Bytes/sec`. If PDH is unavailable or not yet warmed up, values are stored as `NULL` instead of using process totals or fabricating throughput.

## Windows manual acceptance

1. Run `npm run tauri:dev`, wait 30 seconds, and confirm foreground intervals plus CPU/memory samples appear.
2. Switch among Notepad, File Explorer, and a browser. Confirm interval boundaries are normally within 1-2 seconds.
3. Close the main window, wait 30 seconds, launch the shortcut again, and confirm samples continued while the WebView was absent.
4. Launch the app twice and confirm only one collector process and one database writer source exist.
5. Temporarily lower the idle threshold in the database, become idle, then provide input and confirm active/idle splitting.
6. Lock Windows for two minutes, unlock, and confirm locked time is not assigned to the previous app.
7. Suspend and resume Windows and confirm the resource chart and foreground timeline contain a gap.
8. Force-kill the process, restart, and confirm the previous open interval ends at `last_seen_time_ms` with `end_reason = 'recovery'`.
9. Create an interval across local midnight and confirm each date shows only its clipped portion.
10. Hide an app, verify it disappears by default, enable Show hidden, and verify the history returns.
11. Inspect the SQLite schema and confirm there is no window-title column or title data.
12. Pause collection, wait, resume, and confirm the pause is not backfilled.
13. Clear collected data and confirm new intervals can be recorded without dangling foreign keys.

## Roadmap

- Optional ETW enhancement mode for higher-resolution resource diagnostics
- PR-05 backend process rollups and crash evidence are implemented; the later PR-06 UI presentation and user-facing retention settings remain deferred.
- Longer multi-hour performance soak tests and installer upgrade coverage
- Optional higher-resolution attribution, temperature, and anomaly events as separate future modes

## Measured Windows performance

Measured on the development Windows 11 x64 machine using the Release executable with `--background`, no Tauri WebView window, and 5-second process samples for 10 minutes (shortened from the original 30-minute acceptance run by user direction):

- Duration: 601.4 seconds, 121 samples, 16 logical processors
- Average process CPU: 0.0029%
- Average working set: 46.46 MB
- Peak working set: 46.50 MB
- Thread count range: 35-37
- CPU time consumed during the run: 0.281 seconds

The measured run met the reference targets of average CPU below 0.5% and background working set below 50 MB. Memory and thread counts showed no upward trend during the run.
