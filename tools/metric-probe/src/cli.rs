use std::{env, path::PathBuf};

pub const DEFAULT_CORE_INTERVAL_MS: u64 = 2_000;
pub const DEFAULT_PROCESS_INTERVAL_MS: u64 = 5_000;
pub const DEFAULT_DURATION_SECONDS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Inventory,
    Run(RunConfig),
    Lifecycle(LifecycleConfig),
    Scenarios(ScenarioConfig),
    CpuSensors(CpuSensorConfig),
    CpuSensorLifecycle(CpuSensorLifecycleConfig),
    AmdUprof(AmdUprofConfig),
    AmdUprofWorkloadChild { workers: u32 },
    AmdUprofLoadChild { install_root: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunConfig {
    pub duration_seconds: u64,
    pub core_interval_ms: u64,
    pub process_interval_ms: u64,
    pub output_dir: PathBuf,
    pub process_probe: bool,
    pub disk_probe: bool,
    pub network_probe: bool,
    pub power_probe: bool,
    pub gpu_probe: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            duration_seconds: DEFAULT_DURATION_SECONDS,
            core_interval_ms: DEFAULT_CORE_INTERVAL_MS,
            process_interval_ms: DEFAULT_PROCESS_INTERVAL_MS,
            output_dir: PathBuf::from("artifacts/metric-probe/run"),
            process_probe: true,
            disk_probe: true,
            network_probe: true,
            power_probe: true,
            gpu_probe: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleConfig {
    pub output_dir: PathBuf,
    pub enabled_duration_ms: u64,
    pub disabled_duration_ms: u64,
}

impl Default for LifecycleConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("artifacts/metric-probe/spike-01b-lifecycle"),
            enabled_duration_ms: 1_000,
            disabled_duration_ms: 1_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioConfig {
    pub output_dir: PathBuf,
    pub sample_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSensorConfig {
    pub duration_seconds: u64,
    pub poll_interval_ms: u64,
    pub output_dir: PathBuf,
}

impl Default for CpuSensorConfig {
    fn default() -> Self {
        Self {
            duration_seconds: 60,
            poll_interval_ms: 1_000,
            output_dir: PathBuf::from("artifacts/metric-probe/cpu-sensors"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuSensorLifecycleConfig {
    pub enabled_duration_ms: u64,
    pub disabled_duration_ms: u64,
    pub output_dir: PathBuf,
}

impl Default for CpuSensorLifecycleConfig {
    fn default() -> Self {
        Self {
            enabled_duration_ms: 2_000,
            disabled_duration_ms: 2_000,
            output_dir: PathBuf::from("artifacts/metric-probe/cpu-sensor-lifecycle"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmdUprofMode {
    Sanity,
    Cadence,
    Lifecycle,
    Busy,
}

impl AmdUprofMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "sanity" => Ok(Self::Sanity),
            "cadence" => Ok(Self::Cadence),
            "lifecycle" => Ok(Self::Lifecycle),
            "busy" => Ok(Self::Busy),
            other => Err(format!(
                "invalid AMD uProf mode '{other}'; expected sanity, cadence, lifecycle, or busy"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmdUprofConfig {
    pub mode: AmdUprofMode,
    pub duration_seconds: u64,
    pub poll_interval_ms: u64,
    pub phase_duration_ms: u64,
    pub quiescence_ms: u64,
    pub output_dir: PathBuf,
    pub install_root: Option<PathBuf>,
    pub representative_load: bool,
    pub load_workers: u32,
}

impl Default for AmdUprofConfig {
    fn default() -> Self {
        Self {
            mode: AmdUprofMode::Sanity,
            duration_seconds: 30,
            poll_interval_ms: 1_000,
            phase_duration_ms: 5_000,
            quiescence_ms: 2_000,
            output_dir: PathBuf::from("artifacts/metric-probe/cpu-sensor-amd-uprof"),
            install_root: None,
            representative_load: false,
            load_workers: 4,
        }
    }
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("artifacts/metric-probe/spike-01b-scenarios"),
            sample_count: 2,
        }
    }
}

pub fn parse_args<I, S>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let command = args
        .next()
        .ok_or_else(|| "missing command; expected inventory or run".to_string())?;

    match command.as_str() {
        "inventory" => {
            let remaining: Vec<_> = args.collect();
            if remaining.is_empty() {
                Ok(Command::Inventory)
            } else {
                Err(format!(
                    "inventory does not accept arguments: {}",
                    remaining.join(" ")
                ))
            }
        }
        "run" => parse_run_args(args),
        "lifecycle" => parse_lifecycle_args(args),
        "scenarios" => parse_scenario_args(args),
        "cpu-sensors" => parse_cpu_sensor_args(args),
        "cpu-sensor-lifecycle" => parse_cpu_sensor_lifecycle_args(args),
        "cpu-sensor-amd-uprof" => parse_amd_uprof_args(args),
        "amd-uprof-workload-child" => parse_amd_uprof_workload_child_args(args),
        "amd-uprof-load-child" => parse_amd_uprof_load_child_args(args),
        "--help" | "-h" => Err(usage().to_string()),
        "--version" | "-V" => Err("metric-probe 0.1.0".to_string()),
        other => Err(format!(
            "invalid command '{other}'; expected inventory or run"
        )),
    }
}

fn parse_cpu_sensor_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config = CpuSensorConfig::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--duration-seconds" => {
                config.duration_seconds = parse_positive(&arg, args.next())?;
            }
            "--poll-interval-ms" => {
                config.poll_interval_ms = parse_positive(&arg, args.next())?;
            }
            "--output-dir" => config.output_dir = parse_path(&arg, args.next())?,
            "--help" | "-h" => return Err(usage().to_string()),
            other => return Err(format!("invalid cpu-sensors argument '{other}'")),
        }
    }
    Ok(Command::CpuSensors(config))
}

fn parse_cpu_sensor_lifecycle_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config = CpuSensorLifecycleConfig::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--enabled-duration-ms" => {
                config.enabled_duration_ms = parse_positive(&arg, args.next())?;
            }
            "--disabled-duration-ms" => {
                config.disabled_duration_ms = parse_positive(&arg, args.next())?;
            }
            "--output-dir" => config.output_dir = parse_path(&arg, args.next())?,
            "--help" | "-h" => return Err(usage().to_string()),
            other => return Err(format!("invalid cpu-sensor-lifecycle argument '{other}'")),
        }
    }
    Ok(Command::CpuSensorLifecycle(config))
}

fn parse_amd_uprof_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config = AmdUprofConfig::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {arg}"))?;
                config.mode = AmdUprofMode::parse(&value)?;
            }
            "--duration-seconds" => {
                config.duration_seconds = parse_positive(&arg, args.next())?;
            }
            "--poll-interval-ms" => {
                config.poll_interval_ms = parse_positive(&arg, args.next())?;
            }
            "--phase-duration-ms" => {
                config.phase_duration_ms = parse_positive(&arg, args.next())?;
            }
            "--quiescence-ms" => {
                config.quiescence_ms = parse_positive(&arg, args.next())?;
            }
            "--output-dir" => config.output_dir = parse_path(&arg, args.next())?,
            "--install-root" => config.install_root = Some(parse_path(&arg, args.next())?),
            "--load" | "--representative-load" => config.representative_load = true,
            "--load-workers" => {
                config.load_workers = parse_positive_u32(&arg, args.next())?;
            }
            "--help" | "-h" => return Err(usage().to_string()),
            other => return Err(format!("invalid cpu-sensor-amd-uprof argument '{other}'")),
        }
    }
    Ok(Command::AmdUprof(config))
}

