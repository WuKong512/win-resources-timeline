use super::{
    interval_engine::{IntervalAction, IntervalEngine},
    system_metrics::{now_ms, SystemSampler},
};
use crate::{
    db::{writer, Database},
    models::{ActivityState, CollectionSettings, CollectorStatus},
    platform,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub(crate) enum Control {
    SetPaused(bool),
    SessionPause(&'static str),
    SessionResume,
    OpenWindow,
    UpdateSettings(CollectionSettings, Sender<Result<(), String>>),
    Clear(Sender<Result<(), String>>),
    Shutdown,
}

#[derive(Clone)]
pub struct CollectorManager {
    tx: Sender<Control>,
    status: Arc<Mutex<CollectorStatus>>,
}

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

    pub fn shutdown(&self) {
        let _ = self.tx.send(Control::Shutdown);
    }
}

fn run_collector(
    db: Arc<Database>,
    rx: Receiver<Control>,
    status: Arc<Mutex<CollectorStatus>>,
    app: tauri::AppHandle,
) {
    let mut engine = IntervalEngine::default();
    let mut open_interval_id = None;
    let mut last_checkpoint_ms = 0;
    let mut system = SystemSampler::new();
    let mut settings = db
        .with_writer(writer::collection_settings)
        .unwrap_or_default();
    engine.set_expected_tick_ms(settings.foreground_poll_interval_ms);
    let mut tracked_app_keys: HashSet<String> =
        db.with_writer(writer::tracked_app_keys).unwrap_or_default();
    let mut last_foreground =
        Instant::now() - Duration::from_millis(settings.foreground_poll_interval_ms);
    let mut last_system =
        Instant::now() - Duration::from_millis(settings.system_sample_interval_ms);
    let mut last_system_flush = Instant::now();
    let mut frame_writer = writer::FrameWriter::new(64, 5);
    let mut last_prune = Instant::now() - Duration::from_secs(86_400);
    let mut session_suspended = false;
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Control::SetPaused(paused)) => {
                let now = now_ms();
                if paused {
                    apply_actions(
                        &db,
                        &mut open_interval_id,
                        &mut last_checkpoint_ms,
                        engine.pause(now),
                    );
                } else {
                    engine.resume(now);
                    system = SystemSampler::new();
                    last_foreground = Instant::now()
                        - Duration::from_millis(settings.foreground_poll_interval_ms);
                    last_system = Instant::now();
                }
                if let Ok(mut s) = status.lock() {
                    s.paused = paused;
                    s.last_heartbeat_at_ms = Some(now);
                }
                continue;
            }
            Ok(Control::SessionPause(reason)) => {
                let now = now_ms();
                apply_actions(
                    &db,
                    &mut open_interval_id,
                    &mut last_checkpoint_ms,
                    engine.terminate(now, reason),
                );
                let _ = flush_system_samples(&db, &mut frame_writer, true);
                session_suspended = true;
                if let Ok(mut s) = status.lock() {
                    s.last_heartbeat_at_ms = Some(now);
                }
                continue;
            }
            Ok(Control::SessionResume) => {
                let now = now_ms();
                engine.resume(now);
                system = SystemSampler::new();
                last_foreground =
                    Instant::now() - Duration::from_millis(settings.foreground_poll_interval_ms);
                last_system = Instant::now();
                last_system_flush = Instant::now();
                session_suspended = false;
                if let Ok(mut s) = status.lock() {
                    s.last_heartbeat_at_ms = Some(now);
                }
                continue;
            }
            Ok(Control::OpenWindow) => {
                crate::app_lifecycle::show_main_window(&app);
                continue;
            }
            Ok(Control::UpdateSettings(next, reply)) => {
                let now = now_ms();
                let result = db
                    .with_writer(|conn| writer::save_collection_settings(conn, &next, now))
                    .map_err(|error| error.to_string());
                if result.is_ok() {
                    settings = next;
                    engine.set_expected_tick_ms(settings.foreground_poll_interval_ms);
                    system = SystemSampler::new();
                    last_foreground = Instant::now()
                        - Duration::from_millis(settings.foreground_poll_interval_ms);
                    last_system = Instant::now();
                    last_system_flush = Instant::now();
                    last_prune = Instant::now() - Duration::from_secs(86_400);
                }
                let _ = reply.send(result);
                continue;
            }
            Ok(Control::Clear(reply)) => {
                let now = now_ms();
                apply_actions(
                    &db,
                    &mut open_interval_id,
                    &mut last_checkpoint_ms,
                    engine.terminate(now, "paused"),
                );
                frame_writer.discard_for_explicit_clear();
                let result = db
                    .with_writer(writer::clear_collected_data)
                    .map_err(|e| e.to_string());
                engine = IntervalEngine::default();
                engine.set_expected_tick_ms(settings.foreground_poll_interval_ms);
                tracked_app_keys.clear();
                let _ = reply.send(result);
                continue;
            }
            Ok(Control::Shutdown) => {
                apply_actions(
                    &db,
                    &mut open_interval_id,
                    &mut last_checkpoint_ms,
                    engine.terminate(now_ms(), "shutdown"),
                );
                let _ = flush_system_samples(&db, &mut frame_writer, true);
                break;
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }

        let now = now_ms();
        let paused = status.lock().map(|s| s.paused).unwrap_or(false);
        if !paused && !session_suspended {
            if last_foreground.elapsed()
                >= Duration::from_millis(settings.foreground_poll_interval_ms)
            {
                last_foreground = Instant::now();
                let observation = platform::foreground_app().and_then(|app| {
                    tracked_app_keys.insert(app.identity_key.clone());
                    let app_id = db
                        .with_writer(|conn| writer::upsert_app(conn, &app, now))
                        .ok()?;
                    let activity = if platform::idle_for_ms().unwrap_or(0)
                        >= settings.idle_threshold_seconds * 1_000
                    {
                        ActivityState::Idle
                    } else {
                        ActivityState::Active
                    };
                    Some((app_id, activity))
                });
                apply_actions(
                    &db,
                    &mut open_interval_id,
                    &mut last_checkpoint_ms,
                    engine.observe(now, observation),
                );
                if let Ok(mut s) = status.lock() {
                    s.last_foreground_sample_at_ms = Some(now);
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
                if let Ok(mut s) = status.lock() {
                    if let Ok(health) = &result {
                        s.last_system_sample_at_ms = health.last_committed_timestamp_ms;
                        s.dropped_system_samples = health.drop_count;
                    }
                }
            }
        }
        if last_prune.elapsed() >= Duration::from_secs(86_400) {
            last_prune = Instant::now();
            let cutoff = now - settings.system_sample_retention_days as i64 * 86_400_000;
            let _ = db.with_writer(|conn| writer::prune_system_samples(conn, cutoff));
        }
        if let Ok(mut s) = status.lock() {
            s.last_heartbeat_at_ms = Some(now);
        }
    }
    if let Ok(mut s) = status.lock() {
        s.running = false;
    }
}

fn apply_actions(
    db: &Database,
    open_interval_id: &mut Option<i64>,
    last_checkpoint_ms: &mut i64,
    actions: Vec<IntervalAction>,
) {
    for action in actions {
        match action {
            IntervalAction::Start {
                app_id,
                at_ms,
                activity,
            } => {
                *open_interval_id = db
                    .with_writer(|c| writer::begin_interval(c, app_id, at_ms, activity.as_str()))
                    .ok();
                *last_checkpoint_ms = at_ms;
            }
            IntervalAction::Checkpoint { at_ms } => {
                if at_ms - *last_checkpoint_ms >= 15_000 {
                    if let Some(id) = *open_interval_id {
                        let _ = db.with_writer(|c| writer::checkpoint_interval(c, id, at_ms));
                    }
                    *last_checkpoint_ms = at_ms;
                }
            }
            IntervalAction::Close { at_ms, reason } => {
                if let Some(id) = open_interval_id.take() {
                    let _ = db.with_writer(|c| writer::close_interval(c, id, at_ms, reason));
                }
                *last_checkpoint_ms = 0;
            }
        }
    }
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
