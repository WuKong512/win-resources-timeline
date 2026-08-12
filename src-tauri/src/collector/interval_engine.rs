use crate::models::ActivityState;

const NO_FOREGROUND_GRACE_MS: i64 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalAction {
    Start {
        app_id: i64,
        at_ms: i64,
        activity: ActivityState,
    },
    Checkpoint {
        at_ms: i64,
    },
    Close {
        at_ms: i64,
        reason: &'static str,
    },
}

#[derive(Debug, Clone)]
struct OpenState {
    app_id: i64,
    activity: ActivityState,
    last_seen_ms: i64,
}

pub struct IntervalEngine {
    open: Option<OpenState>,
    no_foreground_since_ms: Option<i64>,
    last_tick_ms: Option<i64>,
    paused: bool,
    expected_tick_ms: i64,
}

impl Default for IntervalEngine {
    fn default() -> Self {
        Self {
            open: None,
            no_foreground_since_ms: None,
            last_tick_ms: None,
            paused: false,
            expected_tick_ms: 1_000,
        }
    }
}

impl IntervalEngine {
    pub fn set_expected_tick_ms(&mut self, expected_tick_ms: u64) {
        self.expected_tick_ms = i64::try_from(expected_tick_ms).unwrap_or(i64::MAX).max(1);
    }

    pub fn observe(
        &mut self,
        now_ms: i64,
        observation: Option<(i64, ActivityState)>,
    ) -> Vec<IntervalAction> {
        let mut actions = Vec::new();
        if self.paused {
            self.last_tick_ms = Some(now_ms);
            return actions;
        }
        if let Some(last_tick) = self.last_tick_ms {
            let long_gap_ms = self.expected_tick_ms.saturating_mul(5) / 2;
            if now_ms < last_tick || now_ms - last_tick > long_gap_ms {
                if let Some(open) = self.open.take() {
                    let close_at = if now_ms < last_tick {
                        open.last_seen_ms
                    } else {
                        open.last_seen_ms.saturating_add(self.expected_tick_ms)
                    };
                    actions.push(IntervalAction::Close {
                        at_ms: close_at,
                        reason: "clock_gap",
                    });
                }
            }
        }
        self.last_tick_ms = Some(now_ms);
        match observation {
            Some((app_id, activity)) => {
                self.no_foreground_since_ms = None;
                match &mut self.open {
                    Some(open) if open.app_id == app_id && open.activity == activity => {
                        open.last_seen_ms = now_ms;
                        actions.push(IntervalAction::Checkpoint { at_ms: now_ms });
                    }
                    Some(_) => {
                        let reason = if self.open.as_ref().is_some_and(|o| o.app_id == app_id) {
                            "activity_change"
                        } else {
                            "app_switch"
                        };
                        actions.push(IntervalAction::Close {
                            at_ms: now_ms,
                            reason,
                        });
                        self.open = Some(OpenState {
                            app_id,
                            activity,
                            last_seen_ms: now_ms,
                        });
                        actions.push(IntervalAction::Start {
                            app_id,
                            at_ms: now_ms,
                            activity,
                        });
                    }
                    None => {
                        self.open = Some(OpenState {
                            app_id,
                            activity,
                            last_seen_ms: now_ms,
                        });
                        actions.push(IntervalAction::Start {
                            app_id,
                            at_ms: now_ms,
                            activity,
                        });
                    }
                }
            }
            None => {
                let since = *self.no_foreground_since_ms.get_or_insert(now_ms);
                if now_ms - since >= NO_FOREGROUND_GRACE_MS && self.open.take().is_some() {
                    actions.push(IntervalAction::Close {
                        at_ms: since,
                        reason: "no_foreground",
                    });
                }
            }
        }
        actions
    }

