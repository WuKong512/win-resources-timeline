use super::{
    interval_engine::{IntervalAction, IntervalEngine, UsageEvent},
    system_metrics::{now_ms, SystemSampler},
};
use crate::{
    db::{writer, Database},
    models::{CollectionSettings, CollectorStatus, ComputerState, ForegroundApp},
    platform::{self, PlatformEvent},
};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub(crate) enum Control {
    Platform(PlatformEvent),
    SetPaused(bool),
    OpenWindow,
    UpdateSettings(CollectionSettings, Sender<Result<(), String>>),
    Clear(Sender<Result<(), String>>),
    Shutdown {
        deadline: Instant,
        reply: Sender<Result<(), String>>,
        done: Sender<()>,
    },
}

type ShutdownCompletion = (Sender<Result<(), String>>, Sender<()>, Result<(), String>);

#[derive(Clone)]
pub struct CollectorManager {
    tx: Sender<Control>,
    status: Arc<Mutex<CollectorStatus>>,
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const USAGE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const USAGE_CHECKPOINT_INTERVAL_MS: i64 = 15_000;
const DAILY_ROLLUP_PADDING_MS: i64 = 86_400_000;
const USAGE_WRITE_DEADLINE: Duration = Duration::from_secs(2);
const DAILY_ROLLUP_DEBOUNCE: Duration = Duration::from_secs(2);
const DAILY_ROLLUP_MIN_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
struct DailyRollupScheduler {
    dirty_range: Option<(i64, i64)>,
    due_at: Option<Instant>,
    last_run_at: Option<Instant>,
    run_count: u64,
}

impl DailyRollupScheduler {
    fn mark(&mut self, start_ms: i64, end_ms: i64) {
        if end_ms <= start_ms {
            return;
        }
        let range = (
            start_ms.saturating_sub(DAILY_ROLLUP_PADDING_MS),
            end_ms.saturating_add(DAILY_ROLLUP_PADDING_MS).max(
                start_ms
                    .saturating_sub(DAILY_ROLLUP_PADDING_MS)
                    .saturating_add(1),
            ),
        );
        self.dirty_range = Some(match self.dirty_range {
            Some((current_start, current_end)) => {
                (current_start.min(range.0), current_end.max(range.1))
            }
            None => range,
        });
        self.due_at = Some(Instant::now() + DAILY_ROLLUP_DEBOUNCE);
    }

    fn take_due(&mut self, now: Instant, force: bool) -> Option<(i64, i64)> {
        self.dirty_range.as_ref()?;
        if !force && self.due_at.is_some_and(|due_at| due_at > now) {
            return None;
        }
        if !force
            && self.last_run_at.is_some_and(|last_run| {
                now.saturating_duration_since(last_run) < DAILY_ROLLUP_MIN_INTERVAL
            })
        {
            return None;
        }
        let range = self.dirty_range.take();
        self.due_at = None;
        self.last_run_at = Some(now);
        self.run_count = self.run_count.saturating_add(1);
        range
    }

    fn requeue(&mut self, range: (i64, i64)) {
        self.dirty_range = Some(match self.dirty_range {
            Some((start, end)) => (start.min(range.0), end.max(range.1)),
            None => range,
        });
        self.due_at = Some(Instant::now() + DAILY_ROLLUP_DEBOUNCE);
    }

    #[cfg(test)]
    fn run_count(&self) -> u64 {
        self.run_count
    }
}

#[derive(Debug, Default)]
struct UsageApplyResult {
    dirty_range: Option<(i64, i64)>,
    retries: u32,
}

impl UsageApplyResult {
    fn merge(mut self, other: Self) -> Self {
        self.dirty_range = match (self.dirty_range, other.dirty_range) {
            (Some((start, end)), Some((other_start, other_end))) => {
                Some((start.min(other_start), end.max(other_end)))
            }
            (Some(range), None) | (None, Some(range)) => Some(range),
            (None, None) => None,
        };
        self.retries = self.retries.saturating_add(other.retries);
        self
    }
}

impl CollectorManager {
    pub fn start(db: Arc<Database>, app: tauri::AppHandle) -> Self {
        let (tx, rx) = bounded(32);
        let (critical_tx, critical_rx) = bounded(64);
        let status = Arc::new(Mutex::new(CollectorStatus {
            running: true,
            started_at_ms: Some(now_ms()),
            database_path: db.path().display().to_string(),
            ..CollectorStatus::default()
        }));
        let thread_status = status.clone();
        crate::platform::start_session_observer(tx.clone(), critical_tx);
        thread::spawn(move || run_collector(db, rx, critical_rx, thread_status, app));
        Self { tx, status }
    }

