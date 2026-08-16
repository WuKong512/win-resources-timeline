use super::system_metrics::{DiskCapabilityProbe, PdhDiskCapabilityProbe, SystemSampler};
use crate::models::{
    CapabilityState, CapabilitySupportStatus, CollectionSettings, MetricCapabilityStatus,
    MetricCategory, ProviderErrorCode, ProviderErrorSummary, ProviderLifecycleState,
    ProviderStatus, ResourceSnapshot,
};
use crossbeam_channel::{bounded, Receiver, SendTimeoutError, Sender};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

pub const WINDOWS_BASELINE_PROVIDER_ID: &str = "windows-baseline";

const PROVIDER_CONTROL_TIMEOUT: Duration = Duration::from_secs(2);
const PROVIDER_SAMPLE_TIMEOUT: Duration = Duration::from_millis(250);

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

#[derive(Clone)]
pub struct ProviderCallContext {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

impl ProviderCallContext {
    fn new(deadline: Instant) -> Self {
        Self {
            deadline,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline()
    }

    pub fn check(&self) -> Result<(), ProviderError> {
        if self.is_cancelled() || self.is_expired() {
            Err(ProviderError::without_message(ProviderErrorCode::Timeout))
        } else {
            Ok(())
        }
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
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
    fn probe(
        &mut self,
        context: &ProviderCallContext,
        _requested_categories: &BTreeSet<MetricCategory>,
    ) -> Result<Vec<ProviderCapabilitySpec>, ProviderError> {
        context.check()?;
        Ok(self.descriptor().capabilities.clone())
    }
    fn start(
        &mut self,
        plan: &ProviderPlan,
        context: &ProviderCallContext,
    ) -> Result<(), ProviderError>;
    fn reconfigure(
        &mut self,
        plan: &ProviderPlan,
        context: &ProviderCallContext,
    ) -> Result<(), ProviderError> {
        self.stop(context)?;
        self.start(plan, context).map_err(|error| {
            if error.code == ProviderErrorCode::StartupFailed {
                ProviderError {
                    code: ProviderErrorCode::ReconfigureFailed,
                    message: error.message,
                }
            } else {
                error
            }
        })
    }
    fn sample(
        &mut self,
        context: &ProviderCallContext,
        timestamp_ms: i64,
        tracked_app_keys: &HashSet<String>,
    ) -> Result<Option<ProviderSample>, ProviderError>;
    fn stop(&mut self, context: &ProviderCallContext) -> Result<(), ProviderError>;
    fn health(&self) -> ProviderHealthObservation;
}

enum ProviderCommand {
    Probe {
        context: ProviderCallContext,
        requested_categories: Arc<BTreeSet<MetricCategory>>,
        reply: Sender<ProviderReply>,
    },
    Start {
        plan: ProviderPlan,
        context: ProviderCallContext,
        reply: Sender<ProviderReply>,
    },
    Reconfigure {
        plan: ProviderPlan,
        context: ProviderCallContext,
        reply: Sender<ProviderReply>,
    },
    Sample {
        context: ProviderCallContext,
        timestamp_ms: i64,
        tracked_app_keys: Arc<HashSet<String>>,
        reply: Sender<ProviderReply>,
    },
    Stop {
        context: ProviderCallContext,
        reply: Sender<ProviderReply>,
    },
}

enum ProviderReply {
    Probe {
        result: Result<Vec<ProviderCapabilitySpec>, ProviderError>,
        health: ProviderHealthObservation,
    },
    Lifecycle {
        result: Result<(), ProviderError>,
        health: ProviderHealthObservation,
    },
    Sample {
        result: Result<Option<ProviderSample>, ProviderError>,
        health: ProviderHealthObservation,
    },
}

struct PendingProviderCall {
    context: ProviderCallContext,
    reply: Receiver<ProviderReply>,
}

struct ProviderExecutor {
    command_tx: Sender<ProviderCommand>,
    pending: Option<PendingProviderCall>,
}

impl ProviderExecutor {
    fn new(mut provider: Box<dyn MetricProvider>) -> Self {
        let (command_tx, command_rx) = bounded(1);
        thread::Builder::new()
            .name("provider-executor".to_string())
            .spawn(move || provider_worker(&mut provider, command_rx))
            .expect("provider executor thread should start");
        Self {
            command_tx,
            pending: None,
        }
    }

    fn probe(
        &mut self,
        deadline: Instant,
        requested_categories: Arc<BTreeSet<MetricCategory>>,
    ) -> Result<(Vec<ProviderCapabilitySpec>, ProviderHealthObservation), ProviderError> {
        let reply = self.execute(deadline, |context, reply| ProviderCommand::Probe {
            context,
            requested_categories,
            reply,
        })?;
        match reply {
            ProviderReply::Probe { result, health } => {
                result.map(|capabilities| (capabilities, health))
            }
            ProviderReply::Lifecycle { .. } | ProviderReply::Sample { .. } => {
                Err(ProviderError::new(
                    ProviderErrorCode::ProviderMissing,
                    "invalid provider probe reply",
                ))
            }
        }
    }

    fn start(
        &mut self,
        plan: &ProviderPlan,
        deadline: Instant,
    ) -> Result<ProviderHealthObservation, ProviderError> {
        let reply = self.execute(deadline, |context, reply| ProviderCommand::Start {
            plan: plan.clone(),
            context,
            reply,
        })?;
        lifecycle_reply(reply)
    }

    fn reconfigure(
        &mut self,
        plan: &ProviderPlan,
        deadline: Instant,
    ) -> Result<ProviderHealthObservation, ProviderError> {
        let reply = self.execute(deadline, |context, reply| ProviderCommand::Reconfigure {
            plan: plan.clone(),
            context,
            reply,
        })?;
        lifecycle_reply(reply)
    }

    fn sample(
        &mut self,
        timestamp_ms: i64,
        tracked_app_keys: Arc<HashSet<String>>,
        deadline: Instant,
    ) -> Result<(Option<ProviderSample>, ProviderHealthObservation), ProviderError> {
        let reply = self.execute(deadline, |context, reply| ProviderCommand::Sample {
            context,
            timestamp_ms,
            tracked_app_keys,
            reply,
        })?;
        match reply {
            ProviderReply::Sample { result, health } => result.map(|sample| (sample, health)),
            ProviderReply::Probe { .. } | ProviderReply::Lifecycle { .. } => {
                Err(ProviderError::new(
                    ProviderErrorCode::ProviderMissing,
                    "invalid provider sample reply",
                ))
            }
        }
    }

    fn stop(&mut self, deadline: Instant) -> Result<ProviderHealthObservation, ProviderError> {
        let reply = self.execute(deadline, |context, reply| ProviderCommand::Stop {
            context,
            reply,
        })?;
        lifecycle_reply(reply)
    }

    fn cancel_pending(&self) {
        if let Some(pending) = &self.pending {
            pending.context.cancel();
        }
    }

    fn pending(&self) -> bool {
        self.pending.is_some()
    }

    fn execute<F>(&mut self, deadline: Instant, build: F) -> Result<ProviderReply, ProviderError>
    where
        F: FnOnce(ProviderCallContext, Sender<ProviderReply>) -> ProviderCommand,
    {
        self.drain_pending(deadline)?;
        let context = ProviderCallContext::new(deadline);
        let (reply_tx, reply_rx) = bounded(1);
        let command = build(context.clone(), reply_tx);
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProviderError::without_message(ProviderErrorCode::Timeout));
        }
        match self.command_tx.send_timeout(command, remaining) {
            Ok(()) => {}
            Err(SendTimeoutError::Timeout(_)) => {
                context.cancel();
                return Err(ProviderError::without_message(ProviderErrorCode::Timeout));
            }
            Err(SendTimeoutError::Disconnected(_)) => {
                return Err(ProviderError::without_message(
                    ProviderErrorCode::ProviderMissing,
                ));
            }
        }
        match reply_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(reply) => Ok(reply),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                context.cancel();
                self.pending = Some(PendingProviderCall {
                    context,
                    reply: reply_rx,
                });
                Err(ProviderError::without_message(ProviderErrorCode::Timeout))
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Err(
                ProviderError::without_message(ProviderErrorCode::ProviderMissing),
            ),
        }
    }

