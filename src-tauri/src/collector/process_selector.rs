use std::collections::{BTreeMap, HashSet};

/// The first version deliberately keeps Top-N a backend constant. Five is the existing
/// process-snapshot budget and gives useful coverage without introducing another setting.
pub const DEFAULT_PROCESS_TOP_N: usize = 5;

pub const SELECTION_REASON_CPU_TOP_N: u32 = 1 << 0;
pub const SELECTION_REASON_MEMORY_TOP_N: u32 = 1 << 1;
pub const SELECTION_REASON_IO_TOP_N: u32 = 1 << 2;
pub const SELECTION_REASON_FOREGROUND: u32 = 1 << 3;
#[allow(dead_code)]
pub const SELECTION_REASON_WATCHED: u32 = 1 << 4;
#[allow(dead_code)]
pub const SELECTION_REASON_ANOMALY: u32 = 1 << 5;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessIdentity {
    pub pid: Option<u32>,
    pub creation_time_ms: Option<i64>,
    /// A normalized executable/app identity. It is intentionally not a command line.
    pub executable_key: String,
}

impl ProcessIdentity {
    pub fn stable_key(&self) -> String {
        match (self.pid, self.creation_time_ms) {
            (Some(pid), Some(creation_time_ms)) => format!(
                "process:pid:{pid}:start:{creation_time_ms}:exe:{}",
                self.executable_key
            ),
            (Some(pid), None) => format!("process:pid:{pid}:exe:{}", self.executable_key),
            _ => format!("process:exe:{}", self.executable_key),
        }
    }

    pub fn foreground_key(&self) -> Option<String> {
        self.pid
            .zip(self.creation_time_ms)
            .map(|(pid, creation_time_ms)| foreground_process_key(pid, creation_time_ms))
    }
}