    pub fn status(&self, db_size: u64) -> CollectorStatus {
        let mut value = self
            .status
            .lock()
            .expect("collector status lock poisoned")
            .clone();
        value.database_size_bytes = db_size;
        value
    }

    pub fn set_paused(&self, paused: bool) -> Result<(), String> {
        self.tx
            .send(Control::SetPaused(paused))
            .map_err(|_| "collector stopped".to_string())
    }

    pub fn clear(&self) -> Result<(), String> {
        let (tx, rx) = bounded(1);
        self.tx
            .send(Control::Clear(tx))
            .map_err(|_| "collector stopped".to_string())?;
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| "collector did not acknowledge clear".to_string())?
    }

    pub fn update_settings(&self, settings: CollectionSettings) -> Result<(), String> {
        settings.validate()?;
        let (tx, rx) = bounded(1);
        self.tx
            .send(Control::UpdateSettings(settings, tx))
            .map_err(|_| "collector stopped".to_string())?;
        rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| "collector did not acknowledge settings update".to_string())?
    }

    pub fn shutdown(&self) -> Result<(), String> {
        if !self
            .status
            .lock()
            .map_err(|_| "collector status lock poisoned".to_string())?
            .running
        {
            return Ok(());
        }
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        let (tx, rx) = bounded(1);
        let (done_tx, done_rx) = bounded(1);
        self.tx
            .send_timeout(
                Control::Shutdown {
                    deadline,
                    reply: tx,
                    done: done_tx,
                },
                deadline.saturating_duration_since(Instant::now()),
            )
            .map_err(|_| "collector did not accept shutdown before deadline".to_string())?;
        let result = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| "collector did not acknowledge shutdown".to_string())?;
        done_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| "collector did not finish shutdown".to_string())?;
        result
    }
}

