use crate::db::Database;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

pub const CRASH_EVIDENCE_PROCESSING_VERSION: &str = "crash-evidence-v1";
pub const CRASH_CASE_WINDOW_PRE_MS: i64 = 30 * 60 * 1_000;
pub const CRASH_CASE_WINDOW_POST_MS: i64 = 5 * 60 * 1_000;
pub const CRASH_RETENTION_CASE_LIMIT: usize = 10;
const EVENT_SCAN_BATCH_SIZE: usize = 256;
const EVENT_CONTEXT_LIMIT: i64 = 4_096;
const EVENT_CURSOR_KEY: &str = "crash_event_cursor_v1";
const EVENT_CURSOR_LOOKBACK_MS: i64 = 5 * 60 * 1_000;
const CRASH_REBOOT_MATCH_MS: i64 = 30 * 60 * 1_000;
const CRASH_INCIDENT_CORRELATION_MS: i64 = 30 * 60 * 1_000;
// A following/past EventLog boot marker is an episode identity only when it is close to the
// boundary. The larger reboot/context window remains available for evidence search and cleanup.
const CRASH_BOOT_EPISODE_MATCH_MS: i64 = 5 * 60 * 1_000;
// 6008 positional timestamps are second-granular on some Windows builds; this is physical-time
// precision tolerance, deliberately far smaller than the broad context search window above.
const CRASH_PHYSICAL_ANCHOR_TOLERANCE_MS: i64 = 5_000;
// The system sampler normally publishes a frame every five seconds. Keep partial post-window
// cases retryable for three such cadences so a frame that races the first finalization is still
// observed, while old historical partial cases settle permanently.
const CRASH_POST_FINALIZATION_STABILIZATION_MS: i64 = 3 * CRASH_DETECTOR_POLL_MS as i64;
// Native Event Log startup scans cover the previous seven days. A physical shutdown timestamp
// from a retrieved crash record remains valid throughout that supported history, even when the
// machine stayed powered off for more than one day.
const CRASH_PHYSICAL_ANCHOR_HORIZON_MS: i64 = 7 * 86_400_000;
const CRASH_DETECTOR_POLL_MS: u64 = 5_000;
const CRASH_SCAN_YIELD_PAGES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemEventKind {
    BugCheck,
    UnexpectedShutdown,
    AbnormalRestart,
    NormalBoot,
    NormalShutdown,
    Sleep,
    Resume,
    Supporting,
    ApplicationCrash,
    Other,
}

impl SystemEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BugCheck => "bugcheck",
            Self::UnexpectedShutdown => "unexpected_shutdown",
            Self::AbnormalRestart => "abnormal_restart",
            Self::NormalBoot => "normal_boot",
            Self::NormalShutdown => "normal_shutdown",
            Self::Sleep => "sleep",
            Self::Resume => "resume",
            Self::Supporting => "supporting",
            Self::ApplicationCrash => "application_crash",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventPayloadFacts {
    pub bugcheck_code: Option<String>,
    pub boot_id: Option<String>,
    /// An objective physical-boundary timestamp extracted from event payload when Windows
    /// provides one, such as the previous unexpected shutdown time in EventLog/6008. This is
    /// deliberately separate from `event_time_ms`, which is only the log-record timestamp.
    pub previous_shutdown_time_ms: Option<i64>,
    pub clean_shutdown: Option<bool>,
    pub restart_boundary: Option<bool>,
    pub dump_available: Option<bool>,
    pub dump_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedSystemEvent {
    pub channel: String,
    pub provider: Option<String>,
    pub event_id: String,
    pub record_id: String,
    pub event_time_ms: i64,
    pub kind: SystemEventKind,
    pub payload: EventPayloadFacts,
}

impl NormalizedSystemEvent {
    pub fn from_fields(
        channel: impl Into<String>,
        provider: Option<String>,
        event_id: impl Into<String>,
        record_id: impl Into<String>,
        event_time_ms: i64,
        payload: EventPayloadFacts,
    ) -> Self {
        let channel = channel.into();
        let provider = provider.map(|value| value.trim().to_string());
        let event_id = event_id.into();
        let kind = classify_event_signal(&channel, provider.as_deref(), &event_id, &payload);
        Self {
            channel,
            provider,
            event_id,
            record_id: record_id.into(),
            event_time_ms,
            kind,
            payload,
        }
    }

    #[cfg(test)]
    pub fn fixture(
        kind: SystemEventKind,
        record_id: impl Into<String>,
        event_time_ms: i64,
    ) -> Self {
        let (provider, event_id) = match kind {
            SystemEventKind::BugCheck => (
                Some("Microsoft-Windows-WER-SystemErrorReporting".to_string()),
                "1001",
            ),
            SystemEventKind::UnexpectedShutdown => {
                (Some("Microsoft-Windows-Kernel-Power".to_string()), "41")
            }
            SystemEventKind::NormalBoot => (Some("EventLog".to_string()), "6005"),
            SystemEventKind::NormalShutdown => (Some("EventLog".to_string()), "6006"),
            SystemEventKind::Sleep => (Some("Microsoft-Windows-Kernel-Power".to_string()), "42"),
            SystemEventKind::Resume => (Some("Microsoft-Windows-Kernel-Power".to_string()), "107"),
            SystemEventKind::ApplicationCrash => (Some("Application Error".to_string()), "1000"),
            _ => (Some("ResourceTimeline.Test".to_string()), "0"),
        };
        let payload = EventPayloadFacts {
            previous_shutdown_time_ms: matches!(
                kind,
                SystemEventKind::BugCheck
                    | SystemEventKind::UnexpectedShutdown
                    | SystemEventKind::AbnormalRestart
            )
            .then_some(event_time_ms),
            restart_boundary: matches!(
                kind,
                SystemEventKind::BugCheck
                    | SystemEventKind::UnexpectedShutdown
                    | SystemEventKind::AbnormalRestart
            )
            .then_some(true),
            ..EventPayloadFacts::default()
        };
        let mut event = Self::from_fields(
            "System",
            provider,
            event_id,
            record_id,
            event_time_ms,
            payload,
        );
        event.kind = kind;
        event
    }

    fn payload_summary(&self) -> String {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "kind".to_string(),
            serde_json::Value::String(self.kind.as_str().to_string()),
        );
        if let Some(value) = &self.payload.bugcheck_code {
            payload.insert(
                "bugcheck_code".to_string(),
                serde_json::Value::String(value.clone()),
            );
        }
        if let Some(value) = &self.payload.boot_id {
            payload.insert(
                "boot_id".to_string(),
                serde_json::Value::String(value.clone()),
            );
        }
        if let Some(value) = self.payload.previous_shutdown_time_ms {
            payload.insert(
                "previous_shutdown_time_ms".to_string(),
                serde_json::Value::Number(value.into()),
            );
        }
        if let Some(value) = self.payload.clean_shutdown {
            payload.insert("clean_shutdown".to_string(), serde_json::Value::Bool(value));
        }
        if let Some(value) = self.payload.restart_boundary {
            payload.insert(
                "restart_boundary".to_string(),
                serde_json::Value::Bool(value),
            );
        }
        if let Some(value) = self.payload.dump_available {
            payload.insert("dump_available".to_string(), serde_json::Value::Bool(value));
        }
        if let Some(value) = self.payload.dump_size_bytes {
            payload.insert(
                "dump_size_bytes".to_string(),
                serde_json::Value::Number(value.into()),
            );
        }
        serde_json::Value::Object(payload).to_string()
    }
}

