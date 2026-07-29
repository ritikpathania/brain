use daemon_bridge::config;
use daemon_bridge::host::DaemonHost;

async fn query_http_endpoint(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let port = std::env::var("BRAIN_HEALTH_PORT").unwrap_or_else(|_| "8080".to_string());
    let url = format!("http://127.0.0.1:{}{}", port, path);
    let resp = reqwest::get(&url).await?.text().await?;
    Ok(resp)
}

async fn check_health() -> Result<(), Box<dyn std::error::Error>> {
    match query_http_endpoint("/health").await {
        Ok(body) => {
            if body.contains("ok") {
                println!("Daemon health status: OK");
            } else {
                println!("Daemon health status: UNHEALTHY ({})", body);
            }
        }
        Err(e) => {
            println!("Daemon health status: UNREACHABLE ({})", e);
            println!("\nHint: The background daemon is not running.");
            println!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
        }
    }
    Ok(())
}

async fn run_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Daemon Diagnostics ===");
    match query_http_endpoint("/metrics/runtime").await {
        Ok(body) => {
            println!("{}", body);
        }
        Err(e) => {
            println!("Diagnostics unavailable: {}", e);
            println!("\nHint: The background daemon is not running.");
            println!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
        }
    }
    Ok(())
}

fn print_config() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    println!("=== brain Configurations ===");
    println!("data_dir = \"{}\"", paths.config_dir.display());
    println!(
        "sqlite_runtime_db = \"{}\"",
        paths.config_dir.join("brain_runtime.db").display()
    );
    println!("socket_path = \"{}\"", paths.socket_path.display());
    println!("http_port = 8080");
    println!("uds_timeout_ms = 30000");
    Ok(())
}

fn is_executable(path: &std::path::Path) -> bool {
    if let Ok(metadata) = path.metadata() {
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            return metadata.permissions().mode() & 0o111 != 0;
        }
        #[cfg(not(unix))]
        {
            return true;
        }
    }
    false
}

fn find_brain_path() -> std::path::PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join("brain");
            if is_executable(&sibling) {
                return sibling;
            }
        }
    }
    let cargo_target = std::path::PathBuf::from("./target/debug/brain");
    if is_executable(&cargo_target) {
        cargo_target
    } else {
        std::path::PathBuf::from("brain")
    }
}

fn launch_embedded_tui() -> Result<(), Box<dyn std::error::Error>> {
    let brain_path = find_brain_path();
    let mut child = std::process::Command::new(brain_path).arg("ui").spawn()?;
    let status = child.wait()?;
    std::process::exit(status.code().unwrap_or(0));
}

fn find_cli_adapter_path() -> std::path::PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join("brain-cli-adapter");
            if sibling.exists() {
                return sibling;
            }
        }
    }
    std::path::PathBuf::from("brain-cli-adapter")
}

fn print_daemon_help() {
    println!("brain daemon management commands:");
    println!("  start       Start daemon process in the background");
    println!("  stop        Stop running background daemon process");
    println!("  status      Check if daemon process is currently running");
    println!("  run         Run daemon process in the foreground");
}

fn print_help() {
    println!("brain: AI coding companion engine");
    println!("Usage: brain-daemon <command> [args]");
    println!("\nCommands:");
    println!("  daemon      Manage background daemon process (start, stop, status, run)");
    println!("  ui          Launch interactive console TUI");
    println!("  health      Check if local daemon is running and healthy");
    println!("  diagnostics Retrieve runtime statistics and performance logs");
    println!("  config      Print path resolutions and active configuration");
    println!("  version     Print engine build version");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let cmd = args[1].as_str();
        match cmd {
            "daemon" => {
                if args.len() > 2 {
                    let sub = args[2].as_str();
                    match sub {
                        "start" => DaemonHost::start()?,
                        "stop" => DaemonHost::stop()?,
                        "status" => DaemonHost::status()?,
                        "run" => {
                            let paths = config::resolve_paths();
                            DaemonHost::run_server(paths).await?;
                        }
                        "help" | "--help" | "-h" => {
                            print_daemon_help();
                            std::process::exit(0);
                        }
                        _ => {
                            print_daemon_help();
                            std::process::exit(1);
                        }
                    }
                } else {
                    print_daemon_help();
                    std::process::exit(0);
                }
            }
            "adapter" => {
                let adapter_path = find_cli_adapter_path();
                let sub_args: Vec<String> = args.iter().skip(2).cloned().collect();
                let mut child = std::process::Command::new(adapter_path)
                    .args(sub_args)
                    .spawn()?;
                let status = child.wait()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            "version" => {
                println!("brain version 0.1.0");
            }
            "health" => {
                check_health().await?;
            }
            "diagnostics" => {
                run_diagnostics().await?;
            }
            "config" => {
                print_config()?;
            }
            "ui" => {
                launch_embedded_tui()?;
            }
            "help" | "--help" | "-h" => {
                print_help();
            }
            _ => {
                eprintln!("Unknown command: {}", cmd);
                print_help();
                std::process::exit(1);
            }
        }
    } else {
        launch_embedded_tui()?;
    }

    Ok(())
}