fn run_collector(
    db: Arc<Database>,
    rx: Receiver<Control>,
    critical_rx: Receiver<PlatformEvent>,
    status: Arc<Mutex<CollectorStatus>>,
    app: tauri::AppHandle,
) {
    let mut engine = IntervalEngine::default();
    engine.set_expected_heartbeat_ms(USAGE_HEARTBEAT_INTERVAL.as_millis() as u64);
    let mut persistence = writer::UsagePersistenceState::default();
    let mut rollup_scheduler = DailyRollupScheduler::default();
    let mut system = SystemSampler::new();
    let mut settings = db
        .with_writer(writer::collection_settings)
        .unwrap_or_default();
    let mut tracked_app_keys: HashSet<String> =
        db.with_writer(writer::tracked_app_keys).unwrap_or_default();
    let mut last_heartbeat = Instant::now() - USAGE_HEARTBEAT_INTERVAL;
    let mut last_observation_ms = None;
    let mut last_system =
        Instant::now() - Duration::from_millis(settings.system_sample_interval_ms);
    let mut last_system_flush = Instant::now();
    let mut frame_writer = writer::FrameWriter::new(64, 5);
    let mut last_prune = Instant::now() - Duration::from_secs(86_400);
    let mut shutdown_completion: Option<ShutdownCompletion> = None;
    let mut pending_critical_events = Vec::new();
    let mut observer_gap_pending = false;

    loop {
        let recovery = platform::take_observer_recovery();
        observer_gap_pending |= recovery.overflowed;
        pending_critical_events.extend(recovery.events);
        pending_critical_events.extend(critical_rx.try_iter());

        // An overflow means that one or more critical transitions may be missing. Establish an
        // explicit unknown boundary before applying any later recovery event, otherwise a
        // delivered Resume/Connect could bridge the lost Sleep/Disconnected interval.
        if observer_gap_pending {
            let result = apply_usage_event(
                &db,
                &mut engine,
                &mut persistence,
                &mut rollup_scheduler,
                UsageEvent::ObserverGap { at_ms: now_ms() },
                None,
            );
            record_usage_result(&status, &result);
            if let Err(error) = result {
                eprintln!("collector observer recovery failed: {error}");
                thread::sleep(Duration::from_millis(25));
                continue;
            }
            observer_gap_pending = false;
        }

        let mut critical_events = std::mem::take(&mut pending_critical_events);
        critical_events.sort_by_key(platform_event_timestamp);
        for event in critical_events {
            let result = handle_platform_event(
                &db,
                &mut engine,
                &mut persistence,
                &mut rollup_scheduler,
                &mut tracked_app_keys,
                &settings,
                event,
                None,
            );
            record_usage_result(&status, &result);
            if let Err(error) = result {
                eprintln!("collector critical platform event failed: {error}");
            }
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Control::Platform(event)) => {
                let result = handle_platform_event(
                    &db,
                    &mut engine,
                    &mut persistence,
                    &mut rollup_scheduler,
                    &mut tracked_app_keys,
                    &settings,
                    event,
                    None,
                );
                record_usage_result(&status, &result);
                if let Err(error) = result {
                    eprintln!("collector platform event failed: {error}");
                }
                if let Ok(mut value) = status.lock() {
                    value.last_foreground_sample_at_ms = Some(now_ms());
                }
            }
            Ok(Control::SetPaused(paused)) => {
                let now = now_ms();
                let result = if paused {
                    apply_usage_event(
                        &db,
                        &mut engine,
                        &mut persistence,
                        &mut rollup_scheduler,
                        UsageEvent::Pause { at_ms: now },
                        None,
                    )
                } else {
                    let foreground = resolve_current_foreground(&db, &mut tracked_app_keys, now);
                    apply_usage_event(
                        &db,
                        &mut engine,
                        &mut persistence,
                        &mut rollup_scheduler,
                        UsageEvent::ResumeCollection {
                            at_ms: now,
                            foreground_app_executable_id: foreground.ok().flatten(),
                            state: observed_computer_state(&settings),
                        },
                        None,
                    )
                };
                record_usage_result(&status, &result);
                if let Err(error) = result {
                    eprintln!("collector pause update failed: {error}");
                }
                if let Ok(mut value) = status.lock() {
                    value.paused = paused;
                    value.last_heartbeat_at_ms = Some(now);
                }
                if !paused {
                    last_observation_ms = Some(now);
                    system = SystemSampler::new();
                    last_system = Instant::now();
                }
            }
            Ok(Control::OpenWindow) => {
                crate::app_lifecycle::show_main_window(&app);
            }
            Ok(Control::UpdateSettings(next, reply)) => {
                let now = now_ms();
                let result = db
                    .with_writer(|conn| writer::save_collection_settings(conn, &next, now))
                    .map_err(|error| error.to_string());
                if result.is_ok() {
                    settings = next;
                    system = SystemSampler::new();
                    last_system = Instant::now();
                    last_prune = Instant::now() - Duration::from_secs(86_400);
                }
                let _ = reply.send(result);
            }
            Ok(Control::Clear(reply)) => {
                let now = now_ms();
                let action_result = apply_usage_event(
                    &db,
                    &mut engine,
                    &mut persistence,
                    &mut rollup_scheduler,
                    UsageEvent::Pause { at_ms: now },
                    None,
                );
                record_usage_result(&status, &action_result);
                frame_writer.discard_for_explicit_clear();
                let result = action_result.map(|_| ()).and_then(|_| {
                    db.with_writer(writer::clear_collected_data)
                        .map_err(|error| error.to_string())
                });
                if result.is_ok() {
                    engine = IntervalEngine::default();
                    engine.set_expected_heartbeat_ms(USAGE_HEARTBEAT_INTERVAL.as_millis() as u64);
                    persistence = writer::UsagePersistenceState::default();
                    rollup_scheduler = DailyRollupScheduler::default();
                    tracked_app_keys.clear();
                    last_observation_ms = None;
                }
                let _ = reply.send(result);
            }
            Ok(Control::Shutdown {
                deadline,
                reply,
                done,
            }) => {
                let shutdown_at = now_ms();
                let action_result = apply_usage_event(
                    &db,
                    &mut engine,
                    &mut persistence,
                    &mut rollup_scheduler,
                    UsageEvent::Shutdown { at_ms: shutdown_at },
                    Some(deadline),
                );
                record_usage_result(&status, &action_result);
                let rollup_result =
                    run_daily_rollup(&db, &mut rollup_scheduler, Some(deadline), true)
                        .map_err(|error| format!("final daily usage rollup failed: {error}"));
                if let Err(error) = &rollup_result {
                    record_usage_failure(&status, error);
                }
                let flush_result = flush_system_samples_until(&db, &mut frame_writer, deadline)
                    .map(|_| ())
                    .map_err(|error| format!("final frame flush failed: {error}"));
                sync_writer_status(&status, &frame_writer.health());
                let shutdown_kind =
                    if action_result.is_ok() && rollup_result.is_ok() && flush_result.is_ok() {
                        "clean"
                    } else {
                        "unknown"
                    };
                let session_result = if Instant::now() >= deadline {
                    Err("shutdown deadline expired before final collection session close".into())
                } else {
                    with_writer_deadline(&db, Some(deadline), |conn| {
                        writer::finish_collection_session(conn, shutdown_at, shutdown_kind)
                    })
                    .map_err(|error| format!("final collection session close failed: {error}"))
                };
                let result = [
                    action_result.err(),
                    rollup_result.err(),
                    flush_result.err(),
                    session_result.err(),
                ]
                .into_iter()
                .flatten()
                .next()
                .map_or(Ok(()), Err);
                if let Err(error) = &result {
                    eprintln!("collector shutdown failed: {error}");
                }
                shutdown_completion = Some((reply, done, result));
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }

        let now = now_ms();
        let paused = status.lock().map(|value| value.paused).unwrap_or(false);
        if !paused {
            let observer_dirty = platform::take_observer_dirty();
            if observer_dirty || last_heartbeat.elapsed() >= USAGE_HEARTBEAT_INTERVAL {
                last_heartbeat = Instant::now();
                let foreground = resolve_current_foreground(&db, &mut tracked_app_keys, now);
                let threshold_ms = settings.idle_threshold_seconds.saturating_mul(1_000);
                let state = platform::current_computer_state(threshold_ms)
                    .unwrap_or(ComputerState::Unknown);
                let previous = last_observation_ms.unwrap_or(now);
                let mut events = Vec::with_capacity(3);
                if matches!(state, ComputerState::Active | ComputerState::Idle) {
                    let idle_ms = platform::idle_for_ms().unwrap_or(0);
                    if idle_ms >= threshold_ms {
                        let crossed_at = now
                            .saturating_sub(i64::try_from(idle_ms).unwrap_or(i64::MAX))
                            .saturating_add(i64::try_from(threshold_ms).unwrap_or(i64::MAX))
                            .clamp(previous, now);
                        events.push(UsageEvent::IdleThresholdCrossed { at_ms: crossed_at });
                    } else {
                        let active_at = now
                            .saturating_sub(i64::try_from(idle_ms).unwrap_or(i64::MAX))
                            .clamp(previous, now);
                        events.push(UsageEvent::UserActive { at_ms: active_at });
                    }
                }
                events.push(UsageEvent::Resync {
                    at_ms: now,
                    foreground_app_executable_id: foreground.ok().flatten(),
                    state,
                });
                for event in events {
                    let result = apply_usage_event(
                        &db,
                        &mut engine,
                        &mut persistence,
                        &mut rollup_scheduler,
                        event,
                        None,
                    );
                    record_usage_result(&status, &result);
                    if let Err(error) = result {
                        eprintln!("collector heartbeat update failed: {error}");
                    }
                }
                last_observation_ms = Some(now);
                if let Ok(mut value) = status.lock() {
                    value.last_foreground_sample_at_ms = Some(now);
                }
            }

            if last_system.elapsed() >= Duration::from_millis(settings.system_sample_interval_ms) {
                last_system = Instant::now();
                if let Some(sample) = system.sample(now, &tracked_app_keys) {
                    frame_writer.enqueue(sample);
                }
            }
            if last_system_flush.elapsed() >= Duration::from_millis(250)
                && frame_writer.queue_depth() > 0
            {
                last_system_flush = Instant::now();
                let result = flush_system_samples(&db, &mut frame_writer, false);
                sync_writer_status(&status, &frame_writer.health());
                if let Err(error) = result {
                    eprintln!("collector frame flush failed: {error}");
                }
            }
        }
        if let Err(error) = run_daily_rollup(&db, &mut rollup_scheduler, None, false) {
            record_usage_failure(&status, &error.to_string());
            eprintln!("collector daily usage maintenance failed: {error}");
        }
        if last_prune.elapsed() >= Duration::from_secs(86_400) {
            last_prune = Instant::now();
            let cutoff = now - settings.system_sample_retention_days as i64 * 86_400_000;
            let _ = db.with_writer(|conn| writer::prune_system_samples(conn, cutoff));
        }
        if let Ok(mut value) = status.lock() {
            value.last_heartbeat_at_ms = Some(now);
        }
    }
    if let Ok(mut value) = status.lock() {
        value.running = false;
    }
    if let Some((reply, done, result)) = shutdown_completion {
        if reply.send(result).is_err() {
            eprintln!("collector shutdown acknowledgement receiver dropped");
        }
        if done.send(()).is_err() {
            eprintln!("collector shutdown completion receiver dropped");
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_platform_event(
    db: &Database,
    engine: &mut IntervalEngine,
    persistence: &mut writer::UsagePersistenceState,
    rollup_scheduler: &mut DailyRollupScheduler,
    tracked_app_keys: &mut HashSet<String>,
    settings: &CollectionSettings,
    event: PlatformEvent,
    deadline: Option<Instant>,
) -> Result<UsageApplyResult, String> {
    match event {
        PlatformEvent::ForegroundWindow { hwnd, at_ms } => {
            let usage_event = match platform::resolve_foreground_window(hwnd) {
                Some(app) => UsageEvent::Foreground {
                    app_executable_id: resolve_app(db, tracked_app_keys, &app, at_ms)?,
                    at_ms,
                },
                None => UsageEvent::ForegroundUnavailable { at_ms },
            };
            apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                usage_event,
                deadline,
            )
        }
        PlatformEvent::Locked { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            persistence,
            rollup_scheduler,
            UsageEvent::Locked { at_ms },
            deadline,
        ),
        PlatformEvent::Unlocked { at_ms } => {
            let first = apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                UsageEvent::Unlocked {
                    at_ms,
                    state: observed_computer_state(settings),
                },
                deadline,
            )?;
            let foreground = resolve_current_foreground(db, tracked_app_keys, at_ms);
            let second = apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                foreground
                    .ok()
                    .flatten()
                    .map(|app_executable_id| UsageEvent::Foreground {
                        app_executable_id,
                        at_ms,
                    })
                    .unwrap_or(UsageEvent::ForegroundUnavailable { at_ms }),
                deadline,
            )?;
            Ok(first.merge(second))
        }
        PlatformEvent::Suspended { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            persistence,
            rollup_scheduler,
            UsageEvent::Suspend { at_ms },
            deadline,
        ),
        PlatformEvent::Resumed { at_ms } => {
            let first = apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                UsageEvent::Resume {
                    at_ms,
                    state: observed_computer_state(settings),
                },
                deadline,
            )?;
            let foreground = resolve_current_foreground(db, tracked_app_keys, at_ms);
            let second = apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                foreground
                    .ok()
                    .flatten()
                    .map(|app_executable_id| UsageEvent::Foreground {
                        app_executable_id,
                        at_ms,
                    })
                    .unwrap_or(UsageEvent::ForegroundUnavailable { at_ms }),
                deadline,
            )?;
            Ok(first.merge(second))
        }
        PlatformEvent::Disconnected { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            persistence,
            rollup_scheduler,
            UsageEvent::Disconnected { at_ms },
            deadline,
        ),
        PlatformEvent::Connected { at_ms } => {
            let first = apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                UsageEvent::Connected {
                    at_ms,
                    state: observed_computer_state(settings),
                },
                deadline,
            )?;
            let foreground = resolve_current_foreground(db, tracked_app_keys, at_ms);
            let second = apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                foreground
                    .ok()
                    .flatten()
                    .map(|app_executable_id| UsageEvent::Foreground {
                        app_executable_id,
                        at_ms,
                    })
                    .unwrap_or(UsageEvent::ForegroundUnavailable { at_ms }),
                deadline,
            )?;
            Ok(first.merge(second))
        }
        PlatformEvent::WindowsShutdown { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            persistence,
            rollup_scheduler,
            UsageEvent::WindowsShutdown { at_ms },
            deadline,
        ),
    }
}

