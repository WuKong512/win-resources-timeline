use super::system_metrics::SystemSampler;
use crate::models::{
    CapabilityState, CapabilitySupportStatus, CollectionSettings, MetricCapabilityStatus,
    MetricCategory, ProviderErrorCode, ProviderErrorSummary, ProviderLifecycleState,
    ProviderStatus, ResourceSnapshot,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    time::{Duration, Instant},
};

pub const WINDOWS_BASELINE_PROVIDER_ID: &str = "windows-baseline";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSchedule {
    System,
    #[allow(dead_code)]
    Fixed(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilitySpec {
    pub category: MetricCategory,
    pub support_status: CapabilitySupportStatus,
    pub reason_code: Option<ProviderErrorCode>,
}

impl ProviderCapabilitySpec {
    pub fn supported(category: MetricCategory) -> Self {
        Self {
            category,
            support_status: CapabilitySupportStatus::Supported,
            reason_code: None,
        }
    }

    #[allow(dead_code)]
    pub fn unsupported(category: MetricCategory, reason_code: ProviderErrorCode) -> Self {
        Self {
            category,
            support_status: CapabilitySupportStatus::Unsupported,
            reason_code: Some(reason_code),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDescriptor {
    pub id: String,
    pub display_name: String,
    pub schedule: ProviderSchedule,
    pub capabilities: Vec<ProviderCapabilitySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlan {
    pub provider_id: String,
    pub enabled: bool,
    pub interval_ms: u64,
    pub enabled_categories: Vec<MetricCategory>,
    disabled_reason: Option<ProviderErrorCode>,
}

impl ProviderPlan {
    fn disabled_reason(&self) -> ProviderErrorCode {
        self.disabled_reason
            .unwrap_or(ProviderErrorCode::CategoryDisabled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollectionPlan {
    pub providers: BTreeMap<String, ProviderPlan>,
}

impl CollectionPlan {
    pub fn build(settings: &CollectionSettings, descriptors: &[ProviderDescriptor]) -> Self {
        let requested_categories: BTreeSet<_> =
            settings.enabled_categories.iter().copied().collect();
        let disabled_providers: BTreeSet<_> = settings
            .disabled_providers
            .iter()
            .map(|provider| provider.trim().to_lowercase())
            .collect();
        let mut providers = BTreeMap::new();

        for descriptor in descriptors {
            let provider_disabled =
                disabled_providers.contains(&descriptor.id.trim().to_lowercase());
            let mut enabled_categories: Vec<_> = descriptor
                .capabilities
                .iter()
                .filter(|capability| {
                    !provider_disabled
                        && capability.support_status == CapabilitySupportStatus::Supported
                        && requested_categories.contains(&capability.category)
                })
                .map(|capability| capability.category)
                .collect();
            enabled_categories.sort_unstable();
            enabled_categories.dedup();
            let enabled = !enabled_categories.is_empty();
            let disabled_reason = if provider_disabled {
                Some(ProviderErrorCode::UserDisabled)
            } else if !enabled {
                Some(ProviderErrorCode::CategoryDisabled)
            } else {
                None
            };
            let interval_ms = match descriptor.schedule {
                ProviderSchedule::System => settings.system_sample_interval_ms,
                ProviderSchedule::Fixed(interval_ms) => interval_ms.max(1),
            };
            providers.insert(
                descriptor.id.clone(),
                ProviderPlan {
                    provider_id: descriptor.id.clone(),
                    enabled,
                    interval_ms,
                    enabled_categories,
                    disabled_reason,
                },
            );
        }

        Self { providers }
    }

    pub fn provider(&self, provider_id: &str) -> Option<&ProviderPlan> {
        self.providers.get(provider_id)
    }
}

#[derive(Debug, Clone)]
pub struct ProviderError {
    pub code: ProviderErrorCode,
    pub message: Option<String>,
}

impl ProviderError {
    #[allow(dead_code)]
    pub fn new(code: ProviderErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: Some(short_message(message.into())),
        }
    }

    pub fn without_message(code: ProviderErrorCode) -> Self {
        Self {
            code,
            message: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderHealthObservation {
    pub last_success_at_ms: Option<i64>,
    pub failure_count: u64,
    pub last_error: Option<ProviderErrorSummary>,
}

#[derive(Debug, Clone)]
pub enum ProviderSample {
    ResourceSnapshot(ResourceSnapshot),
}

pub trait MetricProvider: Send {
    fn descriptor(&self) -> &ProviderDescriptor;
    fn start(&mut self, plan: &ProviderPlan) -> Result<(), ProviderError>;
    fn reconfigure(&mut self, plan: &ProviderPlan) -> Result<(), ProviderError> {
        self.stop();
        self.start(plan)
    }
    fn sample(
        &mut self,
        timestamp_ms: i64,
        tracked_app_keys: &HashSet<String>,
    ) -> Result<Option<ProviderSample>, ProviderError>;
    fn stop(&mut self);
    fn health(&self) -> ProviderHealthObservation;
}

struct ProviderRuntime {
    provider: Box<dyn MetricProvider>,
    plan: Option<ProviderPlan>,
    started: bool,
    lifecycle: ProviderLifecycleState,
    last_success_at_ms: Option<i64>,
    failure_count: u64,
    consecutive_failures: u32,
    last_error: Option<ProviderErrorSummary>,
    next_sample_at: Option<Instant>,
}

pub struct ProviderHost {
    providers: BTreeMap<String, ProviderRuntime>,
    plan: CollectionPlan,
    paused: bool,
}

impl ProviderHost {
    pub fn new(providers: Vec<Box<dyn MetricProvider>>) -> Self {
        let mut runtimes = BTreeMap::new();
        for provider in providers {
            let id = provider.descriptor().id.clone();
            assert!(
                runtimes
                    .insert(
                        id,
                        ProviderRuntime {
                            provider,
                            plan: None,
                            started: false,
                            lifecycle: ProviderLifecycleState::Stopped,
                            last_success_at_ms: None,
                            failure_count: 0,
                            consecutive_failures: 0,
                            last_error: None,
                            next_sample_at: None,
                        },
                    )
                    .is_none(),
                "duplicate provider id"
            );
        }
        Self {
            providers: runtimes,
            plan: CollectionPlan::default(),
            paused: false,
        }
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers
            .values()
            .map(|runtime| runtime.provider.descriptor().clone())
            .collect()
    }

    #[allow(dead_code)]
    pub fn plan(&self) -> &CollectionPlan {
        &self.plan
    }

    pub fn apply_plan(&mut self, next_plan: CollectionPlan, now: Instant) {
        self.apply_plan_inner(next_plan, now, false);
    }

    fn apply_plan_inner(&mut self, next_plan: CollectionPlan, now: Instant, force: bool) {
        if !force && next_plan == self.plan {
            return;
        }
        for (provider_id, runtime) in &mut self.providers {
            let Some(next_provider_plan) = next_plan.providers.get(provider_id).cloned() else {
                continue;
            };
            let plan_changed = runtime.plan.as_ref() != Some(&next_provider_plan);
            runtime.plan = Some(next_provider_plan.clone());

            if self.paused {
                if runtime.started {
                    runtime.provider.stop();
                    runtime.started = false;
                }
                runtime.lifecycle = if next_provider_plan.enabled {
                    ProviderLifecycleState::Paused
                } else {
                    ProviderLifecycleState::Stopped
                };
                runtime.next_sample_at = None;
                continue;
            }

            if !next_provider_plan.enabled {
                if runtime.started {
                    runtime.provider.stop();
                    runtime.started = false;
                }
                runtime.lifecycle = ProviderLifecycleState::Stopped;
                runtime.next_sample_at = None;
                continue;
            }

            if runtime.started && !plan_changed {
                continue;
            }
            let result = if runtime.started {
                runtime.started = false;
                runtime.provider.reconfigure(&next_provider_plan)
            } else {
                runtime.provider.start(&next_provider_plan)
            };
            match result {
                Ok(()) => {
                    runtime.started = true;
                    runtime.lifecycle = ProviderLifecycleState::Running;
                    runtime.last_error = None;
                    runtime.consecutive_failures = 0;
                    runtime.next_sample_at = Some(now);
                }
                Err(error) => {
                    runtime.lifecycle = ProviderLifecycleState::Failed;
                    runtime.failure_count = runtime.failure_count.saturating_add(1);
                    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                    runtime.last_error = Some(error_summary(error));
                    runtime.next_sample_at = None;
                }
            }
        }
        self.plan = next_plan;
    }

    pub fn sample_due(
        &mut self,
        now: Instant,
        timestamp_ms: i64,
        tracked_app_keys: &HashSet<String>,
    ) -> Vec<ProviderSample> {
        if self.paused {
            return Vec::new();
        }
        let mut samples = Vec::new();
        for runtime in self.providers.values_mut() {
            let Some(plan) = runtime.plan.as_ref() else {
                continue;
            };
            if !plan.enabled || !runtime.started {
                continue;
            }
            if runtime.next_sample_at.is_some_and(|next| next > now) {
                continue;
            }
            runtime.next_sample_at = Some(now + Duration::from_millis(plan.interval_ms.max(1)));
            match runtime.provider.sample(timestamp_ms, tracked_app_keys) {
                Ok(sample) => {
                    runtime.lifecycle = ProviderLifecycleState::Running;
                    runtime.consecutive_failures = 0;
                    runtime.last_error = None;
                    if sample.is_some() {
                        runtime.last_success_at_ms = Some(timestamp_ms);
                    }
                    if let Some(sample) = sample {
                        samples.push(sample);
                    }
                }
                Err(error) => {
                    runtime.lifecycle = ProviderLifecycleState::Failed;
                    runtime.failure_count = runtime.failure_count.saturating_add(1);
                    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                    runtime.last_error = Some(error_summary(error));
                    let backoff = failure_backoff(plan.interval_ms, runtime.consecutive_failures);
                    runtime.next_sample_at = Some(now + backoff);
                }
            }
        }
        samples
    }

    pub fn pause(&mut self) {
        if self.paused {
            return;
        }
        self.paused = true;
        for runtime in self.providers.values_mut() {
            if runtime.started {
                runtime.provider.stop();
                runtime.started = false;
            }
            runtime.next_sample_at = None;
            if runtime.plan.as_ref().is_some_and(|plan| plan.enabled) {
                runtime.lifecycle = ProviderLifecycleState::Paused;
            }
        }
    }

    pub fn resume(&mut self, now: Instant) {
        if !self.paused {
            return;
        }
        self.paused = false;
        self.apply_plan_inner(self.plan.clone(), now, true);
    }

    pub fn stop_all(&mut self) {
        for runtime in self.providers.values_mut() {
            if runtime.started {
                runtime.provider.stop();
                runtime.started = false;
            }
            runtime.next_sample_at = None;
            runtime.lifecycle = ProviderLifecycleState::Stopped;
        }
    }

    pub fn statuses(&self) -> Vec<ProviderStatus> {
        self.providers
            .iter()
            .map(|(provider_id, runtime)| {
                let descriptor = runtime.provider.descriptor();
                let observed_health = runtime.provider.health();
                let plan = runtime
                    .plan
                    .as_ref()
                    .or_else(|| self.plan.provider(provider_id));
                let supported = descriptor.capabilities.iter().any(|capability| {
                    capability.support_status == CapabilitySupportStatus::Supported
                });
                let enabled = plan.is_some_and(|plan| plan.enabled);
                let capabilities = descriptor
                    .capabilities
                    .iter()
                    .map(|capability| {
                        let category_enabled = plan.is_some_and(|plan| {
                            plan.enabled && plan.enabled_categories.contains(&capability.category)
                        });
                        let (state, reason_code) = match capability.support_status {
                            CapabilitySupportStatus::Unsupported => (
                                CapabilityState::Unsupported,
                                capability
                                    .reason_code
                                    .or(Some(ProviderErrorCode::Unsupported)),
                            ),
                            CapabilitySupportStatus::Supported if !category_enabled => (
                                CapabilityState::SupportedDisabled,
                                plan.map(|plan| {
                                    if plan.enabled {
                                        ProviderErrorCode::CategoryDisabled
                                    } else {
                                        plan.disabled_reason()
                                    }
                                }),
                            ),
                            CapabilitySupportStatus::Supported
                                if runtime.lifecycle == ProviderLifecycleState::Failed =>
                            {
                                (
                                    CapabilityState::Failed,
                                    runtime
                                        .last_error
                                        .as_ref()
                                        .map(|error| error.code)
                                        .or(Some(ProviderErrorCode::SampleFailed)),
                                )
                            }
                            CapabilitySupportStatus::Supported => {
                                (CapabilityState::SupportedEnabled, None)
                            }
                        };
                        MetricCapabilityStatus {
                            provider_id: provider_id.clone(),
                            category: capability.category,
                            support_status: capability.support_status,
                            enabled: category_enabled,
                            can_toggle: capability.support_status
                                == CapabilitySupportStatus::Supported,
                            state,
                            reason_code,
                        }
                    })
                    .collect();
                ProviderStatus {
                    provider_id: provider_id.clone(),
                    display_name: descriptor.display_name.clone(),
                    supported,
                    enabled,
                    lifecycle: runtime.lifecycle,
                    capabilities,
                    last_success_at_ms: runtime
                        .last_success_at_ms
                        .or(observed_health.last_success_at_ms),
                    failure_count: runtime.failure_count.max(observed_health.failure_count),
                    last_error: runtime.last_error.clone().or(observed_health.last_error),
                }
            })
            .collect()
    }
}

fn failure_backoff(interval_ms: u64, consecutive_failures: u32) -> Duration {
    let shift = consecutive_failures.saturating_sub(1).min(4);
    let multiplier = 1_u64 << shift;
    Duration::from_millis(interval_ms.max(1).saturating_mul(multiplier).min(60_000))
}

fn error_summary(error: ProviderError) -> ProviderErrorSummary {
    ProviderErrorSummary {
        code: error.code,
        message: error.message,
    }
}

#[allow(dead_code)]
fn short_message(message: String) -> String {
    const MAX_MESSAGE_BYTES: usize = 160;
    if message.len() <= MAX_MESSAGE_BYTES {
        return message;
    }
    let mut end = MAX_MESSAGE_BYTES;
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &message[..end])
}

pub struct WindowsBaselineProvider {
    descriptor: ProviderDescriptor,
    sampler: Option<SystemSampler>,
    enabled_categories: BTreeSet<MetricCategory>,
    health: ProviderHealthObservation,
}

impl WindowsBaselineProvider {
    pub fn new() -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id: WINDOWS_BASELINE_PROVIDER_ID.to_string(),
                display_name: "Windows baseline".to_string(),
                schedule: ProviderSchedule::System,
                capabilities: vec![
                    ProviderCapabilitySpec::supported(MetricCategory::Cpu),
                    ProviderCapabilitySpec::supported(MetricCategory::Memory),
                    ProviderCapabilitySpec::supported(MetricCategory::Disk),
                    ProviderCapabilitySpec::supported(MetricCategory::Process),
                ],
            },
            sampler: None,
            enabled_categories: BTreeSet::new(),
            health: ProviderHealthObservation::default(),
        }
    }
}

impl Default for WindowsBaselineProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricProvider for WindowsBaselineProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn start(&mut self, plan: &ProviderPlan) -> Result<(), ProviderError> {
        self.enabled_categories = plan.enabled_categories.iter().copied().collect();
        self.sampler = Some(SystemSampler::new_for_categories(&self.enabled_categories));
        self.health.last_error = None;
        Ok(())
    }

    fn sample(
        &mut self,
        timestamp_ms: i64,
        tracked_app_keys: &HashSet<String>,
    ) -> Result<Option<ProviderSample>, ProviderError> {
        let sampler = self
            .sampler
            .as_mut()
            .ok_or_else(|| ProviderError::without_message(ProviderErrorCode::StartupFailed))?;
        let sample = sampler.sample_with_categories(
            timestamp_ms,
            tracked_app_keys,
            &self.enabled_categories,
        );
        if sample.is_some() {
            self.health.last_success_at_ms = Some(timestamp_ms);
        }
        self.health.last_error = None;
        Ok(sample.map(ProviderSample::ResourceSnapshot))
    }

    fn stop(&mut self) {
        self.sampler = None;
        self.enabled_categories.clear();
    }

    fn health(&self) -> ProviderHealthObservation {
        self.health.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppResourceSample, SystemSample};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default, Clone, Copy)]
    struct Counters {
        start_count: u32,
        reconfigure_count: u32,
        sample_count: u32,
        stop_count: u32,
    }

    struct FakeProvider {
        descriptor: ProviderDescriptor,
        counters: Arc<Mutex<Counters>>,
        fail_start: bool,
        sample_failures_remaining: u32,
        health: ProviderHealthObservation,
    }

    impl FakeProvider {
        fn new(
            id: &str,
            categories: Vec<ProviderCapabilitySpec>,
            schedule: ProviderSchedule,
        ) -> (Self, Arc<Mutex<Counters>>) {
            let counters = Arc::new(Mutex::new(Counters::default()));
            (
                Self {
                    descriptor: ProviderDescriptor {
                        id: id.to_string(),
                        display_name: id.to_string(),
                        schedule,
                        capabilities: categories,
                    },
                    counters: counters.clone(),
                    fail_start: false,
                    sample_failures_remaining: 0,
                    health: ProviderHealthObservation::default(),
                },
                counters,
            )
        }

        fn startup_failure(mut self) -> Self {
            self.fail_start = true;
            self
        }

        fn sample_failures(mut self, count: u32) -> Self {
            self.sample_failures_remaining = count;
            self
        }
    }

    impl MetricProvider for FakeProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        fn start(&mut self, _plan: &ProviderPlan) -> Result<(), ProviderError> {
            self.counters.lock().unwrap().start_count += 1;
            if self.fail_start {
                return Err(ProviderError::new(
                    ProviderErrorCode::StartupFailed,
                    "deterministic startup failure",
                ));
            }
            Ok(())
        }

        fn reconfigure(&mut self, plan: &ProviderPlan) -> Result<(), ProviderError> {
            self.counters.lock().unwrap().reconfigure_count += 1;
            self.stop();
            self.start(plan)
        }

        fn sample(
            &mut self,
            timestamp_ms: i64,
            _tracked_app_keys: &HashSet<String>,
        ) -> Result<Option<ProviderSample>, ProviderError> {
            self.counters.lock().unwrap().sample_count += 1;
            if self.sample_failures_remaining > 0 {
                self.sample_failures_remaining -= 1;
                return Err(ProviderError::new(
                    ProviderErrorCode::SampleFailed,
                    "deterministic sample failure",
                ));
            }
            let snapshot = ResourceSnapshot {
                system: SystemSample {
                    timestamp_ms,
                    sample_duration_ms: 1,
                    cpu_percent: Some(0.0),
                    memory_percent: None,
                    memory_used_bytes: None,
                    memory_total_bytes: None,
                    disk_read_bytes_per_sec: None,
                    disk_write_bytes_per_sec: None,
                    has_app_snapshot: false,
                },
                apps: Vec::<AppResourceSample>::new(),
            };
            self.health.last_success_at_ms = Some(timestamp_ms);
            Ok(Some(ProviderSample::ResourceSnapshot(snapshot)))
        }

        fn stop(&mut self) {
            self.counters.lock().unwrap().stop_count += 1;
        }

        fn health(&self) -> ProviderHealthObservation {
            self.health.clone()
        }
    }

    fn settings_with(categories: Vec<MetricCategory>) -> CollectionSettings {
        CollectionSettings {
            enabled_categories: categories,
            ..CollectionSettings::default()
        }
    }

    fn plan_for(host: &ProviderHost, settings: &CollectionSettings) -> CollectionPlan {
        CollectionPlan::build(settings, &host.descriptors())
    }

    fn sample_at(host: &mut ProviderHost, at: Instant, timestamp_ms: i64) -> Vec<ProviderSample> {
        host.sample_due(at, timestamp_ms, &HashSet::new())
    }

    #[test]
    fn capability_states_keep_zero_values_independent() {
        let (provider, _) = FakeProvider::new(
            "fake",
            vec![
                ProviderCapabilitySpec::supported(MetricCategory::Cpu),
                ProviderCapabilitySpec::unsupported(
                    MetricCategory::Gpu,
                    ProviderErrorCode::ProviderMissing,
                ),
            ],
            ProviderSchedule::Fixed(10),
        );
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let now = Instant::now();
        host.apply_plan(plan_for(&host, &CollectionSettings::default()), now);
        let statuses = host.statuses();
        let status = &statuses[0];
        assert_eq!(
            status.capabilities[0].state,
            CapabilityState::SupportedEnabled
        );
        assert_eq!(status.capabilities[1].state, CapabilityState::Unsupported);
        let samples = sample_at(&mut host, now, 100);
        let ProviderSample::ResourceSnapshot(snapshot) = &samples[0];
        assert_eq!(snapshot.system.cpu_percent, Some(0.0));
    }

    #[test]
    fn plan_building_is_deterministic_and_provider_is_not_category() {
        let (cpu_provider, _) = FakeProvider::new(
            "z-provider",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::System,
        );
        let (memory_provider, _) = FakeProvider::new(
            "a-provider",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Memory)],
            ProviderSchedule::Fixed(30),
        );
        let descriptors = vec![
            cpu_provider.descriptor().clone(),
            memory_provider.descriptor().clone(),
        ];
        let settings = settings_with(vec![MetricCategory::Memory, MetricCategory::Cpu]);
        let first = CollectionPlan::build(&settings, &descriptors);
        let second = CollectionPlan::build(&settings, &descriptors);
        assert_eq!(first, second);
        assert_eq!(
            first.providers.keys().cloned().collect::<Vec<_>>(),
            vec!["a-provider", "z-provider"]
        );
        assert_eq!(
            first.provider("z-provider").unwrap().enabled_categories,
            vec![MetricCategory::Cpu]
        );
    }

    #[test]
    fn disabling_stops_sampling_and_reenable_starts_again() {
        let (provider, counters) = FakeProvider::new(
            "fake",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let start = Instant::now();
        host.apply_plan(plan_for(&host, &CollectionSettings::default()), start);
        assert_eq!(sample_at(&mut host, start, 1).len(), 1);
        assert_eq!(counters.lock().unwrap().sample_count, 1);

        host.apply_plan(
            plan_for(&host, &CollectionSettings::default()),
            start + Duration::from_millis(1),
        );
        assert_eq!(counters.lock().unwrap().start_count, 1);
        assert_eq!(counters.lock().unwrap().stop_count, 0);

        let disabled = settings_with(Vec::new());
        host.apply_plan(plan_for(&host, &disabled), start + Duration::from_millis(1));
        assert!(sample_at(&mut host, start + Duration::from_millis(100), 2).is_empty());
        assert_eq!(counters.lock().unwrap().stop_count, 1);

        host.apply_plan(
            plan_for(&host, &settings_with(vec![MetricCategory::Cpu])),
            start + Duration::from_millis(200),
        );
        assert_eq!(counters.lock().unwrap().start_count, 2);
        assert_eq!(
            sample_at(&mut host, start + Duration::from_millis(200), 3).len(),
            1
        );
    }

    #[test]
    fn unsupported_provider_never_starts() {
        let (provider, counters) = FakeProvider::new(
            "unsupported",
            vec![ProviderCapabilitySpec::unsupported(
                MetricCategory::Gpu,
                ProviderErrorCode::ProviderMissing,
            )],
            ProviderSchedule::Fixed(10),
        );
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let settings = settings_with(vec![MetricCategory::Gpu]);
        host.apply_plan(plan_for(&host, &settings), Instant::now());
        assert_eq!(counters.lock().unwrap().start_count, 0);
        assert_eq!(
            host.statuses()[0].capabilities[0].state,
            CapabilityState::Unsupported
        );
    }

    #[test]
    fn startup_failure_isolated_and_unchanged_plan_does_not_retry() {
        let (healthy, healthy_counters) = FakeProvider::new(
            "healthy",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let (failing, failing_counters) = FakeProvider::new(
            "failing",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Memory)],
            ProviderSchedule::Fixed(10),
        );
        let failing = failing.startup_failure();
        let mut host = ProviderHost::new(vec![Box::new(healthy), Box::new(failing)]);
        let settings = settings_with(vec![MetricCategory::Cpu, MetricCategory::Memory]);
        let now = Instant::now();
        host.apply_plan(plan_for(&host, &settings), now);
        assert_eq!(healthy_counters.lock().unwrap().start_count, 1);
        assert_eq!(failing_counters.lock().unwrap().start_count, 1);
        assert_eq!(
            host.statuses()
                .iter()
                .find(|status| status.provider_id == "failing")
                .unwrap()
                .lifecycle,
            ProviderLifecycleState::Failed
        );
        assert_eq!(sample_at(&mut host, now, 1).len(), 1);
        host.apply_plan(plan_for(&host, &settings), now + Duration::from_millis(20));
        assert_eq!(failing_counters.lock().unwrap().start_count, 1);
    }

    #[test]
    fn sample_failure_does_not_stop_healthy_provider_and_can_recover() {
        let (healthy, healthy_counters) = FakeProvider::new(
            "healthy",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let (recovering, recovering_counters) = FakeProvider::new(
            "recovering",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Memory)],
            ProviderSchedule::Fixed(10),
        );
        let recovering = recovering.sample_failures(1);
        let mut host = ProviderHost::new(vec![Box::new(healthy), Box::new(recovering)]);
        let now = Instant::now();
        let settings = settings_with(vec![MetricCategory::Cpu, MetricCategory::Memory]);
        host.apply_plan(plan_for(&host, &settings), now);
        assert_eq!(sample_at(&mut host, now, 1).len(), 1);
        assert_eq!(host.statuses()[1].lifecycle, ProviderLifecycleState::Failed);
        assert_eq!(healthy_counters.lock().unwrap().sample_count, 1);

        let recovered_at = now + Duration::from_millis(20);
        assert_eq!(sample_at(&mut host, recovered_at, 2).len(), 2);
        assert_eq!(
            host.statuses()[1].lifecycle,
            ProviderLifecycleState::Running
        );
        assert_eq!(recovering_counters.lock().unwrap().sample_count, 2);
    }

    #[test]
    fn only_affected_provider_reconfigures_and_pause_is_not_disable() {
        let (system, system_counters) = FakeProvider::new(
            "system",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::System,
        );
        let (fixed, fixed_counters) = FakeProvider::new(
            "fixed",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Memory)],
            ProviderSchedule::Fixed(10),
        );
        let mut host = ProviderHost::new(vec![Box::new(system), Box::new(fixed)]);
        let start = Instant::now();
        let settings = settings_with(vec![MetricCategory::Cpu, MetricCategory::Memory]);
        host.apply_plan(plan_for(&host, &settings), start);
        host.apply_plan(
            plan_for(
                &host,
                &CollectionSettings {
                    system_sample_interval_ms: 10_000,
                    ..settings.clone()
                },
            ),
            start + Duration::from_millis(1),
        );
        assert_eq!(system_counters.lock().unwrap().stop_count, 1);
        assert_eq!(system_counters.lock().unwrap().reconfigure_count, 1);
        assert_eq!(fixed_counters.lock().unwrap().stop_count, 0);

        host.pause();
        assert!(host.plan().provider("system").unwrap().enabled);
        assert_eq!(host.statuses()[1].lifecycle, ProviderLifecycleState::Paused);
        host.resume(start + Duration::from_millis(2));
        assert_eq!(system_counters.lock().unwrap().start_count, 3);
    }

    #[test]
    fn shutdown_stops_each_running_provider_once() {
        let (provider, counters) = FakeProvider::new(
            "fake",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let settings = settings_with(vec![MetricCategory::Cpu]);
        host.apply_plan(plan_for(&host, &settings), Instant::now());
        host.stop_all();
        host.stop_all();
        assert_eq!(counters.lock().unwrap().stop_count, 1);
    }

    #[test]
    fn dto_distinguishes_disabled_unsupported_and_failed() {
        let (disabled, _) = FakeProvider::new(
            "disabled",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let (unsupported, _) = FakeProvider::new(
            "unsupported",
            vec![ProviderCapabilitySpec::unsupported(
                MetricCategory::Gpu,
                ProviderErrorCode::ProviderMissing,
            )],
            ProviderSchedule::Fixed(10),
        );
        let (failed, _) = FakeProvider::new(
            "failed",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Memory)],
            ProviderSchedule::Fixed(10),
        );
        let failed = failed.startup_failure();
        let mut host = ProviderHost::new(vec![
            Box::new(disabled),
            Box::new(unsupported),
            Box::new(failed),
        ]);
        let settings = settings_with(vec![MetricCategory::Gpu, MetricCategory::Memory]);
        host.apply_plan(plan_for(&host, &settings), Instant::now());
        let statuses = host.statuses();
        assert_eq!(statuses[0].provider_id, "disabled");
        assert_eq!(
            statuses[0].capabilities[0].state,
            CapabilityState::SupportedDisabled
        );
        assert!(statuses[0].capabilities[0].can_toggle);
        assert_eq!(statuses[1].provider_id, "failed");
        assert_eq!(statuses[1].capabilities[0].state, CapabilityState::Failed);
        assert!(statuses[1].capabilities[0].can_toggle);
        assert_eq!(statuses[2].provider_id, "unsupported");
        assert_eq!(
            statuses[2].capabilities[0].state,
            CapabilityState::Unsupported
        );
        assert!(!statuses[2].capabilities[0].can_toggle);
    }
}
