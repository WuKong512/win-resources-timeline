mod db;
mod machine;
mod process;
mod stats;

use db::{CollectionConfiguration, DbObservation, ProviderObservation};
use machine::MachineMetadata;
use process::ProcessSampler;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    env,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const EVIDENCE_SCHEMA: &str = "pr-07b-native-evidence/v1";
const CPU_BASIS: &str = "whole_machine_percentage";
const MAX_BUSY_HOLD: Duration = Duration::from_secs(30);

#[derive(Debug)]
enum Command {
    Run(RunConfig),
    Summary {
        input: PathBuf,
        events: Option<PathBuf>,
        output: Option<PathBuf>,
    },
    Mark {
        output: PathBuf,
        kind: String,
    },
    DatabaseBusy {
        database: PathBuf,
        duration: Duration,
        events: Option<PathBuf>,
    },
    Schema {
        database: PathBuf,
    },
}

#[derive(Debug)]
struct RunConfig {
    pid: u32,
    database: PathBuf,
    output: PathBuf,
    events: Option<PathBuf>,
    duration: Duration,
    cadence: Duration,
    warmup: Duration,
    app_version: Option<String>,
    git_commit: Option<String>,
    build_type: String,
    executable_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppIdentity {
    pid: u32,
    executable_name: String,
    app_version: Option<String>,
    git_commit: Option<String>,
    build_type: String,
    creation_time_utc_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunConfiguration {
    duration_ms: u64,
    observation_cadence_ms: u64,
    warmup_ms: u64,
    cpu_basis: String,
    database_file_name: String,
    external_writer_health: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivacyBoundary {
    sanitized: bool,
    omitted: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunHeader {
    evidence_schema: String,
    harness_version: String,
    run_kind: String,
    started_at_utc: String,
    started_at_epoch_ms: i64,
    app: AppIdentity,
    configuration: RunConfiguration,
    machine: MachineMetadata,
    privacy: PrivacyBoundary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    timestamp_utc: String,
    timestamp_epoch_ms: i64,
    elapsed_ms: u64,
    in_warmup: bool,
    wall_gap_ms: i64,
    monotonic_gap_ms: u64,
    wall_clock_moved_backward: bool,
    possible_sleep_or_scheduler_gap: bool,
    process_alive: bool,
    process_restarted: bool,
    process_cpu_percent: Option<f64>,
    process_cpu_basis: String,
    working_set_bytes: Option<u64>,
    private_bytes: Option<u64>,
    thread_count: Option<u32>,
    handle_count: Option<u32>,
    database: DbObservation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperatorEvent {
    event_kind: String,
    timestamp_utc: String,
    timestamp_epoch_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunFooter {
    ended_at_utc: String,
    ended_at_epoch_ms: i64,
    actual_elapsed_ms: u64,
    status: String,
    termination_reason: String,
    observation_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "recordType", rename_all = "camelCase")]
enum RawRecord {
    Header { value: RunHeader },
    Observation { value: Observation },
    Event { value: OperatorEvent },
    Footer { value: RunFooter },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DistributionSummary {
    sample_count: u64,
    average: Option<f64>,
    p50: Option<f64>,
    p95: Option<f64>,
    max: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemorySummary {
    start_bytes: Option<u64>,
    end_bytes: Option<u64>,
    min_bytes: Option<u64>,
    max_bytes: Option<u64>,
    growth_bytes: Option<i64>,
    linear_slope_bytes_per_hour: Option<f64>,
    observed_trend: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileGrowthSummary {
    start_bytes: Option<u64>,
    end_bytes: Option<u64>,
    max_bytes: Option<u64>,
    growth_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StorageSummary {
    main: FileGrowthSummary,
    wal: FileGrowthSummary,
    shm: FileGrowthSummary,
    total: FileGrowthSummary,
    observed_bytes_per_hour: Option<f64>,
    projected_24h_bytes: Option<f64>,
    projected_7d_bytes: Option<f64>,
    projection_kind: &'static str,
    slope_changed_possible: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeObjectSummary {
    thread_start: Option<u32>,
    thread_end: Option<u32>,
    thread_max: Option<u32>,
    thread_growth: Option<i64>,
    handle_start: Option<u32>,
    handle_end: Option<u32>,
    handle_max: Option<u32>,
    handle_growth: Option<i64>,
    process_restarts: u64,
    sustained_growth_is_not_automatically_a_leak: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReliabilitySummary {
    samples_expected: u64,
    samples_observed: u64,
    observation_gap_count: u64,
    wall_gap_total_ms: i64,
    max_wall_gap_ms: i64,
    collector_drop_delta: Option<u64>,
    collector_drop_observation: &'static str,
    writer_drop_delta: Option<u64>,
    writer_drop_observation: &'static str,
    committed_frame_count_end: Option<u64>,
    writer_delay_average_ms: Option<f64>,
    writer_delay_max_ms: Option<i64>,
    provider_failure_transitions: u64,
    provider_recovery_transitions: u64,
    unexpected_termination: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryReport {
    evidence_schema: &'static str,
    status: String,
    started_at_utc: Option<String>,
    ended_at_utc: Option<String>,
    actual_elapsed_ms: Option<u64>,
    app: Option<AppIdentity>,
    configuration: Option<RunConfiguration>,
    cpu: DistributionSummary,
    cpu_all_observations: DistributionSummary,
    memory: MemorySummary,
    private_memory: MemorySummary,
    storage: StorageSummary,
    reliability: ReliabilitySummary,
    runtime_objects: RuntimeObjectSummary,
    database_user_version_start: Option<i64>,
    database_user_version_end: Option<i64>,
    collection_configuration: Option<CollectionConfiguration>,
    provider_health_start: Vec<ProviderObservation>,
    provider_health_end: Vec<ProviderObservation>,
    active_retention_holds_start: Option<u64>,
    active_retention_holds_end: Option<u64>,
    operator_events: Vec<String>,
    limitations: Vec<&'static str>,
}

fn main() {
    if let Err(error) = run_main() {
        eprintln!("release-soak: {error}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    match parse_command(env::args().skip(1).collect())? {
        Command::Run(config) => run(config),
        Command::Summary {
            input,
            events,
            output,
        } => summarize(&input, events.as_deref(), output.as_deref()),
        Command::Mark { output, kind } => mark(&output, &kind),
        Command::DatabaseBusy {
            database,
            duration,
            events,
        } => database_busy(&database, duration, events.as_deref()),
        Command::Schema { database } => schema(&database),
    }
}

fn run(config: RunConfig) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("native release observation requires Windows".to_string());
    }
    if config.duration.is_zero() {
        return Err("duration must be greater than zero".to_string());
    }
    if config.cadence < Duration::from_secs(1) {
        return Err("observation cadence must be at least 1s".to_string());
    }
    if config.warmup >= config.duration {
        return Err("warmup must be shorter than duration".to_string());
    }
    create_parent(&config.output)?;
    if let Some(events) = &config.events {
        create_parent(events)?;
    }
    let machine = machine::collect();
    let mut sampler = ProcessSampler::attach(config.pid, machine.logical_processor_count)?;
    let started_at = Instant::now();
    let started_epoch_ms = unix_now_ms();
    let header = RunHeader {
        evidence_schema: EVIDENCE_SCHEMA.to_string(),
        harness_version: env!("CARGO_PKG_VERSION").to_string(),
        run_kind: "native_process_observation".to_string(),
        started_at_utc: utc_iso(started_epoch_ms),
        started_at_epoch_ms: started_epoch_ms,
        app: AppIdentity {
            pid: config.pid,
            executable_name: sanitize_executable_name(&config.executable_name),
            app_version: config.app_version.clone(),
            git_commit: config.git_commit.clone(),
            build_type: sanitize_token(&config.build_type),
            creation_time_utc_ms: sampler.creation_time_utc_ms(),
        },
        configuration: RunConfiguration {
            duration_ms: config.duration.as_millis().min(u64::MAX as u128) as u64,
            observation_cadence_ms: config.cadence.as_millis().min(u64::MAX as u128) as u64,
            warmup_ms: config.warmup.as_millis().min(u64::MAX as u128) as u64,
            cpu_basis: CPU_BASIS.to_string(),
            database_file_name: file_name(&config.database),
            external_writer_health: "queue_and_drop_counters_not_externally_observable".to_string(),
        },
        machine,
        privacy: PrivacyBoundary {
            sanitized: true,
            omitted: vec![
                "absolute paths".to_string(),
                "user names".to_string(),
                "window titles".to_string(),
                "document/application content".to_string(),
                "process command lines".to_string(),
                "crash raw payloads".to_string(),
                "process executable paths".to_string(),
                "device serials and UUIDs".to_string(),
            ],
        },
    };
    let mut output = BufWriter::new(
        File::create(&config.output).map_err(|_| "create evidence file failed".to_string())?,
    );
    write_record(&mut output, RawRecord::Header { value: header })?;

    let deadline = started_at + config.duration;
    let mut next_observation = started_at;
    let mut previous_epoch_ms = started_epoch_ms;
    let mut previous_elapsed_ms = 0_u64;
    let mut observation_count = 0_u64;
    let mut termination_reason = "duration_reached";
    let mut interrupted = false;

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if now < next_observation {
            thread::sleep((next_observation - now).min(Duration::from_secs(1)));
            continue;
        }
        let timestamp_epoch_ms = unix_now_ms();
        let elapsed_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let wall_gap_ms = timestamp_epoch_ms.saturating_sub(previous_epoch_ms);
        let wall_clock_moved_backward = timestamp_epoch_ms < previous_epoch_ms;
        let monotonic_gap_ms = elapsed_ms.saturating_sub(previous_elapsed_ms);
        let process = sampler.observe();
        let database = db::observe(&config.database, started_epoch_ms);
        let observation = Observation {
            timestamp_utc: utc_iso(timestamp_epoch_ms),
            timestamp_epoch_ms,
            elapsed_ms,
            in_warmup: elapsed_ms < config.warmup.as_millis().min(u64::MAX as u128) as u64,
            wall_gap_ms,
            monotonic_gap_ms,
            wall_clock_moved_backward,
            possible_sleep_or_scheduler_gap: wall_gap_ms
                > config
                    .cadence
                    .as_millis()
                    .saturating_mul(2)
                    .min(i64::MAX as u128) as i64,
            process_alive: process.alive,
            process_restarted: process.restarted,
            process_cpu_percent: process.cpu_percent,
            process_cpu_basis: process.cpu_basis.to_string(),
            working_set_bytes: process.working_set_bytes,
            private_bytes: process.private_bytes,
            thread_count: process.thread_count,
            handle_count: process.handle_count,
            database,
        };
        write_record(&mut output, RawRecord::Observation { value: observation })?;
        output
            .flush()
            .map_err(|_| "flush evidence file failed".to_string())?;
        observation_count = observation_count.saturating_add(1);
        previous_epoch_ms = timestamp_epoch_ms;
        previous_elapsed_ms = elapsed_ms;
        if !process.alive {
            termination_reason = "target_process_exited";
            interrupted = true;
            break;
        }
        if process.restarted {
            termination_reason = "target_process_restarted_or_pid_reused";
            interrupted = true;
            break;
        }
        next_observation = next_observation
            .checked_add(config.cadence)
            .unwrap_or_else(Instant::now);
        if next_observation <= Instant::now() {
            next_observation = Instant::now() + config.cadence;
        }
    }
    let ended_at_epoch_ms = unix_now_ms();
    let actual_elapsed_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;
    if actual_elapsed_ms < config.duration.as_millis().min(u64::MAX as u128) as u64 {
        interrupted = true;
    }
    let footer = RunFooter {
        ended_at_utc: utc_iso(ended_at_epoch_ms),
        ended_at_epoch_ms,
        actual_elapsed_ms,
        status: if interrupted {
            "INTERRUPTED"
        } else {
            "COMPLETE"
        }
        .to_string(),
        termination_reason: termination_reason.to_string(),
        observation_count,
    };
    write_record(&mut output, RawRecord::Footer { value: footer })?;
    output
        .flush()
        .map_err(|_| "flush final evidence file failed".to_string())?;
    println!(
        "native observation {} after {} seconds",
        if interrupted {
            "interrupted"
        } else {
            "complete"
        },
        actual_elapsed_ms / 1000
    );
    if interrupted {
        return Err(format!(
            "qualification run interrupted: {termination_reason}"
        ));
    }
    Ok(())
}

fn summarize(
    input: &Path,
    events_path: Option<&Path>,
    output: Option<&Path>,
) -> Result<(), String> {
    let file = File::open(input).map_err(|_| "open evidence file failed".to_string())?;
    let reader = BufReader::new(file);
    let mut header = None;
    let mut observations = Vec::new();
    let mut footer = None;
    for line in reader.lines() {
        let line = line.map_err(|_| "read evidence file failed".to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<RawRecord>(&line)
            .map_err(|_| "evidence contains an invalid JSON record".to_string())?
        {
            RawRecord::Header { value } => header = Some(value),
            RawRecord::Observation { value } => observations.push(value),
            RawRecord::Footer { value } => footer = Some(value),
            RawRecord::Event { .. } => {}
        }
    }
    let events = if let Some(path) = events_path {
        read_events(path)?
    } else {
        Vec::new()
    };
    let report = build_summary(header, observations, footer, events);
    let json = serde_json::to_string_pretty(&report)
        .map_err(|_| "serialize summary failed".to_string())?;
    if let Some(path) = output {
        create_parent(path)?;
        fs::write(path, format!("{json}\n")).map_err(|_| "write summary failed".to_string())?;
    } else {
        println!("{json}");
    }
    Ok(())
}

fn build_summary(
    header: Option<RunHeader>,
    observations: Vec<Observation>,
    footer: Option<RunFooter>,
    events: Vec<OperatorEvent>,
) -> SummaryReport {
    let actual_elapsed_ms = footer
        .as_ref()
        .map(|value| value.actual_elapsed_ms)
        .or_else(|| observations.last().map(|value| value.elapsed_ms));
    let cadence_ms = header
        .as_ref()
        .map(|value| value.configuration.observation_cadence_ms)
        .unwrap_or(30_000)
        .max(1);
    let all_cpu = observations
        .iter()
        .filter_map(|value| value.process_cpu_percent)
        .collect::<Vec<_>>();
    let post_warmup_cpu = observations
        .iter()
        .filter(|value| !value.in_warmup)
        .filter_map(|value| value.process_cpu_percent)
        .collect::<Vec<_>>();
    let storage = storage_summary(&observations, actual_elapsed_ms);
    let provider_failure_transitions = provider_transition_count(&observations, true);
    let provider_recovery_transitions = provider_transition_count(&observations, false);
    let footer_status = footer
        .as_ref()
        .map(|value| value.status.as_str())
        .unwrap_or("INCOMPLETE")
        .to_string();
    SummaryReport {
        evidence_schema: EVIDENCE_SCHEMA,
        status: footer_status,
        started_at_utc: header.as_ref().map(|value| value.started_at_utc.clone()),
        ended_at_utc: footer.as_ref().map(|value| value.ended_at_utc.clone()),
        actual_elapsed_ms,
        app: header.as_ref().map(|value| value.app.clone()),
        configuration: header.as_ref().map(|value| value.configuration.clone()),
        cpu: distribution(&post_warmup_cpu),
        cpu_all_observations: distribution(&all_cpu),
        memory: memory_summary(&observations, false),
        private_memory: memory_summary(&observations, true),
        storage,
        reliability: ReliabilitySummary {
            samples_expected: actual_elapsed_ms
                .map(|value| expected_observation_count(value, cadence_ms))
                .unwrap_or(0),
            samples_observed: observations.len() as u64,
            observation_gap_count: observations
                .iter()
                .filter(|value| value.possible_sleep_or_scheduler_gap)
                .count() as u64,
            wall_gap_total_ms: observations
                .iter()
                .skip(1)
                .filter(|value| !value.wall_clock_moved_backward)
                .map(|value| value.wall_gap_ms.saturating_sub(cadence_ms as i64))
                .filter(|value| *value > 0)
                .sum(),
            max_wall_gap_ms: observations
                .iter()
                .map(|value| value.wall_gap_ms)
                .max()
                .unwrap_or(0),
            collector_drop_delta: None,
            collector_drop_observation: "not_externally_observable; committed_frame_gaps_and_db_metadata_are_used",
            writer_drop_delta: None,
            writer_drop_observation: "not_externally_observable; writer_delay_is_observed_from_committed_frames",
            committed_frame_count_end: observations
                .last()
                .and_then(|value| value.database.committed_frame_count),
            writer_delay_average_ms: observations
                .last()
                .and_then(|value| value.database.writer_delay_average_ms),
            writer_delay_max_ms: observations
                .last()
                .and_then(|value| value.database.writer_delay_max_ms),
            provider_failure_transitions,
            provider_recovery_transitions,
            unexpected_termination: footer
                .as_ref()
                .is_none_or(|value| value.status != "COMPLETE"),
        },
        runtime_objects: runtime_object_summary(&observations),
        database_user_version_start: observations
            .first()
            .and_then(|value| value.database.user_version),
        database_user_version_end: observations
            .last()
            .and_then(|value| value.database.user_version),
        collection_configuration: observations
            .last()
            .map(|value| value.database.configuration.clone()),
        provider_health_start: observations
            .first()
            .map(|value| value.database.providers.clone())
            .unwrap_or_default(),
        provider_health_end: observations
            .last()
            .map(|value| value.database.providers.clone())
            .unwrap_or_default(),
        active_retention_holds_start: observations
            .first()
            .and_then(|value| value.database.active_retention_hold_count),
        active_retention_holds_end: observations
            .last()
            .and_then(|value| value.database.active_retention_hold_count),
        operator_events: events.into_iter().map(|value| value.event_kind).collect(),
        limitations: vec![
            "The harness does not read arbitrary process command lines or application content.",
            "Runtime collector queue depth, collector drop counter, and FrameWriter drop counter are not externally exported by the frozen production API; the summary records this as unavailable and uses committed-frame gaps plus writer_delay_ms.",
            "Sleep/wake is represented by wall-clock and monotonic observation gaps and operator markers; the harness does not force power state changes.",
            "System-time-change validation uses deterministic product clock seams and is not performed by changing the host OS clock.",
        ],
    }
}

fn distribution(values: &[f64]) -> DistributionSummary {
    DistributionSummary {
        sample_count: values.len() as u64,
        average: stats::average(values),
        p50: stats::percentile(values, 0.50),
        p95: stats::percentile(values, 0.95),
        max: stats::max(values),
    }
}

fn memory_summary(observations: &[Observation], private: bool) -> MemorySummary {
    let values = observations
        .iter()
        .filter_map(|value| {
            if private {
                value.private_bytes
            } else {
                value.working_set_bytes
            }
        })
        .collect::<Vec<_>>();
    let start = values.first().copied();
    let end = values.last().copied();
    let slope_samples = observations
        .iter()
        .filter_map(|value| {
            let memory = if private {
                value.private_bytes
            } else {
                value.working_set_bytes
            }?;
            Some((value.elapsed_ms, memory as f64))
        })
        .collect::<Vec<_>>();
    let slope = stats::linear_slope_per_hour(&slope_samples);
    MemorySummary {
        start_bytes: start,
        end_bytes: end,
        min_bytes: values.iter().copied().min(),
        max_bytes: values.iter().copied().max(),
        growth_bytes: signed_growth(start, end),
        linear_slope_bytes_per_hour: slope,
        observed_trend: slope
            .map(|value| {
                if value.abs() < 1024.0 {
                    "stable_or_inconclusive"
                } else if value > 0.0 {
                    "increasing_observed"
                } else {
                    "decreasing_observed"
                }
            })
            .unwrap_or("insufficient_samples")
            .to_string(),
    }
}

fn storage_summary(observations: &[Observation], actual_elapsed_ms: Option<u64>) -> StorageSummary {
    let main = file_growth(observations, |value| value.database.main_bytes);
    let wal = file_growth(observations, |value| value.database.wal_bytes);
    let shm = file_growth(observations, |value| value.database.shm_bytes);
    let total = file_growth(observations, |value| value.database.total_bytes);
    let observed_bytes_per_hour = match (total.growth_bytes, actual_elapsed_ms) {
        (Some(growth), Some(elapsed)) if elapsed > 0 => {
            Some(growth as f64 / elapsed as f64 * 3_600_000.0)
        }
        _ => None,
    };
    let slope_changed_possible = storage_slope_changed(observations);
    StorageSummary {
        main,
        wal,
        shm,
        total,
        projected_24h_bytes: observed_bytes_per_hour.map(|value| value * 24.0),
        projected_7d_bytes: observed_bytes_per_hour.map(|value| value * 24.0 * 7.0),
        observed_bytes_per_hour,
        projection_kind: "linear_engineering_estimate_not_a_7_day_soak",
        slope_changed_possible,
    }
}

fn file_growth<F>(observations: &[Observation], select: F) -> FileGrowthSummary
where
    F: Fn(&Observation) -> u64,
{
    let values = observations
        .iter()
        .filter(|value| value.database.present)
        .map(select)
        .collect::<Vec<_>>();
    let start = values.first().copied();
    let end = values.last().copied();
    FileGrowthSummary {
        start_bytes: start,
        end_bytes: end,
        max_bytes: values.iter().copied().max(),
        growth_bytes: signed_growth(start, end),
    }
}

fn storage_slope_changed(observations: &[Observation]) -> Option<bool> {
    let values = observations
        .iter()
        .filter(|value| value.database.present)
        .map(|value| (value.elapsed_ms, value.database.total_bytes as f64))
        .collect::<Vec<_>>();
    if values.len() < 4 {
        return None;
    }
    let middle = values.len() / 2;
    let first = stats::linear_slope_per_hour(&values[..middle]);
    let second = stats::linear_slope_per_hour(&values[middle..]);
    match (first, second) {
        (Some(first), Some(second)) => {
            let scale = first.abs().max(second.abs()).max(1.0);
            Some((first - second).abs() / scale > 0.5)
        }
        _ => None,
    }
}

fn runtime_object_summary(observations: &[Observation]) -> RuntimeObjectSummary {
    let threads = observations
        .iter()
        .filter_map(|value| value.thread_count)
        .collect::<Vec<_>>();
    let handles = observations
        .iter()
        .filter_map(|value| value.handle_count)
        .collect::<Vec<_>>();
    RuntimeObjectSummary {
        thread_start: threads.first().copied(),
        thread_end: threads.last().copied(),
        thread_max: threads.iter().copied().max(),
        thread_growth: signed_growth(
            threads.first().copied().map(u64::from),
            threads.last().copied().map(u64::from),
        ),
        handle_start: handles.first().copied(),
        handle_end: handles.last().copied(),
        handle_max: handles.iter().copied().max(),
        handle_growth: signed_growth(
            handles.first().copied().map(u64::from),
            handles.last().copied().map(u64::from),
        ),
        process_restarts: observations
            .iter()
            .filter(|value| value.process_restarted)
            .count() as u64,
        sustained_growth_is_not_automatically_a_leak: true,
    }
}

fn provider_transition_count(observations: &[Observation], failure: bool) -> u64 {
    observations
        .windows(2)
        .map(|pair| {
            let before = provider_failure_set(&pair[0].database.providers);
            let after = provider_failure_set(&pair[1].database.providers);
            if failure {
                after.difference(&before).count() as u64
            } else {
                before.difference(&after).count() as u64
            }
        })
        .sum()
}

fn provider_failure_set(providers: &[ProviderObservation]) -> BTreeSet<String> {
    providers
        .iter()
        .filter(|value| value.persisted_status == "failed" || value.failed_metric_count > 0)
        .map(|value| value.provider.clone())
        .collect()
}

fn database_busy(
    database: &Path,
    duration: Duration,
    events_path: Option<&Path>,
) -> Result<(), String> {
    if duration.is_zero() || duration > MAX_BUSY_HOLD {
        return Err("database-busy duration must be between 1ms and 30s".to_string());
    }
    if let Some(path) = events_path {
        create_parent(path)?;
        append_event(path, "db_busy_start")?;
    }
    let conn = rusqlite::Connection::open_with_flags(
        database,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "open database for bounded busy validation failed".to_string())?;
    conn.busy_timeout(Duration::from_millis(250))
        .map_err(|_| "configure busy timeout failed".to_string())?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|_| "database did not accept controlled immediate lock".to_string())?;
    thread::sleep(duration);
    conn.execute_batch("ROLLBACK")
        .map_err(|_| "rollback controlled database lock failed".to_string())?;
    if let Some(path) = events_path {
        append_event(path, "db_busy_end")?;
    }
    println!(
        "bounded database lock held for {} ms; no data mutation issued",
        duration.as_millis()
    );
    Ok(())
}

fn schema(database: &Path) -> Result<(), String> {
    let report = db::schema_check(database)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|_| "serialize schema result failed".to_string())?
    );
    if !report.passed {
        return Err(
            "schema check did not pass user_version=8, quick_check=ok, foreign_key_check=0"
                .to_string(),
        );
    }
    Ok(())
}

fn mark(output: &Path, kind: &str) -> Result<(), String> {
    if !allowed_event_kind(kind) {
        return Err("unsupported marker kind".to_string());
    }
    create_parent(output)?;
    append_event(output, kind)
}

fn append_event(path: &Path, kind: &str) -> Result<(), String> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| "open event file failed".to_string())?;
    let mut writer = BufWriter::new(file);
    write_record(
        &mut writer,
        RawRecord::Event {
            value: OperatorEvent {
                event_kind: kind.to_string(),
                timestamp_utc: utc_iso(unix_now_ms()),
                timestamp_epoch_ms: unix_now_ms(),
            },
        },
    )?;
    writer
        .flush()
        .map_err(|_| "flush event file failed".to_string())
}

fn read_events(path: &Path) -> Result<Vec<OperatorEvent>, String> {
    let file = File::open(path).map_err(|_| "open event file failed".to_string())?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|_| "read event file failed".to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        if let RawRecord::Event { value } = serde_json::from_str::<RawRecord>(&line)
            .map_err(|_| "event file contains invalid JSON".to_string())?
        {
            events.push(value);
        }
    }
    Ok(events)
}

fn write_record(writer: &mut impl Write, record: RawRecord) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, &record)
        .map_err(|_| "serialize evidence failed".to_string())?;
    writer
        .write_all(b"\n")
        .map_err(|_| "write evidence failed".to_string())
}

fn parse_command(args: Vec<String>) -> Result<Command, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(help());
    };
    let options = parse_options(&args[1..])?;
    match command {
        "run" => Ok(Command::Run(RunConfig {
            pid: required_u32(&options, "pid")?,
            database: required_path(&options, "db")?,
            output: required_path(&options, "output")?,
            events: optional_path(&options, "events"),
            duration: required_duration(&options, "duration")?,
            cadence: optional_duration(&options, "cadence")?.unwrap_or(Duration::from_secs(30)),
            warmup: optional_duration(&options, "warmup")?.unwrap_or_default(),
            app_version: options.get("app-version").cloned(),
            git_commit: options.get("git-commit").cloned(),
            build_type: options
                .get("build-type")
                .cloned()
                .unwrap_or_else(|| "release".to_string()),
            executable_name: options
                .get("executable-name")
                .map(|value| file_name(Path::new(value)))
                .unwrap_or_else(|| "unknown.exe".to_string()),
        })),
        "summary" => Ok(Command::Summary {
            input: required_path(&options, "input")?,
            events: optional_path(&options, "events"),
            output: optional_path(&options, "output"),
        }),
        "mark" => Ok(Command::Mark {
            output: required_path(&options, "output")?,
            kind: options
                .get("kind")
                .cloned()
                .ok_or_else(|| "missing --kind".to_string())?,
        }),
        "database-busy" => Ok(Command::DatabaseBusy {
            database: required_path(&options, "db")?,
            duration: required_duration(&options, "duration")?,
            events: optional_path(&options, "events"),
        }),
        "schema" => Ok(Command::Schema {
            database: required_path(&options, "db")?,
        }),
        "help" | "--help" | "-h" => Err(help()),
        _ => Err(format!("unknown command '{command}'\n\n{}", help())),
    }
}

fn parse_options(args: &[String]) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut options = std::collections::BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let key = args[index]
            .strip_prefix("--")
            .ok_or_else(|| "options must use --name value syntax".to_string())?;
        if key.is_empty() || index + 1 >= args.len() || args[index + 1].starts_with("--") {
            return Err(format!("missing value for --{key}"));
        }
        if options
            .insert(key.to_string(), args[index + 1].clone())
            .is_some()
        {
            return Err(format!("duplicate option --{key}"));
        }
        index += 2;
    }
    Ok(options)
}