fn apply_usage_event(
    db: &Database,
    engine: &mut IntervalEngine,
    persistence: &mut writer::UsagePersistenceState,
    rollup_scheduler: &mut DailyRollupScheduler,
    event: UsageEvent,
    deadline: Option<Instant>,
) -> Result<UsageApplyResult, String> {
    apply_usage_event_with_state(db, engine, persistence, rollup_scheduler, event, deadline)
}

fn apply_usage_event_with_state(
    db: &Database,
    engine: &mut IntervalEngine,
    persistence: &mut writer::UsagePersistenceState,
    rollup_scheduler: &mut DailyRollupScheduler,
    event: UsageEvent,
    deadline: Option<Instant>,
) -> Result<UsageApplyResult, String> {
    let (next_engine, actions) = engine.preview(event);
    let result = apply_actions(db, persistence, actions, deadline)?;
    if let Some((start, end)) = result.dirty_range {
        rollup_scheduler.mark(start, end);
    }
    *engine = next_engine;
    Ok(result)
}

fn apply_actions(
    db: &Database,
    persistence: &mut writer::UsagePersistenceState,
    actions: Vec<IntervalAction>,
    deadline: Option<Instant>,
) -> Result<UsageApplyResult, String> {
    if actions.is_empty() {
        return Ok(UsageApplyResult::default());
    }
    let mut min_at = i64::MAX;
    let mut max_at = i64::MIN;
    let write_actions: Vec<_> = actions
        .iter()
        .map(|action| {
            let at_ms = action_timestamp(action);
            min_at = min_at.min(at_ms);
            max_at = max_at.max(at_ms);
            match action {
                IntervalAction::StartForeground {
                    app_executable_id,
                    at_ms,
                } => writer::UsageWriteAction::StartForeground {
                    app_executable_id: *app_executable_id,
                    at_ms: *at_ms,
                },
                IntervalAction::CheckpointForeground { at_ms } => {
                    writer::UsageWriteAction::CheckpointForeground { at_ms: *at_ms }
                }
                IntervalAction::CloseForeground { at_ms, reason } => {
                    writer::UsageWriteAction::CloseForeground {
                        at_ms: *at_ms,
                        reason,
                    }
                }
                IntervalAction::StartComputerState {
                    state,
                    at_ms,
                    source,
                    quality,
                } => writer::UsageWriteAction::StartComputerState {
                    state: *state,
                    at_ms: *at_ms,
                    source,
                    quality: *quality,
                },
                IntervalAction::CheckpointComputerState { at_ms } => {
                    writer::UsageWriteAction::CheckpointComputerState { at_ms: *at_ms }
                }
                IntervalAction::CloseComputerState { at_ms, reason } => {
                    writer::UsageWriteAction::CloseComputerState {
                        at_ms: *at_ms,
                        reason,
                    }
                }
                IntervalAction::MarkWindowsShutdown { at_ms } => {
                    writer::UsageWriteAction::MarkWindowsShutdown { at_ms: *at_ms }
                }
            }
        })
        .collect();
    let operation_deadline = deadline.unwrap_or_else(|| Instant::now() + USAGE_WRITE_DEADLINE);
    let (next_persistence, retries) = db
        .with_writer_until(operation_deadline, |conn| {
            writer::apply_usage_actions_with_retry(
                conn,
                &write_actions,
                *persistence,
                USAGE_CHECKPOINT_INTERVAL_MS,
                Some(operation_deadline),
            )
        })
        .map_err(|error| error.to_string())?;
    *persistence = next_persistence;
    Ok(UsageApplyResult {
        dirty_range: Some((min_at, max_at.saturating_add(1))),
        retries,
    })
}