pub fn foreground_process_key(pid: u32, creation_time_ms: i64) -> String {
    format!("foreground:pid:{pid}:start:{creation_time_ms}")
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessCandidate {
    pub identity: ProcessIdentity,
    pub app_key: String,
    pub process_name: String,
    pub exe_path: Option<String>,
    pub cpu_percent: Option<f64>,
    pub private_bytes: Option<i64>,
    pub working_set_bytes: Option<i64>,
    pub read_bytes_per_sec: Option<i64>,
    pub write_bytes_per_sec: Option<i64>,
    pub network_bytes_per_sec: Option<i64>,
    pub gpu_percent: Option<f64>,
    pub vram_bytes: Option<i64>,
    pub cpu_time_delta_us: Option<i64>,
    pub quality_mask: i64,
}

impl ProcessCandidate {
    pub fn stable_key(&self) -> String {
        self.identity.stable_key()
    }

    fn memory_value(&self) -> Option<i64> {
        self.private_bytes.or(self.working_set_bytes)
    }

    fn io_value(&self) -> Option<i64> {
        self.read_bytes_per_sec
            .or(self.write_bytes_per_sec)
            .map(|read_or_write| {
                self.read_bytes_per_sec
                    .unwrap_or(0)
                    .saturating_add(self.write_bytes_per_sec.unwrap_or(0))
                    .max(read_or_write)
            })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedProcess {
    pub candidate: ProcessCandidate,
    pub selection_reason_mask: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessSelector {
    top_n: usize,
}

impl Default for ProcessSelector {
    fn default() -> Self {
        Self::new(DEFAULT_PROCESS_TOP_N)
    }
}

impl ProcessSelector {
    pub const fn new(top_n: usize) -> Self {
        Self { top_n }
    }

    #[allow(dead_code)]
    pub const fn top_n(self) -> usize {
        self.top_n
    }

    /// Selects raw process instances before any logical-app aggregation.
    ///
    /// Every ranking has its own missing-value rule: a missing metric is not a measured zero
    /// and therefore cannot win that ranking. Ties use the stable process identity, never map
    /// iteration order. The result is bounded by three independent rankings plus one foreground
    /// process.
    pub fn select(
        &self,
        candidates: &[ProcessCandidate],
        foreground_key: Option<&str>,
    ) -> Vec<SelectedProcess> {
        let mut selected = BTreeMap::<String, (ProcessCandidate, u32)>::new();
        self.add_ranked(
            &mut selected,
            candidates,
            SELECTION_REASON_CPU_TOP_N,
            |candidate| candidate.cpu_percent.filter(|value| value.is_finite()),
        );
        self.add_ranked(
            &mut selected,
            candidates,
            SELECTION_REASON_MEMORY_TOP_N,
            |candidate| candidate.memory_value().map(|value| value as f64),
        );
        self.add_ranked(
            &mut selected,
            candidates,
            SELECTION_REASON_IO_TOP_N,
            |candidate| candidate.io_value().map(|value| value as f64),
        );

        if let Some(foreground_key) = foreground_key {
            if let Some(candidate) = candidates.iter().find(|candidate| {
                candidate
                    .identity
                    .foreground_key()
                    .as_deref()
                    .is_some_and(|key| key == foreground_key)
            }) {
                self.add_reason(&mut selected, candidate, SELECTION_REASON_FOREGROUND);
            }
        }

        selected
            .into_values()
            .map(|(candidate, selection_reason_mask)| SelectedProcess {
                candidate,
                selection_reason_mask,
            })
            .collect()
    }

    /// Compatibility helper for the collector's existing key set. Only a foreground-prefixed
    /// key participates; historical logical app keys are not treated as watched processes.
    pub fn select_with_foreground_keys(
        &self,
        candidates: &[ProcessCandidate],
        foreground_keys: &HashSet<String>,
    ) -> Vec<SelectedProcess> {
        let foreground_key = foreground_keys
            .iter()
            .filter(|key| key.starts_with("foreground:"))
            .min();
        self.select(candidates, foreground_key.map(String::as_str))
    }

    fn add_ranked<F>(
        &self,
        selected: &mut BTreeMap<String, (ProcessCandidate, u32)>,
        candidates: &[ProcessCandidate],
        reason: u32,
        score: F,
    ) where
        F: Fn(&ProcessCandidate) -> Option<f64>,
    {
        let mut ranked: Vec<_> = candidates
            .iter()
            .filter_map(|candidate| score(candidate).map(|value| (candidate, value)))
            .collect();
        ranked.sort_by(|(left, left_value), (right, right_value)| {
            right_value
                .total_cmp(left_value)
                .then_with(|| left.stable_key().cmp(&right.stable_key()))
        });
        for (candidate, _) in ranked.into_iter().take(self.top_n) {
            self.add_reason(selected, candidate, reason);
        }
    }

    fn add_reason(
        &self,
        selected: &mut BTreeMap<String, (ProcessCandidate, u32)>,
        candidate: &ProcessCandidate,
        reason: u32,
    ) {
        selected
            .entry(candidate.stable_key())
            .and_modify(|(_, mask)| *mask |= reason)
            .or_insert_with(|| (candidate.clone(), reason));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        name: &str,
        pid: u32,
        creation_time_ms: i64,
        cpu: Option<f64>,
        memory: Option<i64>,
        io: Option<i64>,
    ) -> ProcessCandidate {
        ProcessCandidate {
            identity: ProcessIdentity {
                pid: Some(pid),
                creation_time_ms: Some(creation_time_ms),
                executable_key: format!("path:{name}"),
            },
            app_key: format!("path:{name}"),
            process_name: format!("{name}.exe"),
            exe_path: Some(format!(r"C:\{name}.exe")),
            cpu_percent: cpu,
            private_bytes: None,
            working_set_bytes: memory,
            read_bytes_per_sec: io,
            write_bytes_per_sec: Some(0),
            network_bytes_per_sec: None,
            gpu_percent: None,
            vram_bytes: None,
            cpu_time_delta_us: None,
            quality_mask: 0,
        }
    }

    #[test]
    fn ranks_each_dimension_and_combines_reasons() {
        let candidates = vec![
            candidate("cpu", 1, 10, Some(90.0), Some(1), Some(1)),
            candidate("memory", 2, 10, Some(1.0), Some(900), Some(1)),
            candidate("io", 3, 10, Some(1.0), Some(1), Some(900)),
        ];
        let selected = ProcessSelector::new(1).select(&candidates, None);
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].candidate.process_name, "cpu.exe");
        assert_eq!(
            selected[0].selection_reason_mask,
            SELECTION_REASON_CPU_TOP_N
        );
        assert_eq!(
            selected[1].selection_reason_mask,
            SELECTION_REASON_MEMORY_TOP_N
        );
        assert_eq!(selected[2].selection_reason_mask, SELECTION_REASON_IO_TOP_N);
    }

    #[test]
    fn one_process_selected_by_multiple_dimensions_has_one_combined_row() {
        let candidates = vec![
            candidate("leader", 1, 10, Some(90.0), Some(900), Some(900)),
            candidate("other", 2, 10, Some(1.0), Some(1), Some(1)),
        ];
        let selected = ProcessSelector::new(1).select(&candidates, None);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].candidate.process_name, "leader.exe");
        assert_eq!(
            selected[0].selection_reason_mask,
            SELECTION_REASON_CPU_TOP_N | SELECTION_REASON_MEMORY_TOP_N | SELECTION_REASON_IO_TOP_N
        );
    }

    #[test]
    fn foreground_is_included_outside_top_n() {
        let candidates = vec![
            candidate("leader", 1, 10, Some(90.0), Some(900), Some(900)),
            candidate("foreground", 2, 10, Some(1.0), Some(1), Some(1)),
        ];
        let key = foreground_process_key(2, 10);
        let selected = ProcessSelector::new(1).select(&candidates, Some(&key));
        assert_eq!(selected.len(), 2);
        assert_eq!(
            selected[1].selection_reason_mask,
            SELECTION_REASON_FOREGROUND
        );
    }

    #[test]
    fn ties_are_stable_and_missing_values_do_not_rank_as_zero() {
        let candidates = vec![
            candidate("z", 2, 10, Some(0.0), Some(0), None),
            candidate("a", 1, 10, Some(0.0), Some(0), None),
            candidate("missing", 3, 10, None, None, None),
        ];
        let selected = ProcessSelector::new(2).select(&candidates, None);
        assert_eq!(
            selected
                .iter()
                .map(|item| item.candidate.process_name.as_str())
                .collect::<Vec<_>>(),
            vec!["a.exe", "z.exe"]
        );
        assert!(!selected
            .iter()
            .any(|item| item.candidate.process_name == "missing.exe"));
    }

    #[test]
    fn pid_reuse_is_a_new_identity() {
        let first = candidate("app", 42, 100, Some(1.0), Some(1), Some(1));
        let second = candidate("app", 42, 200, Some(2.0), Some(2), Some(2));
        assert_ne!(first.stable_key(), second.stable_key());
    }

    #[test]
    fn selected_set_is_bounded() {
        let candidates = (0..100)
            .map(|index| {
                candidate(
                    &format!("app-{index}"),
                    index,
                    10,
                    Some(index as f64),
                    Some((100 - index) as i64),
                    Some((index * 2) as i64),
                )
            })
            .collect::<Vec<_>>();
        let foreground_key = foreground_process_key(50, 10);
        let selected = ProcessSelector::new(5).select(&candidates, Some(&foreground_key));
        assert!(selected.len() <= 16);
        assert!(selected.iter().any(|item| {
            item.candidate.process_name == "app-50.exe"
                && item.selection_reason_mask == SELECTION_REASON_FOREGROUND
        }));
    }
}