fn parse_amd_uprof_workload_child_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut workers = 4;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--workers" => workers = parse_positive_u32(&arg, args.next())?,
            other => return Err(format!("invalid AMD uProf workload argument '{other}'")),
        }
    }
    Ok(Command::AmdUprofWorkloadChild { workers })
}

fn parse_amd_uprof_load_child_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut install_root = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--install-root" => install_root = Some(parse_path(&arg, args.next())?),
            other => return Err(format!("invalid AMD uProf load-check argument '{other}'")),
        }
    }
    install_root
        .map(|install_root| Command::AmdUprofLoadChild { install_root })
        .ok_or_else(|| "AMD uProf load-check requires --install-root".to_string())
}

fn parse_lifecycle_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config = LifecycleConfig::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-dir" => config.output_dir = parse_path(&arg, args.next())?,
            "--enabled-duration-ms" => {
                config.enabled_duration_ms = parse_positive(&arg, args.next())?;
            }
            "--disabled-duration-ms" => {
                config.disabled_duration_ms = parse_positive(&arg, args.next())?;
            }
            "--help" | "-h" => return Err(usage().to_string()),
            other => return Err(format!("invalid lifecycle argument '{other}'")),
        }
    }
    Ok(Command::Lifecycle(config))
}

fn parse_scenario_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config = ScenarioConfig::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-dir" => config.output_dir = parse_path(&arg, args.next())?,
            "--sample-count" => config.sample_count = parse_positive(&arg, args.next())?,
            "--help" | "-h" => return Err(usage().to_string()),
            other => return Err(format!("invalid scenarios argument '{other}'")),
        }
    }
    Ok(Command::Scenarios(config))
}