fn action_timestamp(action: &IntervalAction) -> i64 {
    match action {
        IntervalAction::StartForeground { at_ms, .. }
        | IntervalAction::CheckpointForeground { at_ms }
        | IntervalAction::CloseForeground { at_ms, .. }
        | IntervalAction::StartComputerState { at_ms, .. }
        | IntervalAction::CheckpointComputerState { at_ms }
        | IntervalAction::CloseComputerState { at_ms, .. }
        | IntervalAction::MarkWindowsShutdown { at_ms } => *at_ms,
    }
}

fn resolve_app(
    db: &Database,
    tracked_app_keys: &mut HashSet<String>,
    app: &ForegroundApp,
    now: i64,
) -> Result<i64, String> {
    tracked_app_keys.insert(app.identity_key.clone());
    if let Some(path) = app.exe_path.as_deref() {
        tracked_app_keys.insert(format!("path:{}", normalize_path(path)));
    }
    db.with_writer(|conn| writer::resolve_foreground_app(conn, app, now))
        .map(|resolution| resolution.app_executable_id)
        .map_err(|error| error.to_string())
}

fn resolve_current_foreground(
    db: &Database,
    tracked_app_keys: &mut HashSet<String>,
    now: i64,
) -> Result<Option<i64>, String> {
    let Some(hwnd) = platform::current_foreground_window() else {
        return Ok(None);
    };
    let Some(app) = platform::resolve_foreground_window(hwnd) else {
        return Ok(None);
    };
    resolve_app(db, tracked_app_keys, &app, now).map(Some)
}