    fn drain_pending(&mut self, deadline: Instant) -> Result<(), ProviderError> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        match pending
            .reply
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            Ok(_) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => Ok(()),
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                self.pending = Some(pending);
                Err(ProviderError::without_message(ProviderErrorCode::Timeout))
            }
        }
    }
}

fn lifecycle_reply(reply: ProviderReply) -> Result<ProviderHealthObservation, ProviderError> {
    match reply {
        ProviderReply::Lifecycle { result, health } => result.map(|()| health),
        ProviderReply::Probe { .. } | ProviderReply::Sample { .. } => Err(ProviderError::new(
            ProviderErrorCode::ProviderMissing,
            "invalid provider lifecycle reply",
        )),
    }
}

fn provider_worker(provider: &mut Box<dyn MetricProvider>, command_rx: Receiver<ProviderCommand>) {
    while let Ok(command) = command_rx.recv() {
        let (reply, reply_tx) = match command {
            ProviderCommand::Probe {
                context,
                requested_categories,
                reply,
            } => {
                let result = call_with_context(&context, || {
                    provider.probe(&context, requested_categories.as_ref())
                });
                (
                    ProviderReply::Probe {
                        result,
                        health: provider.health(),
                    },
                    reply,
                )
            }
            ProviderCommand::Start {
                plan,
                context,
                reply,
            } => {
                let result = call_with_context(&context, || provider.start(&plan, &context));
                (
                    ProviderReply::Lifecycle {
                        result,
                        health: provider.health(),
                    },
                    reply,
                )
            }
            ProviderCommand::Reconfigure {
                plan,
                context,
                reply,
            } => {
                let result = call_with_context(&context, || provider.reconfigure(&plan, &context));
                (
                    ProviderReply::Lifecycle {
                        result,
                        health: provider.health(),
                    },
                    reply,
                )
            }
            ProviderCommand::Sample {
                context,
                timestamp_ms,
                tracked_app_keys,
                reply,
            } => {
                let result = call_with_context(&context, || {
                    provider.sample(&context, timestamp_ms, tracked_app_keys.as_ref())
                });
                (
                    ProviderReply::Sample {
                        result,
                        health: provider.health(),
                    },
                    reply,
                )
            }
            ProviderCommand::Stop { context, reply } => {
                let result = call_with_context(&context, || provider.stop(&context));
                (
                    ProviderReply::Lifecycle {
                        result,
                        health: provider.health(),
                    },
                    reply,
                )
            }
        };
        let _ = reply_tx.try_send(reply);
    }
}

fn call_with_context<T>(
    context: &ProviderCallContext,
    operation: impl FnOnce() -> Result<T, ProviderError>,
) -> Result<T, ProviderError> {
    context.check()?;
    let result = operation();
    if context.is_cancelled() || context.is_expired() {
        Err(ProviderError::without_message(ProviderErrorCode::Timeout))
    } else {
        result
    }
}