fn required_path(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<PathBuf, String> {
    options
        .get(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing --{key}"))
}

fn optional_path(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Option<PathBuf> {
    options.get(key).map(PathBuf::from)
}

fn required_u32(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<u32, String> {
    options
        .get(key)
        .ok_or_else(|| format!("missing --{key}"))?
        .parse()
        .map_err(|_| format!("invalid --{key}"))
}

fn required_duration(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<Duration, String> {
    options
        .get(key)
        .ok_or_else(|| format!("missing --{key}"))
        .and_then(|value| parse_duration(value))
}

fn optional_duration(
    options: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<Option<Duration>, String> {
    options
        .get(key)
        .map(|value| parse_duration(value).map(Some))
        .unwrap_or(Ok(None))
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    let split = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let number: u64 = value[..split]
        .parse()
        .map_err(|_| "duration must start with an integer".to_string())?;
    let unit = &value[split..];
    let millis = match unit {
        "ms" => number,
        "s" => number.saturating_mul(1_000),
        "m" => number.saturating_mul(60_000),
        "h" => number.saturating_mul(3_600_000),
        _ => return Err("duration unit must be ms, s, m, or h".to_string()),
    };
    Ok(Duration::from_millis(millis))
}

fn expected_observation_count(actual_elapsed_ms: u64, cadence_ms: u64) -> u64 {
    if cadence_ms == 0 {
        return 0;
    }
    actual_elapsed_ms
        .saturating_add(cadence_ms.saturating_sub(1))
        .checked_div(cadence_ms)
        .unwrap_or(0)
        .max(1)
}

fn allowed_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        "dynamic_disable_gpu"
            | "dynamic_enable_gpu"
            | "dynamic_disable_baseline"
            | "dynamic_enable_baseline"
            | "sleep_start"
            | "sleep_end"
            | "db_busy_start"
            | "db_busy_end"
            | "gui_smoke_start"
            | "gui_smoke_end"
            | "reopen_start"
            | "reopen_end"
    )
}

fn create_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|value| !value.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|_| "create output directory failed".to_string())?;
    }
    Ok(())
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(sanitize_executable_name)
        .unwrap_or_else(|| "unknown".to_string())
}

fn sanitize_executable_name(value: &str) -> String {
    let name = value.rsplit(['\\', '/']).next().unwrap_or(value);
    sanitize_token(name)
}

fn sanitize_token(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | ':')
        })
        .take(128)
        .collect();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