fn observed_computer_state(settings: &CollectionSettings) -> ComputerState {
    platform::current_computer_state(settings.idle_threshold_seconds.saturating_mul(1_000))
        .unwrap_or(ComputerState::Unknown)
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}

fn flush_system_samples(
    db: &Database,
    frame_writer: &mut writer::FrameWriter,
    drain: bool,
) -> rusqlite::Result<writer::WriterHealth> {
    db.with_writer(|conn| {
        if drain {
            frame_writer.flush_all(conn)?;
        } else {
            frame_writer.write_next(conn)?;
        }
        Ok(frame_writer.health())
    })
}

fn flush_system_samples_until(
    db: &Database,
    frame_writer: &mut writer::FrameWriter,
    deadline: Instant,
) -> rusqlite::Result<writer::WriterHealth> {
    db.with_writer_until(deadline, |conn| {
        frame_writer.flush_until(conn, deadline)?;
        Ok(frame_writer.health())
    })
}

fn with_writer_deadline<T>(
    db: &Database,
    deadline: Option<Instant>,
    operation: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T>,
) -> rusqlite::Result<T> {
    match deadline {
        Some(deadline) => db.with_writer_until(deadline, |conn| {
            writer::configure_connection_for_deadline(conn, deadline)?;
            operation(conn)
        }),
        None => db.with_writer(operation),
    }
}

