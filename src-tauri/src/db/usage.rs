use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeRange {
    pub start_ms: i64,
    pub end_ms: i64,
}

impl TimeRange {
    pub fn clipped(self, start_ms: i64, end_ms: i64) -> Option<Self> {
        let start_ms = self.start_ms.max(start_ms);
        let end_ms = self.end_ms.min(end_ms);
        (end_ms > start_ms).then_some(Self { start_ms, end_ms })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRange {
    pub boot_session_id: i64,
    pub state: String,
    pub range: TimeRange,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageDurations {
    pub active_ms: i64,
    pub idle_ms: i64,
}

pub fn intersect_foreground(
    boot_session_id: i64,
    foreground: TimeRange,
    states: &[StateRange],
) -> UsageDurations {
    let mut cursor = foreground.start_ms;
    let mut durations = UsageDurations::default();

    for state in states
        .iter()
        .filter(|state| state.boot_session_id == boot_session_id)
        .filter_map(|state| {
            state
                .range
                .clipped(foreground.start_ms, foreground.end_ms)
                .map(|range| (state, range))
        })
    {
        let (state, range) = state;
        let start_ms = range.start_ms.max(cursor);
        let end_ms = range.end_ms;
        if end_ms <= start_ms {
            continue;
        }
        let duration_ms = end_ms - start_ms;
        match state.state.as_str() {
            "active" => durations.active_ms = durations.active_ms.saturating_add(duration_ms),
            "idle" => durations.idle_ms = durations.idle_ms.saturating_add(duration_ms),
            _ => {}
        }
        cursor = cursor.max(end_ms);
        if cursor >= foreground.end_ms {
            break;
        }
    }

    durations
}

pub fn foreground_state_segments(
    boot_session_id: i64,
    foreground: TimeRange,
    states: &[StateRange],
) -> Vec<(TimeRange, String)> {
    let mut cursor = foreground.start_ms;
    let mut segments = Vec::new();

    for state in states
        .iter()
        .filter(|state| state.boot_session_id == boot_session_id)
        .filter_map(|state| {
            state
                .range
                .clipped(foreground.start_ms, foreground.end_ms)
                .map(|range| (state, range))
        })
    {
        let (state, range) = state;
        let start_ms = range.start_ms.max(cursor);
        let end_ms = range.end_ms;
        if end_ms <= start_ms {
            continue;
        }
        if matches!(state.state.as_str(), "active" | "idle") {
            segments.push((TimeRange { start_ms, end_ms }, state.state.clone()));
        }
        cursor = cursor.max(end_ms);
        if cursor >= foreground.end_ms {
            break;
        }
    }

    segments
}

pub fn computer_active_duration(states: &[StateRange], start_ms: i64, end_ms: i64) -> i64 {
    let boots: BTreeSet<i64> = states.iter().map(|state| state.boot_session_id).collect();
    boots
        .into_iter()
        .map(|boot_session_id| {
            intersect_foreground(boot_session_id, TimeRange { start_ms, end_ms }, states).active_ms
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(boot_session_id: i64, state: &str, start_ms: i64, end_ms: i64) -> StateRange {
        StateRange {
            boot_session_id,
            state: state.into(),
            range: TimeRange { start_ms, end_ms },
        }
    }

    #[test]
    fn intersection_keeps_foreground_whole_but_splits_derived_durations() {
        let states = vec![
            state(1, "active", 0, 5),
            state(1, "idle", 5, 15),
            state(1, "active", 15, 20),
        ];
        assert_eq!(
            intersect_foreground(
                1,
                TimeRange {
                    start_ms: 0,
                    end_ms: 20
                },
                &states
            ),
            UsageDurations {
                active_ms: 10,
                idle_ms: 10,
            }
        );
        assert_eq!(
            foreground_state_segments(
                1,
                TimeRange {
                    start_ms: 0,
                    end_ms: 20
                },
                &states
            )
            .iter()
            .map(|(range, _)| range.end_ms - range.start_ms)
            .sum::<i64>(),
            20
        );
    }

    #[test]
    fn unknown_states_and_gaps_are_not_attributed() {
        let states = vec![state(1, "active", 0, 5), state(1, "unknown", 10, 20)];
        assert_eq!(
            intersect_foreground(
                1,
                TimeRange {
                    start_ms: 0,
                    end_ms: 20
                },
                &states
            ),
            UsageDurations {
                active_ms: 5,
                idle_ms: 0,
            }
        );
    }

    #[test]
    fn overlapping_state_rows_are_counted_once() {
        let states = vec![
            state(1, "active", 0, 10),
            state(1, "idle", 5, 15),
            state(1, "active", 15, 20),
        ];
        assert_eq!(
            intersect_foreground(
                1,
                TimeRange {
                    start_ms: 0,
                    end_ms: 20
                },
                &states
            ),
            UsageDurations {
                active_ms: 15,
                idle_ms: 5,
            }
        );
    }
}
