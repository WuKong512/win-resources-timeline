use super::{
    interval_engine::{IntervalAction, IntervalEngine, UsageEvent},
    system_metrics::{now_ms, SystemSampler},
};
use crate::{
    db::{writer, Database},
    models::{CollectionSettings, CollectorStatus, ComputerState, ForegroundApp},
    platform::{self, PlatformEvent},
};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub(crate) enum Control {
    Platform(platform::PlatformEventEnvelope),
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
const PLATFORM_PENDING_CAPACITY: usize = 128;
const CONTROL_PENDING_CAPACITY: usize = 32;
const PLATFORM_RETRY_BACKOFF: Duration = Duration::from_millis(50);

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

#[derive(Debug, Default)]
struct PendingPlatformEvents {
    events: Vec<platform::PlatformEventEnvelope>,
    last_applied_sequence: Option<u64>,
    gap_pending: bool,
    force_resync: bool,
    failed_event_pending: bool,
    retry_at: Option<Instant>,
}

impl PendingPlatformEvents {
    fn enqueue(&mut self, event: platform::PlatformEventEnvelope) {
        if self.events.len() >= PLATFORM_PENDING_CAPACITY {
            self.gap_pending = true;
            self.force_resync = true;
            return;
        }
        self.events.push(event);
        self.sort();
    }

    fn sort(&mut self) {
        self.events.sort_unstable_by_key(|event| event.sequence);
    }

    fn mark_gap(&mut self) {
        self.gap_pending = true;
        self.force_resync = true;
    }

    fn retry_ready(&self) -> bool {
        self.retry_ready_at(Instant::now())
    }

    fn retry_ready_at(&self, now: Instant) -> bool {
        self.retry_at
            .map(|retry_at| retry_at <= now)
            .unwrap_or(true)
    }

    #[cfg(test)]
    fn retry_delay(&self, now: Instant) -> Duration {
        self.retry_at
            .map(|retry_at| retry_at.saturating_duration_since(now))
            .unwrap_or_default()
    }

    fn mark_event_failure(&mut self) {
        self.failed_event_pending = true;
        self.retry_at = Some(Instant::now() + PLATFORM_RETRY_BACKOFF);
    }

    fn mark_recovery_failure(&mut self) {
        self.retry_at = Some(Instant::now() + PLATFORM_RETRY_BACKOFF);
    }

    fn mark_success(&mut self, sequence: u64) {
        self.last_applied_sequence = Some(
            self.last_applied_sequence
                .map_or(sequence, |last| last.max(sequence)),
        );
        self.failed_event_pending = false;
        self.retry_at = None;
    }

    fn clear_retry(&mut self) {
        self.retry_at = None;
    }

    fn has_pending_events(&self) -> bool {
        !self.events.is_empty()
    }

    fn is_blocked_by_event_failure(&self) -> bool {
        self.failed_event_pending
    }

    fn is_stale(&self, sequence: u64) -> bool {
        self.last_applied_sequence
            .is_some_and(|last| sequence <= last)
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
    critical_rx: Receiver<platform::PlatformEventEnvelope>,
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
    let mut pending_platform_events = PendingPlatformEvents::default();
    let mut pending_controls = VecDeque::new();

    loop {
        let recovery = platform::take_observer_recovery();
        if recovery.overflowed {
            pending_platform_events.mark_gap();
        }
        for event in recovery.events {
            pending_platform_events.enqueue(event);
        }
        collect_platform_events(
            &rx,
            &critical_rx,
            &mut pending_platform_events,
            &mut pending_controls,
        );
        if platform::take_observer_dirty() {
            pending_platform_events.force_resync = true;
        }

        // An overflow means that one or more critical transitions may be missing. Establish an
        // explicit unknown boundary before applying any later recovery event, otherwise a
        // delivered Resume/Connect could bridge the lost Sleep/Disconnected interval.
        if pending_platform_events.gap_pending
            && !pending_platform_events.is_blocked_by_event_failure()
            && pending_platform_events.retry_ready()
        {
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
                pending_platform_events.mark_recovery_failure();
                continue;
            }
            pending_platform_events.gap_pending = false;
            pending_platform_events.clear_retry();
        }

        while pending_platform_events.has_pending_events()
            && !pending_platform_events.gap_pending
            && pending_platform_events.retry_ready()
        {
            let event = pending_platform_events.events[0];
            if pending_platform_events.is_stale(event.sequence) {
                pending_platform_events.events.remove(0);
                pending_platform_events.mark_gap();
                continue;
            }
            let result = handle_platform_event(
                &db,
                &mut engine,
                &mut persistence,
                &mut rollup_scheduler,
                &mut tracked_app_keys,
                &settings,
                event.event,
                None,
            );
            record_usage_result(&status, &result);
            if let Err(error) = result {
                eprintln!("collector critical platform event failed: {error}");
                pending_platform_events.mark_event_failure();
                break;
            }
            pending_platform_events.events.remove(0);
            pending_platform_events.mark_success(event.sequence);
            if matches!(event.event, PlatformEvent::ForegroundWindow { .. }) {
                if let Ok(mut value) = status.lock() {
                    value.last_foreground_sample_at_ms = Some(now_ms());
                }
            }
        }

        let next_control = if pending_platform_events.is_blocked_by_event_failure()
            || pending_platform_events.gap_pending
            || pending_platform_events.has_pending_events()
            || !pending_platform_events.retry_ready()
        {
            let wait = pending_platform_events
                .retry_at
                .map(|retry_at| retry_at.saturating_duration_since(Instant::now()))
                .unwrap_or(Duration::from_millis(1))
                .min(Duration::from_millis(100));
            if !wait.is_zero() {
                thread::sleep(wait);
            }
            None
        } else if let Some(control) = pending_controls.pop_front() {
            Some(control)
        } else {
            let wait = pending_platform_events
                .retry_at
                .map(|retry_at| retry_at.saturating_duration_since(Instant::now()))
                .unwrap_or(Duration::from_millis(100))
                .min(Duration::from_millis(100));
            match rx.recv_timeout(wait) {
                Ok(control) => Some(control),
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
            }
        };

        let next_control = match next_control {
            Some(control) if control_requires_platform_barrier(&control) => {
                collect_platform_events(
                    &rx,
                    &critical_rx,
                    &mut pending_platform_events,
                    &mut pending_controls,
                );
                if pending_platform_events.gap_pending
                    || pending_platform_events.has_pending_events()
                {
                    pending_controls.push_front(control);
                    None
                } else {
                    Some(control)
                }
            }
            other => other,
        };

        match next_control {
            Some(Control::Platform(event)) => pending_platform_events.enqueue(event),
            Some(Control::SetPaused(paused)) => {
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
            Some(Control::OpenWindow) => {
                crate::app_lifecycle::show_main_window(&app);
            }
            Some(Control::UpdateSettings(next, reply)) => {
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
            Some(Control::Clear(reply)) => {
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
            Some(Control::Shutdown {
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
            None => {}
        }

        let now = now_ms();
        let paused = status.lock().map(|value| value.paused).unwrap_or(false);
        if !paused
            && !pending_platform_events.is_blocked_by_event_failure()
            && !pending_platform_events.gap_pending
            && !pending_platform_events.has_pending_events()
            && pending_platform_events.retry_ready()
        {
            if pending_platform_events.force_resync
                || last_heartbeat.elapsed() >= USAGE_HEARTBEAT_INTERVAL
            {
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
                let mut heartbeat_succeeded = true;
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
                        heartbeat_succeeded = false;
                        break;
                    }
                }
                if heartbeat_succeeded {
                    pending_platform_events.force_resync = false;
                    last_observation_ms = Some(now);
                    if let Ok(mut value) = status.lock() {
                        value.last_foreground_sample_at_ms = Some(now);
                    }
                } else {
                    pending_platform_events.mark_recovery_failure();
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

fn collect_platform_events(
    rx: &Receiver<Control>,
    critical_rx: &Receiver<platform::PlatformEventEnvelope>,
    pending_platform_events: &mut PendingPlatformEvents,
    pending_controls: &mut VecDeque<Control>,
) {
    while pending_platform_events.events.len() < PLATFORM_PENDING_CAPACITY {
        match critical_rx.try_recv() {
            Ok(event) => pending_platform_events.enqueue(event),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }

    while pending_platform_events.events.len() < PLATFORM_PENDING_CAPACITY
        && pending_controls.len() < CONTROL_PENDING_CAPACITY
    {
        match rx.try_recv() {
            Ok(Control::Platform(event)) => pending_platform_events.enqueue(event),
            Ok(control) => pending_controls.push_back(control),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
    pending_platform_events.sort();
}

fn control_requires_platform_barrier(control: &Control) -> bool {
    matches!(
        control,
        Control::SetPaused(_) | Control::Clear(_) | Control::Shutdown { .. }
    )
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
            let foreground_app_executable_id =
                resolve_current_foreground(db, tracked_app_keys, at_ms)?;
            apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                UsageEvent::Unlocked {
                    at_ms,
                    state: observed_computer_state(settings),
                    foreground_app_executable_id,
                },
                deadline,
            )
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
            let foreground_app_executable_id =
                resolve_current_foreground(db, tracked_app_keys, at_ms)?;
            apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                UsageEvent::Resume {
                    at_ms,
                    state: observed_computer_state(settings),
                    foreground_app_executable_id,
                },
                deadline,
            )
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
            let foreground_app_executable_id =
                resolve_current_foreground(db, tracked_app_keys, at_ms)?;
            apply_usage_event_with_state(
                db,
                engine,
                persistence,
                rollup_scheduler,
                UsageEvent::Connected {
                    at_ms,
                    state: observed_computer_state(settings),
                    foreground_app_executable_id,
                },
                deadline,
            )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::writer,
        models::ForegroundApp,
        platform::{PlatformEvent, PlatformEventEnvelope},
    };
    use rusqlite::Connection;
    use std::path::PathBuf;

    struct UsageHarness {
        db: Database,
        dir: PathBuf,
        engine: IntervalEngine,
        persistence: writer::UsagePersistenceState,
        rollup_scheduler: DailyRollupScheduler,
        executable_id: i64,
    }

    impl UsageHarness {
        fn new(name: &str) -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let dir = std::env::temp_dir().join(format!(
                "resource-timeline-manager-{name}-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("usage.sqlite3");
            let db = Database::open(path).unwrap();
            let app = ForegroundApp {
                identity_key: format!("name:{name}.exe"),
                process_name: format!("{name}.exe"),
                exe_path: Some(format!(r"C:\{name}.exe")),
                display_name: name.to_string(),
                pid: Some(7_001),
                process_creation_time_ms: Some(1_000),
            };
            let executable_id = db
                .with_writer(|conn| {
                    writer::resolve_foreground_app(conn, &app, 1_000)
                        .map(|resolution| resolution.app_executable_id)
                })
                .unwrap();
            let mut harness = Self {
                db,
                dir,
                engine: IntervalEngine::default(),
                persistence: writer::UsagePersistenceState::default(),
                rollup_scheduler: DailyRollupScheduler::default(),
                executable_id,
            };
            harness
                .apply(UsageEvent::Resync {
                    at_ms: 1_000,
                    foreground_app_executable_id: Some(executable_id),
                    state: ComputerState::Active,
                })
                .unwrap();
            harness
        }

        fn apply(&mut self, event: UsageEvent) -> Result<UsageApplyResult, String> {
            apply_usage_event(
                &self.db,
                &mut self.engine,
                &mut self.persistence,
                &mut self.rollup_scheduler,
                event,
                None,
            )
        }

        fn apply_until(
            &mut self,
            event: UsageEvent,
            deadline: Instant,
        ) -> Result<UsageApplyResult, String> {
            apply_usage_event_with_state(
                &self.db,
                &mut self.engine,
                &mut self.persistence,
                &mut self.rollup_scheduler,
                event,
                Some(deadline),
            )
        }

        fn lock_database(&self) -> Connection {
            let blocker = Connection::open(self.db.path()).unwrap();
            blocker.busy_timeout(Duration::ZERO).unwrap();
            blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
            blocker
        }

        fn assert_open_intervals_are_non_overlapping(&self) {
            self.db
                .read(|conn| {
                    let overlapping_foreground: i64 = conn.query_row(
                        "SELECT COUNT(*)
                         FROM foreground_interval first_interval
                         JOIN foreground_interval second_interval
                           ON first_interval.id < second_interval.id
                          AND first_interval.start_time_ms < COALESCE(second_interval.end_time_ms, 9223372036854775807)
                          AND second_interval.start_time_ms < COALESCE(first_interval.end_time_ms, 9223372036854775807)",
                        [],
                        |row| row.get(0),
                    )?;
                    let overlapping_state: i64 = conn.query_row(
                        "SELECT COUNT(*)
                         FROM computer_state_interval first_state
                         JOIN computer_state_interval second_state
                           ON first_state.id < second_state.id
                          AND first_state.start_ts < COALESCE(second_state.end_ts, 9223372036854775807)
                          AND second_state.start_ts < COALESCE(first_state.end_ts, 9223372036854775807)",
                        [],
                        |row| row.get(0),
                    )?;
                    assert_eq!(overlapping_foreground, 0);
                    assert_eq!(overlapping_state, 0);
                    Ok::<_, rusqlite::Error>(())
                })
                .unwrap();
        }

        fn finish(self) {
            let dir = self.dir.clone();
            drop(self);
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    fn envelope(sequence: u64, event: PlatformEvent) -> PlatformEventEnvelope {
        PlatformEventEnvelope::with_sequence(sequence, event)
    }

    fn foreground(sequence: u64, at_ms: i64) -> PlatformEventEnvelope {
        envelope(sequence, PlatformEvent::ForegroundWindow { hwnd: 1, at_ms })
    }

    fn process_event_after_failure(
        pending: &mut PendingPlatformEvents,
        event: PlatformEventEnvelope,
        apply: impl FnOnce() -> Result<(), String>,
    ) {
        assert_eq!(pending.events.first().copied(), Some(event));
        if apply().is_ok() {
            pending.events.remove(0);
            pending.mark_success(event.sequence);
        } else {
            pending.mark_event_failure();
        }
    }

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

    #[test]
    fn critical_event_failure_stays_pending_until_locked_interval_commits() {
        let mut harness = UsageHarness::new("locked-retry");
        let event = envelope(10, PlatformEvent::Locked { at_ms: 2_000 });
        let mut pending = PendingPlatformEvents::default();
        pending.enqueue(event);
        let blocker = harness.lock_database();
        let first = harness.apply_until(
            UsageEvent::Locked { at_ms: 2_000 },
            Instant::now() + Duration::from_millis(100),
        );
        assert!(first.is_err());
        pending.mark_event_failure();
        assert_eq!(pending.events.first().copied(), Some(event));
        assert!(pending.is_blocked_by_event_failure());
        drop(blocker);

        let second = harness.apply(UsageEvent::Locked { at_ms: 2_000 });
        assert!(second.is_ok());
        pending.events.remove(0);
        pending.mark_success(event.sequence);
        assert!(pending.events.is_empty());
        assert!(!pending.is_blocked_by_event_failure());
        harness
            .db
            .read(|conn| {
                let foreground_end: i64 = conn.query_row(
                    "SELECT end_time_ms FROM foreground_interval WHERE id = 1",
                    [],
                    |row| row.get(0),
                )?;
                let active_end: i64 = conn.query_row(
                    "SELECT end_ts FROM computer_state_interval WHERE state = 'active'",
                    [],
                    |row| row.get(0),
                )?;
                let locked_start: i64 = conn.query_row(
                    "SELECT start_ts FROM computer_state_interval WHERE state = 'locked'",
                    [],
                    |row| row.get(0),
                )?;
                assert_eq!(foreground_end, 2_000);
                assert_eq!(active_end, 2_000);
                assert_eq!(locked_start, 2_000);
                Ok::<_, rusqlite::Error>(())
            })
            .unwrap();
        harness.assert_open_intervals_are_non_overlapping();
        harness.finish();
    }

    #[test]
    fn failed_suspend_blocks_resume_until_sleep_boundary_commits() {
        let mut harness = UsageHarness::new("suspend-retry");
        let suspend = envelope(20, PlatformEvent::Suspended { at_ms: 2_000 });
        let resume = envelope(21, PlatformEvent::Resumed { at_ms: 3_000 });
        let mut pending = PendingPlatformEvents::default();
        pending.enqueue(suspend);
        pending.enqueue(resume);
        let blocker = harness.lock_database();
        let first = harness.apply_until(
            UsageEvent::Suspend { at_ms: 2_000 },
            Instant::now() + Duration::from_millis(100),
        );
        assert!(first.is_err());
        pending.mark_event_failure();
        assert_eq!(pending.events[0].sequence, 20);
        assert_eq!(pending.events[1].sequence, 21);
        drop(blocker);

        process_event_after_failure(&mut pending, suspend, || {
            harness
                .apply(UsageEvent::Suspend { at_ms: 2_000 })
                .map(|_| ())
        });
        process_event_after_failure(&mut pending, resume, || {
            harness
                .apply(UsageEvent::Resume {
                    at_ms: 3_000,
                    state: ComputerState::Active,
                    foreground_app_executable_id: Some(harness.executable_id),
                })
                .map(|_| ())
        });
        assert!(pending.events.is_empty());
        harness
            .db
            .read(|conn| {
                let sleep: (i64, i64) = conn.query_row(
                    "SELECT start_ts, end_ts FROM computer_state_interval WHERE state = 'sleep'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(sleep, (2_000, 3_000));
                let active: Vec<(i64, Option<i64>)> = conn
                    .prepare(
                        "SELECT start_ts, end_ts FROM computer_state_interval WHERE state = 'active' ORDER BY start_ts",
                    )?
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<_, _>>()?;
                assert_eq!(active, vec![(1_000, Some(2_000)), (3_000, None)]);
                Ok::<_, rusqlite::Error>(())
            })
            .unwrap();
        harness.assert_open_intervals_are_non_overlapping();
        harness.finish();
    }

    #[test]
    fn failed_disconnect_blocks_connect_until_disconnected_boundary_commits() {
        let mut harness = UsageHarness::new("disconnect-retry");
        let disconnect = envelope(30, PlatformEvent::Disconnected { at_ms: 2_000 });
        let connect = envelope(31, PlatformEvent::Connected { at_ms: 3_000 });
        let mut pending = PendingPlatformEvents::default();
        pending.enqueue(disconnect);
        pending.enqueue(connect);
        let blocker = harness.lock_database();
        let first = harness.apply_until(
            UsageEvent::Disconnected { at_ms: 2_000 },
            Instant::now() + Duration::from_millis(100),
        );
        assert!(first.is_err());
        pending.mark_event_failure();
        drop(blocker);

        process_event_after_failure(&mut pending, disconnect, || {
            harness
                .apply(UsageEvent::Disconnected { at_ms: 2_000 })
                .map(|_| ())
        });
        process_event_after_failure(&mut pending, connect, || {
            harness
                .apply(UsageEvent::Connected {
                    at_ms: 3_000,
                    state: ComputerState::Active,
                    foreground_app_executable_id: Some(harness.executable_id),
                })
                .map(|_| ())
        });
        assert!(pending.events.is_empty());
        harness
            .db
            .read(|conn| {
                let disconnected: (i64, i64) = conn.query_row(
                    "SELECT start_ts, end_ts FROM computer_state_interval WHERE state = 'disconnected'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(disconnected, (2_000, 3_000));
                Ok::<_, rusqlite::Error>(())
            })
            .unwrap();
        harness.assert_open_intervals_are_non_overlapping();
        harness.finish();
    }

    #[test]
    fn observer_gap_failure_keeps_recovery_pending_before_resume() {
        let mut harness = UsageHarness::new("gap-retry");
        let mut pending = PendingPlatformEvents::default();
        pending.mark_gap();
        let blocker = harness.lock_database();
        let first = harness.apply_until(
            UsageEvent::ObserverGap { at_ms: 2_000 },
            Instant::now() + Duration::from_millis(100),
        );
        assert!(first.is_err());
        pending.mark_recovery_failure();
        assert!(pending.gap_pending);
        assert!(!pending.is_blocked_by_event_failure());
        drop(blocker);

        assert!(harness
            .apply(UsageEvent::ObserverGap { at_ms: 2_000 })
            .is_ok());
        pending.gap_pending = false;
        pending.clear_retry();
        assert!(!pending.gap_pending);
        assert!(harness
            .apply(UsageEvent::Resume {
                at_ms: 3_000,
                state: ComputerState::Active,
                foreground_app_executable_id: Some(harness.executable_id),
            })
            .is_ok());
        harness
            .db
            .read(|conn| {
                let active: Vec<(i64, Option<i64>)> = conn
                    .prepare(
                        "SELECT start_ts, end_ts FROM computer_state_interval WHERE state = 'active' ORDER BY start_ts",
                    )?
                    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                    .collect::<Result<_, _>>()?;
                assert_eq!(active, vec![(1_000, Some(1_000)), (3_000, None)]);
                let unknown: (i64, i64) = conn.query_row(
                    "SELECT start_ts, end_ts FROM computer_state_interval WHERE state = 'unknown'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                assert_eq!(unknown, (1_000, 3_000));
                Ok::<_, rusqlite::Error>(())
            })
            .unwrap();
        harness.assert_open_intervals_are_non_overlapping();
        harness.finish();
    }

    #[test]
    fn critical_pending_queue_is_bounded_and_retry_is_backed_off() {
        let mut pending = PendingPlatformEvents::default();
        for sequence in 1..=(PLATFORM_PENDING_CAPACITY as u64 + 32) {
            pending.enqueue(foreground(sequence, sequence as i64));
        }
        assert_eq!(pending.events.len(), PLATFORM_PENDING_CAPACITY);
        assert!(pending.gap_pending);
        assert!(pending.force_resync);

        let before = Instant::now();
        pending.mark_event_failure();
        assert!(!pending.retry_ready_at(before));
        assert!(pending.retry_delay(before) >= PLATFORM_RETRY_BACKOFF);
        assert!(pending.events.len() <= PLATFORM_PENDING_CAPACITY);
    }

    #[test]
    fn platform_events_from_normal_and_critical_lanes_are_sequence_ordered() {
        let (normal_tx, normal_rx) = bounded(8);
        let (critical_tx, critical_rx) = bounded(8);
        normal_tx
            .send(Control::Platform(foreground(10, 100)))
            .unwrap();
        critical_tx
            .send(envelope(11, PlatformEvent::Locked { at_ms: 110 }))
            .unwrap();
        critical_tx
            .send(envelope(12, PlatformEvent::Unlocked { at_ms: 120 }))
            .unwrap();
        normal_tx
            .send(Control::Platform(foreground(13, 130)))
            .unwrap();

        let mut pending = PendingPlatformEvents::default();
        let mut controls = VecDeque::new();
        collect_platform_events(&normal_rx, &critical_rx, &mut pending, &mut controls);
        assert!(controls.is_empty());
        assert_eq!(
            pending
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![10, 11, 12, 13]
        );
    }

    #[test]
    fn fallback_recovery_and_critical_channel_keep_sequence_order() {
        let (normal_tx, normal_rx) = bounded(8);
        let (critical_tx, critical_rx) = bounded(8);
        normal_tx
            .send(Control::Platform(foreground(13, 130)))
            .unwrap();
        critical_tx
            .send(envelope(12, PlatformEvent::Connected { at_ms: 120 }))
            .unwrap();

        let mut pending = PendingPlatformEvents::default();
        let recovery = platform::ObserverRecovery {
            events: vec![envelope(11, PlatformEvent::Suspended { at_ms: 110 })],
            overflowed: false,
        };
        for event in recovery.events {
            pending.enqueue(event);
        }
        collect_platform_events(&normal_rx, &critical_rx, &mut pending, &mut VecDeque::new());
        assert_eq!(
            pending
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![11, 12, 13]
        );
    }

    #[test]
    fn stale_foreground_event_cannot_rewrite_truth_after_unlock() {
        let mut pending = PendingPlatformEvents::default();
        pending.enqueue(envelope(12, PlatformEvent::Unlocked { at_ms: 120 }));
        let unlock = pending.events[0];
        pending.events.remove(0);
        pending.mark_success(unlock.sequence);
        pending.enqueue(foreground(10, 100));
        assert!(pending.is_stale(pending.events[0].sequence));
        pending.events.remove(0);
        pending.mark_gap();
        assert!(pending.gap_pending);
    }

    #[test]
    fn failed_sequence_head_prevents_later_platform_event_from_overtaking() {
        let mut pending = PendingPlatformEvents::default();
        let first = envelope(11, PlatformEvent::Locked { at_ms: 110 });
        let later = foreground(13, 130);
        pending.enqueue(first);
        pending.mark_event_failure();
        pending.enqueue(later);
        assert!(pending.is_blocked_by_event_failure());
        assert_eq!(pending.events[0].sequence, 11);
        assert_eq!(pending.events[1].sequence, 13);
        assert!(!pending.retry_ready());
    }

    #[test]
    fn equal_timestamps_still_use_sequence_as_the_deterministic_order() {
        let mut pending = PendingPlatformEvents::default();
        pending.enqueue(foreground(20, 500));
        pending.enqueue(envelope(18, PlatformEvent::Locked { at_ms: 500 }));
        pending.enqueue(envelope(19, PlatformEvent::Unlocked { at_ms: 500 }));
        assert_eq!(
            pending
                .events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            vec![18, 19, 20]
        );
    }
}
