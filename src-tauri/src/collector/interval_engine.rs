use crate::models::ComputerState;

const DEFAULT_HEARTBEAT_MS: i64 = 20_000;
const GAP_MULTIPLIER: i64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageEvent {
    Foreground {
        app_executable_id: i64,
        at_ms: i64,
    },
    ForegroundUnavailable {
        at_ms: i64,
    },
    #[allow(dead_code)]
    ComputerState {
        state: ComputerState,
        at_ms: i64,
        source: &'static str,
        quality: i64,
    },
    IdleThresholdCrossed {
        at_ms: i64,
    },
    UserActive {
        at_ms: i64,
    },
    Locked {
        at_ms: i64,
    },
    Unlocked {
        at_ms: i64,
        state: ComputerState,
    },
    Suspend {
        at_ms: i64,
    },
    Resume {
        at_ms: i64,
        state: ComputerState,
    },
    Disconnected {
        at_ms: i64,
    },
    Connected {
        at_ms: i64,
        state: ComputerState,
    },
    Heartbeat {
        at_ms: i64,
        foreground_app_executable_id: Option<i64>,
        state: ComputerState,
    },
    Pause {
        at_ms: i64,
    },
    ResumeCollection {
        at_ms: i64,
        foreground_app_executable_id: Option<i64>,
        state: ComputerState,
    },
    Shutdown {
        at_ms: i64,
    },
    WindowsShutdown {
        at_ms: i64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalAction {
    StartForeground {
        app_executable_id: i64,
        at_ms: i64,
    },
    CheckpointForeground {
        at_ms: i64,
    },
    CloseForeground {
        at_ms: i64,
        reason: &'static str,
    },
    StartComputerState {
        state: ComputerState,
        at_ms: i64,
        source: &'static str,
        quality: i64,
    },
    CheckpointComputerState {
        at_ms: i64,
    },
    CloseComputerState {
        at_ms: i64,
        reason: &'static str,
    },
    MarkWindowsShutdown {
        at_ms: i64,
    },
}

#[derive(Debug, Clone)]
struct OpenForeground {
    app_executable_id: i64,
    last_seen_ms: i64,
}

#[derive(Debug, Clone)]
struct OpenComputerState {
    state: ComputerState,
    last_seen_ms: i64,
}

pub struct IntervalEngine {
    foreground: Option<OpenForeground>,
    computer_state: Option<OpenComputerState>,
    last_event_ms: Option<i64>,
    paused: bool,
    expected_heartbeat_ms: i64,
}

impl Default for IntervalEngine {
    fn default() -> Self {
        Self {
            foreground: None,
            computer_state: None,
            last_event_ms: None,
            paused: false,
            expected_heartbeat_ms: DEFAULT_HEARTBEAT_MS,
        }
    }
}

impl IntervalEngine {
    pub fn set_expected_heartbeat_ms(&mut self, expected_ms: u64) {
        self.expected_heartbeat_ms = i64::try_from(expected_ms).unwrap_or(i64::MAX).max(1);
    }

    pub fn handle(&mut self, event: UsageEvent) -> Vec<IntervalAction> {
        let mut actions = Vec::new();
        let at_ms = self.prepare_timestamp(event_timestamp(&event), &mut actions);
        match event {
            UsageEvent::Foreground {
                app_executable_id, ..
            } => self.observe_foreground(app_executable_id, at_ms, &mut actions),
            UsageEvent::ForegroundUnavailable { .. } => {
                self.close_foreground(at_ms, "unknown-foreground", &mut actions)
            }
            UsageEvent::ComputerState {
                state,
                source,
                quality,
                ..
            } => self.observe_computer_state(state, at_ms, source, quality, &mut actions),
            UsageEvent::IdleThresholdCrossed { .. } => {
                if self.current_computer_state() == Some(ComputerState::Active) {
                    self.observe_computer_state(
                        ComputerState::Idle,
                        at_ms,
                        "idle-threshold",
                        0,
                        &mut actions,
                    );
                }
            }
            UsageEvent::UserActive { .. } => {
                if self.current_computer_state() == Some(ComputerState::Idle) {
                    self.observe_computer_state(
                        ComputerState::Active,
                        at_ms,
                        "user-input",
                        0,
                        &mut actions,
                    );
                }
            }
            UsageEvent::Locked { .. } => self.lock(at_ms, &mut actions),
            UsageEvent::Unlocked { state, .. } => self.unlock(at_ms, state, &mut actions),
            UsageEvent::Suspend { .. } => self.suspend(at_ms, &mut actions),
            UsageEvent::Resume { state, .. } => self.resume(at_ms, state, &mut actions),
            UsageEvent::Disconnected { .. } => self.disconnect(at_ms, &mut actions),
            UsageEvent::Connected { state, .. } => self.connect(at_ms, state, &mut actions),
            UsageEvent::Heartbeat {
                foreground_app_executable_id,
                state,
                ..
            } => {
                if !self.paused {
                    self.observe_computer_state(state, at_ms, "heartbeat", 0, &mut actions);
                    if self.foreground_allowed() {
                        match foreground_app_executable_id {
                            Some(app_executable_id) => {
                                self.observe_foreground(app_executable_id, at_ms, &mut actions)
                            }
                            None => self.close_foreground(at_ms, "no_foreground", &mut actions),
                        }
                    } else {
                        self.close_foreground(at_ms, "system_state", &mut actions);
                    }
                }
            }
            UsageEvent::Pause { .. } => self.pause(at_ms, &mut actions),
            UsageEvent::ResumeCollection {
                foreground_app_executable_id,
                state,
                ..
            } => {
                self.paused = false;
                self.observe_computer_state(state, at_ms, "resume", 0, &mut actions);
                if self.foreground_allowed() {
                    if let Some(app_executable_id) = foreground_app_executable_id {
                        self.observe_foreground(app_executable_id, at_ms, &mut actions);
                    }
                }
            }
            UsageEvent::Shutdown { .. } => self.shutdown(at_ms, "resource-shutdown", &mut actions),
            UsageEvent::WindowsShutdown { at_ms } => {
                self.shutdown(at_ms, "windows-shutdown", &mut actions);
                actions.push(IntervalAction::MarkWindowsShutdown { at_ms });
            }
        }
        actions
    }

    fn prepare_timestamp(&mut self, timestamp_ms: i64, actions: &mut Vec<IntervalAction>) -> i64 {
        let at_ms = self
            .last_event_ms
            .map_or(timestamp_ms, |last| timestamp_ms.max(last));
        let gap = self.last_event_ms.is_some_and(|last| {
            timestamp_ms < last
                || timestamp_ms.saturating_sub(last)
                    > self.expected_heartbeat_ms.saturating_mul(GAP_MULTIPLIER)
        });
        if gap {
            if let Some(recovery_at) = self.foreground.as_ref().map(|open| open.last_seen_ms) {
                self.close_foreground(recovery_at, "clock-gap", actions);
            }
            if let Some(recovery_at) = self.computer_state.as_ref().map(|open| open.last_seen_ms) {
                self.close_computer_state(recovery_at, "clock-gap", actions);
            }
        }
        self.last_event_ms = Some(at_ms);
        at_ms
    }

    fn observe_foreground(
        &mut self,
        app_executable_id: i64,
        at_ms: i64,
        actions: &mut Vec<IntervalAction>,
    ) {
        if self.paused || !self.foreground_allowed() {
            return;
        }
        match self.foreground.as_mut() {
            Some(open) if open.app_executable_id == app_executable_id => {
                open.last_seen_ms = open.last_seen_ms.max(at_ms);
                actions.push(IntervalAction::CheckpointForeground { at_ms });
            }
            Some(_) => {
                self.close_foreground(at_ms, "app-switch", actions);
                self.foreground = Some(OpenForeground {
                    app_executable_id,
                    last_seen_ms: at_ms,
                });
                actions.push(IntervalAction::StartForeground {
                    app_executable_id,
                    at_ms,
                });
            }
            None => {
                self.foreground = Some(OpenForeground {
                    app_executable_id,
                    last_seen_ms: at_ms,
                });
                actions.push(IntervalAction::StartForeground {
                    app_executable_id,
                    at_ms,
                });
            }
        }
    }

    fn observe_computer_state(
        &mut self,
        state: ComputerState,
        at_ms: i64,
        source: &'static str,
        quality: i64,
        actions: &mut Vec<IntervalAction>,
    ) {
        if self.paused || !can_replace_state(self.current_computer_state(), state) {
            return;
        }
        match self.computer_state.as_mut() {
            Some(open) if open.state == state => {
                open.last_seen_ms = open.last_seen_ms.max(at_ms);
                actions.push(IntervalAction::CheckpointComputerState { at_ms });
            }
            Some(_) => {
                self.close_computer_state(at_ms, "state-change", actions);
                self.computer_state = Some(OpenComputerState {
                    state,
                    last_seen_ms: at_ms,
                });
                actions.push(IntervalAction::StartComputerState {
                    state,
                    at_ms,
                    source,
                    quality,
                });
            }
            None => {
                self.computer_state = Some(OpenComputerState {
                    state,
                    last_seen_ms: at_ms,
                });
                actions.push(IntervalAction::StartComputerState {
                    state,
                    at_ms,
                    source,
                    quality,
                });
            }
        }
    }

    fn lock(&mut self, at_ms: i64, actions: &mut Vec<IntervalAction>) {
        self.close_foreground(at_ms, "lock", actions);
        self.observe_computer_state(ComputerState::Locked, at_ms, "wts-lock", 0, actions);
    }

    fn unlock(&mut self, at_ms: i64, state: ComputerState, actions: &mut Vec<IntervalAction>) {
        self.observe_computer_state_forced(state, at_ms, "wts-unlock", 0, actions);
    }

    fn suspend(&mut self, at_ms: i64, actions: &mut Vec<IntervalAction>) {
        self.close_foreground(at_ms, "sleep", actions);
        self.observe_computer_state(ComputerState::Sleep, at_ms, "power-suspend", 0, actions);
    }

    fn resume(&mut self, at_ms: i64, state: ComputerState, actions: &mut Vec<IntervalAction>) {
        self.observe_computer_state_forced(state, at_ms, "power-resume", 0, actions);
    }

    fn disconnect(&mut self, at_ms: i64, actions: &mut Vec<IntervalAction>) {
        self.close_foreground(at_ms, "disconnected", actions);
        self.observe_computer_state(
            ComputerState::Disconnected,
            at_ms,
            "wts-disconnect",
            0,
            actions,
        );
    }

    fn connect(&mut self, at_ms: i64, state: ComputerState, actions: &mut Vec<IntervalAction>) {
        self.observe_computer_state_forced(state, at_ms, "wts-connect", 0, actions);
    }

    fn observe_computer_state_forced(
        &mut self,
        state: ComputerState,
        at_ms: i64,
        source: &'static str,
        quality: i64,
        actions: &mut Vec<IntervalAction>,
    ) {
        if self.paused {
            return;
        }
        match self.computer_state.as_mut() {
            Some(open) if open.state == state => {
                open.last_seen_ms = open.last_seen_ms.max(at_ms);
                actions.push(IntervalAction::CheckpointComputerState { at_ms });
            }
            Some(_) => {
                self.close_computer_state(at_ms, "trusted-state-change", actions);
                self.computer_state = Some(OpenComputerState {
                    state,
                    last_seen_ms: at_ms,
                });
                actions.push(IntervalAction::StartComputerState {
                    state,
                    at_ms,
                    source,
                    quality,
                });
            }
            None => {
                self.computer_state = Some(OpenComputerState {
                    state,
                    last_seen_ms: at_ms,
                });
                actions.push(IntervalAction::StartComputerState {
                    state,
                    at_ms,
                    source,
                    quality,
                });
            }
        }
    }

    fn pause(&mut self, at_ms: i64, actions: &mut Vec<IntervalAction>) {
        self.close_foreground(at_ms, "pause", actions);
        self.close_computer_state(at_ms, "pause", actions);
        self.paused = true;
    }

    fn shutdown(&mut self, at_ms: i64, reason: &'static str, actions: &mut Vec<IntervalAction>) {
        self.close_foreground(at_ms, reason, actions);
        self.close_computer_state(at_ms, reason, actions);
        self.paused = true;
    }

    fn close_foreground(
        &mut self,
        at_ms: i64,
        reason: &'static str,
        actions: &mut Vec<IntervalAction>,
    ) {
        if self.foreground.take().is_some() {
            actions.push(IntervalAction::CloseForeground { at_ms, reason });
        }
    }

    fn close_computer_state(
        &mut self,
        at_ms: i64,
        reason: &'static str,
        actions: &mut Vec<IntervalAction>,
    ) {
        if self.computer_state.take().is_some() {
            actions.push(IntervalAction::CloseComputerState { at_ms, reason });
        }
    }

    fn foreground_allowed(&self) -> bool {
        !self.paused
            && !matches!(
                self.current_computer_state(),
                Some(ComputerState::Locked | ComputerState::Sleep | ComputerState::Disconnected)
            )
    }

    fn current_computer_state(&self) -> Option<ComputerState> {
        self.computer_state.as_ref().map(|open| open.state)
    }

    #[cfg(test)]
    fn open_foreground(&self) -> Option<(i64, i64)> {
        self.foreground
            .as_ref()
            .map(|open| (open.app_executable_id, open.last_seen_ms))
    }

    #[cfg(test)]
    fn open_computer_state(&self) -> Option<(ComputerState, i64)> {
        self.computer_state
            .as_ref()
            .map(|open| (open.state, open.last_seen_ms))
    }
}

fn event_timestamp(event: &UsageEvent) -> i64 {
    match event {
        UsageEvent::Foreground { at_ms, .. }
        | UsageEvent::ForegroundUnavailable { at_ms }
        | UsageEvent::ComputerState { at_ms, .. }
        | UsageEvent::IdleThresholdCrossed { at_ms }
        | UsageEvent::UserActive { at_ms }
        | UsageEvent::Locked { at_ms }
        | UsageEvent::Unlocked { at_ms, .. }
        | UsageEvent::Suspend { at_ms }
        | UsageEvent::Resume { at_ms, .. }
        | UsageEvent::Disconnected { at_ms }
        | UsageEvent::Connected { at_ms, .. }
        | UsageEvent::Heartbeat { at_ms, .. }
        | UsageEvent::Pause { at_ms }
        | UsageEvent::ResumeCollection { at_ms, .. }
        | UsageEvent::Shutdown { at_ms }
        | UsageEvent::WindowsShutdown { at_ms } => *at_ms,
    }
}

fn state_rank(state: ComputerState) -> u8 {
    match state {
        ComputerState::Sleep => 5,
        ComputerState::Locked => 4,
        ComputerState::Disconnected => 3,
        ComputerState::Unknown => 2,
        ComputerState::Active | ComputerState::Idle => 1,
    }
}

fn can_replace_state(current: Option<ComputerState>, next: ComputerState) -> bool {
    current.is_none_or(|current| {
        state_rank(next) >= state_rank(current)
            || matches!(current, ComputerState::Active | ComputerState::Idle)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heartbeat(
        engine: &mut IntervalEngine,
        at_ms: i64,
        app_executable_id: Option<i64>,
        state: ComputerState,
    ) -> Vec<IntervalAction> {
        engine.handle(UsageEvent::Heartbeat {
            at_ms,
            foreground_app_executable_id: app_executable_id,
            state,
        })
    }

    #[test]
    fn idle_transition_does_not_split_foreground() {
        let mut engine = IntervalEngine::default();
        heartbeat(&mut engine, 0, Some(1), ComputerState::Active);
        heartbeat(&mut engine, 5_000, Some(1), ComputerState::Idle);
        heartbeat(&mut engine, 15_000, Some(1), ComputerState::Active);
        assert_eq!(engine.open_foreground(), Some((1, 15_000)));
        assert_eq!(
            engine.open_computer_state(),
            Some((ComputerState::Active, 15_000))
        );
    }

    #[test]
    fn app_switch_closes_before_starting_next_app() {
        let mut engine = IntervalEngine::default();
        heartbeat(&mut engine, 0, Some(1), ComputerState::Active);
        let actions = heartbeat(&mut engine, 10_000, Some(2), ComputerState::Active);
        assert_eq!(
            actions,
            vec![
                IntervalAction::CheckpointComputerState { at_ms: 10_000 },
                IntervalAction::CloseForeground {
                    at_ms: 10_000,
                    reason: "app-switch"
                },
                IntervalAction::StartForeground {
                    app_executable_id: 2,
                    at_ms: 10_000
                }
            ]
        );
    }

    #[test]
    fn lock_and_sleep_close_both_axes_without_bridging() {
        let mut engine = IntervalEngine::default();
        heartbeat(&mut engine, 0, Some(1), ComputerState::Active);
        let lock = engine.handle(UsageEvent::Locked { at_ms: 5_000 });
        assert!(lock.iter().any(|action| matches!(
            action,
            IntervalAction::CloseForeground {
                at_ms: 5_000,
                reason: "lock"
            }
        )));
        assert!(lock.iter().any(|action| matches!(
            action,
            IntervalAction::StartComputerState {
                state: ComputerState::Locked,
                at_ms: 5_000,
                ..
            }
        )));
        let unlock = engine.handle(UsageEvent::Unlocked {
            at_ms: 10_000,
            state: ComputerState::Active,
        });
        assert!(unlock.iter().any(|action| matches!(
            action,
            IntervalAction::CloseComputerState { at_ms: 10_000, .. }
        )));
        assert!(unlock.iter().any(|action| matches!(
            action,
            IntervalAction::StartComputerState {
                state: ComputerState::Active,
                at_ms: 10_000,
                ..
            }
        )));
        let resume = engine.handle(UsageEvent::ResumeCollection {
            at_ms: 20_000,
            foreground_app_executable_id: Some(1),
            state: ComputerState::Active,
        });
        assert!(
            resume.iter().any(|action| matches!(
                action,
                IntervalAction::StartForeground {
                    app_executable_id: 1,
                    at_ms: 20_000
                }
            )) || engine.open_foreground() == Some((1, 20_000))
        );
    }

    #[test]
    fn duplicate_events_are_debounced() {
        let mut engine = IntervalEngine::default();
        heartbeat(&mut engine, 0, Some(1), ComputerState::Active);
        let actions = heartbeat(&mut engine, 1_000, Some(1), ComputerState::Active);
        assert_eq!(
            actions,
            vec![
                IntervalAction::CheckpointComputerState { at_ms: 1_000 },
                IntervalAction::CheckpointForeground { at_ms: 1_000 }
            ]
        );
    }

    #[test]
    fn long_gap_closes_at_last_trusted_timestamp() {
        let mut engine = IntervalEngine::default();
        engine.set_expected_heartbeat_ms(10_000);
        heartbeat(&mut engine, 1_000, Some(1), ComputerState::Active);
        let actions = heartbeat(&mut engine, 40_000, Some(1), ComputerState::Active);
        assert!(actions.iter().any(|action| matches!(
            action,
            IntervalAction::CloseForeground {
                at_ms: 1_000,
                reason: "clock-gap"
            }
        )));
        assert!(actions.iter().any(|action| matches!(
            action,
            IntervalAction::StartForeground {
                app_executable_id: 1,
                at_ms: 40_000
            }
        )));
    }

    #[test]
    fn unknown_foreground_is_not_previous_app() {
        let mut engine = IntervalEngine::default();
        heartbeat(&mut engine, 0, Some(1), ComputerState::Active);
        let actions = heartbeat(&mut engine, 1_000, Some(99), ComputerState::Active);
        assert!(actions.iter().any(|action| matches!(
            action,
            IntervalAction::CloseForeground {
                at_ms: 1_000,
                reason: "app-switch"
            }
        )));
        assert_eq!(engine.open_foreground(), Some((99, 1_000)));
    }
}
