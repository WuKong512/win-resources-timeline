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

impl CollectorManager {
    pub fn start(db: Arc<Database>, app: tauri::AppHandle) -> Self {
        let (tx, rx) = bounded(32);
        let status = Arc::new(Mutex::new(CollectorStatus {
            running: true,
            started_at_ms: Some(now_ms()),
            database_path: db.path().display().to_string(),
            ..CollectorStatus::default()
        }));
        let thread_status = status.clone();
        crate::platform::start_session_observer(tx.clone());
        thread::spawn(move || run_collector(db, rx, thread_status, app));
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
    status: Arc<Mutex<CollectorStatus>>,
    app: tauri::AppHandle,
) {
    let mut engine = IntervalEngine::default();
    engine.set_expected_heartbeat_ms(USAGE_HEARTBEAT_INTERVAL.as_millis() as u64);
    let mut open_foreground_id = None;
    let mut open_computer_state_id = None;
    let mut last_foreground_checkpoint_ms = 0;
    let mut last_computer_checkpoint_ms = 0;
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

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Control::Platform(event)) => {
                if let Err(error) = handle_platform_event(
                    &db,
                    &mut engine,
                    &mut open_foreground_id,
                    &mut open_computer_state_id,
                    &mut last_foreground_checkpoint_ms,
                    &mut last_computer_checkpoint_ms,
                    &mut tracked_app_keys,
                    &settings,
                    event,
                    None,
                ) {
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
                        &mut open_foreground_id,
                        &mut open_computer_state_id,
                        &mut last_foreground_checkpoint_ms,
                        &mut last_computer_checkpoint_ms,
                        UsageEvent::Pause { at_ms: now },
                        None,
                    )
                } else {
                    let foreground = resolve_current_foreground(&db, &mut tracked_app_keys, now);
                    apply_usage_event(
                        &db,
                        &mut engine,
                        &mut open_foreground_id,
                        &mut open_computer_state_id,
                        &mut last_foreground_checkpoint_ms,
                        &mut last_computer_checkpoint_ms,
                        UsageEvent::ResumeCollection {
                            at_ms: now,
                            foreground_app_executable_id: foreground.ok().flatten(),
                            state: observed_computer_state(&settings),
                        },
                        None,
                    )
                };
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
                    &mut open_foreground_id,
                    &mut open_computer_state_id,
                    &mut last_foreground_checkpoint_ms,
                    &mut last_computer_checkpoint_ms,
                    UsageEvent::Pause { at_ms: now },
                    None,
                );
                frame_writer.discard_for_explicit_clear();
                let result = action_result
                    .and_then(|_| {
                        db.with_writer(writer::clear_collected_data)
                            .map_err(|error| error.to_string())
                    })
                    .map_err(|error| error.to_string());
                engine = IntervalEngine::default();
                engine.set_expected_heartbeat_ms(USAGE_HEARTBEAT_INTERVAL.as_millis() as u64);
                open_foreground_id = None;
                open_computer_state_id = None;
                tracked_app_keys.clear();
                last_observation_ms = None;
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
                    &mut open_foreground_id,
                    &mut open_computer_state_id,
                    &mut last_foreground_checkpoint_ms,
                    &mut last_computer_checkpoint_ms,
                    UsageEvent::Shutdown { at_ms: shutdown_at },
                    Some(deadline),
                );
                let flush_result = flush_system_samples_until(&db, &mut frame_writer, deadline)
                    .map(|_| ())
                    .map_err(|error| format!("final frame flush failed: {error}"));
                sync_writer_status(&status, &frame_writer.health());
                let shutdown_kind = if action_result.is_ok() && flush_result.is_ok() {
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
                let result = [action_result, flush_result, session_result]
                    .into_iter()
                    .find_map(Result::err)
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
                let idle_ms = platform::idle_for_ms().unwrap_or(0);
                let threshold_ms = settings.idle_threshold_seconds.saturating_mul(1_000);
                let previous = last_observation_ms.unwrap_or(now);
                let mut events = Vec::with_capacity(2);
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
                events.push(UsageEvent::Heartbeat {
                    at_ms: now,
                    foreground_app_executable_id: foreground.ok().flatten(),
                    state: if idle_ms >= threshold_ms {
                        ComputerState::Idle
                    } else {
                        ComputerState::Active
                    },
                });
                for event in events {
                    if let Err(error) = apply_usage_event(
                        &db,
                        &mut engine,
                        &mut open_foreground_id,
                        &mut open_computer_state_id,
                        &mut last_foreground_checkpoint_ms,
                        &mut last_computer_checkpoint_ms,
                        event,
                        None,
                    ) {
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
    open_foreground_id: &mut Option<i64>,
    open_computer_state_id: &mut Option<i64>,
    last_foreground_checkpoint_ms: &mut i64,
    last_computer_checkpoint_ms: &mut i64,
    tracked_app_keys: &mut HashSet<String>,
    settings: &CollectionSettings,
    event: PlatformEvent,
    deadline: Option<Instant>,
) -> Result<(), String> {
    match event {
        PlatformEvent::ForegroundWindow { hwnd, at_ms } => {
            let foreground = platform::resolve_foreground_window(hwnd);
            let usage_event = match foreground {
                Some(app) => UsageEvent::Foreground {
                    app_executable_id: resolve_app(db, tracked_app_keys, &app, at_ms)?,
                    at_ms,
                },
                None => UsageEvent::ForegroundUnavailable { at_ms },
            };
            apply_usage_event_with_state(
                db,
                engine,
                open_foreground_id,
                open_computer_state_id,
                last_foreground_checkpoint_ms,
                last_computer_checkpoint_ms,
                usage_event,
                deadline,
            )
        }
        PlatformEvent::Locked { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            open_foreground_id,
            open_computer_state_id,
            last_foreground_checkpoint_ms,
            last_computer_checkpoint_ms,
            UsageEvent::Locked { at_ms },
            deadline,
        ),
        PlatformEvent::Unlocked { at_ms } => {
            apply_usage_event_with_state(
                db,
                engine,
                open_foreground_id,
                open_computer_state_id,
                last_foreground_checkpoint_ms,
                last_computer_checkpoint_ms,
                UsageEvent::Unlocked {
                    at_ms,
                    state: observed_computer_state(settings),
                },
                deadline,
            )?;
            let foreground = resolve_current_foreground(db, tracked_app_keys, at_ms);
            apply_usage_event_with_state(
                db,
                engine,
                open_foreground_id,
                open_computer_state_id,
                last_foreground_checkpoint_ms,
                last_computer_checkpoint_ms,
                foreground
                    .ok()
                    .flatten()
                    .map(|app_executable_id| UsageEvent::Foreground {
                        app_executable_id,
                        at_ms,
                    })
                    .unwrap_or(UsageEvent::ForegroundUnavailable { at_ms }),
                deadline,
            )
        }
        PlatformEvent::Suspended { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            open_foreground_id,
            open_computer_state_id,
            last_foreground_checkpoint_ms,
            last_computer_checkpoint_ms,
            UsageEvent::Suspend { at_ms },
            deadline,
        ),
        PlatformEvent::Resumed { at_ms } => {
            apply_usage_event_with_state(
                db,
                engine,
                open_foreground_id,
                open_computer_state_id,
                last_foreground_checkpoint_ms,
                last_computer_checkpoint_ms,
                UsageEvent::Resume {
                    at_ms,
                    state: observed_computer_state(settings),
                },
                deadline,
            )?;
            let foreground = resolve_current_foreground(db, tracked_app_keys, at_ms);
            apply_usage_event_with_state(
                db,
                engine,
                open_foreground_id,
                open_computer_state_id,
                last_foreground_checkpoint_ms,
                last_computer_checkpoint_ms,
                foreground
                    .ok()
                    .flatten()
                    .map(|app_executable_id| UsageEvent::Foreground {
                        app_executable_id,
                        at_ms,
                    })
                    .unwrap_or(UsageEvent::ForegroundUnavailable { at_ms }),
                deadline,
            )
        }
        PlatformEvent::Disconnected { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            open_foreground_id,
            open_computer_state_id,
            last_foreground_checkpoint_ms,
            last_computer_checkpoint_ms,
            UsageEvent::Disconnected { at_ms },
            deadline,
        ),
        PlatformEvent::Connected { at_ms } => {
            apply_usage_event_with_state(
                db,
                engine,
                open_foreground_id,
                open_computer_state_id,
                last_foreground_checkpoint_ms,
                last_computer_checkpoint_ms,
                UsageEvent::Connected {
                    at_ms,
                    state: observed_computer_state(settings),
                },
                deadline,
            )?;
            let foreground = resolve_current_foreground(db, tracked_app_keys, at_ms);
            apply_usage_event_with_state(
                db,
                engine,
                open_foreground_id,
                open_computer_state_id,
                last_foreground_checkpoint_ms,
                last_computer_checkpoint_ms,
                foreground
                    .ok()
                    .flatten()
                    .map(|app_executable_id| UsageEvent::Foreground {
                        app_executable_id,
                        at_ms,
                    })
                    .unwrap_or(UsageEvent::ForegroundUnavailable { at_ms }),
                deadline,
            )
        }
        PlatformEvent::WindowsShutdown { at_ms } => apply_usage_event_with_state(
            db,
            engine,
            open_foreground_id,
            open_computer_state_id,
            last_foreground_checkpoint_ms,
            last_computer_checkpoint_ms,
            UsageEvent::WindowsShutdown { at_ms },
            deadline,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_usage_event(
    db: &Database,
    engine: &mut IntervalEngine,
    open_foreground_id: &mut Option<i64>,
    open_computer_state_id: &mut Option<i64>,
    last_foreground_checkpoint_ms: &mut i64,
    last_computer_checkpoint_ms: &mut i64,
    event: UsageEvent,
    deadline: Option<Instant>,
) -> Result<(), String> {
    apply_usage_event_with_state(
        db,
        engine,
        open_foreground_id,
        open_computer_state_id,
        last_foreground_checkpoint_ms,
        last_computer_checkpoint_ms,
        event,
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
fn apply_usage_event_with_state(
    db: &Database,
    engine: &mut IntervalEngine,
    open_foreground_id: &mut Option<i64>,
    open_computer_state_id: &mut Option<i64>,
    last_foreground_checkpoint_ms: &mut i64,
    last_computer_checkpoint_ms: &mut i64,
    event: UsageEvent,
    deadline: Option<Instant>,
) -> Result<(), String> {
    apply_actions(
        db,
        open_foreground_id,
        open_computer_state_id,
        last_foreground_checkpoint_ms,
        last_computer_checkpoint_ms,
        engine.handle(event),
        deadline,
    )
}

fn apply_actions(
    db: &Database,
    open_foreground_id: &mut Option<i64>,
    open_computer_state_id: &mut Option<i64>,
    last_foreground_checkpoint_ms: &mut i64,
    last_computer_checkpoint_ms: &mut i64,
    actions: Vec<IntervalAction>,
    deadline: Option<Instant>,
) -> Result<(), String> {
    let mut first_error = None;
    let mut min_at = None;
    let mut max_at = None;
    for action in actions {
        let at_ms = action_timestamp(&action);
        min_at = Some(min_at.map_or(at_ms, |value: i64| value.min(at_ms)));
        max_at = Some(max_at.map_or(at_ms, |value: i64| value.max(at_ms)));
        match action {
            IntervalAction::StartForeground {
                app_executable_id,
                at_ms,
            } => {
                match with_writer_deadline(db, deadline, |conn| {
                    writer::begin_foreground_interval(conn, app_executable_id, at_ms)
                }) {
                    Ok(interval_id) => *open_foreground_id = Some(interval_id),
                    Err(error) => {
                        first_error.get_or_insert_with(|| error.to_string());
                    }
                }
            }
            IntervalAction::CheckpointForeground { at_ms } => {
                if at_ms.saturating_sub(*last_foreground_checkpoint_ms)
                    >= USAGE_CHECKPOINT_INTERVAL_MS
                {
                    if let Some(interval_id) = *open_foreground_id {
                        if let Err(error) = with_writer_deadline(db, deadline, |conn| {
                            writer::checkpoint_foreground_interval(conn, interval_id, at_ms)
                        }) {
                            first_error.get_or_insert_with(|| error.to_string());
                        }
                    }
                    *last_foreground_checkpoint_ms = at_ms;
                }
            }
            IntervalAction::CloseForeground { at_ms, reason } => {
                if let Some(interval_id) = *open_foreground_id {
                    match with_writer_deadline(db, deadline, |conn| {
                        writer::close_foreground_interval(conn, interval_id, at_ms, reason)
                    }) {
                        Ok(()) => *open_foreground_id = None,
                        Err(error) => {
                            first_error.get_or_insert_with(|| error.to_string());
                        }
                    }
                }
                *last_foreground_checkpoint_ms = 0;
            }
            IntervalAction::StartComputerState {
                state,
                at_ms,
                source,
                quality,
            } => {
                match with_writer_deadline(db, deadline, |conn| {
                    writer::begin_computer_state_interval(conn, state, at_ms, source, quality)
                }) {
                    Ok(interval_id) => *open_computer_state_id = Some(interval_id),
                    Err(error) => {
                        first_error.get_or_insert_with(|| error.to_string());
                    }
                }
            }
            IntervalAction::CheckpointComputerState { at_ms } => {
                if at_ms.saturating_sub(*last_computer_checkpoint_ms)
                    >= USAGE_CHECKPOINT_INTERVAL_MS
                {
                    if let Err(error) = with_writer_deadline(db, deadline, |conn| {
                        writer::checkpoint_computer_state(conn, at_ms)
                    }) {
                        first_error.get_or_insert_with(|| error.to_string());
                    }
                    *last_computer_checkpoint_ms = at_ms;
                }
            }
            IntervalAction::CloseComputerState { at_ms, reason } => {
                if let Some(interval_id) = *open_computer_state_id {
                    match with_writer_deadline(db, deadline, |conn| {
                        writer::close_computer_state_interval(conn, interval_id, at_ms, reason)
                    }) {
                        Ok(()) => *open_computer_state_id = None,
                        Err(error) => {
                            first_error.get_or_insert_with(|| error.to_string());
                        }
                    }
                }
                *last_computer_checkpoint_ms = 0;
            }
            IntervalAction::MarkWindowsShutdown { at_ms } => {
                if let Err(error) = with_writer_deadline(db, deadline, |conn| {
                    writer::mark_windows_shutdown(conn, at_ms, "clean")
                }) {
                    first_error.get_or_insert_with(|| error.to_string());
                }
            }
        }
    }
    if let (Some(start), Some(end)) = (min_at, max_at) {
        let rollup_start = start.saturating_sub(DAILY_ROLLUP_PADDING_MS);
        let rollup_end = end
            .saturating_add(DAILY_ROLLUP_PADDING_MS)
            .max(rollup_start + 1);
        if let Err(error) = with_writer_deadline(db, deadline, |conn| {
            writer::rebuild_daily_usage(conn, rollup_start, rollup_end)
        }) {
            first_error.get_or_insert_with(|| error.to_string());
        }
    }
    first_error.map_or(Ok(()), Err)
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
    let idle_ms = platform::idle_for_ms().unwrap_or(0);
    if idle_ms >= settings.idle_threshold_seconds.saturating_mul(1_000) {
        ComputerState::Idle
    } else {
        ComputerState::Active
    }
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
