fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--synthetic-child") {
        std::thread::sleep(std::time::Duration::from_secs(30));
        return;
    }
    if args.first().is_some_and(|arg| arg == "--synthetic") {
        let evidence_root = args
            .windows(2)
            .find(|pair| pair[0] == "--evidence-root")
            .map(|pair| std::path::PathBuf::from(&pair[1]));
        match amd_privilege_qualification::synthetic::run(evidence_root.as_deref()) {
            Ok(summary) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("synthetic summary serializes")
                );
                if summary.result != "PASS" {
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("synthetic qualification failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    #[cfg(windows)]
    {
        match args.first().map(String::as_str) {
            Some("--broker") if args.len() == 1 => {
                if let Err(error) = amd_privilege_qualification::windows::run_service() {
                    eprintln!("broker service failed: {error}");
                    std::process::exit(1);
                }
            }
            Some("--client") => {
                let operation = args.get(1).map(String::as_str).unwrap_or("get-status");
                if !matches!(operation, "get-status" | "counter-discovery" | "start") {
                    eprintln!("client operation must be get-status, counter-discovery, or start");
                    std::process::exit(2);
                }
                let duration_ms = option_value(&args, "--duration-ms").unwrap_or(10_000);
                let interval_ms = option_value(&args, "--interval-ms").unwrap_or(1_000);
                let options = amd_privilege_qualification::windows::ClientOptions {
                    operation: operation.to_owned(),
                    duration_ms,
                    interval_ms,
                };
                if let Err(error) = amd_privilege_qualification::windows::run_client(options) {
                    eprintln!("qualification client failed: {error}");
                    std::process::exit(1);
                }
            }
            _ => usage_and_exit(),
        }
    }

    #[cfg(not(windows))]
    {
        let _ = args;
        eprintln!(
            "amd-privilege-qualification is Windows-only; --synthetic is supported on all hosts"
        );
        std::process::exit(2);
    }
}

#[cfg(windows)]
fn option_value(args: &[String], name: &str) -> Option<u32> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .and_then(|pair| pair[1].parse::<u32>().ok())
}

fn usage_and_exit() -> ! {
    eprintln!(
        "usage: amd-privilege-qualification --synthetic [--evidence-root PATH] | --broker | --client get-status|counter-discovery|start"
    );
    std::process::exit(2)
}
