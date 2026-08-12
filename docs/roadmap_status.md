# Roadmap Status

Updated: 2026-07-18

## App-Centric Resource History

- Version 0.3.1 moves per-app resource history out of the System Resources view into a dedicated App Resources page.
- Resource apps are now grouped case-insensitively by process name, while retaining the latest executable path for display. Historical WindowsApps version paths such as three separate `ChatGPT.exe` installs therefore appear as one application node and query as one continuous history.
- The app page owns its date state and supports a single day, trailing 7/30-day windows, or an explicit inclusive start/end date. It shows captured-sample count, average/peak CPU, peak memory, peak combined process I/O, and the existing detailed chart.
- Today, Timeline, and System Resources no longer share one selected date. Today uses the union of foreground and system dates; Timeline uses foreground dates; System Resources uses system-sample dates. No-data and future dates are disabled on all three calendars.
- Windows autostart now normalizes the current portable executable to a quoted `"<path with spaces>" --background` Run command after both release startup and settings reconciliation.
- Verification passed TypeScript lint, 4 Vitest tests, 16 Rust tests, Rust formatting, the Vite production build, and an incremental no-bundle Tauri Release build with four Cargo jobs. The stable portable v0.3.1 was restarted in background mode; a 12-second WAL observation confirmed collection, and the quoted Windows Run value was verified. Setup and MSI remain at 0.2.0.

## UI Workbench Redesign

- Version 0.3.0 redesigns the management WebView without changing the collector, database schema, sampling semantics, or privacy boundary.
- The navigation is now a compact dark workbench sidebar; the content area uses a warm neutral canvas, quieter borders, tighter cards, tabular metrics, and one shared teal accent.
- Today emphasizes four sampled metrics, visualizes top-app share, and combines collector health with direct Timeline and Resources navigation.
- Timeline uses compact switches, clearer no-data controls, active-duration labels, quieter grid lines, and more legible active/idle bars.
- Resources now leads with the system chart and places the selected sample's app snapshot directly after it; per-app history moved to its own application-centric page. ECharts colors, axes, tooltips, zoom controls, and units are visually unified.
- Settings keeps the existing controls and sampling explanations while improving grouping, density, status visibility, and application-table scanning.
- Verification for this UI slice passed TypeScript lint, 4 Vitest tests, the production Vite build, and an incremental no-bundle Tauri Release build with four Cargo jobs. The stable `release/Resource Timeline Portable.exe` is now 0.3.0 and was restarted with `--background`; a fixed 12-second WAL observation confirmed that database writes resumed. Setup and MSI remain unchanged at 0.2.0.

## Background Startup And Tray Fix

- Portable version 0.2.2 includes schema v6 and persists `start_with_windows`, defaulting it to enabled for existing and new databases.
- Release startup reconciles the current-user Windows Run entry to the stable `Resource Timeline Portable.exe --background` path. The Run entry and Windows StartupApproved state were verified on the real machine.
- `--background` starts with no management window. Re-launching the same portable executable opens the existing collector's window through single-instance signaling.
- The tray now explicitly uses the packaged icon. Its menu separates **Hide window (keep collecting)** from **Stop collection and exit**.
- Real-machine verification closed the management window, confirmed the process remained alive with no main-window handle, and observed the foreground checkpoint advance by about 15 seconds over a 17-second wait.
- This iteration updates only the portable executable. Setup and MSI remain at 0.2.0 until the optimization phase is complete.
- Build caches may be retained temporarily during local development, but `src-tauri/target`, `node_modules`, `.vite`, and `dist` are reproducible and excluded from the source repository. Compilation remains limited to four jobs when rebuilding locally.
- Fixed the shared Switch thumb positioning by adding an explicit left origin, clipping, fixed shrink behavior, and proper switch accessibility state. This corrects the startup toggle and app-visibility toggles.

## Foreground Accuracy And Release Naming Fix

- Fixed the interval engine to scale scheduler-gap detection and coverage by the configured foreground polling interval. Previously a 5-second setting was incorrectly compared with a fixed 2.5-second threshold, recording only about one second out of each five.
- Added schema v5 to repair only the characteristic short `clock_gap` fragments followed by a regular configured polling observation.
- Idle visibility is no longer disabled or labeled unavailable at zero; it stays interactive, shows zero duration, explains the configured threshold, and refreshes current-day data automatically.
- Today now refreshes automatically while viewing the current date. The application version is displayed in the sidebar.
- Release artifact names are stable and versionless: `Resource Timeline Portable.exe`, `Resource Timeline Setup.exe`, and `Resource Timeline.msi`.
- Windows verification migrated the live database to schema v5, repaired 1,086 characteristic fragments, raised the current-day active total from roughly four minutes to roughly 23 minutes, and confirmed the new open interval continued across 5-second polls.