struct ProviderRuntime {
    descriptor: ProviderDescriptor,
    executor: ProviderExecutor,
    plan: Option<ProviderPlan>,
    started: bool,
    lifecycle: ProviderLifecycleState,
    last_success_at_ms: Option<i64>,
    failure_count: u64,
    consecutive_failures: u32,
    last_error: Option<ProviderErrorSummary>,
    next_sample_at: Option<Instant>,
    retry_action: Option<ProviderRetryAction>,
    retry_at: Option<Instant>,
    provider_health: ProviderHealthObservation,
    probe_failed: bool,
    stop_failure: Option<ProviderErrorSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderRetryAction {
    Start,
    Reconfigure,
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
            let descriptor = provider.descriptor().clone();
            assert!(
                runtimes
                    .insert(
                        id,
                        ProviderRuntime {
                            descriptor,
                            executor: ProviderExecutor::new(provider),
                            plan: None,
                            started: false,
                            lifecycle: ProviderLifecycleState::Stopped,
                            last_success_at_ms: None,
                            failure_count: 0,
                            consecutive_failures: 0,
                            last_error: None,
                            next_sample_at: None,
                            retry_action: None,
                            retry_at: None,
                            provider_health: ProviderHealthObservation::default(),
                            probe_failed: false,
                            stop_failure: None,
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

    #[allow(dead_code)]
    pub fn probe_all(&mut self, deadline: Instant) {
        let requested_categories: BTreeSet<_> = MetricCategory::ALL.into_iter().collect();
        self.probe_all_with_categories(deadline, &requested_categories, &BTreeSet::new());
    }

    pub fn probe_all_for_settings(&mut self, deadline: Instant, settings: &CollectionSettings) {
        let requested_categories: BTreeSet<_> =
            settings.enabled_categories.iter().copied().collect();
        let disabled_providers: BTreeSet<_> = settings
            .disabled_providers
            .iter()
            .map(|provider| provider.trim().to_lowercase())
            .collect();
        self.probe_all_with_categories(deadline, &requested_categories, &disabled_providers);
    }

    fn probe_all_with_categories(
        &mut self,
        deadline: Instant,
        requested_categories: &BTreeSet<MetricCategory>,
        disabled_providers: &BTreeSet<String>,
    ) {
        for runtime in self.providers.values_mut() {
            let provider_id = runtime.descriptor.id.trim().to_lowercase();
            let requested_categories = if disabled_providers.contains(&provider_id) {
                Arc::new(BTreeSet::new())
            } else {
                Arc::new(requested_categories.clone())
            };
            match runtime.executor.probe(deadline, requested_categories) {
                Ok((capabilities, health)) => {
                    runtime.descriptor.capabilities = capabilities;
                    runtime.provider_health = health;
                    runtime.probe_failed = false;
                    runtime.consecutive_failures = 0;
                    runtime.last_error = None;
                    if !runtime.started {
                        runtime.lifecycle = ProviderLifecycleState::Stopped;
                    }
                }
                Err(error) => {
                    record_failure(runtime, error.clone());
                    runtime.probe_failed = true;
                    runtime.descriptor.capabilities = runtime
                        .descriptor
                        .capabilities
                        .iter()
                        .cloned()
                        .map(|mut capability| {
                            if capability.support_status == CapabilitySupportStatus::Supported {
                                capability.support_status = CapabilitySupportStatus::Unsupported;
                                capability.reason_code = Some(error.code);
                            }
                            capability
                        })
                        .collect();
                    runtime.lifecycle = ProviderLifecycleState::Failed;
                    runtime.retry_action = None;
                    runtime.retry_at = None;
                }
            }
        }
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers
            .values()
            .map(|runtime| runtime.descriptor.clone())
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
        let control_deadline = Instant::now() + PROVIDER_CONTROL_TIMEOUT;
        for (provider_id, runtime) in &mut self.providers {
            let Some(next_provider_plan) = next_plan.providers.get(provider_id).cloned() else {
                continue;
            };
            let plan_changed = runtime.plan.as_ref() != Some(&next_provider_plan);
            runtime.plan = Some(next_provider_plan.clone());

            if self.paused {
                let stop_result = stop_runtime(runtime, control_deadline);
                runtime.lifecycle = if next_provider_plan.enabled {
                    if stop_result.is_ok() {
                        ProviderLifecycleState::Paused
                    } else {
                        ProviderLifecycleState::Failed
                    }
                } else {
                    ProviderLifecycleState::Stopped
                };
                runtime.next_sample_at = None;
                runtime.retry_action = None;
                runtime.retry_at = None;
                continue;
            }

            if !next_provider_plan.enabled {
                let stop_result = stop_runtime(runtime, control_deadline);
                runtime.lifecycle = if stop_result.is_ok() {
                    ProviderLifecycleState::Stopped
                } else {
                    ProviderLifecycleState::Failed
                };
                runtime.next_sample_at = None;
                runtime.retry_action = None;
                runtime.retry_at = None;
                continue;
            }

            if runtime.started && !plan_changed {
                continue;
            }
            let action = if runtime.started {
                runtime.started = false;
                ProviderRetryAction::Reconfigure
            } else {
                ProviderRetryAction::Start
            };
            attempt_start(runtime, &next_provider_plan, action, now, control_deadline);
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
        let tracked_app_keys = Arc::new(tracked_app_keys.clone());
        for runtime in self.providers.values_mut() {
            let Some(plan) = runtime.plan.clone() else {
                continue;
            };
            if !plan.enabled {
                continue;
            }
            if !runtime.started {
                let Some(action) = runtime.retry_action else {
                    continue;
                };
                if runtime.retry_at.is_some_and(|retry_at| retry_at > now) {
                    continue;
                }
                attempt_start(
                    runtime,
                    &plan,
                    action,
                    now,
                    Instant::now() + PROVIDER_CONTROL_TIMEOUT,
                );
                if !runtime.started {
                    continue;
                }
            }
            if runtime.next_sample_at.is_some_and(|next| next > now) {
                continue;
            }
            runtime.next_sample_at = Some(now + Duration::from_millis(plan.interval_ms.max(1)));
            match runtime.executor.sample(
                timestamp_ms,
                tracked_app_keys.clone(),
                Instant::now() + PROVIDER_SAMPLE_TIMEOUT,
            ) {
                Ok((sample, health)) => {
                    update_health(runtime, health);
                    runtime.lifecycle = ProviderLifecycleState::Running;
                    runtime.consecutive_failures = 0;
                    runtime.last_error = None;
                    runtime.provider_health.last_error = None;
                    if sample.is_some() {
                        runtime.last_success_at_ms = Some(timestamp_ms);
                    }
                    if let Some(sample) = sample {
                        samples.push(sample);
                    }
                }
                Err(error) => {
                    record_failure(runtime, error);
                    let backoff = failure_backoff(plan.interval_ms, runtime.consecutive_failures);
                    runtime.next_sample_at = Some(now + backoff);
                }
            }
        }
        samples
    }

    pub fn pause(&mut self, deadline: Instant) -> Result<(), ProviderError> {
        if self.paused {
            return Ok(());
        }
        self.paused = true;
        let mut first_error = None;
        for runtime in self.providers.values_mut() {
            let stop_result = stop_runtime(runtime, deadline);
            runtime.next_sample_at = None;
            runtime.retry_action = None;
            runtime.retry_at = None;
            if runtime.plan.as_ref().is_some_and(|plan| plan.enabled) {
                runtime.lifecycle = if stop_result.is_ok() {
                    ProviderLifecycleState::Paused
                } else {
                    ProviderLifecycleState::Failed
                };
            }
            if let Err(error) = stop_result {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub fn resume(&mut self, now: Instant) {
        if !self.paused {
            return;
        }
        self.paused = false;
        self.apply_plan_inner(self.plan.clone(), now, true);
    }

    pub fn stop_all(&mut self, deadline: Instant) -> Result<(), ProviderError> {
        let mut first_error = None;
        for runtime in self.providers.values_mut() {
            let stop_result = stop_runtime(runtime, deadline);
            runtime.next_sample_at = None;
            runtime.retry_action = None;
            runtime.retry_at = None;
            if let Err(error) = stop_result {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub fn statuses(&self) -> Vec<ProviderStatus> {
        self.providers
            .iter()
            .map(|(provider_id, runtime)| {
                let descriptor = &runtime.descriptor;
                let observed_health = &runtime.provider_health;
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
                        let stop_failed = runtime.lifecycle == ProviderLifecycleState::Failed
                            && runtime.last_error.as_ref().is_some_and(|error| {
                                matches!(
                                    error.code,
                                    ProviderErrorCode::StopFailed | ProviderErrorCode::Timeout
                                )
                            });
                        let (state, reason_code) = match capability.support_status {
                            CapabilitySupportStatus::Unsupported => (
                                CapabilityState::Unsupported,
                                capability
                                    .reason_code
                                    .or(Some(ProviderErrorCode::Unsupported)),
                            ),
                            CapabilitySupportStatus::Supported if stop_failed => (
                                CapabilityState::Failed,
                                runtime
                                    .last_error
                                    .as_ref()
                                    .map(|error| error.code)
                                    .or(Some(ProviderErrorCode::StopFailed)),
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
                    last_error: runtime
                        .last_error
                        .clone()
                        .or_else(|| observed_health.last_error.clone()),
                }
            })
            .collect()
    }
}

fn update_health(runtime: &mut ProviderRuntime, health: ProviderHealthObservation) {
    runtime.last_success_at_ms = health.last_success_at_ms.or(runtime.last_success_at_ms);
    runtime.failure_count = runtime.failure_count.max(health.failure_count);
    runtime.provider_health = health;
}

fn clear_retry(runtime: &mut ProviderRuntime) {
    runtime.retry_action = None;
    runtime.retry_at = None;
}

fn record_failure(runtime: &mut ProviderRuntime, error: ProviderError) {
    runtime.lifecycle = ProviderLifecycleState::Failed;
    runtime.failure_count = runtime.failure_count.saturating_add(1);
    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
    let summary = error_summary(error);
    runtime.last_error = Some(summary.clone());
    runtime.provider_health.failure_count = runtime
        .provider_health
        .failure_count
        .max(runtime.failure_count);
    runtime.provider_health.last_error = Some(summary);
}

fn attempt_start(
    runtime: &mut ProviderRuntime,
    plan: &ProviderPlan,
    action: ProviderRetryAction,
    now: Instant,
    deadline: Instant,
) {
    let result = match action {
        ProviderRetryAction::Start => runtime.executor.start(plan, deadline),
        ProviderRetryAction::Reconfigure => runtime.executor.reconfigure(plan, deadline),
    };
    match result {
        Ok(health) => {
            update_health(runtime, health);
            runtime.started = true;
            runtime.lifecycle = ProviderLifecycleState::Running;
            runtime.last_error = None;
            runtime.stop_failure = None;
            runtime.provider_health.last_error = None;
            runtime.consecutive_failures = 0;
            runtime.next_sample_at = Some(now);
            clear_retry(runtime);
        }
        Err(error) => {
            runtime.started = false;
            record_failure(runtime, error);
            runtime.retry_action = Some(action);
            runtime.retry_at =
                Some(now + failure_backoff(plan.interval_ms, runtime.consecutive_failures));
            runtime.next_sample_at = None;
        }
    }
}

fn stop_runtime(runtime: &mut ProviderRuntime, deadline: Instant) -> Result<(), ProviderError> {
    runtime.executor.cancel_pending();
    let needs_stop = runtime.started || runtime.executor.pending();
    if !needs_stop {
        runtime.started = false;
        runtime.next_sample_at = None;
        clear_retry(runtime);
        if let Some(error) = &runtime.stop_failure {
            return Err(ProviderError {
                code: error.code,
                message: error.message.clone(),
            });
        }
        if runtime.lifecycle != ProviderLifecycleState::Failed {
            runtime.lifecycle = ProviderLifecycleState::Stopped;
        }
        return Ok(());
    }
    let result = runtime.executor.stop(deadline);
    runtime.started = false;
    runtime.next_sample_at = None;
    clear_retry(runtime);
    match result {
        Ok(health) => {
            update_health(runtime, health);
            runtime.lifecycle = ProviderLifecycleState::Stopped;
            runtime.last_error = None;
            runtime.stop_failure = None;
            Ok(())
        }
        Err(error) => {
            record_failure(runtime, error.clone());
            runtime.stop_failure = runtime.last_error.clone();
            runtime.lifecycle = ProviderLifecycleState::Failed;
            Err(error)
        }
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
    declared_capabilities: Vec<ProviderCapabilitySpec>,
    sampler: Option<SystemSampler>,
    enabled_categories: BTreeSet<MetricCategory>,
    health: ProviderHealthObservation,
    disk_probe: Box<dyn DiskCapabilityProbe>,
}

impl WindowsBaselineProvider {
    pub fn new() -> Self {
        Self::with_disk_probe(Box::new(PdhDiskCapabilityProbe))
    }

    pub fn with_disk_probe(disk_probe: Box<dyn DiskCapabilityProbe>) -> Self {
        let declared_capabilities = vec![
            ProviderCapabilitySpec::supported(MetricCategory::Cpu),
            ProviderCapabilitySpec::supported(MetricCategory::Memory),
            ProviderCapabilitySpec::supported(MetricCategory::Disk),
            ProviderCapabilitySpec::supported(MetricCategory::Process),
        ];
        Self {
            descriptor: ProviderDescriptor {
                id: WINDOWS_BASELINE_PROVIDER_ID.to_string(),
                display_name: "Windows baseline".to_string(),
                schedule: ProviderSchedule::System,
                capabilities: declared_capabilities.clone(),
            },
            declared_capabilities,
            sampler: None,
            enabled_categories: BTreeSet::new(),
            health: ProviderHealthObservation::default(),
            disk_probe,
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

    fn probe(
        &mut self,
        context: &ProviderCallContext,
        requested_categories: &BTreeSet<MetricCategory>,
    ) -> Result<Vec<ProviderCapabilitySpec>, ProviderError> {
        context.check()?;
        let disk_result = requested_categories
            .contains(&MetricCategory::Disk)
            .then(|| self.disk_probe.probe());
        let capabilities = self
            .declared_capabilities
            .clone()
            .into_iter()
            .map(|mut capability| {
                if capability.category == MetricCategory::Disk {
                    match disk_result.as_ref() {
                        Some(Ok(())) | None => {
                            capability.support_status = CapabilitySupportStatus::Supported;
                            capability.reason_code = None;
                        }
                        Some(Err(reason_code)) => {
                            capability.support_status = CapabilitySupportStatus::Unsupported;
                            capability.reason_code = Some(*reason_code);
                        }
                    }
                }
                capability
            })
            .collect::<Vec<_>>();
        self.descriptor.capabilities = capabilities.clone();
        Ok(capabilities)
    }

    fn start(
        &mut self,
        plan: &ProviderPlan,
        context: &ProviderCallContext,
    ) -> Result<(), ProviderError> {
        context.check()?;
        self.enabled_categories = plan.enabled_categories.iter().copied().collect();
        let mut sampler = SystemSampler::new_for_categories(&self.enabled_categories);
        if self.enabled_categories.contains(&MetricCategory::Disk) && !sampler.disk_available() {
            let reason_code = self
                .disk_probe
                .probe()
                .err()
                .unwrap_or(ProviderErrorCode::ProviderMissing);
            self.set_disk_unavailable(reason_code);
            self.enabled_categories.remove(&MetricCategory::Disk);
            sampler = SystemSampler::new_for_categories(&self.enabled_categories);
        }
        self.sampler = Some(sampler);
        self.health.last_error = None;
        Ok(())
    }

    fn sample(
        &mut self,
        context: &ProviderCallContext,
        timestamp_ms: i64,
        tracked_app_keys: &HashSet<String>,
    ) -> Result<Option<ProviderSample>, ProviderError> {
        context.check()?;
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

    fn stop(&mut self, context: &ProviderCallContext) -> Result<(), ProviderError> {
        context.check()?;
        self.sampler = None;
        self.enabled_categories.clear();
        Ok(())
    }

    fn health(&self) -> ProviderHealthObservation {
        self.health.clone()
    }
}

impl WindowsBaselineProvider {
    fn set_disk_unavailable(&mut self, reason_code: ProviderErrorCode) {
        for capability in &mut self.descriptor.capabilities {
            if capability.category == MetricCategory::Disk {
                capability.support_status = CapabilitySupportStatus::Unsupported;
                capability.reason_code = Some(reason_code);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppResourceSample, SystemSample};
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

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
        start_failures_remaining: Arc<Mutex<u32>>,
        reconfigure_failures_remaining: Arc<Mutex<u32>>,
        sample_failures_remaining: Arc<Mutex<u32>>,
        sample_delay: Option<Duration>,
        stop_failure: Option<ProviderErrorCode>,
        stop_delay: Option<Duration>,
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
                    start_failures_remaining: Arc::new(Mutex::new(0)),
                    reconfigure_failures_remaining: Arc::new(Mutex::new(0)),
                    sample_failures_remaining: Arc::new(Mutex::new(0)),
                    sample_delay: None,
                    stop_failure: None,
                    stop_delay: None,
                    health: ProviderHealthObservation::default(),
                },
                counters,
            )
        }

        fn startup_failure(self) -> Self {
            self.startup_failures(1)
        }

        fn startup_failures(self, count: u32) -> Self {
            *self.start_failures_remaining.lock().unwrap() = count;
            self
        }

        fn reconfigure_failures(self, count: u32) -> Self {
            *self.reconfigure_failures_remaining.lock().unwrap() = count;
            self
        }

        fn sample_failures(self, count: u32) -> Self {
            *self.sample_failures_remaining.lock().unwrap() = count;
            self
        }

        fn sample_delay(mut self, delay: Duration) -> Self {
            self.sample_delay = Some(delay);
            self
        }

        fn stop_failure(mut self, code: ProviderErrorCode) -> Self {
            self.stop_failure = Some(code);
            self
        }

        fn stop_delay(mut self, delay: Duration) -> Self {
            self.stop_delay = Some(delay);
            self
        }
    }

    impl MetricProvider for FakeProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        fn start(
            &mut self,
            _plan: &ProviderPlan,
            context: &ProviderCallContext,
        ) -> Result<(), ProviderError> {
            context.check()?;
            self.counters.lock().unwrap().start_count += 1;
            let should_fail = {
                let mut remaining = self.start_failures_remaining.lock().unwrap();
                let should_fail = *remaining > 0;
                *remaining = remaining.saturating_sub(1);
                should_fail
            };
            if should_fail {
                return Err(ProviderError::new(
                    ProviderErrorCode::StartupFailed,
                    "deterministic startup failure",
                ));
            }
            Ok(())
        }

        fn reconfigure(
            &mut self,
            plan: &ProviderPlan,
            context: &ProviderCallContext,
        ) -> Result<(), ProviderError> {
            context.check()?;
            self.counters.lock().unwrap().reconfigure_count += 1;
            let should_fail = {
                let mut remaining = self.reconfigure_failures_remaining.lock().unwrap();
                let should_fail = *remaining > 0;
                *remaining = remaining.saturating_sub(1);
                should_fail
            };
            if should_fail {
                return Err(ProviderError::new(
                    ProviderErrorCode::ReconfigureFailed,
                    "deterministic reconfigure failure",
                ));
            }
            self.stop(context)?;
            self.start(plan, context)
        }

        fn sample(
            &mut self,
            context: &ProviderCallContext,
            timestamp_ms: i64,
            _tracked_app_keys: &HashSet<String>,
        ) -> Result<Option<ProviderSample>, ProviderError> {
            context.check()?;
            self.counters.lock().unwrap().sample_count += 1;
            if let Some(delay) = self.sample_delay {
                let deadline = Instant::now() + delay;
                while Instant::now() < deadline {
                    context.check()?;
                    thread::sleep(Duration::from_millis(2));
                }
            }
            let should_fail = {
                let mut remaining = self.sample_failures_remaining.lock().unwrap();
                let should_fail = *remaining > 0;
                *remaining = remaining.saturating_sub(1);
                should_fail
            };
            if should_fail {
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
                    disk_read_bytes_per_sec: Some(0),
                    disk_write_bytes_per_sec: Some(0),
                    has_app_snapshot: false,
                },
                apps: Vec::<AppResourceSample>::new(),
            };
            self.health.last_success_at_ms = Some(timestamp_ms);
            Ok(Some(ProviderSample::ResourceSnapshot(snapshot)))
        }

        fn stop(&mut self, context: &ProviderCallContext) -> Result<(), ProviderError> {
            self.counters.lock().unwrap().stop_count += 1;
            if let Some(delay) = self.stop_delay {
                let deadline = Instant::now() + delay;
                while Instant::now() < deadline {
                    context.check()?;
                    thread::sleep(Duration::from_millis(2));
                }
            }
            if let Some(code) = self.stop_failure {
                return Err(ProviderError::without_message(code));
            }
            Ok(())
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
                ProviderCapabilitySpec::supported(MetricCategory::Disk),
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
        let capabilities = &statuses[0].capabilities;
        let cpu = capabilities
            .iter()
            .find(|capability| capability.category == MetricCategory::Cpu)
            .unwrap();
        assert_eq!(cpu.state, CapabilityState::SupportedEnabled);
        let samples = sample_at(&mut host, now, 100);
        let ProviderSample::ResourceSnapshot(snapshot) = &samples[0];
        assert_eq!(snapshot.system.cpu_percent, Some(0.0));
        assert_eq!(snapshot.system.disk_read_bytes_per_sec, Some(0));
        assert_eq!(snapshot.system.disk_write_bytes_per_sec, Some(0));
        assert_eq!(
            capabilities
                .iter()
                .find(|capability| capability.category == MetricCategory::Gpu)
                .unwrap()
                .state,
            CapabilityState::Unsupported
        );
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

    struct FakeDiskProbe {
        result: Result<(), ProviderErrorCode>,
        calls: Arc<Mutex<u32>>,
    }

    impl FakeDiskProbe {
        fn new(result: Result<(), ProviderErrorCode>) -> (Self, Arc<Mutex<u32>>) {
            let calls = Arc::new(Mutex::new(0));
            (
                Self {
                    result,
                    calls: calls.clone(),
                },
                calls,
            )
        }
    }

    impl DiskCapabilityProbe for FakeDiskProbe {
        fn probe(&self) -> Result<(), ProviderErrorCode> {
            *self.calls.lock().unwrap() += 1;
            self.result
        }
    }

    fn capability_for(
        descriptor: &ProviderDescriptor,
        category: MetricCategory,
    ) -> &ProviderCapabilitySpec {
        descriptor
            .capabilities
            .iter()
            .find(|capability| capability.category == category)
            .unwrap()
    }

    #[test]
    fn disk_capability_probe_success_is_supported() {
        let (probe, calls) = FakeDiskProbe::new(Ok(()));
        let provider = WindowsBaselineProvider::with_disk_probe(Box::new(probe));
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        host.probe_all(Instant::now() + Duration::from_secs(2));
        assert_eq!(*calls.lock().unwrap(), 1);
        let descriptor = &host.descriptors()[0];
        assert_eq!(
            capability_for(descriptor, MetricCategory::Disk).support_status,
            CapabilitySupportStatus::Supported
        );
        assert_eq!(
            capability_for(descriptor, MetricCategory::Disk).reason_code,
            None
        );
    }

    #[test]
    fn disk_capability_unavailable_excludes_only_disk() {
        let (probe, calls) = FakeDiskProbe::new(Err(ProviderErrorCode::ProviderMissing));
        let provider = WindowsBaselineProvider::with_disk_probe(Box::new(probe));
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        host.probe_all(Instant::now() + Duration::from_secs(2));
        assert_eq!(*calls.lock().unwrap(), 1);
        let settings = settings_with(vec![
            MetricCategory::Cpu,
            MetricCategory::Memory,
            MetricCategory::Disk,
            MetricCategory::Process,
        ]);
        let plan = plan_for(&host, &settings);
        assert!(!plan
            .provider(WINDOWS_BASELINE_PROVIDER_ID)
            .unwrap()
            .enabled_categories
            .contains(&MetricCategory::Disk));
        assert!(plan
            .provider(WINDOWS_BASELINE_PROVIDER_ID)
            .unwrap()
            .enabled_categories
            .contains(&MetricCategory::Cpu));
        host.apply_plan(plan, Instant::now());
        let status = &host.statuses()[0];
        assert_eq!(
            status
                .capabilities
                .iter()
                .find(|capability| capability.category == MetricCategory::Disk)
                .unwrap()
                .state,
            CapabilityState::Unsupported
        );
        for category in [
            MetricCategory::Cpu,
            MetricCategory::Memory,
            MetricCategory::Process,
        ] {
            assert_eq!(
                status
                    .capabilities
                    .iter()
                    .find(|capability| capability.category == category)
                    .unwrap()
                    .state,
                CapabilityState::SupportedEnabled
            );
        }
        assert_eq!(
            status
                .capabilities
                .iter()
                .find(|capability| capability.category == MetricCategory::Disk)
                .unwrap()
                .reason_code,
            Some(ProviderErrorCode::ProviderMissing)
        );
    }

    #[test]
    fn disabled_disk_does_not_probe_or_create_disk_sampling_resources() {
        let (probe, calls) = FakeDiskProbe::new(Err(ProviderErrorCode::ProviderMissing));
        let provider = WindowsBaselineProvider::with_disk_probe(Box::new(probe));
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let settings = settings_with(vec![
            MetricCategory::Cpu,
            MetricCategory::Memory,
            MetricCategory::Process,
        ]);
        host.probe_all_for_settings(Instant::now() + Duration::from_secs(2), &settings);
        let plan = plan_for(&host, &settings);
        host.apply_plan(plan, Instant::now());
        assert_eq!(*calls.lock().unwrap(), 0);
        let statuses = host.statuses();
        let disk = statuses[0]
            .capabilities
            .iter()
            .find(|capability| capability.category == MetricCategory::Disk)
            .unwrap();
        assert_eq!(disk.state, CapabilityState::SupportedDisabled);
    }

    #[test]
    fn startup_failure_retries_with_backoff_and_recovers_without_affecting_healthy_provider() {
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
        let failing = failing.startup_failures(1);
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
        assert_eq!(failing_counters.lock().unwrap().start_count, 1);

        let before_retry = now + Duration::from_millis(5);
        assert_eq!(sample_at(&mut host, before_retry, 2).len(), 0);
        assert_eq!(failing_counters.lock().unwrap().start_count, 1);
        assert_eq!(healthy_counters.lock().unwrap().sample_count, 1);

        let recovered_at = now + Duration::from_millis(20);
        assert_eq!(sample_at(&mut host, recovered_at, 3).len(), 2);
        assert_eq!(failing_counters.lock().unwrap().start_count, 2);
        let status = host
            .statuses()
            .into_iter()
            .find(|status| status.provider_id == "failing")
            .unwrap();
        assert_eq!(status.lifecycle, ProviderLifecycleState::Running);
        assert_eq!(status.failure_count, 1);
        assert!(status.last_error.is_none());
    }

    #[test]
    fn reconfigure_failure_retries_after_backoff() {
        let (provider, counters) = FakeProvider::new(
            "system",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::System,
        );
        let provider = provider.reconfigure_failures(1);
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let initial = settings_with(vec![MetricCategory::Cpu]);
        let now = Instant::now();
        host.apply_plan(plan_for(&host, &initial), now);
        let changed = CollectionSettings {
            system_sample_interval_ms: 10_000,
            ..initial
        };
        host.apply_plan(plan_for(&host, &changed), now + Duration::from_millis(1));
        assert_eq!(counters.lock().unwrap().reconfigure_count, 1);
        assert_eq!(host.statuses()[0].lifecycle, ProviderLifecycleState::Failed);
        assert!(sample_at(&mut host, now + Duration::from_secs(1), 1).is_empty());
        assert_eq!(counters.lock().unwrap().reconfigure_count, 1);

        assert_eq!(
            sample_at(&mut host, now + Duration::from_secs(11), 2).len(),
            1
        );
        assert_eq!(counters.lock().unwrap().reconfigure_count, 2);
        assert_eq!(
            host.statuses()[0].lifecycle,
            ProviderLifecycleState::Running
        );
    }

    #[test]
    fn disable_cancels_pending_start_retry() {
        let (provider, counters) = FakeProvider::new(
            "failing",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let provider = provider.startup_failure();
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let settings = settings_with(vec![MetricCategory::Cpu]);
        let now = Instant::now();
        host.apply_plan(plan_for(&host, &settings), now);
        host.apply_plan(
            plan_for(&host, &settings_with(Vec::new())),
            now + Duration::from_millis(1),
        );
        assert!(sample_at(&mut host, now + Duration::from_secs(1), 1).is_empty());
        assert_eq!(counters.lock().unwrap().start_count, 1);
        assert_eq!(
            host.statuses()[0].capabilities[0].state,
            CapabilityState::SupportedDisabled
        );
    }

    #[test]
    fn pause_cancels_start_retry_and_resume_restarts_plan() {
        let (provider, counters) = FakeProvider::new(
            "pausable",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let provider = provider.startup_failure();
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let settings = settings_with(vec![MetricCategory::Cpu]);
        let now = Instant::now();
        host.apply_plan(plan_for(&host, &settings), now);
        host.pause(Instant::now() + Duration::from_secs(2)).unwrap();
        assert_eq!(host.statuses()[0].lifecycle, ProviderLifecycleState::Paused);
        assert!(sample_at(&mut host, now + Duration::from_secs(1), 1).is_empty());
        assert_eq!(counters.lock().unwrap().start_count, 1);
        host.resume(now + Duration::from_secs(1));
        assert_eq!(counters.lock().unwrap().start_count, 2);
        assert_eq!(
            host.statuses()[0].lifecycle,
            ProviderLifecycleState::Running
        );
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
    fn sample_timeout_isolated_from_healthy_provider() {
        let (slow, slow_counters) = FakeProvider::new(
            "a-slow",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let slow = slow.sample_delay(Duration::from_secs(2));
        let (healthy, healthy_counters) = FakeProvider::new(
            "b-healthy",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Memory)],
            ProviderSchedule::Fixed(10),
        );
        let mut host = ProviderHost::new(vec![Box::new(slow), Box::new(healthy)]);
        let now = Instant::now();
        let settings = settings_with(vec![MetricCategory::Cpu, MetricCategory::Memory]);
        host.apply_plan(plan_for(&host, &settings), now);
        let samples = sample_at(&mut host, now, 1);
        assert_eq!(samples.len(), 1);
        assert_eq!(slow_counters.lock().unwrap().sample_count, 1);
        assert_eq!(healthy_counters.lock().unwrap().sample_count, 1);
        let slow_status = host
            .statuses()
            .into_iter()
            .find(|status| status.provider_id == "a-slow")
            .unwrap();
        assert_eq!(slow_status.lifecycle, ProviderLifecycleState::Failed);
        assert_eq!(
            slow_status.last_error.unwrap().code,
            ProviderErrorCode::Timeout
        );
    }

    #[test]
    fn stop_failure_is_reported_and_stop_is_idempotent() {
        let (provider, counters) = FakeProvider::new(
            "stop-fails",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let provider = provider.stop_failure(ProviderErrorCode::StopFailed);
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let settings = settings_with(vec![MetricCategory::Cpu]);
        host.apply_plan(plan_for(&host, &settings), Instant::now());
        assert!(host
            .stop_all(Instant::now() + Duration::from_secs(2))
            .is_err());
        assert!(host
            .stop_all(Instant::now() + Duration::from_secs(2))
            .is_err());
        assert_eq!(counters.lock().unwrap().stop_count, 1);
        let status = &host.statuses()[0];
        assert_eq!(status.lifecycle, ProviderLifecycleState::Failed);
        assert_eq!(
            status.last_error.as_ref().unwrap().code,
            ProviderErrorCode::StopFailed
        );
        assert_eq!(status.capabilities[0].state, CapabilityState::Failed);
    }

    #[test]
    fn stop_timeout_does_not_wait_past_deadline() {
        let (provider, counters) = FakeProvider::new(
            "stop-timeout",
            vec![ProviderCapabilitySpec::supported(MetricCategory::Cpu)],
            ProviderSchedule::Fixed(10),
        );
        let provider = provider.stop_delay(Duration::from_secs(1));
        let mut host = ProviderHost::new(vec![Box::new(provider)]);
        let settings = settings_with(vec![MetricCategory::Cpu]);
        host.apply_plan(plan_for(&host, &settings), Instant::now());
        let deadline = Instant::now() + Duration::from_millis(30);
        assert!(host.stop_all(deadline).is_err());
        assert_eq!(counters.lock().unwrap().stop_count, 1);
        assert_eq!(
            host.statuses()[0].last_error.as_ref().unwrap().code,
            ProviderErrorCode::Timeout
        );
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

        host.pause(Instant::now() + Duration::from_secs(2)).unwrap();
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
        host.stop_all(Instant::now() + Duration::from_secs(2))
            .unwrap();
        host.stop_all(Instant::now() + Duration::from_secs(2))
            .unwrap();
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