fn parse_path(flag: &str, value: Option<String>) -> Result<PathBuf, String> {
    let value = value.ok_or_else(|| format!("missing value for {flag}"))?;
    if value.trim().is_empty() {
        return Err(format!("value for {flag} must not be empty"));
    }
    Ok(PathBuf::from(value))
}

fn parse_run_args<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = String>,
{
    let mut config = RunConfig::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--duration-seconds" => {
                config.duration_seconds = parse_positive(&arg, args.next())?;
            }
            "--core-interval-ms" => {
                config.core_interval_ms = parse_positive(&arg, args.next())?;
            }
            "--process-interval-ms" => {
                config.process_interval_ms = parse_positive(&arg, args.next())?;
            }
            "--output-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("missing value for {arg}"))?;
                if value.trim().is_empty() {
                    return Err(format!("value for {arg} must not be empty"));
                }
                config.output_dir = PathBuf::from(value);
            }
            "--no-process-probe" => config.process_probe = false,
            "--no-disk-probe" => config.disk_probe = false,
            "--no-network-probe" => config.network_probe = false,
            "--no-power-probe" => config.power_probe = false,
            "--no-gpu-probe" => config.gpu_probe = false,
            "--help" | "-h" => return Err(usage().to_string()),
            other => return Err(format!("invalid run argument '{other}'")),
        }
    }

    Ok(Command::Run(config))
}

fn parse_positive(flag: &str, value: Option<String>) -> Result<u64, String> {
    let value = value.ok_or_else(|| format!("missing value for {flag}"))?;
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("value for {flag} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("value for {flag} must be greater than zero"));
    }
    Ok(parsed)
}

fn parse_positive_u32(flag: &str, value: Option<String>) -> Result<u32, String> {
    let parsed = parse_positive(flag, value)?;
    u32::try_from(parsed).map_err(|_| format!("value for {flag} is too large"))
}

pub fn usage() -> &'static str {
    "Usage:\n  metric-probe inventory\n  metric-probe run [options]\n  metric-probe lifecycle [options]\n  metric-probe scenarios [options]\n  metric-probe cpu-sensors [options]\n  metric-probe cpu-sensor-lifecycle [options]\n  metric-probe cpu-sensor-amd-uprof [options]\n\nRun options:\n  --duration-seconds <n>\n  --core-interval-ms <n>\n  --process-interval-ms <n>\n  --output-dir <path>\n  --no-process-probe\n  --no-disk-probe\n  --no-network-probe\n  --no-power-probe\n  --no-gpu-probe\n\nLifecycle options:\n  --enabled-duration-ms <n>\n  --disabled-duration-ms <n>\n  --output-dir <path>\n\nScenario options:\n  --sample-count <n>\n  --output-dir <path>\n\nCPU sensor options:\n  --duration-seconds <n>\n  --poll-interval-ms <n>\n  --output-dir <path>\n\nCPU sensor lifecycle options:\n  --enabled-duration-ms <n>\n  --disabled-duration-ms <n>\n  --output-dir <path>\n\nAMD uProf qualification options:\n  --mode sanity|cadence|lifecycle|busy\n  --duration-seconds <n>\n  --poll-interval-ms <n>\n  --phase-duration-ms <n>\n  --quiescence-ms <n>\n  --install-root <absolute-path>\n  --output-dir <path>\n  --load\n  --load-workers <n>"
}

pub fn args() -> Vec<String> {
    env::args().collect()
}

#[cfg(test)]
mod tests {
    use super::{
        parse_args, AmdUprofConfig, AmdUprofMode, Command, CpuSensorConfig,
        CpuSensorLifecycleConfig, LifecycleConfig, RunConfig, ScenarioConfig,
    };
    use std::path::PathBuf;

    #[test]
    fn parses_run_defaults() {
        assert_eq!(
            parse_args(["metric-probe", "run"]).unwrap(),
            Command::Run(RunConfig::default())
        );
    }