fn sync_writer_status(status: &Arc<Mutex<CollectorStatus>>, health: &writer::WriterHealth) {
    if let Ok(mut value) = status.lock() {
        value.last_system_sample_at_ms = health.last_committed_timestamp_ms;
        value.dropped_system_samples = health.drop_count;
    }
}

fn record_usage_result(
    status: &Arc<Mutex<CollectorStatus>>,
    result: &Result<UsageApplyResult, String>,
) {
    match result {
        Ok(value) => {
            if let Ok(mut status) = status.lock() {
                status.usage_write_retries = status
                    .usage_write_retries
                    .saturating_add(u64::from(value.retries));
            }
        }
        Err(error) => record_usage_failure(status, error),
    }
}

fn record_usage_failure(status: &Arc<Mutex<CollectorStatus>>, error: &str) {
    if let Ok(mut status) = status.lock() {
        status.usage_write_failures = status.usage_write_failures.saturating_add(1);
        status.last_usage_write_error = Some(error.to_string());
    }
}

fn run_daily_rollup(
    db: &Database,
    scheduler: &mut DailyRollupScheduler,
    deadline: Option<Instant>,
    force: bool,
) -> rusqlite::Result<()> {
    let Some(range) = scheduler.take_due(Instant::now(), force) else {
        return Ok(());
    };
    let result = with_writer_deadline(db, deadline, |conn| {
        writer::rebuild_daily_usage(conn, range.0, range.1)
    });
    if result.is_err() {
        scheduler.requeue(range);
    }
    result
}

fn platform_event_timestamp(event: &PlatformEvent) -> i64 {
    match event {
        PlatformEvent::ForegroundWindow { at_ms, .. }
        | PlatformEvent::Locked { at_ms }
        | PlatformEvent::Unlocked { at_ms }
        | PlatformEvent::Suspended { at_ms }
        | PlatformEvent::Resumed { at_ms }
        | PlatformEvent::Disconnected { at_ms }
        | PlatformEvent::Connected { at_ms }
        | PlatformEvent::WindowsShutdown { at_ms } => *at_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daily_rollup_scheduler_debounces_repeated_heartbeats() {
        let mut scheduler = DailyRollupScheduler::default();
        scheduler.mark(10_000, 10_001);
        assert!(scheduler.take_due(Instant::now(), false).is_none());
        assert_eq!(scheduler.run_count(), 0);

        assert!(scheduler.take_due(Instant::now(), true).is_some());
        assert_eq!(scheduler.run_count(), 1);

        scheduler.mark(20_000, 20_001);
        assert!(scheduler.take_due(Instant::now(), false).is_none());
        assert_eq!(scheduler.run_count(), 1);
    }
}