fn signed_growth(start: Option<u64>, end: Option<u64>) -> Option<i64> {
    Some((end? as i128 - start? as i128).clamp(i64::MIN as i128, i64::MAX as i128) as i64)
}

fn unix_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn utc_iso(timestamp_ms: i64) -> String {
    let seconds = timestamp_ms.div_euclid(1_000);
    let milliseconds = timestamp_ms.rem_euclid(1_000);
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, i64, i64) {
    let shifted = days_since_epoch + 719_468;
    let era = if shifted >= 0 {
        shifted / 146_097
    } else {
        (shifted - 146_096) / 146_097
    };
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

fn help() -> String {
    "Usage:\n  release-soak run --pid PID --db PATH --duration 10m --output RAW.jsonl [--events EVENTS.jsonl] [--cadence 30s] [--warmup 2m] [--executable-name resource-timeline.exe] [--app-version 0.3.2] [--git-commit SHA] [--build-type release]\n  release-soak summary --input RAW.jsonl [--events EVENTS.jsonl] [--output SUMMARY.json]\n  release-soak mark --output EVENTS.jsonl --kind dynamic_disable_gpu\n  release-soak database-busy --db PATH --duration 2s [--events EVENTS.jsonl]\n  release-soak schema --db PATH\n\nThe harness attaches to an already-running native process, never records command lines, and only reads SQLite during observation.\nDuration units: ms, s, m, h.".to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        civil_from_days, expected_observation_count, parse_duration, sanitize_executable_name,
        utc_iso,
    };
    use std::time::Duration;

    #[test]
    fn duration_parser_accepts_smoke_and_formal_values() {
        assert_eq!(parse_duration("10m").unwrap(), Duration::from_secs(600));
        assert_eq!(parse_duration("24h").unwrap(), Duration::from_secs(86_400));
        assert!(parse_duration("10").is_err());
    }

    #[test]
    fn utc_formatter_is_epoch_stable() {
        assert_eq!(utc_iso(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn executable_identity_drops_private_path() {
        assert_eq!(
            sanitize_executable_name(r"C:\Users\person\app.exe"),
            "app.exe"
        );
    }

    #[test]
    fn expected_observation_count_matches_sampling_loop() {
        assert_eq!(expected_observation_count(600_000, 30_000), 20);
        assert_eq!(expected_observation_count(60_000, 10_000), 6);
        assert_eq!(expected_observation_count(1, 30_000), 1);
    }
}