    #[test]
    fn parses_run_options_and_switches() {
        let command = parse_args([
            "metric-probe",
            "run",
            "--duration-seconds",
            "12",
            "--core-interval-ms",
            "1000",
            "--process-interval-ms",
            "3000",
            "--output-dir",
            "tmp/report",
            "--no-process-probe",
            "--no-disk-probe",
            "--no-network-probe",
            "--no-power-probe",
            "--no-gpu-probe",
        ])
        .unwrap();
        assert_eq!(
            command,
            Command::Run(RunConfig {
                duration_seconds: 12,
                core_interval_ms: 1000,
                process_interval_ms: 3000,
                output_dir: PathBuf::from("tmp/report"),
                process_probe: false,
                disk_probe: false,
                network_probe: false,
                power_probe: false,
                gpu_probe: false,
            })
        );
    }

    #[test]
    fn rejects_unknown_and_invalid_arguments() {
        assert!(parse_args(["metric-probe", "run", "--unknown"]).is_err());
        assert!(parse_args(["metric-probe", "run", "--duration-seconds", "0"]).is_err());
        assert!(parse_args(["metric-probe", "run", "--core-interval-ms"]).is_err());
        assert!(parse_args(["metric-probe", "inventory", "--output-dir", "x"]).is_err());
    }

    #[test]
    fn parses_lifecycle_and_scenario_commands() {
        assert_eq!(
            parse_args([
                "metric-probe",
                "lifecycle",
                "--enabled-duration-ms",
                "20",
                "--disabled-duration-ms",
                "30",
                "--output-dir",
                "tmp/lifecycle",
            ])
            .unwrap(),
            Command::Lifecycle(LifecycleConfig {
                output_dir: PathBuf::from("tmp/lifecycle"),
                enabled_duration_ms: 20,
                disabled_duration_ms: 30,
            })
        );
        assert_eq!(
            parse_args([
                "metric-probe",
                "scenarios",
                "--sample-count",
                "3",
                "--output-dir",
                "tmp/scenarios",
            ])
            .unwrap(),
            Command::Scenarios(ScenarioConfig {
                output_dir: PathBuf::from("tmp/scenarios"),
                sample_count: 3,
            })
        );
    }

    #[test]
    fn parses_cpu_sensor_commands() {
        assert_eq!(
            parse_args([
                "metric-probe",
                "cpu-sensors",
                "--duration-seconds",
                "300",
                "--poll-interval-ms",
                "2500",
                "--output-dir",
                "tmp/cpu",
            ])
            .unwrap(),
            Command::CpuSensors(CpuSensorConfig {
                duration_seconds: 300,
                poll_interval_ms: 2500,
                output_dir: PathBuf::from("tmp/cpu"),
            })
        );
        assert_eq!(
            parse_args([
                "metric-probe",
                "cpu-sensor-lifecycle",
                "--enabled-duration-ms",
                "1000",
                "--disabled-duration-ms",
                "2000",
                "--output-dir",
                "tmp/lifecycle",
            ])
            .unwrap(),
            Command::CpuSensorLifecycle(CpuSensorLifecycleConfig {
                enabled_duration_ms: 1000,
                disabled_duration_ms: 2000,
                output_dir: PathBuf::from("tmp/lifecycle"),
            })
        );
    }

    #[test]
    fn parses_amd_uprof_qualification_commands() {
        assert_eq!(
            parse_args([
                "metric-probe",
                "cpu-sensor-amd-uprof",
                "--mode",
                "cadence",
                "--duration-seconds",
                "30",
                "--poll-interval-ms",
                "500",
                "--phase-duration-ms",
                "1000",
                "--quiescence-ms",
                "2000",
                "--install-root",
                "D:/apps/AMDuProf",
                "--output-dir",
                "tmp/amd",
                "--load",
                "--load-workers",
                "2",
            ])
            .unwrap(),
            Command::AmdUprof(AmdUprofConfig {
                mode: AmdUprofMode::Cadence,
                duration_seconds: 30,
                poll_interval_ms: 500,
                phase_duration_ms: 1000,
                quiescence_ms: 2000,
                output_dir: PathBuf::from("tmp/amd"),
                install_root: Some(PathBuf::from("D:/apps/AMDuProf")),
                representative_load: true,
                load_workers: 2,
            })
        );
        assert_eq!(
            parse_args(["metric-probe", "amd-uprof-workload-child", "--workers", "3"]).unwrap(),
            Command::AmdUprofWorkloadChild { workers: 3 }
        );
    }

    #[test]
    fn rejects_unknown_amd_uprof_mode() {
        assert!(parse_args(["metric-probe", "cpu-sensor-amd-uprof", "--mode", "all"]).is_err());
    }
}