## App Resource Snapshot Slice

- Added schema v4 with explicit snapshot coverage markers on top of v3 app resource rows. Existing databases migrate in place and older system samples remain untouched.
- Each resource tick now aggregates processes sharing an executable and stores the union of the top five apps by CPU, resident memory, and process I/O (at most 15 rows per system sample).
- The Resources chart now accepts point selection, snaps to the nearest real sample, preserves the current zoom, and shows system totals plus the captured app details.
- Old samples without v3 details show an explicit unavailable state. Process I/O is labeled separately from physical-disk throughput.
- Snapshot rows are removed automatically when their parent system samples expire or the user clears collected data.
- Added per-app resource history. Apps that have appeared in the foreground are retained whenever running, even outside the resource Top 5.
- Resource tooltips now format disk throughput as readable KB/MB/GB per second; point selection shows whether app details exist and scrolls the detail card into view.
- Settings now explains 5/10/30/60-second accuracy and cost tradeoffs, including why per-app CPU can undercount short-lived work.
- Timeline now uses a data-aware calendar with unavailable dates disabled. Idle filtering is performed locally and shows the recorded idle duration.
- Verification for the latest portable slice: 14 Rust tests, 4 frontend tests, TypeScript lint, production frontend build, Rust formatting, and a no-bundle Release build passed. Full Clippy and installer builds are deferred during active optimization.

## Completed In The Current Slice

- Added a discoverable local management window entry through normal launch, second launch, tray menu, and tray left-click.
- Added pause/resume, background-only Windows autostart, configurable foreground/system sampling intervals, idle threshold, and raw sample retention.
- Added schema v2 with non-destructive `INSERT OR IGNORE` defaults.
- Added runtime settings reload without restarting the collector.
- Reset system-sampler baselines after pause/resume or frequency changes to avoid invalid cross-gap rates.
- Added migration preservation and settings round-trip tests.
- Confirmed the 9 GB workspace growth was reproducible build output, not application or user data.

## Verification Evidence

- Frontend: TypeScript lint passed; 4 Vitest tests passed; production build passed.
- Rust: formatting passed; 9 tests passed; strict Clippy passed.
- Windows baseline before this slice: close-window collection, single instance, real foreground/system/disk sampling, MSI, and NSIS verified.
- The earlier 10-minute background baseline remains historical evidence for the pre-snapshot build only: 0.0029% average CPU, 46.46 MB average working set, 46.50 MB peak. Schema-v3 process snapshots require a new measurement.

## Release Result

- Final Release executable, NSIS installer, and MSI installer are preserved in `release/`.
- Release builds now use the Windows GUI subsystem, removing the extra console window.
- English / Simplified Chinese selection is persisted locally and was verified across a close/relaunch cycle.
- Portable launch, second-launch reopen, settings persistence, and pause/resume were repeated successfully. Tray left-click is compiled but was not separately automated in the final pass.
- The project was reduced from about 9.47 GB to 18.2 MB by removing only reproducible `target`, `node_modules`, and `dist` directories.
- The current-user SQLite database remained intact at 112 KB.
- MSI ICE validation could not access the Windows Installer service on this machine. The MSI was linked from the generated WiX objects with ICE validation suppressed; NSIS built normally and is the recommended installer for routine use.
- This handoff revision passed frontend lint/build and 4 frontend tests, 9 Rust tests, Rust fmt, and strict Clippy. The final Release executable was opened on Windows and checked for the absence of a console window and for Simplified Chinese persistence.

## Deferred

- Seven-day dogfood observation remains waiting for real elapsed use.
- One-minute aggregates and week/month history are the recommended next product slice after the operational-control release is observed in daily use.
- Process attribution, ETW, temperature, networking, and anomaly detection remain optional later phases.

## Next Recommended Stage

Run the schema-v3 build in normal daily use and record process-snapshot CPU cost, database growth, WAL behavior, unknown-app ratio, missing intervals, autostart behavior, and collector health. Do not increase process sampling frequency or retained detail until that evidence is stable.
