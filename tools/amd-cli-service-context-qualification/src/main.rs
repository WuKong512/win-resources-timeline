#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod windows_service;

#[cfg(windows)]
fn main() {
    use amd_cli_service_context_probe::{validate_service_arguments, write_text};
    use std::path::PathBuf;

    let program_data = match std::env::var_os("ProgramData") {
        Some(value) => PathBuf::from(value),
        None => {
            eprintln!("ProgramData is unavailable");
            std::process::exit(2);
        }
    };
    let allowed_base = program_data
        .join("ResourceTimeline")
        .join("qualification")
        .join("amd-service-context");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let run_root = match validate_service_arguments(&args, &allowed_base) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("invalid service arguments: {error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = windows_service::run(run_root) {
        let _ = write_text(
            &allowed_base.join("service-harness-startup-error.txt"),
            &error,
        );
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("amd-cli-service-context-probe is Windows-service-only");
    std::process::exit(2);
}