    pub fn pause(&mut self, now_ms: i64) -> Vec<IntervalAction> {
        self.paused = true;
        self.no_foreground_since_ms = None;
        self.open
            .take()
            .map(|_| {
                vec![IntervalAction::Close {
                    at_ms: now_ms,
                    reason: "paused",
                }]
            })
            .unwrap_or_default()
    }
    pub fn resume(&mut self, now_ms: i64) {
        self.paused = false;
        self.last_tick_ms = Some(now_ms);
    }
    pub fn terminate(&mut self, now_ms: i64, reason: &'static str) -> Vec<IntervalAction> {
        self.open
            .take()
            .map(|_| {
                vec![IntervalAction::Close {
                    at_ms: now_ms,
                    reason,
                }]
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starts_and_continues_without_duplicate_interval() {
        let mut e = IntervalEngine::default();
        assert!(matches!(
            e.observe(1_000, Some((1, ActivityState::Active)))
                .as_slice(),
            [IntervalAction::Start { .. }]
        ));
        assert!(matches!(
            e.observe(2_000, Some((1, ActivityState::Active)))
                .as_slice(),
            [IntervalAction::Checkpoint { .. }]
        ));
    }
    #[test]
    fn changes_split_intervals() {
        let mut e = IntervalEngine::default();
        e.observe(1_000, Some((1, ActivityState::Active)));
        assert!(matches!(
            e.observe(2_000, Some((1, ActivityState::Idle))).as_slice(),
            [
                IntervalAction::Close {
                    reason: "activity_change",
                    ..
                },
                IntervalAction::Start { .. }
            ]
        ));
        assert!(matches!(
            e.observe(3_000, Some((2, ActivityState::Idle))).as_slice(),
            [
                IntervalAction::Close {
                    reason: "app_switch",
                    ..
                },
                IntervalAction::Start { .. }
            ]
        ));
    }
    #[test]
    fn missing_foreground_has_grace_period() {
        let mut e = IntervalEngine::default();
        e.observe(1_000, Some((1, ActivityState::Active)));
        assert!(e.observe(2_000, None).is_empty());
        assert!(e.observe(3_000, None).is_empty());
        assert!(matches!(
            e.observe(4_000, None).as_slice(),
            [IntervalAction::Close {
                reason: "no_foreground",
                ..
            }]
        ));
    }
    #[test]
    fn long_gap_is_not_attributed() {
        let mut e = IntervalEngine::default();
        e.observe(1_000, Some((1, ActivityState::Active)));
        e.observe(2_000, Some((1, ActivityState::Active)));
        assert!(matches!(
            e.observe(10_000, Some((1, ActivityState::Active)))
                .as_slice(),
            [
                IntervalAction::Close {
                    at_ms: 3_000,
                    reason: "clock_gap"
                },
                IntervalAction::Start { at_ms: 10_000, .. }
            ]
        ));
    }
    #[test]
    fn configured_poll_interval_is_not_mistaken_for_a_gap() {
        let mut e = IntervalEngine::default();
        e.set_expected_tick_ms(5_000);
        e.observe(1_000, Some((1, ActivityState::Active)));
        assert!(matches!(
            e.observe(6_000, Some((1, ActivityState::Active)))
                .as_slice(),
            [IntervalAction::Checkpoint { .. }]
        ));
        assert!(matches!(
            e.observe(19_000, Some((1, ActivityState::Active)))
                .as_slice(),
            [
                IntervalAction::Close {
                    at_ms: 11_000,
                    reason: "clock_gap"
                },
                IntervalAction::Start { at_ms: 19_000, .. }
            ]
        ));
    }
    #[test]
    fn pause_resume_shutdown_do_not_bridge_time() {
        let mut e = IntervalEngine::default();
        e.observe(1_000, Some((1, ActivityState::Active)));
        assert!(matches!(
            e.pause(2_000).as_slice(),
            [IntervalAction::Close {
                reason: "paused",
                ..
            }]
        ));
        e.resume(9_000);
        assert!(matches!(
            e.observe(10_000, Some((1, ActivityState::Active)))
                .as_slice(),
            [IntervalAction::Start { .. }]
        ));
        assert!(matches!(
            e.terminate(11_000, "shutdown").as_slice(),
            [IntervalAction::Close {
                reason: "shutdown",
                ..
            }]
        ));
    }
}