fn classify_event_signal(
    channel: &str,
    provider: Option<&str>,
    event_id: &str,
    payload: &EventPayloadFacts,
) -> SystemEventKind {
    let channel = channel.trim().to_ascii_lowercase();
    let provider = provider.unwrap_or_default().trim().to_ascii_lowercase();
    let event_id = event_id.trim();
    if channel != "system" {
        if provider.contains("application error") || provider.contains("wer") {
            return SystemEventKind::ApplicationCrash;
        }
        return SystemEventKind::Other;
    }
    if provider.contains("systemerrorreporting") && event_id == "1001" {
        return SystemEventKind::BugCheck;
    }
    if provider.contains("kernel-power") && event_id == "41" {
        if has_nonzero_bugcheck_code(payload) {
            return SystemEventKind::BugCheck;
        }
        return if payload.clean_shutdown == Some(true) {
            SystemEventKind::NormalShutdown
        } else {
            // Event 41 is itself Windows' unexpected-shutdown boundary. The native reader does
            // not need to synthesize a fixture-only restart flag for this semantic.
            SystemEventKind::UnexpectedShutdown
        };
    }
    if provider == "eventlog" && event_id == "6008" {
        return SystemEventKind::UnexpectedShutdown;
    }
    if provider == "eventlog" && event_id == "6005" {
        return SystemEventKind::NormalBoot;
    }
    if provider == "eventlog" && event_id == "6006" {
        return SystemEventKind::NormalShutdown;
    }
    if provider.contains("kernel-power") && event_id == "42" {
        return SystemEventKind::Sleep;
    }
    if (provider.contains("kernel-power") && event_id == "107")
        || (provider.contains("power-troubleshooter") && event_id == "1")
    {
        return SystemEventKind::Resume;
    }
    if provider.contains("whea")
        || provider.contains("display")
        || provider.contains("dxgkrnl")
        || provider.contains("nvlddmkm")
    {
        return SystemEventKind::Supporting;
    }
    if provider.contains("application error") || provider.contains("wer") {
        return SystemEventKind::ApplicationCrash;
    }
    SystemEventKind::Other
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCursor {
    pub channel: String,
    pub record_id: String,
    pub event_time_ms: i64,
}

#[derive(Debug, Clone, Default)]
pub struct EventReadBatch {
    pub events: Vec<NormalizedSystemEvent>,
    pub next_cursors: BTreeMap<String, EventCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventReaderError {
    PermissionDenied(String),
    #[allow(dead_code)]
    Unsupported(String),
    Failed(String),
}

pub trait SystemEventReader: Send {
    fn read(
        &mut self,
        cursors: &BTreeMap<String, EventCursor>,
        limit: usize,
    ) -> Result<EventReadBatch, EventReaderError>;
}

#[derive(Debug, Default)]
pub struct WindowsEventLogReader;

impl SystemEventReader for WindowsEventLogReader {
    fn read(
        &mut self,
        cursors: &BTreeMap<String, EventCursor>,
        limit: usize,
    ) -> Result<EventReadBatch, EventReaderError> {
        #[cfg(windows)]
        {
            read_native_event_log(cursors, limit)
        }
        #[cfg(not(windows))]
        {
            let _ = (cursors, limit);
            Err(EventReaderError::Unsupported(
                "Windows Event Log is only available on Windows".to_string(),
            ))
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub struct FixtureEventReader {
    pub events: Vec<NormalizedSystemEvent>,
    pub error: Option<EventReaderError>,
}

#[cfg(test)]
impl SystemEventReader for FixtureEventReader {
    fn read(
        &mut self,
        cursors: &BTreeMap<String, EventCursor>,
        limit: usize,
    ) -> Result<EventReadBatch, EventReaderError> {
        if let Some(error) = self.error.clone() {
            return Err(error);
        }
        let mut events: Vec<_> = self
            .events
            .iter()
            .filter(|event| {
                cursors
                    .get(&event.channel)
                    .is_none_or(|cursor| cursor_is_after(event, cursor))
            })
            .cloned()
            .collect();
        events.sort_by(|left, right| {
            left.event_time_ms
                .cmp(&right.event_time_ms)
                .then_with(|| left.record_id.cmp(&right.record_id))
        });
        let mut next_cursors = BTreeMap::new();
        for event in events.iter().take(limit.max(1)) {
            next_cursors
                .entry(event.channel.clone())
                .and_modify(|cursor: &mut EventCursor| {
                    if cursor_is_after(event, cursor) {
                        *cursor = cursor_for(event);
                    }
                })
                .or_insert_with(|| cursor_for(event));
        }
        Ok(EventReadBatch {
            events: events.into_iter().take(limit.max(1)).collect(),
            next_cursors,
        })
    }
}

fn cursor_for(event: &NormalizedSystemEvent) -> EventCursor {
    EventCursor {
        channel: event.channel.clone(),
        record_id: event.record_id.clone(),
        event_time_ms: event.event_time_ms,
    }
}

fn cursor_is_after(event: &NormalizedSystemEvent, cursor: &EventCursor) -> bool {
    event.event_time_ms > cursor.event_time_ms
        || (event.event_time_ms == cursor.event_time_ms
            && compare_record_ids(&event.record_id, &cursor.record_id).is_gt())
}

fn compare_record_ids(left: &str, right: &str) -> std::cmp::Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CrashClassification {
    Bsod,
    UnexpectedShutdown,
    AbnormalRestart,
    #[allow(dead_code)]
    InsufficientEvidence,
}

impl CrashClassification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bsod => "bsod",
            Self::UnexpectedShutdown => "unexpected_shutdown",
            Self::AbnormalRestart => "abnormal_restart",
            Self::InsufficientEvidence => "insufficient_evidence",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashCandidate {
    pub classification: CrashClassification,
    pub anchor_time_ms: i64,
    pub primary_event: NormalizedSystemEvent,
    /// Stable reboot-episode identity when objective boot/session evidence is available. An
    /// unresolved candidate is retained only for backwards-compatible exact-anchor matching.
    pub episode_key: Option<String>,
    anchor_quality: u8,
}

#[allow(dead_code)]
pub fn classify_events(events: &[NormalizedSystemEvent]) -> Vec<CrashCandidate> {
    classify_events_with_context(events, None)
}

fn classify_events_with_context(
    events: &[NormalizedSystemEvent],
    fallback_anchor_time_ms: Option<i64>,
) -> Vec<CrashCandidate> {
    classify_events_with_episode_context(
        events,
        &CrashClassificationContext {
            legacy_fallback_anchor_time_ms: fallback_anchor_time_ms,
            ..CrashClassificationContext::default()
        },
    )
}

#[derive(Debug, Clone, Default)]
struct CrashClassificationContext {
    // Used only by the direct classification compatibility helper. Production persistence loads
    // episode-scoped collection boundaries instead of supplying one batch-global fallback.
    legacy_fallback_anchor_time_ms: Option<i64>,
    collection_boundary_times_ms: Vec<i64>,
    boot_sessions: Vec<BootSessionContext>,
}

#[derive(Debug, Clone)]
struct BootSessionContext {
    id: i64,
    boot_time_ms: Option<i64>,
    observed_start_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RebootEpisode {
    key: Option<String>,
    boot_time_ms: Option<i64>,
    boot_session_id: Option<i64>,
}

fn classify_events_with_episode_context(
    events: &[NormalizedSystemEvent],
    context: &CrashClassificationContext,
) -> Vec<CrashCandidate> {
    let mut ordered = events.to_vec();
    ordered.sort_by(|left, right| {
        left.event_time_ms
            .cmp(&right.event_time_ms)
            .then_with(|| compare_record_ids(&left.record_id, &right.record_id))
    });
    let mut candidates = Vec::new();
    for boundary in ordered.iter().filter(|event| {
        matches!(
            event.kind,
            SystemEventKind::BugCheck
                | SystemEventKind::UnexpectedShutdown
                | SystemEventKind::AbnormalRestart
        )
    }) {
        let episode = resolve_reboot_episode(&ordered, boundary, context);
        let Some((anchor_time_ms, anchor_quality)) =
            incident_anchor_for_boundary(&ordered, boundary, &episode, context)
        else {
            continue;
        };
        let boot_time_ms = ordered
            .iter()
            .filter(|event| {
                event.kind == SystemEventKind::NormalBoot
                    && event.event_time_ms >= anchor_time_ms
                    && within_time(event.event_time_ms, anchor_time_ms, CRASH_REBOOT_MATCH_MS)
            })
            .min_by_key(|event| time_distance(event.event_time_ms, anchor_time_ms))
            .map(|event| event.event_time_ms);
        let clean_boundary = ordered.iter().any(|event| {
            event.kind == SystemEventKind::NormalShutdown
                && event.event_time_ms >= anchor_time_ms
                && event.event_time_ms
                    <= boot_time_ms.unwrap_or(boundary.event_time_ms.max(anchor_time_ms))
        });
        if clean_boundary {
            continue;
        }
        let classification = if boundary.kind == SystemEventKind::BugCheck
            || has_nonzero_bugcheck_code(&boundary.payload)
        {
            CrashClassification::Bsod
        } else if boundary.kind == SystemEventKind::UnexpectedShutdown {
            CrashClassification::UnexpectedShutdown
        } else {
            CrashClassification::AbnormalRestart
        };
        let candidate = CrashCandidate {
            classification,
            anchor_time_ms,
            primary_event: boundary.clone(),
            episode_key: episode.key,
            anchor_quality,
        };
        if let Some(index) = candidates
            .iter()
            .position(|existing: &CrashCandidate| same_candidate_episode(existing, &candidate))
        {
            let existing = &candidates[index];
            let replace = classification_rank(candidate.classification)
                > classification_rank(existing.classification)
                || (classification_rank(candidate.classification)
                    == classification_rank(existing.classification)
                    && candidate.anchor_quality > existing.anchor_quality);
            if replace {
                candidates[index] = candidate;
            }
        } else {
            candidates.push(candidate);
        }
    }
    candidates
}

fn same_candidate_episode(left: &CrashCandidate, right: &CrashCandidate) -> bool {
    match (&left.episode_key, &right.episode_key) {
        (Some(left), Some(right)) => left == right,
        // Preserve PR-05's conservative compatibility behavior only when no objective episode
        // identity exists. Production candidates with boot/session evidence never use anchor
        // proximity as identity.
        (None, None) => within_time(
            left.anchor_time_ms,
            right.anchor_time_ms,
            CRASH_PHYSICAL_ANCHOR_TOLERANCE_MS,
        ),
        _ => false,
    }
}

fn resolve_reboot_episode(
    ordered: &[NormalizedSystemEvent],
    boundary: &NormalizedSystemEvent,
    context: &CrashClassificationContext,
) -> RebootEpisode {
    let boot = episode_boot_for_boundary(ordered, boundary);
    let boot_time_ms = boot.map(|event| event.event_time_ms);
    let boot_session = match boot_time_ms {
        Some(time_ms) => context.boot_session_near(time_ms),
        None => context.boot_session_near(boundary.event_time_ms),
    };
    let boot_session_id = boot_session.map(|session| session.id);
    let key = boot_session_id
        .map(|id| format!("boot-session:{id}"))
        .or_else(|| {
            boundary
                .payload
                .boot_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("boot-id:{value}"))
        })
        .or_else(|| boot.map(|event| format!("boot-event:{}:{}", event.channel, event.record_id)));
    RebootEpisode {
        key,
        boot_time_ms: boot_time_ms.or_else(|| boot_session.and_then(boot_session_time)),
        boot_session_id,
    }
}

impl CrashClassificationContext {
    fn boot_session_near(&self, target_time_ms: i64) -> Option<&BootSessionContext> {
        self.boot_sessions
            .iter()
            .filter_map(|session| {
                let distance = boot_session_time_distance(session, target_time_ms)?;
                (distance <= CRASH_BOOT_EPISODE_MATCH_MS as u64).then_some((distance, session))
            })
            .min_by_key(|(distance, session)| (*distance, session.id))
            .map(|(_, session)| session)
    }
}

fn boot_session_time_distance(session: &BootSessionContext, target_time_ms: i64) -> Option<u64> {
    [session.boot_time_ms, session.observed_start_ms]
        .into_iter()
        .flatten()
        .map(|time_ms| time_distance(time_ms, target_time_ms))
        .min()
}

fn boot_session_time(session: &BootSessionContext) -> Option<i64> {
    session.boot_time_ms.or(session.observed_start_ms)
}

fn incident_anchor_for_boundary(
    ordered: &[NormalizedSystemEvent],
    boundary: &NormalizedSystemEvent,
    episode: &RebootEpisode,
    context: &CrashClassificationContext,
) -> Option<(i64, u8)> {
    if let Some(anchor) = boundary
        .payload
        .previous_shutdown_time_ms
        .filter(|anchor| valid_physical_anchor(*anchor, boundary.event_time_ms))
    {
        return Some((anchor, 3));
    }

    // A later WER record often has no physical timestamp of its own. Only use an authoritative
    // timestamp from another event after resolving that source event to the same episode.
    if let Some(anchor) = ordered
        .iter()
        .filter_map(|event| {
            let anchor = event.payload.previous_shutdown_time_ms?;
            (episode.key.is_some()
                && valid_physical_anchor(anchor, event.event_time_ms)
                && resolve_reboot_episode(ordered, event, context).key == episode.key)
                .then_some(anchor)
        })
        .min_by_key(|anchor| time_distance(*anchor, boundary.event_time_ms))
    {
        return Some((anchor, 3));
    }

    if let Some(anchor) =
        collection_boundary_anchor_for_episode(ordered, boundary, episode, context)
    {
        return Some((anchor, 2));
    }

    // This branch exists only for the direct classification compatibility helper. Production
    // persistence passes no batch-global fallback and therefore always falls back per episode.
    if episode.key.is_none() {
        if let Some(anchor) = context.legacy_fallback_anchor_time_ms.filter(|anchor| {
            within_time(
                *anchor,
                boundary.event_time_ms,
                CRASH_INCIDENT_CORRELATION_MS,
            )
        }) {
            return Some((anchor, 1));
        }
    }

    episode.boot_time_ms.map(|time_ms| (time_ms, 1))
}

fn valid_physical_anchor(anchor_time_ms: i64, event_time_ms: i64) -> bool {
    anchor_time_ms <= event_time_ms
        && event_time_ms.saturating_sub(anchor_time_ms) <= CRASH_PHYSICAL_ANCHOR_HORIZON_MS
}

fn collection_boundary_anchor_for_episode(
    ordered: &[NormalizedSystemEvent],
    boundary: &NormalizedSystemEvent,
    episode: &RebootEpisode,
    context: &CrashClassificationContext,
) -> Option<i64> {
    let boot_time_ms = episode.boot_time_ms?;
    context
        .collection_boundary_times_ms
        .iter()
        .copied()
        .filter(|candidate| {
            *candidate < boot_time_ms
                && within_time(
                    *candidate,
                    boundary.event_time_ms,
                    CRASH_INCIDENT_CORRELATION_MS,
                )
                && within_time(*candidate, boot_time_ms, CRASH_INCIDENT_CORRELATION_MS)
                && !has_intervening_boot(ordered, *candidate, boot_time_ms, episode, context)
        })
        .max()
}

fn has_intervening_boot(
    ordered: &[NormalizedSystemEvent],
    boundary_time_ms: i64,
    boot_time_ms: i64,
    episode: &RebootEpisode,
    context: &CrashClassificationContext,
) -> bool {
    ordered.iter().any(|event| {
        event.kind == SystemEventKind::NormalBoot
            && event.event_time_ms > boundary_time_ms
            && event.event_time_ms < boot_time_ms
    }) || context.boot_sessions.iter().any(|session| {
        Some(session.id) != episode.boot_session_id
            && boot_session_time(session)
                .is_some_and(|time_ms| time_ms > boundary_time_ms && time_ms < boot_time_ms)
    })
}

fn episode_boot_for_boundary<'a>(
    ordered: &'a [NormalizedSystemEvent],
    boundary: &NormalizedSystemEvent,
) -> Option<&'a NormalizedSystemEvent> {
    let past = ordered
        .iter()
        .filter(|event| {
            event.kind == SystemEventKind::NormalBoot
                && event.event_time_ms <= boundary.event_time_ms
                && within_time(
                    event.event_time_ms,
                    boundary.event_time_ms,
                    CRASH_BOOT_EPISODE_MATCH_MS,
                )
        })
        .max_by_key(|event| (event.event_time_ms, event.record_id.as_str()));
    let future = ordered
        .iter()
        .filter(|event| {
            event.kind == SystemEventKind::NormalBoot
                && event.event_time_ms > boundary.event_time_ms
                && within_time(
                    event.event_time_ms,
                    boundary.event_time_ms,
                    CRASH_BOOT_EPISODE_MATCH_MS,
                )
        })
        .min_by_key(|event| (event.event_time_ms, event.record_id.as_str()));
    [past, future].into_iter().flatten().min_by_key(|event| {
        (
            time_distance(event.event_time_ms, boundary.event_time_ms),
            event.record_id.as_str(),
        )
    })
}

fn has_nonzero_bugcheck_code(payload: &EventPayloadFacts) -> bool {
    let Some(value) = payload.bugcheck_code.as_deref() else {
        return false;
    };
    let value = value.trim();
    let parsed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map(|value| u64::from_str_radix(value, 16))
        .unwrap_or_else(|| value.parse::<u64>());
    parsed.is_ok_and(|value| value != 0)
}

fn classification_rank(classification: CrashClassification) -> u8 {
    match classification {
        CrashClassification::AbnormalRestart => 1,
        CrashClassification::UnexpectedShutdown => 2,
        CrashClassification::Bsod => 3,
        CrashClassification::InsufficientEvidence => 0,
    }
}

fn time_distance(left: i64, right: i64) -> u64 {
    left.abs_diff(right)
}

fn within_time(left: i64, right: i64, window_ms: i64) -> bool {
    time_distance(left, right) <= window_ms.max(0) as u64
}

/// Creates the stable identity for a reboot episode. Classification and physical anchor evidence
/// are intentionally absent; later observations can refine both without changing this key. An
/// unresolved candidate retains the old exact-anchor compatibility form because no safe episode
/// identity exists to use in that case.
pub fn stable_crash_key(candidate: &CrashCandidate) -> String {
    candidate.episode_key.as_deref().map_or_else(
        || format!("incident:unresolved:{}", candidate.anchor_time_ms),
        |episode_key| format!("incident:episode:{episode_key}"),
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashDetectorStatus {
    pub state: String,
    pub last_successful_scan_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectorState {
    Idle,
    Scanning,
    Ready,
    PermissionDenied,
    Failed,
}

impl DetectorState {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Scanning => "scanning",
            Self::Ready => "ready",
            Self::PermissionDenied => "permission_denied",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
struct DetectorStatusInner {
    state: DetectorState,
    last_successful_scan_at_ms: Option<i64>,
    last_error: Option<String>,
}

#[derive(Clone)]
pub struct CrashDetectorHandle {
    status: Arc<Mutex<DetectorStatusInner>>,
}

impl CrashDetectorHandle {
    pub fn start(db: Arc<Database>) -> Self {
        let status = Arc::new(Mutex::new(DetectorStatusInner {
            state: DetectorState::Idle,
            last_successful_scan_at_ms: None,
            last_error: None,
        }));
        let thread_status = status.clone();
        thread::Builder::new()
            .name("crash-detector".to_string())
            .spawn(move || {
                let mut reader = WindowsEventLogReader;
                loop {
                    if let Ok(mut value) = thread_status.lock() {
                        value.state = DetectorState::Scanning;
                        value.last_error = None;
                    }
                    let now = now_ms();
                    let result = scan_until_caught_up(&db, &mut reader, now).and_then(|_| {
                        rebuild_due_cases(&db, now)
                            .map(|_| ())
                            .map_err(|error| EventReaderError::Failed(error.to_string()))
                    });
                    if let Ok(mut value) = thread_status.lock() {
                        match result {
                            Ok(()) => {
                                value.state = DetectorState::Ready;
                                value.last_successful_scan_at_ms = Some(now);
                                value.last_error = None;
                            }
                            Err(EventReaderError::PermissionDenied(error)) => {
                                value.state = DetectorState::PermissionDenied;
                                value.last_error = Some(error);
                            }
                            Err(error) => {
                                value.state = DetectorState::Failed;
                                value.last_error = Some(format_event_reader_error(&error));
                            }
                        }
                    }
                    thread::sleep(std::time::Duration::from_millis(CRASH_DETECTOR_POLL_MS));
                }
            })
            .expect("crash detector thread should start");
        Self { status }
    }

    pub fn status(&self) -> CrashDetectorStatus {
        let value = self.status.lock().expect("crash detector status poisoned");
        CrashDetectorStatus {
            state: value.state.as_str().to_string(),
            last_successful_scan_at_ms: value.last_successful_scan_at_ms,
            last_error: value.last_error.clone(),
        }
    }
}

#[allow(dead_code)]
pub fn scan_once(
    db: &Database,
    reader: &mut dyn SystemEventReader,
    now_ms: i64,
) -> Result<(), EventReaderError> {
    scan_page(db, reader, now_ms).map(|_| ())
}

fn scan_page(
    db: &Database,
    reader: &mut dyn SystemEventReader,
    now_ms: i64,
) -> Result<bool, EventReaderError> {
    let cursors = db
        .read(load_cursors)
        .map_err(|error| EventReaderError::Failed(error.to_string()))?;
    let batch = reader.read(&cursors, EVENT_SCAN_BATCH_SIZE)?;
    if batch.events.is_empty() {
        return Ok(false);
    }
    let case_ids = db
        .with_writer(|conn| persist_event_batch(conn, &batch, now_ms))
        .map_err(|error| EventReaderError::Failed(error.to_string()))?;
    for case_id in case_ids {
        if let Err(error) = build_case_with_failure_status_at(db, case_id, now_ms) {
            eprintln!("crash evidence build failed for case {case_id}: {error}");
        }
    }
    Ok(true)
}

fn scan_until_caught_up(
    db: &Database,
    reader: &mut dyn SystemEventReader,
    now_ms: i64,
) -> Result<usize, EventReaderError> {
    let mut pages = 0usize;
    let mut pages_since_yield = 0usize;
    loop {
        if !scan_page(db, reader, now_ms)? {
            return Ok(pages);
        }
        pages = pages.saturating_add(1);
        pages_since_yield = pages_since_yield.saturating_add(1);
        if pages_since_yield >= CRASH_SCAN_YIELD_PAGES {
            pages_since_yield = 0;
            thread::yield_now();
        }
    }
}

fn rebuild_due_cases(db: &Database, as_of_ms: i64) -> rusqlite::Result<usize> {
    let case_ids = db.read(|conn| {
        let mut statement = conn.prepare(
            "SELECT id FROM crash_case
             WHERE window_end_ms <= ?1
               AND (
                    evidence_status IN ('pending', 'post_pending')
                    OR (
                        evidence_status = 'partial'
                        AND ?1 <= window_end_ms + ?2
                    )
               )
             ORDER BY window_end_ms, id",
        )?;
        let ids = statement
            .query_map(
                params![as_of_ms, CRASH_POST_FINALIZATION_STABILIZATION_MS],
                |row| row.get::<_, i64>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>();
        ids
    })?;
    let mut rebuilt = 0;
    for case_id in case_ids {
        build_case_with_failure_status_at(db, case_id, as_of_ms)?;
        rebuilt += 1;
    }
    Ok(rebuilt)
}

fn persist_event_batch(
    conn: &Connection,
    batch: &EventReadBatch,
    now_ms: i64,
) -> rusqlite::Result<Vec<i64>> {
    let tx = conn.unchecked_transaction()?;
    for event in &batch.events {
        tx.execute(
            "INSERT OR IGNORE INTO system_event(channel, event_id, record_id, event_time_ms, provider, payload_summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                event.channel,
                event.event_id,
                event.record_id,
                event.event_time_ms,
                event.provider,
                event.payload_summary(),
            ],
        )?;
    }
    let context_start_ms = batch
        .events
        .iter()
        .map(|event| event.event_time_ms)
        .min()
        .unwrap_or(now_ms)
        .saturating_sub(CRASH_CASE_WINDOW_PRE_MS);
    let context_end_ms = batch
        .events
        .iter()
        .map(|event| event.event_time_ms)
        .max()
        .unwrap_or(now_ms)
        .saturating_add(CRASH_INCIDENT_CORRELATION_MS);
    let mut persisted_events = load_events_in_range(&tx, context_start_ms, context_end_ms)?;
    let classification_context =
        load_crash_classification_context(&tx, context_start_ms, context_end_ms)?;
    let candidates =
        classify_events_with_episode_context(&persisted_events, &classification_context);
    let mut case_ids = Vec::new();
    for candidate in candidates {
        case_ids.push(create_case_and_hold_tx(&tx, &candidate, now_ms)?);
    }
    save_cursors_tx(&tx, &batch.next_cursors)?;
    tx.commit()?;
    persisted_events.clear();
    case_ids.sort_unstable();
    case_ids.dedup();
    Ok(case_ids)
}

fn load_crash_classification_context(
    conn: &Connection,
    context_start_ms: i64,
    context_end_ms: i64,
) -> rusqlite::Result<CrashClassificationContext> {
    let boot_sessions = conn
        .prepare(
            "SELECT id, boot_time_ms, observed_start_ms
             FROM boot_session
             ORDER BY COALESCE(boot_time_ms, observed_start_ms), id",
        )?
        .query_map([], |row| {
            Ok(BootSessionContext {
                id: row.get(0)?,
                boot_time_ms: row.get(1)?,
                observed_start_ms: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let current_boot_session_id: Option<i64> = conn
        .query_row(
            "SELECT CAST(value AS INTEGER)
             FROM settings WHERE key = 'runtime_boot_session_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let mut boundary_times_ms = if let Some(current_boot_session_id) = current_boot_session_id {
        let mut statement = conn.prepare(
            "SELECT f.ts + f.duration_ms
             FROM sample_frame f
             JOIN collection_session cs ON cs.id = f.collection_session_id
             WHERE cs.boot_session_id <> ?1
               AND f.ts < ?2
               AND f.ts + f.duration_ms >= ?3
             UNION ALL
             SELECT bs.observed_end_ms
             FROM boot_session bs
             WHERE bs.id <> ?1
               AND bs.observed_end_ms IS NOT NULL
               AND bs.observed_end_ms <= ?2
               AND bs.observed_end_ms >= ?3",
        )?;
        let values = statement
            .query_map(
                params![current_boot_session_id, context_end_ms, context_start_ms],
                |row| row.get::<_, Option<i64>>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
    } else {
        let mut statement = conn.prepare(
            "SELECT f.ts + f.duration_ms
             FROM sample_frame f
             WHERE f.ts < ?1
               AND f.ts + f.duration_ms >= ?2
             UNION ALL
             SELECT bs.observed_end_ms
             FROM boot_session bs
             WHERE bs.observed_end_ms IS NOT NULL
               AND bs.observed_end_ms <= ?1
               AND bs.observed_end_ms >= ?2",
        )?;
        let values = statement
            .query_map(params![context_end_ms, context_start_ms], |row| {
                row.get::<_, Option<i64>>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
    };
    let mut collection_boundary_times_ms =
        boundary_times_ms.drain(..).flatten().collect::<Vec<_>>();
    collection_boundary_times_ms.sort_unstable();
    collection_boundary_times_ms.dedup();
    Ok(CrashClassificationContext {
        legacy_fallback_anchor_time_ms: None,
        collection_boundary_times_ms,
        boot_sessions,
    })
}

fn load_cursors(conn: &Connection) -> rusqlite::Result<BTreeMap<String, EventCursor>> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [EVENT_CURSOR_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

fn save_cursors_tx(
    tx: &rusqlite::Transaction<'_>,
    cursors: &BTreeMap<String, EventCursor>,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO settings(key, value, updated_at_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
        params![EVENT_CURSOR_KEY, serde_json::to_string(cursors).unwrap_or_else(|_| "{}".into()), now_ms()],
    )?;
    Ok(())
}

fn load_events_in_range(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<Vec<NormalizedSystemEvent>> {
    let mut statement = conn.prepare(
        "SELECT channel, provider, event_id, record_id, event_time_ms, payload_summary
         FROM system_event
         WHERE event_time_ms >= ?1 AND event_time_ms <= ?2
           AND (
                event_id IN ('41', '6008', '1001', '6005', '6006')
                OR LOWER(payload_summary) LIKE '%bugcheck%'
                OR LOWER(payload_summary) LIKE '%unexpected_shutdown%'
                OR LOWER(payload_summary) LIKE '%abnormal_restart%'
                OR LOWER(payload_summary) LIKE '%normal_boot%'
                OR LOWER(payload_summary) LIKE '%normal_shutdown%'
           )
         ORDER BY event_time_ms, record_id
         LIMIT ?3",
    )?;
    let result = statement
        .query_map(params![start_ms, end_ms, EVENT_CONTEXT_LIMIT], |row| {
            let payload_summary: String = row.get(5)?;
            let value: serde_json::Value = serde_json::from_str(&payload_summary)
                .unwrap_or_else(|_| serde_json::json!({"kind":"other"}));
            let kind = value
                .get("kind")
                .and_then(|value| value.as_str())
                .and_then(parse_system_event_kind)
                .unwrap_or(SystemEventKind::Other);
            Ok(NormalizedSystemEvent {
                channel: row.get(0)?,
                provider: row.get(1)?,
                event_id: row.get(2)?,
                record_id: row.get(3)?,
                event_time_ms: row.get(4)?,
                kind,
                payload: serde_json::from_value(value).unwrap_or_default(),
            })
        })?
        .collect();
    result
}

fn parse_system_event_kind(value: &str) -> Option<SystemEventKind> {
    Some(match value {
        "bugcheck" => SystemEventKind::BugCheck,
        "unexpected_shutdown" => SystemEventKind::UnexpectedShutdown,
        "abnormal_restart" => SystemEventKind::AbnormalRestart,
        "normal_boot" => SystemEventKind::NormalBoot,
        "normal_shutdown" => SystemEventKind::NormalShutdown,
        "sleep" => SystemEventKind::Sleep,
        "resume" => SystemEventKind::Resume,
        "supporting" => SystemEventKind::Supporting,
        "application_crash" => SystemEventKind::ApplicationCrash,
        "other" => SystemEventKind::Other,
        _ => return None,
    })
}

fn create_case_and_hold_tx(
    tx: &rusqlite::Transaction<'_>,
    candidate: &CrashCandidate,
    now_ms: i64,
) -> rusqlite::Result<i64> {
    let window_start_ms = candidate
        .anchor_time_ms
        .saturating_sub(CRASH_CASE_WINDOW_PRE_MS);
    let window_end_ms = candidate
        .anchor_time_ms
        .saturating_add(CRASH_CASE_WINDOW_POST_MS);
    let stable_key = stable_crash_key(candidate);
    let existing: Option<(i64, String, i64, String)> = if candidate.episode_key.is_some() {
        tx.query_row(
            "SELECT id, stable_key, anchor_time_ms, classification
             FROM crash_case
             WHERE stable_key = ?1
             LIMIT 1",
            [&stable_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
    } else {
        // Older unresolved fixtures may not have any boot/session identity. Keep their narrow
        // exact-anchor compatibility path, but never apply it to an episode-resolved candidate.
        tx.query_row(
            "SELECT id, stable_key, anchor_time_ms, classification
             FROM crash_case
             WHERE stable_key = ?1
                OR (stable_key LIKE 'incident:unresolved:%'
                    AND ABS(anchor_time_ms - ?2) <= ?3)
             ORDER BY CASE WHEN stable_key = ?1 THEN 0 ELSE 1 END,
                      ABS(anchor_time_ms - ?2), id
             LIMIT 1",
            params![
                stable_key,
                candidate.anchor_time_ms,
                CRASH_PHYSICAL_ANCHOR_TOLERANCE_MS,
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
    };
    let case_id = if let Some((case_id, _stable_key, _old_anchor, old_classification)) = existing {
        let old_classification = parse_crash_classification(&old_classification);
        let classification = if classification_rank(candidate.classification)
            > classification_rank(old_classification)
        {
            candidate.classification.as_str()
        } else {
            old_classification.as_str()
        };
        tx.execute(
            "UPDATE crash_case
             SET anchor_time_ms = ?1,
                 classification = ?2,
                 window_start_ms = ?3,
                 window_end_ms = ?4,
                 evidence_status = 'pending',
                 processing_version = ?5
             WHERE id = ?6",
            params![
                candidate.anchor_time_ms,
                classification,
                window_start_ms,
                window_end_ms,
                CRASH_EVIDENCE_PROCESSING_VERSION,
                case_id,
            ],
        )?;
        case_id
    } else {
        tx.execute(
            "INSERT INTO crash_case(
                 stable_key, anchor_time_ms, classification, window_start_ms, window_end_ms,
                 evidence_status, processing_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
            params![
                stable_key,
                candidate.anchor_time_ms,
                candidate.classification.as_str(),
                window_start_ms,
                window_end_ms,
                CRASH_EVIDENCE_PROCESSING_VERSION,
            ],
        )?;
        tx.last_insert_rowid()
    };
    tx.execute(
        "UPDATE retention_hold
        SET start_ms = ?1, end_ms = ?2
         WHERE crash_case_id = ?3 AND released_at_ms IS NULL",
        params![window_start_ms, window_end_ms, case_id],
    )?;
    tx.execute(
        "INSERT INTO retention_hold(crash_case_id, start_ms, end_ms)
         SELECT ?1, ?2, ?3
         WHERE NOT EXISTS (
             SELECT 1 FROM retention_hold
             WHERE crash_case_id = ?1 AND released_at_ms IS NULL
         )",
        params![case_id, window_start_ms, window_end_ms],
    )?;
    let mut statement = tx.prepare(
        "SELECT h.id FROM retention_hold h JOIN crash_case c ON c.id=h.crash_case_id
         WHERE h.released_at_ms IS NULL ORDER BY c.anchor_time_ms DESC, h.id DESC",
    )?;
    let old_holds: Vec<i64> = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(statement);
    for hold_id in old_holds.into_iter().skip(CRASH_RETENTION_CASE_LIMIT) {
        tx.execute(
            "UPDATE retention_hold SET released_at_ms = COALESCE(released_at_ms, ?1) WHERE id = ?2",
            params![now_ms, hold_id],
        )?;
    }
    Ok(case_id)
}

fn parse_crash_classification(value: &str) -> CrashClassification {
    match value {
        "bsod" => CrashClassification::Bsod,
        "unexpected_shutdown" => CrashClassification::UnexpectedShutdown,
        "abnormal_restart" => CrashClassification::AbnormalRestart,
        _ => CrashClassification::InsufficientEvidence,
    }
}

pub fn release_case_hold(
    conn: &Connection,
    case_id: i64,
    released_at_ms: i64,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE retention_hold SET released_at_ms = COALESCE(released_at_ms, ?1)
         WHERE crash_case_id = ?2 AND released_at_ms IS NULL",
        params![released_at_ms, case_id],
    )
}

/// The evidence windows are deliberately named product concepts.  They are also part of the
/// persisted summary key because the v8 schema uniquely identifies a summary by `(case_id,
/// metric_key)` and has no separate window column in that constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CrashEvidenceWindow {
    #[serde(rename = "pre_1m")]
    Pre1m,
    #[serde(rename = "pre_5m")]
    Pre5m,
    #[serde(rename = "pre_30m")]
    Pre30m,
    #[serde(rename = "post_5m")]
    Post5m,
}

impl CrashEvidenceWindow {
    pub const ALL: [Self; 4] = [Self::Pre1m, Self::Pre5m, Self::Pre30m, Self::Post5m];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pre1m => "pre_1m",
            Self::Pre5m => "pre_5m",
            Self::Pre30m => "pre_30m",
            Self::Post5m => "post_5m",
        }
    }

    const fn duration_ms(self) -> i64 {
        match self {
            Self::Pre1m => 60_000,
            Self::Pre5m => 5 * 60_000,
            Self::Pre30m => CRASH_CASE_WINDOW_PRE_MS,
            Self::Post5m => CRASH_CASE_WINDOW_POST_MS,
        }
    }

    fn bounds(self, anchor_time_ms: i64) -> (i64, i64) {
        match self {
            Self::Pre1m | Self::Pre5m | Self::Pre30m => (
                anchor_time_ms.saturating_sub(self.duration_ms()),
                anchor_time_ms,
            ),
            Self::Post5m => (
                anchor_time_ms,
                anchor_time_ms.saturating_add(self.duration_ms()),
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashCaseSummary {
    pub id: i64,
    pub stable_key: String,
    pub anchor_time_ms: i64,
    pub classification: String,
    pub window_start_ms: i64,
    pub window_end_ms: i64,
    pub evidence_status: String,
    pub processing_version: String,
    pub has_active_hold: bool,
    pub summary_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashSystemEvent {
    pub id: i64,
    pub channel: String,
    pub provider: Option<String>,
    pub event_id: String,
    pub record_id: String,
    pub event_time_ms: i64,
    pub kind: String,
    pub bugcheck_code: Option<String>,
    pub boot_id: Option<String>,
    pub previous_shutdown_time_ms: Option<i64>,
    pub clean_shutdown: Option<bool>,
    pub restart_boundary: Option<bool>,
    pub dump_available: Option<bool>,
    pub dump_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashEvidenceMetric {
    pub metric_key: String,
    pub metric: String,
    pub window: CrashEvidenceWindow,
    pub device_key: Option<String>,
    pub process_identity_key: Option<String>,
    pub window_start_ms: i64,
    pub window_end_ms: i64,
    pub avg: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub delta: Option<f64>,
    pub peak_time_ms: Option<i64>,
    pub sample_count: i64,
    pub coverage: f64,
    pub evidence_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashEvidenceProcessEntry {
    pub window: CrashEvidenceWindow,
    pub process_identity_key: String,
    pub app_key: String,
    pub process_name: String,
    pub pid: Option<u32>,
    pub process_creation_time_ms: Option<i64>,
    pub cpu_avg_percent: Option<f64>,
    pub cpu_peak_percent: Option<f64>,
    pub cpu_delta_percent: Option<f64>,
    pub memory_peak_bytes: Option<i64>,
    pub memory_delta_bytes: Option<i64>,
    pub read_bytes: i64,
    pub write_bytes: i64,
    pub selection_reason_mask: i64,
    pub coverage: f64,
    pub sample_count: i64,
    pub cpu_rank: Option<usize>,
    pub memory_rank: Option<usize>,
    pub io_rank: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashEvidenceDetail {
    pub case: CrashCaseSummary,
    pub events: Vec<CrashSystemEvent>,
    pub metrics: Vec<CrashEvidenceMetric>,
    pub processes: Vec<CrashEvidenceProcessEntry>,
}

#[derive(Debug, Clone)]
struct CrashCaseRow {
    id: i64,
    stable_key: String,
    anchor_time_ms: i64,
    classification: String,
    window_start_ms: i64,
    window_end_ms: i64,
    evidence_status: String,
    processing_version: String,
}

impl CrashCaseRow {
    fn summary(&self, conn: &Connection) -> rusqlite::Result<CrashCaseSummary> {
        let has_active_hold: i64 = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM retention_hold WHERE crash_case_id = ?1 AND released_at_ms IS NULL)",
            [self.id],
            |row| row.get(0),
        )?;
        let summary_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM crash_evidence_summary WHERE crash_case_id = ?1",
            [self.id],
            |row| row.get(0),
        )?;
        Ok(CrashCaseSummary {
            id: self.id,
            stable_key: self.stable_key.clone(),
            anchor_time_ms: self.anchor_time_ms,
            classification: self.classification.clone(),
            window_start_ms: self.window_start_ms,
            window_end_ms: self.window_end_ms,
            evidence_status: self.evidence_status.clone(),
            processing_version: self.processing_version.clone(),
            has_active_hold: has_active_hold != 0,
            summary_count,
        })
    }
}

#[derive(Debug, Clone)]
struct EvidenceSummaryRow {
    metric_key: String,
    start_ms: i64,
    end_ms: i64,
    avg: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    delta: Option<f64>,
    peak_time_ms: Option<i64>,
    coverage: f64,
    evidence_ref: String,
}

#[derive(Debug, Clone, Copy)]
struct NumericObservation {
    time_ms: i64,
    value: f64,
    duration_ms: i64,
}

#[derive(Debug, Clone, Default)]
struct NumericAccumulator {
    weighted_sum: f64,
    covered_ms: i64,
    sample_count: i64,
    min: Option<f64>,
    max: Option<f64>,
    peak_time_ms: Option<i64>,
    first: Option<NumericObservation>,
    last: Option<NumericObservation>,
}

impl NumericAccumulator {
    fn add(&mut self, observation: NumericObservation) {
        if !observation.value.is_finite() || observation.duration_ms <= 0 {
            return;
        }
        self.weighted_sum += observation.value * observation.duration_ms as f64;
        self.covered_ms = self.covered_ms.saturating_add(observation.duration_ms);
        self.sample_count = self.sample_count.saturating_add(1);
        self.min = Some(
            self.min
                .map_or(observation.value, |value| value.min(observation.value)),
        );
        if self.max.is_none_or(|value| observation.value > value) {
            self.max = Some(observation.value);
            self.peak_time_ms = Some(observation.time_ms);
        }
        if self.first.is_none() {
            self.first = Some(observation);
        }
        self.last = Some(observation);
    }

    fn avg(&self) -> Option<f64> {
        (self.covered_ms > 0).then_some(self.weighted_sum / self.covered_ms as f64)
    }

    fn delta(&self) -> Option<f64> {
        Some(self.last?.value - self.first?.value)
    }

    fn coverage(&self, window_ms: i64) -> f64 {
        if window_ms <= 0 {
            return 0.0;
        }
        (self.covered_ms as f64 / window_ms as f64).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone)]
struct ProcessEvidenceAggregate {
    process_identity_key: String,
    app_key: String,
    process_name: String,
    pid: Option<u32>,
    process_creation_time_ms: Option<i64>,
    cpu: NumericAccumulator,
    memory: NumericAccumulator,
    read: NumericAccumulator,
    write: NumericAccumulator,
    observed_ms: i64,
    sample_count: i64,
    selection_reason_mask: i64,
    read_bytes: i64,
    write_bytes: i64,
}

impl ProcessEvidenceAggregate {
    fn new(
        process_identity_key: String,
        app_key: String,
        process_name: String,
        pid: Option<u32>,
        process_creation_time_ms: Option<i64>,
    ) -> Self {
        Self {
            process_identity_key,
            app_key,
            process_name,
            pid,
            process_creation_time_ms,
            cpu: NumericAccumulator::default(),
            memory: NumericAccumulator::default(),
            read: NumericAccumulator::default(),
            write: NumericAccumulator::default(),
            observed_ms: 0,
            sample_count: 0,
            selection_reason_mask: 0,
            read_bytes: 0,
            write_bytes: 0,
        }
    }

    fn coverage(&self, window_ms: i64) -> f64 {
        if window_ms <= 0 {
            return 0.0;
        }
        (self.observed_ms as f64 / window_ms as f64).clamp(0.0, 1.0)
    }
}

const MAX_EVIDENCE_OBSERVATION_MS: i64 = 15_000;

fn observation_duration(
    timestamp_ms: i64,
    duration_ms: i64,
    next_timestamp_ms: Option<i64>,
    start_ms: i64,
    end_ms: i64,
) -> i64 {
    if duration_ms <= 0 || end_ms <= start_ms {
        return 0;
    }
    let mut observed_end = timestamp_ms.saturating_add(duration_ms.max(1));
    if let Some(next_timestamp_ms) = next_timestamp_ms.filter(|value| *value > timestamp_ms) {
        let gap_ms = next_timestamp_ms.saturating_sub(timestamp_ms);
        observed_end = if gap_ms <= MAX_EVIDENCE_OBSERVATION_MS || gap_ms == duration_ms.max(1) {
            observed_end.min(next_timestamp_ms)
        } else {
            observed_end.min(timestamp_ms.saturating_add(MAX_EVIDENCE_OBSERVATION_MS))
        };
    }
    observed_end
        .min(end_ms)
        .saturating_sub(timestamp_ms.max(start_ms))
        .max(0)
}

fn summary_key(
    window: CrashEvidenceWindow,
    metric: &str,
    device_key: Option<&str>,
    process_identity_key: Option<&str>,
) -> String {
    let mut key = format!("window:{}:metric:{}", window.as_str(), metric);
    if let Some(device_key) = device_key {
        key.push_str(":device:");
        key.push_str(device_key);
    }
    if let Some(process_identity_key) = process_identity_key {
        key.push_str(":process:");
        key.push_str(process_identity_key);
    }
    key
}

fn evidence_ref(
    source: &str,
    window: CrashEvidenceWindow,
    metric: &str,
    sample_count: i64,
    device_key: Option<&str>,
    process_identity_key: Option<&str>,
) -> String {
    serde_json::json!({
        "source": source,
        "window": window.as_str(),
        "metric": metric,
        "sample_count": sample_count,
        "device_key": device_key,
        "process_identity_key": process_identity_key,
    })
    .to_string()
}

#[allow(clippy::too_many_arguments)]
fn summary_from_accumulator(
    window: CrashEvidenceWindow,
    metric: &str,
    device_key: Option<String>,
    process_identity_key: Option<String>,
    start_ms: i64,
    end_ms: i64,
    accumulator: NumericAccumulator,
    source: &str,
) -> EvidenceSummaryRow {
    let sample_count = accumulator.sample_count;
    EvidenceSummaryRow {
        metric_key: summary_key(
            window,
            metric,
            device_key.as_deref(),
            process_identity_key.as_deref(),
        ),
        start_ms,
        end_ms,
        avg: accumulator.avg(),
        min: accumulator.min,
        max: accumulator.max,
        delta: accumulator.delta(),
        peak_time_ms: accumulator.peak_time_ms,
        coverage: accumulator.coverage(end_ms.saturating_sub(start_ms)),
        evidence_ref: evidence_ref(
            source,
            window,
            metric,
            sample_count,
            device_key.as_deref(),
            process_identity_key.as_deref(),
        ),
    }
}

fn load_scalar_metric(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    expression: &str,
    joins: &str,
) -> rusqlite::Result<NumericAccumulator> {
    let sql = format!(
        r#"WITH ordered AS (
            SELECT f.ts, f.duration_ms, {expression} AS value,
                   LEAD(f.ts) OVER (ORDER BY f.ts, f.id) AS next_ts
            FROM sample_frame f {joins}
            WHERE f.ts < ?2 AND f.ts + f.duration_ms > ?1
         )
         SELECT ts, duration_ms, next_ts, value FROM ordered
         WHERE value IS NOT NULL ORDER BY ts"#
    );
    let mut statement = conn.prepare(&sql)?;
    let mut accumulator = NumericAccumulator::default();
    let rows = statement.query_map(params![start_ms, end_ms], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })?;
    for row in rows {
        let (timestamp_ms, duration_ms, next_timestamp_ms, value) = row?;
        let duration_ms = observation_duration(
            timestamp_ms,
            duration_ms,
            next_timestamp_ms,
            start_ms,
            end_ms,
        );
        accumulator.add(NumericObservation {
            time_ms: timestamp_ms.max(start_ms),
            value,
            duration_ms,
        });
    }
    Ok(accumulator)
}

fn load_gpu_metrics(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
    _metric: &str,
    expression: &str,
) -> rusqlite::Result<BTreeMap<String, NumericAccumulator>> {
    let sql = format!(
        r#"WITH ordered AS (
            SELECT d.stable_key, f.ts, f.duration_ms, {expression} AS value,
                   LEAD(f.ts) OVER (PARTITION BY g.device_id ORDER BY f.ts, f.id) AS next_ts
            FROM gpu_sample g
            JOIN sample_frame f ON f.id = g.frame_id
            JOIN hardware_device d ON d.id = g.device_id
            WHERE f.ts < ?2 AND f.ts + f.duration_ms > ?1
         )
         SELECT stable_key, ts, duration_ms, next_ts, value FROM ordered
         WHERE value IS NOT NULL ORDER BY stable_key, ts"#
    );
    let mut statement = conn.prepare(&sql)?;
    let mut accumulators = BTreeMap::<String, NumericAccumulator>::new();
    let rows = statement.query_map(params![start_ms, end_ms], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;
    for row in rows {
        let (device_key, timestamp_ms, duration_ms, next_timestamp_ms, value) = row?;
        let duration_ms = observation_duration(
            timestamp_ms,
            duration_ms,
            next_timestamp_ms,
            start_ms,
            end_ms,
        );
        accumulators
            .entry(device_key)
            .or_default()
            .add(NumericObservation {
                time_ms: timestamp_ms.max(start_ms),
                value,
                duration_ms,
            });
    }
    Ok(accumulators)
}

fn load_process_evidence(
    conn: &Connection,
    start_ms: i64,
    end_ms: i64,
) -> rusqlite::Result<BTreeMap<String, ProcessEvidenceAggregate>> {
    let mut statement = conn.prepare(
        "WITH ordered AS (
            SELECT p.process_instance_id, i.stable_key, i.pid, i.create_time_ms,
                   a.stable_key AS app_key, a.process_name, f.ts, f.duration_ms,
                   LEAD(f.ts) OVER (PARTITION BY p.process_instance_id ORDER BY f.ts, f.id) AS next_ts,
                   p.cpu_pct, p.working_set_bytes, p.private_bytes, p.read_bps, p.write_bps,
                   p.selection_reason
            FROM process_sample p
            JOIN sample_frame f ON f.id = p.frame_id
            JOIN process_instance i ON i.id = p.process_instance_id
            JOIN app_executable e ON e.id = i.app_executable_id
            JOIN app a ON a.id = e.app_id
            WHERE f.ts < ?2 AND f.ts + f.duration_ms > ?1
        )
        SELECT process_instance_id, stable_key, pid, create_time_ms, app_key, process_name,
               ts, duration_ms, next_ts, cpu_pct, working_set_bytes, private_bytes,
               read_bps, write_bps, selection_reason
        FROM ordered ORDER BY stable_key, ts",
    )?;
    let rows = statement.query_map(params![start_ms, end_ms], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, Option<i64>>(8)?,
            row.get::<_, Option<f64>>(9)?,
            row.get::<_, Option<i64>>(10)?,
            row.get::<_, Option<i64>>(11)?,
            row.get::<_, Option<i64>>(12)?,
            row.get::<_, Option<i64>>(13)?,
            row.get::<_, i64>(14)?,
        ))
    })?;
    let mut aggregates = BTreeMap::<String, ProcessEvidenceAggregate>::new();
    for row in rows {
        let (
            _process_instance_id,
            process_identity_key,
            pid,
            process_creation_time_ms,
            app_key,
            process_name,
            timestamp_ms,
            frame_duration_ms,
            next_timestamp_ms,
            cpu_pct,
            working_set_bytes,
            private_bytes,
            read_bps,
            write_bps,
            selection_reason,
        ) = row?;
        let duration_ms = observation_duration(
            timestamp_ms,
            frame_duration_ms,
            next_timestamp_ms,
            start_ms,
            end_ms,
        );
        let aggregate = aggregates
            .entry(process_identity_key.clone())
            .or_insert_with(|| {
                ProcessEvidenceAggregate::new(
                    process_identity_key.clone(),
                    app_key.clone(),
                    process_name.clone(),
                    pid.and_then(|value| u32::try_from(value).ok()),
                    process_creation_time_ms,
                )
            });
        aggregate.observed_ms = aggregate.observed_ms.saturating_add(duration_ms);
        aggregate.sample_count = aggregate.sample_count.saturating_add(1);
        aggregate.selection_reason_mask |= selection_reason;
        if let Some(value) = cpu_pct {
            aggregate.cpu.add(NumericObservation {
                time_ms: timestamp_ms.max(start_ms),
                value,
                duration_ms,
            });
        }
        if let Some(value) = private_bytes.or(working_set_bytes) {
            aggregate.memory.add(NumericObservation {
                time_ms: timestamp_ms.max(start_ms),
                value: value as f64,
                duration_ms,
            });
        }
        if let Some(value) = read_bps {
            aggregate.read.add(NumericObservation {
                time_ms: timestamp_ms.max(start_ms),
                value: value as f64,
                duration_ms,
            });
            aggregate.read_bytes = aggregate
                .read_bytes
                .saturating_add(integrate_rate(value, duration_ms));
        }
        if let Some(value) = write_bps {
            aggregate.write.add(NumericObservation {
                time_ms: timestamp_ms.max(start_ms),
                value: value as f64,
                duration_ms,
            });
            aggregate.write_bytes = aggregate
                .write_bytes
                .saturating_add(integrate_rate(value, duration_ms));
        }
    }
    Ok(aggregates)
}

fn integrate_rate(rate_per_second: i64, duration_ms: i64) -> i64 {
    ((rate_per_second as i128 * duration_ms as i128) / 1_000)
        .clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn build_window_summaries(
    conn: &Connection,
    anchor_time_ms: i64,
    window: CrashEvidenceWindow,
) -> rusqlite::Result<(
    Vec<EvidenceSummaryRow>,
    Vec<CrashEvidenceProcessEntry>,
    bool,
)> {
    let (start_ms, end_ms) = window.bounds(anchor_time_ms);
    let scalar_metrics = [
        ("cpu_percent", "cpu.usage_pct", "LEFT JOIN cpu_sample cpu ON cpu.frame_id = f.id"),
        (
            "memory_percent",
            "memory.usage_pct",
            "LEFT JOIN memory_sample memory ON memory.frame_id = f.id",
        ),
        (
            "memory_used_bytes",
            "memory.used_bytes",
            "LEFT JOIN memory_sample memory ON memory.frame_id = f.id",
        ),
        (
            "disk_read_bps",
            "disk.read_bps",
            "LEFT JOIN (SELECT frame_id, SUM(read_bps) AS read_bps, SUM(write_bps) AS write_bps FROM disk_sample GROUP BY frame_id) disk ON disk.frame_id = f.id",
        ),
        (
            "disk_write_bps",
            "disk.write_bps",
            "LEFT JOIN (SELECT frame_id, SUM(read_bps) AS read_bps, SUM(write_bps) AS write_bps FROM disk_sample GROUP BY frame_id) disk ON disk.frame_id = f.id",
        ),
        ("writer_delay_ms", "f.writer_delay_ms", ""),
    ];
    let mut summaries = Vec::new();
    for (metric, expression, joins) in scalar_metrics {
        let accumulator = load_scalar_metric(conn, start_ms, end_ms, expression, joins)?;
        if accumulator.sample_count > 0 {
            summaries.push(summary_from_accumulator(
                window,
                metric,
                None,
                None,
                start_ms,
                end_ms,
                accumulator,
                "sample_frame",
            ));
        }
    }
    let gpu_metrics = [
        ("gpu_usage_percent", "g.usage_pct"),
        ("gpu_temperature_celsius", "g.temp_c"),
        ("gpu_power_watts", "g.board_power_w"),
        ("gpu_core_clock_mhz", "g.core_clock_mhz"),
        ("gpu_memory_clock_mhz", "g.memory_clock_mhz"),
        ("gpu_vram_used_bytes", "g.vram_used_bytes"),
    ];
    for (metric, expression) in gpu_metrics {
        for (device_key, accumulator) in
            load_gpu_metrics(conn, start_ms, end_ms, metric, expression)?
        {
            if accumulator.sample_count > 0 {
                summaries.push(summary_from_accumulator(
                    window,
                    metric,
                    Some(device_key),
                    None,
                    start_ms,
                    end_ms,
                    accumulator,
                    "gpu_sample",
                ));
            }
        }
    }

    let aggregates = load_process_evidence(conn, start_ms, end_ms)?;
    let mut process_values: Vec<_> = aggregates.values().cloned().collect();
    process_values.sort_by(|left, right| {
        right
            .cpu
            .avg()
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&left.cpu.avg().unwrap_or(f64::NEG_INFINITY))
            .then_with(|| left.process_identity_key.cmp(&right.process_identity_key))
    });
    let cpu_ranks: BTreeMap<_, _> = process_values
        .iter()
        .filter(|value| value.cpu.avg().is_some())
        .enumerate()
        .map(|(index, value)| (value.process_identity_key.clone(), index + 1))
        .collect();
    process_values.sort_by(|left, right| {
        right
            .memory
            .max
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&left.memory.max.unwrap_or(f64::NEG_INFINITY))
            .then_with(|| left.process_identity_key.cmp(&right.process_identity_key))
    });
    let memory_ranks: BTreeMap<_, _> = process_values
        .iter()
        .filter(|value| value.memory.max.is_some())
        .enumerate()
        .map(|(index, value)| (value.process_identity_key.clone(), index + 1))
        .collect();
    process_values.sort_by(|left, right| {
        let left_total = left.read_bytes.saturating_add(left.write_bytes);
        let right_total = right.read_bytes.saturating_add(right.write_bytes);
        right_total
            .cmp(&left_total)
            .then_with(|| left.process_identity_key.cmp(&right.process_identity_key))
    });
    let io_ranks: BTreeMap<_, _> = process_values
        .iter()
        .filter(|value| value.read_bytes != 0 || value.write_bytes != 0)
        .enumerate()
        .map(|(index, value)| (value.process_identity_key.clone(), index + 1))
        .collect();

    let mut process_entries = Vec::new();
    for aggregate in aggregates.values() {
        let cpu_rank = cpu_ranks.get(&aggregate.process_identity_key).copied();
        let memory_rank = memory_ranks.get(&aggregate.process_identity_key).copied();
        let io_rank = io_ranks.get(&aggregate.process_identity_key).copied();
        let entry = CrashEvidenceProcessEntry {
            window,
            process_identity_key: aggregate.process_identity_key.clone(),
            app_key: aggregate.app_key.clone(),
            process_name: aggregate.process_name.clone(),
            pid: aggregate.pid,
            process_creation_time_ms: aggregate.process_creation_time_ms,
            cpu_avg_percent: aggregate.cpu.avg(),
            cpu_peak_percent: aggregate.cpu.max,
            cpu_delta_percent: aggregate.cpu.delta(),
            memory_peak_bytes: aggregate.memory.max.map(|value| value.max(0.0) as i64),
            memory_delta_bytes: aggregate.memory.delta().map(|value| value as i64),
            read_bytes: aggregate.read_bytes,
            write_bytes: aggregate.write_bytes,
            selection_reason_mask: aggregate.selection_reason_mask,
            coverage: aggregate.coverage(end_ms.saturating_sub(start_ms)),
            sample_count: aggregate.sample_count,
            cpu_rank,
            memory_rank,
            io_rank,
        };
        let ref_json = serde_json::json!({
            "source": "process_sample",
            "window": window.as_str(),
            "process_identity_key": entry.process_identity_key.clone(),
            "app_key": entry.app_key.clone(),
            "process_name": entry.process_name.clone(),
            "pid": entry.pid,
            "process_creation_time_ms": entry.process_creation_time_ms,
            "selection_reason_mask": entry.selection_reason_mask,
            "sample_count": entry.sample_count,
            "coverage": entry.coverage,
            "cpu_rank": entry.cpu_rank,
            "memory_rank": entry.memory_rank,
            "io_rank": entry.io_rank,
            "read_bytes": entry.read_bytes,
            "write_bytes": entry.write_bytes,
        })
        .to_string();
        for (metric, accumulator) in [
            ("process_cpu_percent", aggregate.cpu.clone()),
            ("process_memory_bytes", aggregate.memory.clone()),
            ("process_read_bps", aggregate.read.clone()),
            ("process_write_bps", aggregate.write.clone()),
        ] {
            if accumulator.sample_count > 0 {
                let mut row = summary_from_accumulator(
                    window,
                    metric,
                    None,
                    Some(aggregate.process_identity_key.clone()),
                    start_ms,
                    end_ms,
                    accumulator,
                    "process_sample",
                );
                row.evidence_ref = ref_json.clone();
                summaries.push(row);
            }
        }
        process_entries.push(entry);
    }
    process_entries.sort_by(|left, right| {
        left.cpu_rank
            .or(left.memory_rank)
            .or(left.io_rank)
            .unwrap_or(usize::MAX)
            .cmp(
                &right
                    .cpu_rank
                    .or(right.memory_rank)
                    .or(right.io_rank)
                    .unwrap_or(usize::MAX),
            )
            .then_with(|| left.process_identity_key.cmp(&right.process_identity_key))
    });
    let frame_coverage = load_frame_coverage(conn, start_ms, end_ms)?;
    Ok((summaries, process_entries, frame_coverage >= 0.999))
}

fn load_frame_coverage(conn: &Connection, start_ms: i64, end_ms: i64) -> rusqlite::Result<f64> {
    if end_ms <= start_ms {
        return Ok(0.0);
    }
    let mut statement = conn.prepare(
        "SELECT ts, duration_ms
         FROM sample_frame
         WHERE ts < ?2 AND ts + duration_ms > ?1
         ORDER BY ts, id",
    )?;
    let rows = statement.query_map(params![start_ms, end_ms], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut covered_until = start_ms;
    let mut covered_ms = 0_i64;
    for row in rows {
        let (timestamp_ms, duration_ms) = row?;
        let frame_start = timestamp_ms.max(start_ms);
        let frame_end = timestamp_ms.saturating_add(duration_ms.max(0)).min(end_ms);
        if frame_end <= frame_start {
            continue;
        }
        let uncovered_start = frame_start.max(covered_until);
        if frame_end > uncovered_start {
            covered_ms = covered_ms.saturating_add(frame_end - uncovered_start);
            covered_until = covered_until.max(frame_end);
        }
    }
    Ok((covered_ms as f64 / (end_ms - start_ms) as f64).clamp(0.0, 1.0))
}

fn load_case_row(conn: &Connection, case_id: i64) -> rusqlite::Result<CrashCaseRow> {
    conn.query_row(
        "SELECT id, stable_key, anchor_time_ms, classification, window_start_ms, window_end_ms,
                evidence_status, processing_version
         FROM crash_case WHERE id = ?1",
        [case_id],
        |row| {
            Ok(CrashCaseRow {
                id: row.get(0)?,
                stable_key: row.get(1)?,
                anchor_time_ms: row.get(2)?,
                classification: row.get(3)?,
                window_start_ms: row.get(4)?,
                window_end_ms: row.get(5)?,
                evidence_status: row.get(6)?,
                processing_version: row.get(7)?,
            })
        },
    )
}

#[allow(dead_code)]
fn build_case(db: &Database, case_id: i64) -> rusqlite::Result<()> {
    build_case_at(db, case_id, now_ms())
}

fn build_case_at(db: &Database, case_id: i64, as_of_ms: i64) -> rusqlite::Result<()> {
    db.with_writer(|conn| {
        let tx = conn.unchecked_transaction()?;
        let case_row = load_case_row(&tx, case_id)?;
        let has_active_hold: i64 = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM retention_hold WHERE crash_case_id = ?1 AND released_at_ms IS NULL)",
            [case_id],
            |row| row.get(0),
        )?;
        if has_active_hold == 0 {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::other("crash evidence requires an active retention hold"),
            )));
        }
        let mut summaries = Vec::new();
        let mut all_windows_have_frames = true;
        let post_elapsed = as_of_ms >= case_row.anchor_time_ms.saturating_add(CRASH_CASE_WINDOW_POST_MS);
        for window in CrashEvidenceWindow::ALL {
            if window == CrashEvidenceWindow::Post5m && !post_elapsed {
                continue;
            }
            let (window_summaries, _process_entries, has_frame) =
                build_window_summaries(&tx, case_row.anchor_time_ms, window)?;
            all_windows_have_frames &= has_frame && !window_summaries.is_empty();
            summaries.extend(window_summaries);
        }
        if summaries.is_empty() {
            all_windows_have_frames = false;
        }
        tx.execute(
            "DELETE FROM crash_evidence_summary WHERE crash_case_id = ?1",
            [case_id],
        )?;
        let mut insert = tx.prepare(
            "INSERT INTO crash_evidence_summary(
                crash_case_id, metric_key, window_start_ms, window_end_ms,
                avg_value, min_value, max_value, delta_value, peak_time_ms,
                coverage, evidence_ref, processing_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        for summary in summaries {
            insert.execute(params![
                case_id,
                summary.metric_key,
                summary.start_ms,
                summary.end_ms,
                summary.avg,
                summary.min,
                summary.max,
                summary.delta,
                summary.peak_time_ms,
                summary.coverage.clamp(0.0, 1.0),
                summary.evidence_ref,
                CRASH_EVIDENCE_PROCESSING_VERSION,
            ])?;
        }
        drop(insert);
        let status = if !post_elapsed {
            "post_pending"
        } else if all_windows_have_frames {
            "complete"
        } else {
            "partial"
        };
        tx.execute(
            "UPDATE crash_case SET evidence_status = ?1, processing_version = ?2 WHERE id = ?3",
            params![status, CRASH_EVIDENCE_PROCESSING_VERSION, case_id],
        )?;
        tx.commit()
    })
}

fn load_case_events(
    conn: &Connection,
    case_row: &CrashCaseRow,
) -> rusqlite::Result<Vec<CrashSystemEvent>> {
    let mut statement = conn.prepare(
        "SELECT id, channel, provider, event_id, record_id, event_time_ms, payload_summary
         FROM system_event
         WHERE (event_time_ms >= ?1 AND event_time_ms <= ?2)
            OR ABS(event_time_ms - ?3) <= ?4
            OR payload_summary LIKE '%\"previous_shutdown_time_ms\":' || CAST(?3 AS TEXT) || '%'
         ORDER BY event_time_ms, id",
    )?;
    let rows = statement.query_map(
        params![
            case_row.window_start_ms,
            case_row.window_end_ms,
            case_row.anchor_time_ms,
            CRASH_INCIDENT_CORRELATION_MS,
        ],
        |row| {
            let payload_summary: Option<String> = row.get(6)?;
            let facts = payload_summary
                .as_deref()
                .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                .unwrap_or_default();
            let kind = facts
                .get("kind")
                .and_then(|value| value.as_str())
                .unwrap_or("other")
                .to_string();
            Ok(CrashSystemEvent {
                id: row.get(0)?,
                channel: row.get(1)?,
                provider: row.get(2)?,
                event_id: row.get(3)?,
                record_id: row.get(4)?,
                event_time_ms: row.get(5)?,
                kind,
                bugcheck_code: facts
                    .get("bugcheck_code")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                boot_id: facts
                    .get("boot_id")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                previous_shutdown_time_ms: facts
                    .get("previous_shutdown_time_ms")
                    .and_then(|value| value.as_i64()),
                clean_shutdown: facts
                    .get("clean_shutdown")
                    .and_then(|value| value.as_bool()),
                restart_boundary: facts
                    .get("restart_boundary")
                    .and_then(|value| value.as_bool()),
                dump_available: facts
                    .get("dump_available")
                    .and_then(|value| value.as_bool()),
                dump_size_bytes: facts
                    .get("dump_size_bytes")
                    .and_then(|value| value.as_u64()),
            })
        },
    )?;
    rows.collect()
}

fn load_case_metrics(
    conn: &Connection,
    case_id: i64,
) -> rusqlite::Result<Vec<CrashEvidenceMetric>> {
    let mut statement = conn.prepare(
        "SELECT metric_key, window_start_ms, window_end_ms, avg_value, min_value, max_value,
                delta_value, peak_time_ms, coverage, evidence_ref
         FROM crash_evidence_summary WHERE crash_case_id = ?1 ORDER BY window_start_ms, metric_key",
    )?;
    let rows = statement.query_map([case_id], |row| {
        let metric_key: String = row.get(0)?;
        let (window, metric, device_key, process_identity_key) = parse_summary_key(&metric_key);
        let evidence_ref: Option<String> = row.get(9)?;
        let sample_count = evidence_ref
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .and_then(|value| value.get("sample_count").and_then(|value| value.as_i64()))
            .unwrap_or(0);
        Ok(CrashEvidenceMetric {
            metric_key,
            metric,
            window,
            device_key,
            process_identity_key,
            window_start_ms: row.get(1)?,
            window_end_ms: row.get(2)?,
            avg: row.get(3)?,
            min: row.get(4)?,
            max: row.get(5)?,
            delta: row.get(6)?,
            peak_time_ms: row.get(7)?,
            sample_count,
            coverage: row.get::<_, Option<f64>>(8)?.unwrap_or(0.0).clamp(0.0, 1.0),
            evidence_ref,
        })
    })?;
    rows.collect()
}

fn parse_summary_key(key: &str) -> (CrashEvidenceWindow, String, Option<String>, Option<String>) {
    let window_name = key
        .strip_prefix("window:")
        .and_then(|value| value.split_once(":metric:").map(|(window, _)| window));
    let window = match window_name {
        Some("pre_1m") => CrashEvidenceWindow::Pre1m,
        Some("pre_5m") => CrashEvidenceWindow::Pre5m,
        Some("pre_30m") => CrashEvidenceWindow::Pre30m,
        Some("post_5m") => CrashEvidenceWindow::Post5m,
        _ => CrashEvidenceWindow::Pre30m,
    };
    let (without_process, process_identity_key) = match key.split_once(":process:") {
        Some((prefix, process_identity_key)) => (prefix, Some(process_identity_key.to_string())),
        None => (key, None),
    };
    let (without_device, device_key) = match without_process.split_once(":device:") {
        Some((prefix, device_key)) => (prefix, Some(device_key.to_string())),
        None => (without_process, None),
    };
    let metric = without_device
        .split_once(":metric:")
        .map(|(_, metric)| metric)
        .unwrap_or("unknown")
        .to_string();
    (window, metric, device_key, process_identity_key)
}

fn load_case_processes(
    conn: &Connection,
    case_row: &CrashCaseRow,
) -> rusqlite::Result<Vec<CrashEvidenceProcessEntry>> {
    let mut all = Vec::new();
    for window in CrashEvidenceWindow::ALL {
        let (_, entries, _) = build_window_summaries(conn, case_row.anchor_time_ms, window)?;
        all.extend(entries);
    }
    Ok(all)
}

pub fn list_crash_cases(conn: &Connection) -> rusqlite::Result<Vec<CrashCaseSummary>> {
    let mut statement =
        conn.prepare("SELECT id FROM crash_case ORDER BY anchor_time_ms DESC, id DESC")?;
    let ids = statement.query_map([], |row| row.get::<_, i64>(0))?;
    let mut cases = Vec::new();
    for id in ids {
        let case_row = load_case_row(conn, id?)?;
        cases.push(case_row.summary(conn)?);
    }
    Ok(cases)
}

pub fn get_crash_case_detail(
    conn: &Connection,
    case_id: i64,
) -> rusqlite::Result<CrashEvidenceDetail> {
    let case_row = load_case_row(conn, case_id)?;
    Ok(CrashEvidenceDetail {
        case: case_row.summary(conn)?,
        events: load_case_events(conn, &case_row)?,
        metrics: load_case_metrics(conn, case_id)?,
        processes: load_case_processes(conn, &case_row)?,
    })
}

pub fn build_case_with_failure_status(db: &Database, case_id: i64) -> rusqlite::Result<()> {
    build_case_with_failure_status_at(db, case_id, now_ms())
}

fn build_case_with_failure_status_at(
    db: &Database,
    case_id: i64,
    as_of_ms: i64,
) -> rusqlite::Result<()> {
    match build_case_at(db, case_id, as_of_ms) {
        Ok(()) => Ok(()),
        Err(error) => {
            let message = error.to_string();
            let _ = db.with_writer(|conn| {
                conn.execute(
                    "UPDATE crash_case SET evidence_status='failed', processing_version=?1 WHERE id=?2",
                    params![CRASH_EVIDENCE_PROCESSING_VERSION, case_id],
                )
            });
            Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::other(message),
            )))
        }
    }
}

fn format_event_reader_error(error: &EventReaderError) -> String {
    match error {
        EventReaderError::PermissionDenied(message)
        | EventReaderError::Unsupported(message)
        | EventReaderError::Failed(message) => message.clone(),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn map_event_log_error_code(code: i32, message: impl Into<String>) -> Option<EventReaderError> {
    match (code as u32) & 0xffff {
        259 => None,
        5 => Some(EventReaderError::PermissionDenied(message.into())),
        _ => Some(EventReaderError::Failed(message.into())),
    }
}

#[cfg(windows)]
fn read_native_event_log(
    cursors: &BTreeMap<String, EventCursor>,
    limit: usize,
) -> Result<EventReadBatch, EventReaderError> {
    use windows::{
        core::PCWSTR,
        Win32::System::EventLog::{
            EvtClose, EvtCreateRenderContext, EvtNext, EvtQuery, EvtQueryChannelPath,
            EvtQueryForwardDirection, EvtRenderContextSystem, EVT_HANDLE,
        },
    };
    let channel = "System";
    let cursor_time = cursors
        .get(channel)
        .map(|cursor| cursor.event_time_ms)
        .unwrap_or_else(|| now_ms().saturating_sub(7 * 86_400_000));
    let query = format!(
        "*[System[TimeCreated[timediff(@SystemTime) <= {}]]]",
        (now_ms().saturating_sub(cursor_time.saturating_sub(EVENT_CURSOR_LOOKBACK_MS))).max(1)
    );
    let path = wide(channel);
    let query = wide(&query);
    let query_handle = unsafe {
        EvtQuery(
            EVT_HANDLE::default(),
            PCWSTR::from_raw(path.as_ptr()),
            PCWSTR::from_raw(query.as_ptr()),
            EvtQueryChannelPath.0 | EvtQueryForwardDirection.0,
        )
    }
    .map_err(|error| {
        map_event_log_error_code(error.code().0, error.to_string())
            .unwrap_or_else(|| EventReaderError::Failed(error.to_string()))
    })?;
    let render_context = match unsafe { EvtCreateRenderContext(None, EvtRenderContextSystem.0) } {
        Ok(context) => context,
        Err(error) => {
            unsafe {
                let _ = EvtClose(query_handle);
            }
            return Err(EventReaderError::Failed(error.to_string()));
        }
    };
    let mut handles = vec![0isize; limit.clamp(1, 64)];
    let mut events = Vec::new();
    let mut next_cursors = BTreeMap::new();
    let mut read_error = None;
    loop {
        let mut returned = 0u32;
        let result = unsafe { EvtNext(query_handle, &mut handles, 0, 0, &mut returned) };
        if let Err(error) = result {
            read_error = map_event_log_error_code(error.code().0, error.to_string());
            break;
        }
        if returned == 0 {
            break;
        }
        for raw_handle in handles.iter().take(returned as usize) {
            let event_handle = EVT_HANDLE(*raw_handle);
            let parsed = unsafe { render_native_event(render_context, event_handle) };
            unsafe {
                let _ = EvtClose(event_handle);
            }
            let event = match parsed {
                Ok(event) => event,
                Err(_) => continue,
            };
            let cursor = cursors.get(&event.channel);
            if cursor.is_some_and(|cursor| !cursor_is_after(&event, cursor)) {
                continue;
            }
            next_cursors
                .entry(event.channel.clone())
                .and_modify(|current: &mut EventCursor| {
                    if cursor_is_after(&event, current) {
                        *current = cursor_for(&event);
                    }
                })
                .or_insert_with(|| cursor_for(&event));
            events.push(event);
            if events.len() >= limit.max(1) {
                break;
            }
        }
        if events.len() >= limit.max(1) {
            break;
        }
    }
    unsafe {
        let _ = EvtClose(render_context);
        let _ = EvtClose(query_handle);
    }
    if let Some(error) = read_error {
        return Err(error);
    }
    Ok(EventReadBatch {
        events,
        next_cursors,
    })
}

#[cfg(windows)]
unsafe fn render_native_event(
    render_context: windows::Win32::System::EventLog::EVT_HANDLE,
    event: windows::Win32::System::EventLog::EVT_HANDLE,
) -> Result<NormalizedSystemEvent, ()> {
    use windows::Win32::System::EventLog::{
        EvtRender, EvtRenderEventValues, EvtSystemChannel, EvtSystemEventID,
        EvtSystemEventRecordId, EvtSystemProviderName, EvtSystemTimeCreated, EVT_VARIANT,
    };
    let mut values = vec![EVT_VARIANT::default(); 24];
    let mut used = 0u32;
    let mut count = 0u32;
    EvtRender(
        render_context,
        event,
        EvtRenderEventValues.0,
        (values.len() * std::mem::size_of::<EVT_VARIANT>()) as u32,
        Some(values.as_mut_ptr().cast()),
        &mut used,
        &mut count,
    )
    .map_err(|_| ())?;
    let provider =
        variant_string(values.get(EvtSystemProviderName.0 as usize).ok_or(())?).ok_or(())?;
    let channel = variant_string(values.get(EvtSystemChannel.0 as usize).ok_or(())?).ok_or(())?;
    let event_id = variant_u64(values.get(EvtSystemEventID.0 as usize).ok_or(())?)
        .ok_or(())?
        .to_string();
    let record_id = variant_u64(values.get(EvtSystemEventRecordId.0 as usize).ok_or(())?)
        .ok_or(())?
        .to_string();
    let event_time_ms = filetime_to_unix_ms(
        variant_u64(values.get(EvtSystemTimeCreated.0 as usize).ok_or(())?).ok_or(())?,
    );
    let provider_lower = provider.to_ascii_lowercase();
    let payload = if (provider_lower.contains("systemerrorreporting") && event_id == "1001")
        || (provider_lower.contains("kernel-power") && event_id == "41")
        || (provider_lower == "eventlog" && event_id == "6008")
    {
        render_event_payload_facts(event, &provider, &event_id)
    } else {
        EventPayloadFacts::default()
    };
    Some(NormalizedSystemEvent::from_fields(
        channel,
        Some(provider),
        event_id,
        record_id,
        event_time_ms,
        payload,
    ))
    .ok_or(())
}

#[cfg(windows)]
unsafe fn render_event_payload_facts(
    event: windows::Win32::System::EventLog::EVT_HANDLE,
    provider: &str,
    event_id: &str,
) -> EventPayloadFacts {
    use windows::Win32::System::EventLog::{EvtRender, EvtRenderEventXml, EVT_HANDLE};

    let mut used = 0u32;
    let mut property_count = 0u32;
    let _ = EvtRender(
        EVT_HANDLE::default(),
        event,
        EvtRenderEventXml.0,
        0,
        None,
        &mut used,
        &mut property_count,
    );
    if used == 0 {
        return EventPayloadFacts::default();
    }
    let mut buffer = vec![0u16; (used as usize).div_ceil(std::mem::size_of::<u16>())];
    if EvtRender(
        EVT_HANDLE::default(),
        event,
        EvtRenderEventXml.0,
        (buffer.len() * std::mem::size_of::<u16>()) as u32,
        Some(buffer.as_mut_ptr().cast()),
        &mut used,
        &mut property_count,
    )
    .is_err()
    {
        return EventPayloadFacts::default();
    }
    let xml = String::from_utf16_lossy(&buffer);
    let mut facts = parse_event_payload_facts_from_xml(&xml, provider, event_id);
    if provider.trim().eq_ignore_ascii_case("EventLog") && event_id.trim() == "6008" {
        let fields = extract_event_data_fields(&xml);
        let named_numeric = [
            "PreviousShutdownTime",
            "PreviousShutdownTimeUtc",
            "PreviousTime",
            "LastShutdownTime",
        ]
        .iter()
        .any(|name| {
            fields
                .iter()
                .find(|field| {
                    field
                        .name
                        .as_deref()
                        .is_some_and(|field_name| field_name.eq_ignore_ascii_case(name))
                })
                .and_then(|field| parse_event_time_value(&field.value))
                .is_some()
        });
        if !named_numeric {
            if let Some((date, time)) = parse_6008_positional_date_time(&fields) {
                if let Some(timestamp_ms) = local_6008_datetime_to_unix_ms(date, time) {
                    facts.previous_shutdown_time_ms = Some(timestamp_ms);
                }
            }
        }
    }
    facts
}

fn parse_bool_value(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn parse_event_time_value(value: &str) -> Option<i64> {
    let value = value.trim();
    let numeric = value.parse::<u64>().ok()?;
    if numeric >= 116_444_736_000_000_000 {
        return Some(filetime_to_unix_ms(numeric));
    }
    if numeric >= 1_000_000_000_000 {
        return i64::try_from(numeric).ok();
    }
    if numeric >= 1_000_000_000 {
        return i64::try_from(numeric.saturating_mul(1_000)).ok();
    }
    None
}

#[derive(Debug, Clone)]
struct EventDataField {
    name: Option<String>,
    value: String,
}

/// Extract only the small, named/positional EventData values needed for crash classification.
/// The raw XML is intentionally never persisted.
fn parse_event_payload_facts_from_xml(
    xml: &str,
    provider: &str,
    event_id: &str,
) -> EventPayloadFacts {
    let fields = extract_event_data_fields(xml);
    let provider_lower = provider.trim().to_ascii_lowercase();
    let is_wer_1001 = provider_lower.contains("systemerrorreporting") && event_id == "1001";
    let is_kernel_power_41 = provider_lower.contains("kernel-power") && event_id == "41";
    let is_eventlog_6008 = provider_lower == "eventlog" && event_id == "6008";
    let named = |names: &[&str]| {
        names.iter().find_map(|name| {
            fields
                .iter()
                .find(|field| {
                    field
                        .name
                        .as_deref()
                        .is_some_and(|field_name| field_name.eq_ignore_ascii_case(name))
                })
                .map(|field| field.value.as_str())
        })
    };
    let bugcheck_code = if is_wer_1001 || is_kernel_power_41 {
        named(&["BugcheckCode", "BugCheckCode", "bugcheckCode"]).map(str::to_string)
    } else {
        None
    };
    let previous_shutdown_time_ms = if is_kernel_power_41 || is_eventlog_6008 {
        named(&[
            "PreviousShutdownTime",
            "PreviousShutdownTimeUtc",
            "PreviousTime",
            "LastShutdownTime",
        ])
        .and_then(parse_event_time_value)
        .or_else(|| is_eventlog_6008.then(|| parse_6008_positional_previous_shutdown(&fields))?)
    } else {
        None
    };
    let clean_shutdown = named(&["CleanShutdown", "cleanShutdown"]).and_then(parse_bool_value);
    EventPayloadFacts {
        bugcheck_code,
        previous_shutdown_time_ms,
        clean_shutdown,
        ..EventPayloadFacts::default()
    }
}

fn extract_event_data_fields(xml: &str) -> Vec<EventDataField> {
    let mut fields = Vec::new();
    let mut cursor = 0usize;
    while cursor < xml.len() {
        let Some(relative_start) = xml[cursor..].find("<Data") else {
            break;
        };
        let start = cursor + relative_start;
        let after_name = start + "<Data".len();
        let next = xml[after_name..].chars().next();
        if !matches!(next, Some(' ' | '\t' | '\r' | '\n' | '>')) {
            cursor = after_name;
            continue;
        }
        let Some(relative_tag_end) = xml[after_name..].find('>') else {
            break;
        };
        let tag_end = after_name + relative_tag_end;
        let Some(relative_close_start) = xml[tag_end + 1..].find("</Data>") else {
            break;
        };
        let close_start = tag_end + 1 + relative_close_start;
        let raw_value = &xml[tag_end + 1..close_start];
        let value = clean_event_text(raw_value);
        if !value.is_empty() && !value.contains('<') {
            fields.push(EventDataField {
                name: extract_xml_attribute(&xml[start..=tag_end], "Name"),
                value,
            });
        }
        cursor = close_start + "</Data>".len();
    }
    fields
}

fn extract_xml_attribute(tag: &str, attribute: &str) -> Option<String> {
    let marker = format!("{attribute}=");
    let start = tag.find(&marker)?.saturating_add(marker.len());
    let quote = tag.get(start..)?.chars().next()?;
    if quote != '\"' && quote != '\'' {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let end = tag
        .get(value_start..)?
        .find(quote)?
        .saturating_add(value_start);
    Some(tag.get(value_start..end)?.to_string())
}

fn clean_event_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !matches!(
                character,
                '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'
                    | '\u{202b}'
                    | '\u{202c}'
                    | '\u{202d}'
                    | '\u{202e}'
            )
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn parse_6008_positional_previous_shutdown(fields: &[EventDataField]) -> Option<i64> {
    let (date, time) = parse_6008_positional_date_time(fields)?;
    date_time_to_unix_ms(date, time)
}

fn parse_6008_positional_date_time(
    fields: &[EventDataField],
) -> Option<(ParsedCalendarDate, ParsedClock)> {
    let param1 = fields.iter().find(|field| {
        field
            .name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case("param1"))
    });
    let param2 = fields.iter().find(|field| {
        field
            .name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case("param2"))
    });
    let (first, second) = if let (Some(param1), Some(param2)) = (param1, param2) {
        (param1.value.as_str(), param2.value.as_str())
    } else {
        let positional = fields
            .iter()
            .filter(|field| field.name.is_none())
            .map(|field| field.value.as_str())
            .take(2)
            .collect::<Vec<_>>();
        if positional.len() != 2 {
            return None;
        }
        (positional[0], positional[1])
    };

    [(first, second), (second, first)]
        .into_iter()
        .find_map(|(time, date)| {
            let time = parse_clock_value(time)?;
            let date = parse_calendar_date(date)?;
            Some((date, time))
        })
}

#[derive(Debug, Clone, Copy)]
struct ParsedClock {
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: u32,
}

#[derive(Debug, Clone, Copy)]
struct ParsedCalendarDate {
    year: i64,
    month: u32,
    day: u32,
}

fn parse_clock_value(value: &str) -> Option<ParsedClock> {
    let value = clean_event_text(value)
        .to_ascii_uppercase()
        .replace('.', "");
    let (value, meridiem) = if let Some(value) = value.strip_suffix("AM") {
        (value.trim(), Some(false))
    } else if let Some(value) = value.strip_suffix("PM") {
        (value.trim(), Some(true))
    } else {
        (value.trim(), None)
    };
    let components = value.split(':').collect::<Vec<_>>();
    if components.len() != 3 {
        return None;
    }
    let hour = components[0].parse::<u32>().ok()?;
    let minute = components[1].parse::<u32>().ok()?;
    let (second, millisecond) = if let Some((second, fraction)) = components[2].split_once('.') {
        let second = second.parse::<u32>().ok()?;
        let fraction = fraction
            .chars()
            .take(3)
            .collect::<String>()
            .parse::<u32>()
            .ok()?;
        let millisecond = fraction.saturating_mul(match components[2].split_once('.')?.1.len() {
            1 => 100,
            2 => 10,
            _ => 1,
        });
        (second, millisecond)
    } else {
        (components[2].parse::<u32>().ok()?, 0)
    };
    if minute >= 60 || second >= 60 || millisecond >= 1_000 {
        return None;
    }
    let hour = match meridiem {
        Some(is_pm) if (1..=12).contains(&hour) => {
            if is_pm {
                if hour == 12 {
                    12
                } else {
                    hour + 12
                }
            } else if hour == 12 {
                0
            } else {
                hour
            }
        }
        Some(_) => return None,
        None if hour < 24 => hour,
        None => return None,
    };
    Some(ParsedClock {
        hour,
        minute,
        second,
        millisecond,
    })
}

fn parse_calendar_date(value: &str) -> Option<ParsedCalendarDate> {
    let value = clean_event_text(value);
    let components = value
        .split(['/', '-', '.'])
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    if components.len() != 3 {
        return None;
    }
    let numbers = components
        .iter()
        .map(|component| component.parse::<u32>().ok())
        .collect::<Option<Vec<_>>>()?;
    let (year, month, day) = if components[0].len() == 4 {
        (numbers[0] as i64, numbers[1], numbers[2])
    } else if components[2].len() == 4 {
        let first = numbers[0];
        let second = numbers[1];
        let (month, day) = if first > 12 && second <= 12 {
            (second, first)
        } else if second > 12 && first <= 12 {
            (first, second)
        } else if first == second {
            // Both locale interpretations produce the same physical date.
            (first, second)
        } else {
            // Do not silently guess between e.g. 03/04 and 04/03.
            return None;
        };
        (numbers[2] as i64, month, day)
    } else {
        return None;
    };
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some(ParsedCalendarDate { year, month, day })
}

fn date_time_to_unix_ms(date: ParsedCalendarDate, time: ParsedClock) -> Option<i64> {
    // Cross-platform fixtures use UTC for deterministic assertions. The native Windows reader
    // reinterprets positional 6008 values through the machine's timezone before persisting them.
    let days = days_from_civil(date.year, date.month, date.day)?;
    days.checked_mul(86_400_000)?
        .checked_add(i64::from(time.hour) * 3_600_000)
        .and_then(|value| value.checked_add(i64::from(time.minute) * 60_000))
        .and_then(|value| value.checked_add(i64::from(time.second) * 1_000))
        .and_then(|value| value.checked_add(i64::from(time.millisecond)))
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

#[cfg(windows)]
unsafe fn variant_string(value: &windows::Win32::System::EventLog::EVT_VARIANT) -> Option<String> {
    use windows::Win32::System::EventLog::{
        EvtVarTypeString, EvtVarTypeUInt16, EvtVarTypeUInt32, EvtVarTypeUInt64,
    };
    let kind = value.Type & 0x0fff;
    if kind == EvtVarTypeString.0 as u32 {
        let pointer = value.Anonymous.StringVal;
        if pointer.is_null() {
            return None;
        }
        return pointer.to_string().ok();
    }
    if kind == EvtVarTypeUInt16.0 as u32 {
        return Some(value.Anonymous.UInt16Val.to_string());
    }
    if kind == EvtVarTypeUInt32.0 as u32 {
        return Some(value.Anonymous.UInt32Val.to_string());
    }
    if kind == EvtVarTypeUInt64.0 as u32 {
        return Some(value.Anonymous.UInt64Val.to_string());
    }
    None
}

#[cfg(windows)]
unsafe fn variant_u64(value: &windows::Win32::System::EventLog::EVT_VARIANT) -> Option<u64> {
    use windows::Win32::System::EventLog::{
        EvtVarTypeFileTime, EvtVarTypeUInt16, EvtVarTypeUInt32, EvtVarTypeUInt64,
    };
    match value.Type & 0x0fff {
        kind if kind == EvtVarTypeUInt16.0 as u32 => Some(value.Anonymous.UInt16Val as u64),
        kind if kind == EvtVarTypeUInt32.0 as u32 => Some(value.Anonymous.UInt32Val as u64),
        kind if kind == EvtVarTypeUInt64.0 as u32 => Some(value.Anonymous.UInt64Val),
        kind if kind == EvtVarTypeFileTime.0 as u32 => Some(value.Anonymous.FileTimeVal),
        _ => None,
    }
}

fn filetime_to_unix_ms(filetime: u64) -> i64 {
    filetime
        .saturating_sub(116_444_736_000_000_000)
        .checked_div(10_000)
        .and_then(|value| i64::try_from(value).ok())
        .unwrap_or(0)
}

#[cfg(windows)]
fn local_6008_datetime_to_unix_ms(date: ParsedCalendarDate, time: ParsedClock) -> Option<i64> {
    use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows::Win32::System::Time::{SystemTimeToFileTime, TzSpecificLocalTimeToSystemTimeEx};

    let local = SYSTEMTIME {
        wYear: u16::try_from(date.year).ok()?,
        wMonth: u16::try_from(date.month).ok()?,
        wDay: u16::try_from(date.day).ok()?,
        wHour: u16::try_from(time.hour).ok()?,
        wMinute: u16::try_from(time.minute).ok()?,
        wSecond: u16::try_from(time.second).ok()?,
        wMilliseconds: u16::try_from(time.millisecond).ok()?,
        ..SYSTEMTIME::default()
    };
    let mut utc = SYSTEMTIME::default();
    unsafe {
        TzSpecificLocalTimeToSystemTimeEx(None, &local, &mut utc).ok()?;
    }
    let mut filetime = FILETIME::default();
    unsafe {
        SystemTimeToFileTime(&utc, &mut filetime).ok()?;
    }
    let value = (u64::from(filetime.dwHighDateTime) << 32) | u64::from(filetime.dwLowDateTime);
    Some(filetime_to_unix_ms(value))
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;
    use rusqlite::params;

    fn db() -> Database {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "resource-timeline-crash-test-{}-{}.sqlite3",
            std::process::id(),
            nonce
        ));
        Database::open(path).unwrap()
    }

    fn insert_boot_session_fixture(db: &Database, boot_id: &str, boot_time_ms: i64) -> i64 {
        db.with_writer(|conn| {
            conn.execute(
                "INSERT INTO boot_session(
                    boot_id, boot_time_ms, observed_start_ms, created_at_ms
                 ) VALUES (?1, ?2, ?2, ?2)",
                params![boot_id, boot_time_ms],
            )?;
            Ok(conn.last_insert_rowid())
        })
        .unwrap()
    }

    fn insert_collection_boundary_fixture(
        db: &Database,
        boot_session_id: i64,
        boundary_time_ms: i64,
    ) {
        db.with_writer(|conn| {
            conn.execute(
                "INSERT INTO collection_session(
                    boot_session_id, started_at_ms, ended_at_ms, app_version, schema_version
                 ) VALUES (?1, ?2, ?3, 'crash-test', 8)",
                params![boot_session_id, boundary_time_ms - 60_000, boundary_time_ms],
            )?;
            let collection_session_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO sample_frame(
                    collection_session_id, ts, sequence, duration_ms, process_snapshot_present
                 ) VALUES (?1, ?2, 1, 1, 0)",
                params![collection_session_id, boundary_time_ms - 1],
            )?;
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn bugcheck_plus_reboot_is_one_bsod_case() {
        let events = vec![
            NormalizedSystemEvent::fixture(SystemEventKind::BugCheck, "10", 1_000),
            NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "11", 2_000),
        ];
        let candidates = classify_events(&events);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].classification, CrashClassification::Bsod);
    }

    #[test]
    fn normal_shutdown_sleep_and_application_crash_are_not_system_crashes() {
        let normal = vec![
            NormalizedSystemEvent::fixture(SystemEventKind::NormalShutdown, "1", 1_000),
            NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "2", 2_000),
        ];
        assert!(classify_events(&normal).is_empty());
        let sleep = vec![
            NormalizedSystemEvent::fixture(SystemEventKind::Sleep, "3", 1_000),
            NormalizedSystemEvent::fixture(SystemEventKind::Resume, "4", 2_000),
        ];
        assert!(classify_events(&sleep).is_empty());
        let app = vec![NormalizedSystemEvent::fixture(
            SystemEventKind::ApplicationCrash,
            "5",
            1_000,
        )];
        assert!(classify_events(&app).is_empty());
    }

    #[test]
    fn event_id_alone_does_not_classify_a_case() {
        let event = NormalizedSystemEvent::from_fields(
            "Application",
            Some("unrelated-provider".into()),
            "41",
            "1",
            1_000,
            EventPayloadFacts::default(),
        );
        let boot = NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "2", 2_000);
        assert!(classify_events(&[event, boot]).is_empty());
    }

    #[test]
    fn native_event_id_semantics_do_not_depend_on_fixture_restart_flags() {
        let kernel_power = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-Kernel-Power".into()),
            "41",
            "41",
            2_000,
            EventPayloadFacts::default(),
        );
        let eventlog = NormalizedSystemEvent::from_fields(
            "System",
            Some("EventLog".into()),
            "6008",
            "6008",
            3_000,
            EventPayloadFacts::default(),
        );
        assert_eq!(kernel_power.kind, SystemEventKind::UnexpectedShutdown);
        assert_eq!(eventlog.kind, SystemEventKind::UnexpectedShutdown);
    }

    #[test]
    fn event_41_bugcheck_code_controls_bsod_classification() {
        let anchor = 10_000_i64;
        let nonzero = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-Kernel-Power".into()),
            "41",
            "41-nonzero",
            anchor + 1_000,
            EventPayloadFacts {
                bugcheck_code: Some("0x0000009f".into()),
                previous_shutdown_time_ms: Some(anchor),
                ..EventPayloadFacts::default()
            },
        );
        assert_eq!(
            classify_events(&[nonzero])[0].classification,
            CrashClassification::Bsod
        );

        for (record_id, bugcheck_code) in [("41-zero", Some("0")), ("41-missing", None)] {
            let event = NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                record_id,
                anchor + 2_000,
                EventPayloadFacts {
                    bugcheck_code: bugcheck_code.map(str::to_string),
                    previous_shutdown_time_ms: Some(anchor),
                    ..EventPayloadFacts::default()
                },
            );
            assert_eq!(
                classify_events(&[event])[0].classification,
                CrashClassification::UnexpectedShutdown
            );
        }
    }

    #[test]
    fn later_wer_refines_zero_bugcheck_event_41_without_creating_a_case() {
        let db = db();
        let anchor = 20_000_i64;
        let event_41 = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-Kernel-Power".into()),
            "41",
            "41",
            anchor + 1_000,
            EventPayloadFacts {
                bugcheck_code: Some("0".into()),
                previous_shutdown_time_ms: Some(anchor),
                ..EventPayloadFacts::default()
            },
        );
        let first_batch = EventReadBatch {
            events: vec![event_41.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&event_41))]),
        };
        let case_id = db
            .with_writer(|conn| persist_event_batch(conn, &first_batch, anchor + 2_000))
            .unwrap()[0];
        let wer = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-WER-SystemErrorReporting".into()),
            "1001",
            "1001",
            anchor + 60_000,
            EventPayloadFacts {
                bugcheck_code: Some("0x0000009f".into()),
                previous_shutdown_time_ms: Some(anchor),
                ..EventPayloadFacts::default()
            },
        );
        let second_batch = EventReadBatch {
            events: vec![wer.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&wer))]),
        };
        let ids = db
            .with_writer(|conn| persist_event_batch(conn, &second_batch, anchor + 61_000))
            .unwrap();
        assert_eq!(ids, vec![case_id]);
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |row| row
                    .get::<_, i64>(0))?,
                1
            );
            assert_eq!(
                conn.query_row(
                    "SELECT classification FROM crash_case WHERE id = ?1",
                    [case_id],
                    |row| row.get::<_, String>(0),
                )?,
                "bsod"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn eventlog_6008_parses_named_and_positional_previous_shutdown_times() {
        let named = parse_event_payload_facts_from_xml(
            r#"<EventData><Data Name="PreviousShutdownTime">1683287458000</Data></EventData>"#,
            "EventLog",
            "6008",
        );
        assert_eq!(named.previous_shutdown_time_ms, Some(1_683_287_458_000));

        let positional = parse_event_payload_facts_from_xml(
            r#"<EventData><Data>11:50:58</Data><Data>05/05/2023</Data></EventData>"#,
            "EventLog",
            "6008",
        );
        assert_eq!(
            positional.previous_shutdown_time_ms,
            Some(1_683_287_458_000)
        );

        let param_style = parse_event_payload_facts_from_xml(
            r#"<EventData><Data Name="param1">11:50:58</Data><Data Name="param2">05/05/2023</Data></EventData>"#,
            "EventLog",
            "6008",
        );
        assert_eq!(
            param_style.previous_shutdown_time_ms,
            Some(1_683_287_458_000)
        );

        let ambiguous = parse_event_payload_facts_from_xml(
            r#"<EventData><Data>11:50:58</Data><Data>03/04/2023</Data></EventData>"#,
            "EventLog",
            "6008",
        );
        assert_eq!(ambiguous.previous_shutdown_time_ms, None);
        let unparseable = parse_event_payload_facts_from_xml(
            r#"<EventData><Data Name="param1">not-a-time</Data><Data Name="param2">not-a-date</Data></EventData>"#,
            "EventLog",
            "6008",
        );
        assert_eq!(unparseable.previous_shutdown_time_ms, None);
    }

    #[test]
    fn delayed_6008_uses_physical_time_and_collection_boundary_is_fallback() {
        let anchor = 30_000_i64;
        let delayed = NormalizedSystemEvent::from_fields(
            "System",
            Some("EventLog".into()),
            "6008",
            "6008-delayed",
            anchor + 60 * 60 * 1_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(anchor),
                ..EventPayloadFacts::default()
            },
        );
        let candidate = classify_events(&[delayed]).pop().expect("6008 candidate");
        assert_eq!(candidate.anchor_time_ms, anchor);

        let unparseable = NormalizedSystemEvent::from_fields(
            "System",
            Some("EventLog".into()),
            "6008",
            "6008-unparseable",
            anchor + 2_000,
            EventPayloadFacts::default(),
        );
        let fallback = classify_events_with_context(&[unparseable], Some(anchor));
        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].anchor_time_ms, anchor);
    }

    #[test]
    fn event_log_error_mapping_distinguishes_end_access_and_failure() {
        assert!(map_event_log_error_code(259, "end").is_none());
        assert!(matches!(
            map_event_log_error_code(0x8007_0005u32 as i32, "denied"),
            Some(EventReaderError::PermissionDenied(_))
        ));
        assert!(matches!(
            map_event_log_error_code(0x8007_0057u32 as i32, "failed"),
            Some(EventReaderError::Failed(_))
        ));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "read-only machine smoke; run explicitly with --ignored"]
    fn native_event_log_smoke_read_only() {
        let mut reader = WindowsEventLogReader;
        let cursor = EventCursor {
            channel: "System".into(),
            record_id: "0".into(),
            event_time_ms: now_ms().saturating_sub(24 * 60 * 60 * 1_000),
        };
        let batch = match reader.read(&BTreeMap::from([("System".into(), cursor)]), 64) {
            Ok(batch) => batch,
            Err(EventReaderError::PermissionDenied(error)) => {
                eprintln!("Windows Event Log smoke permission denied: {error}");
                return;
            }
            Err(EventReaderError::Unsupported(error)) => {
                eprintln!("Windows Event Log smoke unsupported: {error}");
                return;
            }
            Err(EventReaderError::Failed(error)) => {
                panic!("Windows Event Log smoke failed: {error}")
            }
        };
        assert!(batch.events.len() <= 64);
        let event_ids = batch
            .events
            .iter()
            .map(|event| {
                format!(
                    "{}:{}",
                    event.provider.as_deref().unwrap_or(""),
                    event.event_id
                )
            })
            .collect::<Vec<_>>();
        eprintln!(
            "Windows Event Log smoke: observed {} events, next cursors={}, ids={event_ids:?}",
            batch.events.len(),
            batch.next_cursors.len()
        );
        if !batch.events.is_empty() {
            let next = reader
                .read(&batch.next_cursors, 64)
                .expect("second bounded native Event Log read should succeed");
            assert!(next.events.iter().all(|event| {
                batch
                    .next_cursors
                    .get(&event.channel)
                    .is_none_or(|cursor| cursor_is_after(event, cursor))
            }));
        }
    }

    #[test]
    fn kernel_power_and_eventlog_boundaries_are_contextual_and_bugcheck_dominates() {
        let kernel_power = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-Kernel-Power".into()),
            "41",
            "41",
            1_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(900),
                restart_boundary: Some(true),
                ..EventPayloadFacts::default()
            },
        );
        let boot = NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "42", 2_000);
        let candidates = classify_events(&[kernel_power.clone(), boot.clone()]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].classification,
            CrashClassification::UnexpectedShutdown
        );

        let eventlog = NormalizedSystemEvent::from_fields(
            "System",
            Some("EventLog".into()),
            "6008",
            "43",
            3_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(2_900),
                ..EventPayloadFacts::default()
            },
        );
        let eventlog_boot =
            NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "44", 4_000);
        assert_eq!(
            classify_events(&[eventlog, eventlog_boot])[0].classification,
            CrashClassification::UnexpectedShutdown
        );

        let bugcheck = NormalizedSystemEvent::fixture(SystemEventKind::BugCheck, "45", 5_000);
        let kernel_power_after_bugcheck = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-Kernel-Power".into()),
            "41",
            "46",
            6_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(5_000),
                restart_boundary: Some(true),
                ..EventPayloadFacts::default()
            },
        );
        let boot_after_bugcheck =
            NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "47", 7_000);
        let candidates =
            classify_events(&[bugcheck, kernel_power_after_bugcheck, boot_after_bugcheck]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].classification, CrashClassification::Bsod);
    }

    #[test]
    fn out_of_order_events_are_sorted_and_unfinished_boundaries_are_suppressed() {
        let boundary =
            NormalizedSystemEvent::fixture(SystemEventKind::UnexpectedShutdown, "51", 10_000);
        let boot = NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "52", 11_000);
        assert_eq!(classify_events(&[boot, boundary]).len(), 1);

        let unfinished =
            NormalizedSystemEvent::fixture(SystemEventKind::UnexpectedShutdown, "53", 20_000);
        assert_eq!(classify_events(&[unfinished]).len(), 1);
    }

    #[test]
    fn malformed_persisted_event_payload_is_safe_and_non_qualifying() {
        let db = db();
        db.with_writer(|conn| {
            conn.execute(
                "INSERT INTO system_event(
                    channel, event_id, record_id, event_time_ms, provider, payload_summary
                 ) VALUES ('System', '41', 'malformed', 1000, 'unknown', '{not-json')",
                [],
            )?;
            let events = load_events_in_range(conn, 0, 2_000)?;
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].kind, SystemEventKind::Other);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn fixture_reader_honors_cursor_and_access_failures_are_isolated() {
        let first = NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "10", 1_000);
        let second = NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "11", 2_000);
        let mut reader = FixtureEventReader {
            events: vec![first.clone(), second.clone()],
            error: None,
        };
        let first_batch = reader.read(&BTreeMap::new(), 1).unwrap();
        assert_eq!(first_batch.events.len(), 1);
        let next = reader.read(&first_batch.next_cursors, 10).unwrap();
        assert_eq!(next.events, vec![second]);

        let mut denied = FixtureEventReader {
            events: Vec::new(),
            error: Some(EventReaderError::PermissionDenied("fixture denied".into())),
        };
        assert!(matches!(
            denied.read(&BTreeMap::new(), 10),
            Err(EventReaderError::PermissionDenied(_))
        ));
    }

    #[test]
    fn repeated_batch_is_idempotent_and_hold_is_atomic() {
        let db = db();
        let events = vec![
            NormalizedSystemEvent::fixture(SystemEventKind::UnexpectedShutdown, "10", 1_000),
            NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "11", 2_000),
        ];
        let batch = EventReadBatch {
            events: events.clone(),
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&events[1]))]),
        };
        db.with_writer(|conn| persist_event_batch(conn, &batch, 3_000))
            .unwrap();
        db.with_writer(|conn| persist_event_batch(conn, &batch, 3_000))
            .unwrap();
        db.read(|conn| {
            let cases: i64 =
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |row| row.get(0))?;
            let holds: i64 =
                conn.query_row("SELECT COUNT(*) FROM retention_hold", [], |row| row.get(0))?;
            let events: i64 =
                conn.query_row("SELECT COUNT(*) FROM system_event", [], |row| row.get(0))?;
            assert_eq!(cases, 1);
            assert_eq!(holds, 1);
            assert_eq!(events, 2);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn repeated_scan_fixture_is_idempotent() {
        let db = db();
        let mut reader = FixtureEventReader {
            events: vec![
                NormalizedSystemEvent::fixture(SystemEventKind::UnexpectedShutdown, "20", 10_000),
                NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "21", 11_000),
            ],
            error: None,
        };
        scan_once(&db, &mut reader, 12_000).unwrap();
        scan_once(&db, &mut reader, 13_000).unwrap();
        db.read(|conn| {
            let cases: i64 =
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |row| row.get(0))?;
            let holds: i64 = conn.query_row(
                "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                [],
                |row| row.get(0),
            )?;
            let events: i64 =
                conn.query_row("SELECT COUNT(*) FROM system_event", [], |row| row.get(0))?;
            assert_eq!(cases, 1);
            assert_eq!(holds, 1);
            assert_eq!(events, 2);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn same_batch_physical_anchor_correlates_41_6008_and_wer_into_one_bsod_case() {
        let db = db();
        let anchor = 1_000_000;
        let events = vec![
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "41",
                anchor + 10_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(anchor),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("EventLog".into()),
                "6008",
                "6008",
                anchor + 20_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(anchor),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-WER-SystemErrorReporting".into()),
                "1001",
                "1001",
                anchor + 30_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(anchor),
                    bugcheck_code: Some("0x00000116".into()),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("EventLog".into()),
                "6005",
                "6005",
                anchor + 40_000,
                EventPayloadFacts::default(),
            ),
        ];
        let batch = EventReadBatch {
            events: events.clone(),
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&events[3]))]),
        };
        let case_ids = db
            .with_writer(|conn| persist_event_batch(conn, &batch, anchor + 40_000))
            .unwrap();
        assert_eq!(case_ids.len(), 1);
        db.read(|conn| {
            let case: (i64, i64, String, i64, i64) = conn.query_row(
                "SELECT id, anchor_time_ms, classification, window_start_ms, window_end_ms
                 FROM crash_case",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )?;
            assert_eq!(case.0, case_ids[0]);
            assert_eq!(case.1, anchor);
            assert_eq!(case.2, "bsod");
            assert_eq!(case.3, anchor - CRASH_CASE_WINDOW_PRE_MS);
            assert_eq!(case.4, anchor + CRASH_CASE_WINDOW_POST_MS);
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM retention_hold", [], |row| row
                    .get::<_, i64>(0))?,
                1
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn independent_reboot_episodes_ten_minutes_apart_remain_two_cases() {
        let db = db();
        let first_anchor = 1_000_000_i64;
        let second_anchor = first_anchor + 10 * 60 * 1_000;
        let events = vec![
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "a-41",
                first_anchor + 10_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(first_anchor),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("EventLog".into()),
                "6008",
                "a-6008",
                first_anchor + 20_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(first_anchor),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::fixture(
                SystemEventKind::NormalBoot,
                "a-boot",
                first_anchor + 30_000,
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "b-41",
                second_anchor + 10_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(second_anchor),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("EventLog".into()),
                "6008",
                "b-6008",
                second_anchor + 20_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(second_anchor),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::fixture(
                SystemEventKind::NormalBoot,
                "b-boot",
                second_anchor + 30_000,
            ),
        ];
        let batch = EventReadBatch {
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&events[5]))]),
            events,
        };
        let case_ids = db
            .with_writer(|conn| persist_event_batch(conn, &batch, second_anchor + 40_000))
            .unwrap();
        assert_eq!(case_ids.len(), 2);
        db.read(|conn| {
            let rows = conn
                .prepare(
                    "SELECT anchor_time_ms, classification FROM crash_case
                     ORDER BY anchor_time_ms",
                )?
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert_eq!(
                rows,
                vec![
                    (first_anchor, "unexpected_shutdown".to_string()),
                    (second_anchor, "unexpected_shutdown".to_string())
                ]
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                2
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn production_persistence_scopes_collection_fallback_to_reboot_episode() {
        let db = db();
        let crash_a = 1_000_000_i64;
        let boot_a = crash_a + 60_000;
        let crash_b = crash_a + 10 * 60 * 1_000;
        let boot_b = crash_b + 60_000;
        let session_a = insert_boot_session_fixture(&db, "episode-a", boot_a);
        let session_b = insert_boot_session_fixture(&db, "episode-b", boot_b);
        // This is the collection boundary immediately before reboot episode B. It is within
        // the broad context window of both crash records, but must not anchor episode A.
        insert_collection_boundary_fixture(&db, session_a, crash_b);

        let events = vec![
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "episode-a-41",
                crash_a,
                EventPayloadFacts::default(),
            ),
            NormalizedSystemEvent::fixture(
                SystemEventKind::NormalBoot,
                "episode-a-boot-event",
                boot_a,
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "episode-b-41",
                crash_b,
                EventPayloadFacts::default(),
            ),
            NormalizedSystemEvent::fixture(
                SystemEventKind::NormalBoot,
                "episode-b-boot-event",
                boot_b,
            ),
        ];
        let batch = EventReadBatch {
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&events[3]))]),
            events,
        };
        let case_ids = db
            .with_writer(|conn| persist_event_batch(conn, &batch, boot_b + 1_000))
            .unwrap();
        assert_eq!(case_ids.len(), 2);

        db.read(|conn| {
            let rows = conn
                .prepare(
                    "SELECT anchor_time_ms, stable_key
                     FROM crash_case ORDER BY anchor_time_ms",
                )?
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert_eq!(
                rows.iter().map(|(anchor, _)| *anchor).collect::<Vec<_>>(),
                vec![boot_a, crash_b]
            );
            assert!(rows
                .iter()
                .all(|(_, key)| key.starts_with("incident:episode:")));
            assert_ne!(rows[0].1, rows[1].1);
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |row| row
                    .get::<_, i64>(0),)?,
                2
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                    [],
                    |row| row.get::<_, i64>(0),
                )?,
                2
            );
            Ok(())
        })
        .unwrap();
        let _ = (session_a, session_b);
    }

    #[test]
    fn independent_reboots_without_physical_payload_use_distinct_boot_boundaries() {
        let first_event_time = 4_000_000_i64;
        let second_event_time = first_event_time + 10 * 60 * 1_000;
        let first_boot_time = first_event_time + 20_000;
        let second_boot_time = second_event_time + 20_000;
        let events = vec![
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "a-41-no-payload",
                first_event_time,
                EventPayloadFacts::default(),
            ),
            NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "a-boot", first_boot_time),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "b-41-no-payload",
                second_event_time,
                EventPayloadFacts::default(),
            ),
            NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "b-boot", second_boot_time),
        ];
        let candidates = classify_events(&events);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].anchor_time_ms, first_boot_time);
        assert_eq!(candidates[1].anchor_time_ms, second_boot_time);
    }

    #[test]
    fn cross_batch_signal_refinement_reuses_case_identity_and_hold() {
        let db = db();
        let anchor = 2_000_000;
        let first_events = vec![
            NormalizedSystemEvent::from_fields(
                "System",
                Some("Microsoft-Windows-Kernel-Power".into()),
                "41",
                "201",
                anchor + 10_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(anchor),
                    ..EventPayloadFacts::default()
                },
            ),
            NormalizedSystemEvent::from_fields(
                "System",
                Some("EventLog".into()),
                "6008",
                "202",
                anchor + 20_000,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(anchor),
                    ..EventPayloadFacts::default()
                },
            ),
        ];
        let first_batch = EventReadBatch {
            events: first_events.clone(),
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&first_events[1]))]),
        };
        let first_case_id = db
            .with_writer(|conn| persist_event_batch(conn, &first_batch, anchor + 25_000))
            .unwrap()[0];
        let first_key: String = db
            .read(|conn| {
                conn.query_row(
                    "SELECT stable_key FROM crash_case WHERE id = ?1",
                    [first_case_id],
                    |row| row.get(0),
                )
            })
            .unwrap();

        let second_event = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-WER-SystemErrorReporting".into()),
            "1001",
            "203",
            anchor + 30_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(anchor),
                bugcheck_code: Some("0x00000116".into()),
                ..EventPayloadFacts::default()
            },
        );
        let second_batch = EventReadBatch {
            events: vec![second_event.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&second_event))]),
        };
        let second_case_ids = db
            .with_writer(|conn| persist_event_batch(conn, &second_batch, anchor + 35_000))
            .unwrap();
        assert_eq!(second_case_ids, vec![first_case_id]);
        db.read(|conn| {
            let row: (i64, String, String, i64) = conn.query_row(
                "SELECT id, stable_key, classification, anchor_time_ms FROM crash_case",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            assert_eq!(row.0, first_case_id);
            assert_eq!(row.1, first_key);
            assert_eq!(row.2, "bsod");
            assert_eq!(row.3, anchor);
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn production_persistence_refines_one_episode_anchor_and_hold_across_batches() {
        let db = db();
        let physical_anchor = 2_000_000_i64;
        let boot_time = physical_anchor + 20_000;
        let source_session =
            insert_boot_session_fixture(&db, "refinement-source", physical_anchor - 60_000);
        let _episode_session = insert_boot_session_fixture(&db, "refinement-episode", boot_time);
        insert_collection_boundary_fixture(&db, source_session, physical_anchor - 8_000);

        let event_41 = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-Kernel-Power".into()),
            "41",
            "refinement-41",
            physical_anchor + 10_000,
            EventPayloadFacts::default(),
        );
        let boot = NormalizedSystemEvent::fixture(
            SystemEventKind::NormalBoot,
            "refinement-boot",
            boot_time,
        );
        let batch_a = EventReadBatch {
            events: vec![event_41.clone(), boot.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&boot))]),
        };
        let case_id = db
            .with_writer(|conn| persist_event_batch(conn, &batch_a, boot_time + 1_000))
            .unwrap()[0];
        let stable_key: String = db
            .read(|conn| {
                conn.query_row(
                    "SELECT stable_key FROM crash_case WHERE id = ?1",
                    [case_id],
                    |row| row.get(0),
                )
            })
            .unwrap();
        db.read(|conn| {
            let row: (i64, i64, i64, i64) = conn.query_row(
                "SELECT anchor_time_ms, window_start_ms, window_end_ms,
                        (SELECT COUNT(*) FROM retention_hold WHERE crash_case_id = crash_case.id
                         AND released_at_ms IS NULL)
                 FROM crash_case WHERE id = ?1",
                [case_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            assert_eq!(row.0, physical_anchor - 8_000);
            assert_eq!(row.1, physical_anchor - 8_000 - CRASH_CASE_WINDOW_PRE_MS);
            assert_eq!(row.2, physical_anchor - 8_000 + CRASH_CASE_WINDOW_POST_MS);
            assert_eq!(row.3, 1);
            Ok(())
        })
        .unwrap();

        let event_6008 = NormalizedSystemEvent::from_fields(
            "System",
            Some("EventLog".into()),
            "6008",
            "refinement-6008",
            physical_anchor + 30_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(physical_anchor),
                ..EventPayloadFacts::default()
            },
        );
        let batch_b = EventReadBatch {
            events: vec![event_6008.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&event_6008))]),
        };
        assert_eq!(
            db.with_writer(|conn| persist_event_batch(conn, &batch_b, physical_anchor + 31_000,))
                .unwrap(),
            vec![case_id]
        );
        db.read(|conn| {
            let row: (i64, String, i64, i64, i64, i64) = conn.query_row(
                "SELECT id, stable_key, anchor_time_ms, window_start_ms, window_end_ms,
                        (SELECT COUNT(*) FROM retention_hold WHERE crash_case_id = crash_case.id
                         AND released_at_ms IS NULL)
                 FROM crash_case",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )?;
            assert_eq!(row.0, case_id);
            assert_eq!(row.1, stable_key);
            assert_eq!(row.2, physical_anchor);
            assert_eq!(row.3, physical_anchor - CRASH_CASE_WINDOW_PRE_MS);
            assert_eq!(row.4, physical_anchor + CRASH_CASE_WINDOW_POST_MS);
            assert_eq!(row.5, 1);
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |value| value
                    .get::<_, i64>(0))?,
                1
            );
            Ok(())
        })
        .unwrap();

        let wer = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-WER-SystemErrorReporting".into()),
            "1001",
            "refinement-wer",
            physical_anchor + 40_000,
            EventPayloadFacts {
                bugcheck_code: Some("0x00000116".into()),
                ..EventPayloadFacts::default()
            },
        );
        let batch_c = EventReadBatch {
            events: vec![wer.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&wer))]),
        };
        assert_eq!(
            db.with_writer(|conn| persist_event_batch(conn, &batch_c, physical_anchor + 41_000,))
                .unwrap(),
            vec![case_id]
        );

        for (batch, now_ms) in [
            (&batch_a, boot_time + 2_000),
            (&batch_b, physical_anchor + 32_000),
            (&batch_c, physical_anchor + 42_000),
        ] {
            assert_eq!(
                db.with_writer(|conn| persist_event_batch(conn, batch, now_ms))
                    .unwrap(),
                vec![case_id]
            );
        }
        db.read(|conn| {
            let row: (i64, String, String, i64) = conn.query_row(
                "SELECT id, stable_key, classification, anchor_time_ms FROM crash_case",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
            assert_eq!(row.0, case_id);
            assert_eq!(row.1, stable_key);
            assert_eq!(row.2, "bsod");
            assert_eq!(row.3, physical_anchor);
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                    [],
                    |value| value.get::<_, i64>(0),
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn long_offline_physical_shutdown_anchor_remains_authoritative() {
        let physical_anchor = 5_000_000_i64;
        for (record_id, delay_ms) in [
            ("long-offline-48h", 48 * 60 * 60 * 1_000_i64),
            ("long-offline-72h", 72 * 60 * 60 * 1_000_i64),
        ] {
            let db = db();
            let event = NormalizedSystemEvent::from_fields(
                "System",
                Some("EventLog".into()),
                "6008",
                record_id,
                physical_anchor + delay_ms,
                EventPayloadFacts {
                    previous_shutdown_time_ms: Some(physical_anchor),
                    ..EventPayloadFacts::default()
                },
            );
            let batch = EventReadBatch {
                events: vec![event.clone()],
                next_cursors: BTreeMap::from([("System".into(), cursor_for(&event))]),
            };
            let case_ids = db
                .with_writer(|conn| {
                    persist_event_batch(conn, &batch, physical_anchor + delay_ms + 1_000)
                })
                .unwrap();
            assert_eq!(case_ids.len(), 1);
            db.read(|conn| {
                assert_eq!(
                    conn.query_row("SELECT anchor_time_ms FROM crash_case", [], |row| row
                        .get::<_, i64>(0),)?,
                    physical_anchor
                );
                Ok(())
            })
            .unwrap();
        }
    }

    #[test]
    fn future_physical_shutdown_timestamp_is_rejected() {
        let db = db();
        let event = NormalizedSystemEvent::from_fields(
            "System",
            Some("EventLog".into()),
            "6008",
            "future-6008",
            6_000_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(6_000_001),
                ..EventPayloadFacts::default()
            },
        );
        let batch = EventReadBatch {
            events: vec![event.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&event))]),
        };
        assert!(db
            .with_writer(|conn| persist_event_batch(conn, &batch, 6_001_000))
            .unwrap()
            .is_empty());
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn cross_batch_refinement_from_abnormal_to_unexpected_to_bsod_stays_one_case() {
        let db = db();
        let anchor = 3_000_000;
        let mut abnormal = NormalizedSystemEvent::fixture(
            SystemEventKind::AbnormalRestart,
            "301",
            anchor + 10_000,
        );
        abnormal.payload.previous_shutdown_time_ms = Some(anchor);
        let first_batch = EventReadBatch {
            events: vec![abnormal.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&abnormal))]),
        };
        let case_id = db
            .with_writer(|conn| persist_event_batch(conn, &first_batch, anchor + 15_000))
            .unwrap()[0];
        let unexpected = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-Kernel-Power".into()),
            "41",
            "302",
            anchor + 20_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(anchor),
                ..EventPayloadFacts::default()
            },
        );
        let second_batch = EventReadBatch {
            events: vec![unexpected.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&unexpected))]),
        };
        db.with_writer(|conn| persist_event_batch(conn, &second_batch, anchor + 25_000))
            .unwrap();
        let bugcheck = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-WER-SystemErrorReporting".into()),
            "1001",
            "303",
            anchor + 30_000,
            EventPayloadFacts {
                previous_shutdown_time_ms: Some(anchor),
                ..EventPayloadFacts::default()
            },
        );
        let third_batch = EventReadBatch {
            events: vec![bugcheck.clone()],
            next_cursors: BTreeMap::from([("System".into(), cursor_for(&bugcheck))]),
        };
        db.with_writer(|conn| persist_event_batch(conn, &third_batch, anchor + 35_000))
            .unwrap();
        db.read(|conn| {
            let row: (i64, String, String) = conn.query_row(
                "SELECT id, classification, stable_key FROM crash_case",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            assert_eq!(row.0, case_id);
            assert_eq!(row.1, "bsod");
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                1
            );
            assert!(!row.2.is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn detector_drains_more_than_one_event_page_and_repeated_drain_is_idempotent() {
        let db = db();
        let mut events = (0..300)
            .map(|index| {
                NormalizedSystemEvent::fixture(
                    SystemEventKind::Other,
                    index.to_string(),
                    index as i64 + 1_000,
                )
            })
            .collect::<Vec<_>>();
        events.push(NormalizedSystemEvent::fixture(
            SystemEventKind::UnexpectedShutdown,
            "crash",
            400_000,
        ));
        let mut reader = FixtureEventReader {
            events,
            error: None,
        };
        assert_eq!(scan_until_caught_up(&db, &mut reader, 500_000).unwrap(), 2);
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM system_event", [], |row| row
                    .get::<_, i64>(0))?,
                301
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |row| row
                    .get::<_, i64>(0))?,
                1
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(scan_until_caught_up(&db, &mut reader, 500_000).unwrap(), 0);
        db.read(|conn| {
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM system_event", [], |row| row
                    .get::<_, i64>(0))?,
                301
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn strong_kill_fixture_without_system_boundary_is_not_a_crash_case() {
        let db = db();
        db.with_writer(|conn| {
            crate::db::writer::start_runtime_session(conn, 1_000)?;
            crate::db::writer::recover_open_intervals(conn, 2_000)?;
            Ok(())
        })
        .unwrap();
        let mut reader = FixtureEventReader {
            events: vec![NormalizedSystemEvent::fixture(
                SystemEventKind::ApplicationCrash,
                "30",
                1_500,
            )],
            error: None,
        };
        scan_once(&db, &mut reader, 2_000).unwrap();
        db.read(|conn| {
            let cases: i64 =
                conn.query_row("SELECT COUNT(*) FROM crash_case", [], |row| row.get(0))?;
            let holds: i64 =
                conn.query_row("SELECT COUNT(*) FROM retention_hold", [], |row| row.get(0))?;
            assert_eq!(cases, 0);
            assert_eq!(holds, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn active_and_overlapping_holds_protect_delete_selection_until_released() {
        let db = db();
        db.with_writer(|conn| {
            let session_id: i64 = conn.query_row(
                "SELECT id FROM collection_session ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO sample_frame(
                    collection_session_id, ts, sequence, duration_ms, process_snapshot_present
                 ) VALUES (?1, 100, 1, 100, 0)",
                [session_id],
            )?;
            let frame_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO crash_case(
                    stable_key, anchor_time_ms, classification, window_start_ms, window_end_ms,
                    evidence_status, processing_version
                 ) VALUES ('hold-fixture', 100, 'unexpected_shutdown', 0, 200, 'pending', ?1)",
                [CRASH_EVIDENCE_PROCESSING_VERSION],
            )?;
            let case_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO retention_hold(crash_case_id, start_ms, end_ms) VALUES (?1, 0, 200)",
                [case_id],
            )?;
            assert!(crate::db::writer::select_prunable_frame_ids(conn, 1_000, 10)?.is_empty());
            conn.execute(
                "INSERT INTO retention_hold(crash_case_id, start_ms, end_ms) VALUES (?1, 50, 150)",
                [case_id],
            )?;
            release_case_hold(conn, case_id, 300)?;
            assert_eq!(
                crate::db::writer::select_prunable_frame_ids(conn, 1_000, 10)?,
                vec![frame_id]
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn clear_collected_data_refuses_active_holds_and_preserves_events_until_release() {
        let db = db();
        let (before_id, inside_id, after_id) = db
            .with_writer(|conn| {
                let session_id: i64 = conn.query_row(
                    "SELECT id FROM collection_session ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )?;
                let mut ids = Vec::new();
                for (timestamp_ms, record_id) in [(100_i64, "before"), (200, "inside"), (300, "after")] {
                    conn.execute(
                        "INSERT INTO sample_frame(
                            collection_session_id, ts, sequence, duration_ms, process_snapshot_present
                         ) VALUES (?1, ?2, ?3, 50, 0)",
                        params![session_id, timestamp_ms, timestamp_ms],
                    )?;
                    ids.push((record_id, conn.last_insert_rowid()));
                }
                conn.execute(
                    "INSERT INTO system_event(channel, event_id, record_id, event_time_ms, provider, payload_summary)
                     VALUES ('System', '41', 'held-event', 200, 'Microsoft-Windows-Kernel-Power', '{\"kind\":\"unexpected_shutdown\"}')",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO crash_case(
                        stable_key, anchor_time_ms, classification, window_start_ms, window_end_ms,
                        evidence_status, processing_version
                     ) VALUES ('clear-hold-fixture', 200, 'unexpected_shutdown', 190, 250, 'pending', ?1)",
                    [CRASH_EVIDENCE_PROCESSING_VERSION],
                )?;
                let case_id = conn.last_insert_rowid();
                conn.execute(
                    "INSERT INTO retention_hold(crash_case_id, start_ms, end_ms) VALUES (?1, 190, 250)",
                    [case_id],
                )?;
                Ok((
                    ids[0].1,
                    ids[1].1,
                    ids[2].1,
                ))
            })
            .unwrap();
        db.with_writer(|conn| {
            let prunable = crate::db::writer::select_prunable_frame_ids(conn, 1_000, 10)?;
            assert_eq!(prunable, vec![before_id, after_id]);
            assert_eq!(crate::db::writer::prune_system_samples(conn, 1_000)?, 2);
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM sample_frame WHERE id = ?1",
                    [inside_id],
                    |row| row.get::<_, i64>(0)
                )?,
                1
            );
            let error = crate::db::writer::clear_collected_data(conn).unwrap_err();
            assert!(error.to_string().contains("release holds"));
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM sample_frame WHERE id = ?1",
                    [inside_id],
                    |row| row.get::<_, i64>(0)
                )?,
                1
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM system_event WHERE record_id = 'held-event'",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                1
            );
            let case_id: i64 = conn.query_row(
                "SELECT id FROM crash_case WHERE stable_key = 'clear-hold-fixture'",
                [],
                |row| row.get(0),
            )?;
            release_case_hold(conn, case_id, 500)?;
            crate::db::writer::clear_collected_data(conn)?;
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM sample_frame WHERE id = ?1",
                    [inside_id],
                    |row| row.get::<_, i64>(0)
                )?,
                0
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM system_event WHERE record_id = 'held-event'",
                    [],
                    |row| row.get::<_, i64>(0)
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn only_the_ten_most_recent_cases_keep_active_holds() {
        let db = db();
        db.with_writer(|conn| {
            let tx = conn.unchecked_transaction()?;
            for index in 0..11 {
                let anchor_time_ms = index * (CRASH_INCIDENT_CORRELATION_MS + 1_000);
                let event = NormalizedSystemEvent::fixture(
                    SystemEventKind::UnexpectedShutdown,
                    format!("hold-{index}"),
                    anchor_time_ms,
                );
                let candidate = CrashCandidate {
                    classification: CrashClassification::UnexpectedShutdown,
                    anchor_time_ms,
                    primary_event: event,
                    episode_key: None,
                    anchor_quality: 3,
                };
                create_case_and_hold_tx(&tx, &candidate, 99_000)?;
            }
            tx.commit()?;
            let active: i64 = conn.query_row(
                "SELECT COUNT(*) FROM retention_hold WHERE released_at_ms IS NULL",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(active, 10);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn payload_summary_is_normalized_and_does_not_store_raw_content() {
        let event = NormalizedSystemEvent::from_fields(
            "System",
            Some("Microsoft-Windows-WER-SystemErrorReporting".into()),
            "1001",
            "1",
            1_000,
            EventPayloadFacts {
                bugcheck_code: Some("0x00000116".into()),
                ..EventPayloadFacts::default()
            },
        );
        let summary = event.payload_summary();
        assert!(summary.contains("bugcheck_code"));
        assert!(!summary.contains("<Event"));
        assert!(!summary.contains("window title"));
        assert!(!summary.contains("http"));
        assert!(!summary.contains("command line"));
    }

    #[test]
    fn cursor_overlap_deduplicates_record_ids() {
        let event = NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "42", 1_000);
        let cursor = cursor_for(&event);
        assert!(!cursor_is_after(&event, &cursor));
        let later = NormalizedSystemEvent::fixture(SystemEventKind::NormalBoot, "43", 1_000);
        assert!(cursor_is_after(&later, &cursor));
    }

    #[test]
    fn schema_version_is_not_changed_by_crash_module() {
        let mut conn = Connection::open_in_memory().unwrap();
        schema::migrate(&mut conn).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 8);
    }

    fn insert_evidence_fixture(db: &Database, anchor_time_ms: i64) -> i64 {
        db.with_writer(|conn| {
            let session_id: i64 = conn.query_row(
                "SELECT id FROM collection_session ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )?;
            let mut frame_ids = Vec::new();
            for (index, timestamp_ms) in [anchor_time_ms - 60_000, anchor_time_ms - 30_000]
                .into_iter()
                .enumerate()
            {
                conn.execute(
                    "INSERT INTO sample_frame(
                        collection_session_id, ts, sequence, duration_ms, writer_delay_ms,
                        process_snapshot_present
                     ) VALUES (?1, ?2, ?3, 30_000, 25, 1)",
                    params![session_id, timestamp_ms, index as i64 + 1],
                )?;
                let frame_id = conn.last_insert_rowid();
                frame_ids.push(frame_id);
                conn.execute(
                    "INSERT INTO cpu_sample(frame_id, usage_pct) VALUES (?1, ?2)",
                    params![frame_id, if index == 0 { 10.0 } else { 30.0 }],
                )?;
                conn.execute(
                    "INSERT INTO memory_sample(frame_id, used_bytes, usage_pct)
                     VALUES (?1, ?2, ?3)",
                    params![frame_id, if index == 0 { 100_i64 } else { 300_i64 }, 20.0 + index as f64 * 10.0],
                )?;
            }
            conn.execute(
                "INSERT INTO app(stable_key, process_name, display_name, first_seen_at_ms, last_seen_at_ms)
                 VALUES ('app:test', 'fixture', 'Fixture', ?1, ?1)",
                [anchor_time_ms],
            )?;
            let app_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO app_executable(app_id, normalized_path, first_seen_at_ms, last_seen_at_ms)
                 VALUES (?1, 'path:C:\\fixture.exe', ?2, ?2)",
                params![app_id, anchor_time_ms],
            )?;
            let executable_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO process_instance(
                    app_executable_id, stable_key, pid, create_time_ms, source
                 ) VALUES (?1, 'process:pid:77:start:1:exe:path:C:\\fixture.exe', 77, 1, 'runtime')",
                params![executable_id],
            )?;
            let process_id = conn.last_insert_rowid();
            for (index, frame_id) in frame_ids.iter().copied().enumerate() {
                conn.execute(
                    "INSERT INTO process_sample(
                        frame_id, process_instance_id, cpu_pct, working_set_bytes,
                        private_bytes, read_bps, write_bps, selection_reason, quality_mask
                     ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, 0)",
                    params![
                        frame_id,
                        process_id,
                        if index == 0 { 5.0 } else { 15.0 },
                        if index == 0 { 100_i64 } else { 300_i64 },
                        if index == 0 { 1_000_i64 } else { 2_000_i64 },
                        if index == 0 { 100_i64 } else { 200_i64 },
                        if index == 0 { 1_i64 } else { 2_i64 },
                    ],
                )?;
            }
            for (device_key, temperatures) in [
                ("gpu:fixture:a", (40.0, 80.0)),
                ("gpu:fixture:b", (50.0, 70.0)),
            ] {
                conn.execute(
                    "INSERT INTO hardware_device(
                        stable_key, category, first_seen_at_ms, last_seen_at_ms
                     ) VALUES (?1, 'gpu', ?2, ?2)",
                    params![device_key, anchor_time_ms],
                )?;
                let device_id = conn.last_insert_rowid();
                for (index, frame_id) in frame_ids.iter().copied().enumerate() {
                    conn.execute(
                        "INSERT INTO gpu_sample(
                            frame_id, device_id, usage_pct, temp_c, vram_used_bytes,
                            quality_mask
                         ) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                        params![
                            frame_id,
                            device_id,
                            if index == 0 { 10.0 } else { 30.0 },
                            if index == 0 { temperatures.0 } else { temperatures.1 },
                            if index == 0 { 100_i64 } else { 300_i64 },
                        ],
                    )?;
                }
            }
            conn.execute(
                "INSERT INTO crash_case(
                    stable_key, anchor_time_ms, classification, window_start_ms, window_end_ms,
                    evidence_status, processing_version
                 ) VALUES ('fixture-case', ?1, 'unexpected_shutdown', ?2, ?3, 'pending', ?4)",
                params![
                    anchor_time_ms,
                    anchor_time_ms - CRASH_CASE_WINDOW_PRE_MS,
                    anchor_time_ms + CRASH_CASE_WINDOW_POST_MS,
                    CRASH_EVIDENCE_PROCESSING_VERSION,
                ],
            )?;
            let case_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO retention_hold(crash_case_id, start_ms, end_ms)
                 VALUES (?1, ?2, ?3)",
                params![
                    case_id,
                    anchor_time_ms - CRASH_CASE_WINDOW_PRE_MS,
                    anchor_time_ms + CRASH_CASE_WINDOW_POST_MS,
                ],
            )?;
            Ok(case_id)
        })
        .unwrap()
    }

    fn insert_complete_pre_window_fixture(db: &Database, anchor_time_ms: i64) -> i64 {
        let case_id = insert_evidence_fixture(db, anchor_time_ms);
        db.with_writer(|conn| {
            let session_id: i64 = conn.query_row(
                "SELECT id FROM collection_session ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO sample_frame(
                    collection_session_id, ts, sequence, duration_ms, writer_delay_ms,
                    process_snapshot_present
                 ) VALUES (?1, ?2, 3, ?3, 25, 0)",
                params![
                    session_id,
                    anchor_time_ms - CRASH_CASE_WINDOW_PRE_MS,
                    CRASH_CASE_WINDOW_PRE_MS - 60_000,
                ],
            )?;
            let frame_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO cpu_sample(frame_id, usage_pct) VALUES (?1, 20.0)",
                [frame_id],
            )?;
            Ok(())
        })
        .unwrap();
        case_id
    }

    #[test]
    fn evidence_builder_persists_objective_windowed_math_and_rebuilds_idempotently() {
        let db = db();
        let anchor_time_ms = 60_000;
        let case_id = insert_evidence_fixture(&db, anchor_time_ms);
        build_case(&db, case_id).unwrap();

        let cpu: (f64, f64, f64, f64, i64, f64, String) = db
            .read(|conn| {
                conn.query_row(
                    "SELECT avg_value, min_value, max_value, delta_value, peak_time_ms,
                            coverage, evidence_ref
                     FROM crash_evidence_summary
                     WHERE crash_case_id = ?1
                       AND metric_key = 'window:pre_1m:metric:cpu_percent'",
                    [case_id],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                        ))
                    },
                )
            })
            .unwrap();
        assert_eq!(cpu.0, 20.0);
        assert_eq!(cpu.1, 10.0);
        assert_eq!(cpu.2, 30.0);
        assert_eq!(cpu.3, 20.0);
        assert_eq!(cpu.4, anchor_time_ms - 30_000);
        assert_eq!(cpu.5, 1.0);
        assert!(cpu.6.contains("\"sample_count\":2"));

        let (status, summary_count, active_holds, gpu_rows): (String, i64, i64, i64) = db
            .read(|conn| {
                Ok((
                    conn.query_row(
                        "SELECT evidence_status FROM crash_case WHERE id = ?1",
                        [case_id],
                        |row| row.get(0),
                    )?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM crash_evidence_summary WHERE crash_case_id = ?1",
                        [case_id],
                        |row| row.get(0),
                    )?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM retention_hold
                         WHERE crash_case_id = ?1 AND released_at_ms IS NULL",
                        [case_id],
                        |row| row.get(0),
                    )?,
                    conn.query_row(
                        "SELECT COUNT(*) FROM crash_evidence_summary
                         WHERE crash_case_id = ?1
                           AND metric_key LIKE 'window:pre_1m:metric:gpu_temperature_celsius:device:%'",
                        [case_id],
                        |row| row.get(0),
                    )?,
                ))
            })
            .unwrap();
        assert_eq!(status, "partial");
        assert!(summary_count > 0);
        assert_eq!(active_holds, 1);
        assert_eq!(gpu_rows, 2);

        let detail = db
            .read(|conn| get_crash_case_detail(conn, case_id))
            .unwrap();
        assert_eq!(detail.case.id, case_id);
        assert!(detail.events.is_empty());
        assert!(detail.metrics.iter().any(|metric| {
            metric.window == CrashEvidenceWindow::Pre1m
                && metric.metric == "cpu_percent"
                && metric.sample_count == 2
        }));
        assert!(detail.processes.iter().any(|process| {
            process.window == CrashEvidenceWindow::Pre1m
                && process.process_identity_key.contains("pid:77")
                && process.selection_reason_mask == 3
                && process.read_bytes == 90_000
        }));

        build_case(&db, case_id).unwrap();
        db.read(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM crash_evidence_summary WHERE crash_case_id = ?1",
                [case_id],
                |row| row.get(0),
            )?;
            assert_eq!(count, summary_count);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn post_window_is_pending_until_elapsed_then_deferred_retry_builds_it() {
        let db = db();
        let anchor_time_ms = 1_000_000;
        let case_id = insert_evidence_fixture(&db, anchor_time_ms);
        build_case_with_failure_status_at(&db, case_id, anchor_time_ms + 2 * 60_000).unwrap();
        db.read(|conn| {
            let status: String = conn.query_row(
                "SELECT evidence_status FROM crash_case WHERE id = ?1",
                [case_id],
                |row| row.get(0),
            )?;
            assert_eq!(status, "post_pending");
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM crash_evidence_summary
                     WHERE crash_case_id = ?1 AND metric_key LIKE 'window:post_5m:%'",
                    [case_id],
                    |row| row.get::<_, i64>(0),
                )?,
                0
            );
            Ok(())
        })
        .unwrap();
        db.with_writer(|conn| {
            let session_id: i64 = conn.query_row(
                "SELECT id FROM collection_session ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO sample_frame(
                    collection_session_id, ts, sequence, duration_ms, process_snapshot_present
                 ) VALUES (?1, ?2, 3, 30_000, 0)",
                params![session_id, anchor_time_ms + 60_000],
            )?;
            let frame_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO cpu_sample(frame_id, usage_pct) VALUES (?1, 40.0)",
                [frame_id],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            rebuild_due_cases(&db, anchor_time_ms + CRASH_CASE_WINDOW_POST_MS).unwrap(),
            1
        );
        db.read(|conn| {
            let status: String = conn.query_row(
                "SELECT evidence_status FROM crash_case WHERE id = ?1",
                [case_id],
                |row| row.get(0),
            )?;
            assert_eq!(status, "partial");
            assert!(
                conn.query_row(
                    "SELECT COUNT(*) FROM crash_evidence_summary
                 WHERE crash_case_id = ?1 AND metric_key LIKE 'window:post_5m:%'",
                    [case_id],
                    |row| row.get::<_, i64>(0),
                )? > 0
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            rebuild_due_cases(
                &db,
                anchor_time_ms
                    + CRASH_CASE_WINDOW_POST_MS
                    + CRASH_POST_FINALIZATION_STABILIZATION_MS
                    + 1,
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn post_window_late_tail_retries_after_first_partial_finalization_and_completes() {
        let db = db();
        let anchor_time_ms = 2_000_000_i64;
        let case_id = insert_complete_pre_window_fixture(&db, anchor_time_ms);
        let window_end_ms = anchor_time_ms + CRASH_CASE_WINDOW_POST_MS;

        // This is the race: the first finalization runs at exactly T+5m, before the final frame
        // is committed.
        build_case_with_failure_status_at(&db, case_id, window_end_ms).unwrap();
        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT evidence_status FROM crash_case WHERE id = ?1",
                    [case_id],
                    |row| row.get::<_, String>(0),
                )?,
                "partial"
            );
            Ok(())
        })
        .unwrap();

        db.with_writer(|conn| {
            let session_id: i64 = conn.query_row(
                "SELECT id FROM collection_session ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO sample_frame(
                    collection_session_id, ts, sequence, duration_ms, writer_delay_ms,
                    process_snapshot_present
                 ) VALUES (?1, ?2, 4, ?3, 25, 0)",
                params![session_id, anchor_time_ms, CRASH_CASE_WINDOW_POST_MS],
            )?;
            let frame_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO cpu_sample(frame_id, usage_pct) VALUES (?1, 40.0)",
                [frame_id],
            )?;
            Ok(())
        })
        .unwrap();

        assert_eq!(rebuild_due_cases(&db, window_end_ms + 800).unwrap(), 1);
        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT evidence_status FROM crash_case WHERE id = ?1",
                    [case_id],
                    |row| row.get::<_, String>(0),
                )?,
                "complete"
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(
            rebuild_due_cases(
                &db,
                window_end_ms + CRASH_POST_FINALIZATION_STABILIZATION_MS + 1,
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn post_window_true_missing_settles_partial_without_infinite_retries() {
        let db = db();
        let anchor_time_ms = 3_000_000_i64;
        let case_id = insert_complete_pre_window_fixture(&db, anchor_time_ms);
        let window_end_ms = anchor_time_ms + CRASH_CASE_WINDOW_POST_MS;
        build_case_with_failure_status_at(&db, case_id, window_end_ms).unwrap();

        assert_eq!(
            rebuild_due_cases(
                &db,
                window_end_ms + CRASH_POST_FINALIZATION_STABILIZATION_MS + 1,
            )
            .unwrap(),
            0
        );
        db.read(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT evidence_status FROM crash_case WHERE id = ?1",
                    [case_id],
                    |row| row.get::<_, String>(0),
                )?,
                "partial"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn evidence_failure_marks_case_failed_and_keeps_hold() {
        let db = db();
        let case_id = insert_evidence_fixture(&db, 60_000);
        db.with_writer(|conn| {
            conn.execute("DROP TABLE cpu_sample", [])?;
            Ok(())
        })
        .unwrap();

        assert!(build_case_with_failure_status(&db, case_id).is_err());
        db.read(|conn| {
            let status: String = conn.query_row(
                "SELECT evidence_status FROM crash_case WHERE id = ?1",
                [case_id],
                |row| row.get(0),
            )?;
            let active_hold: i64 = conn.query_row(
                "SELECT COUNT(*) FROM retention_hold
                 WHERE crash_case_id = ?1 AND released_at_ms IS NULL",
                [case_id],
                |row| row.get(0),
            )?;
            assert_eq!(status, "failed");
            assert_eq!(active_hold, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn summary_key_parsing_preserves_dimension_identity_colons() {
        let (window, metric, device_key, process_key) =
            parse_summary_key("window:pre_1m:metric:gpu_temperature_celsius:device:gpu:fixture:a");
        assert_eq!(window, CrashEvidenceWindow::Pre1m);
        assert_eq!(metric, "gpu_temperature_celsius");
        assert_eq!(device_key.as_deref(), Some("gpu:fixture:a"));
        assert_eq!(process_key, None);

        let (_, metric, device_key, process_key) = parse_summary_key(
            "window:pre_1m:metric:process_cpu_percent:process:pid:77:start:1:exe:path:x",
        );
        assert_eq!(metric, "process_cpu_percent");
        assert_eq!(device_key, None);
        assert_eq!(process_key.as_deref(), Some("pid:77:start:1:exe:path:x"));
    }
}
