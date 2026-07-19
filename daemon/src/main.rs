use std::fs;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info, warn};

use brain_services::BrainRuntime;

use daemon_bridge::config::{self, BrainPaths};
use daemon_bridge::server::{start_health_server, start_uds_listener};
use daemon_bridge::DaemonMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DaemonState {
    Starting,
    Running,
    Draining,
    Stopped,
}

struct DaemonCleanupGuard {
    pid_path: Option<std::path::PathBuf>,
    socket_path: Option<std::path::PathBuf>,
}

impl DaemonCleanupGuard {
    fn new(pid_path: std::path::PathBuf, socket_path: std::path::PathBuf) -> Self {
        Self {
            pid_path: Some(pid_path),
            socket_path: Some(socket_path),
        }
    }

    fn disarm(&mut self) {
        self.pid_path = None;
        self.socket_path = None;
    }
}

impl Drop for DaemonCleanupGuard {
    fn drop(&mut self) {
        if let Some(path) = self.pid_path.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Failed to remove stale PID file: {}", e);
                }
            }
        }
        if let Some(path) = self.socket_path.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Failed to remove stale UDS socket file: {}", e);
                }
            }
        }
    }
}

fn is_pid_running(pid: i32) -> bool {
    unsafe {
        let res = libc::kill(pid, 0);
        res == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

fn daemonize_start() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    if paths.pid_path.exists() {
        if let Ok(pid_str) = fs::read_to_string(&paths.pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if is_pid_running(pid) {
                    println!("Daemon is already running (PID: {}).", pid);
                    return Ok(());
                }
            }
        }
        let _ = fs::remove_file(&paths.pid_path);
    }

    let exe = std::env::current_exe()?;
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_path)?;

    println!("Starting brain daemon in background...");

    let child = std::process::Command::new(exe)
        .arg("daemon")
        .arg("run")
        .stdout(std::process::Stdio::from(log_file.try_clone()?))
        .stderr(std::process::Stdio::from(log_file))
        .spawn()?;

    let pid = child.id() as i32;
    fs::write(&paths.pid_path, pid.to_string())?;
    println!("Daemon started successfully (PID: {}).", pid);

    Ok(())
}

fn daemonize_stop() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    if !paths.pid_path.exists() {
        println!("Daemon is not running (no PID file found).");
        return Ok(());
    }

    let pid_str = fs::read_to_string(&paths.pid_path)?;
    let pid = pid_str.trim().parse::<i32>()?;

    println!("Stopping daemon (PID: {})...", pid);
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }

    // Wait for the process to stop
    for _ in 0..50 {
        if !is_pid_running(pid) {
            println!("Daemon stopped.");
            let _ = fs::remove_file(&paths.pid_path);
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    println!("Daemon did not stop gracefully. Forcing exit...");
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    let _ = fs::remove_file(&paths.pid_path);
    Ok(())
}

fn daemonize_status() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    if !paths.pid_path.exists() {
        println!("Status: Stopped");
        return Ok(());
    }

    let pid_str = fs::read_to_string(&paths.pid_path)?;
    let pid = pid_str.trim().parse::<i32>()?;

    if is_pid_running(pid) {
        println!("Status: Running (PID: {})", pid);
    } else {
        println!("Status: Stale PID file (Process not running)");
    }
    Ok(())
}

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

fn find_brain_path() -> std::path::PathBuf {
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let sibling = parent.join("brain");
            if sibling.exists() {
                return sibling;
            }
        }
    }
    let cargo_target = std::path::PathBuf::from("./target/debug/brain");
    if cargo_target.exists() {
        cargo_target
    } else {
        std::path::PathBuf::from("brain")
    }
}

fn launch_embedded_tui() -> Result<(), Box<dyn std::error::Error>> {
    let brain_path = find_brain_path();
    let mut child = std::process::Command::new(brain_path).arg("tui").spawn()?;
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
                        "start" => daemonize_start()?,
                        "stop" => daemonize_stop()?,
                        "status" => daemonize_status()?,
                        "run" => {
                            let paths = config::resolve_paths();
                            run_daemon_server(paths).await?;
                        }
                        _ => {
                            print_daemon_help();
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Missing daemon subcommand. Use: start, stop, status, run, help");
                    std::process::exit(1);
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

async fn run_daemon_server(paths: BrainPaths) -> Result<(), Box<dyn std::error::Error>> {
    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let use_json = std::env::var("LOG_FORMAT").as_deref() == Ok("json");

    daemon_bridge::telemetry::init_subscriber(&log_level, use_json);

    info!(component = "main", "Starting brain Daemon...");

    let metrics = Arc::new(DaemonMetrics::new());

    let health_metrics = Arc::clone(&metrics);
    // Initialize Cleanup Guard
    let mut cleanup_guard =
        DaemonCleanupGuard::new(paths.pid_path.clone(), paths.socket_path.clone());

    // Test-only: Simulates a startup panic to verify stack unwinding RAII cleanup.
    // Ignored entirely in release builds via #[cfg(debug_assertions)].
    #[cfg(debug_assertions)]
    if std::env::var("BRAIN_TEST_PANIC_STARTUP").is_ok() {
        panic!("Simulating panic during startup");
    }

    let brain_runtime_db_path = paths.config_dir.join("brain_runtime.db");

    let brain_runtime = match BrainRuntime::new(brain_runtime_db_path.to_str().unwrap()) {
        Ok(rt) => {
            info!(
                component = "runtime",
                db_path = %brain_runtime_db_path.display(),
                "BrainRuntime initialized"
            );
            Arc::new(rt)
        }
        Err(e) => {
            error!(
                component = "runtime",
                "Failed to initialize BrainRuntime: {}", e
            );
            return Err(e.into());
        }
    };

    let rt_metrics_ref = Arc::clone(&brain_runtime);
    tokio::spawn(async move {
        start_health_server(health_metrics, rt_metrics_ref).await;
    });

    let mut state = DaemonState::Starting;
    info!(component = "daemon", "Daemon state: {:?}", state);

    if paths.socket_path.exists() {
        match UnixStream::connect(&paths.socket_path).await {
            Ok(_) => {
                error!(
                    component = "socket",
                    "UDS socket at '{}' is active. Another daemon is running. Aborting startup.",
                    paths.socket_path.display()
                );
                return Err("Daemon already running".into());
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                error!(
                    component = "socket",
                    "Permission denied checking socket status at '{}': {}. Aborting startup.",
                    paths.socket_path.display(),
                    e
                );
                return Err(e.into());
            }
            Err(e) => {
                info!(
                    component = "socket",
                    "Stale or invalid file/socket detected at '{}' (Error: {}). Cleaning up...",
                    paths.socket_path.display(),
                    e
                );
                if let Err(err) = fs::remove_file(&paths.socket_path) {
                    if err.kind() != std::io::ErrorKind::NotFound {
                        warn!(
                            component = "socket",
                            "Failed to remove stale UDS file: {}", err
                        );
                    }
                }
            }
        }
    }

    let listener = UnixListener::bind(&paths.socket_path)?;
    info!(component = "socket", socket_path = %paths.socket_path.display(), "Socket bound successfully");

    if std::env::var("BRAIN_TEST_PANIC_BEFORE_SERVING").is_ok() {
        panic!("Simulating panic after socket creation but before serving");
    }

    state = DaemonState::Running;
    info!(component = "daemon", "Daemon state: {:?}", state);

    // Spawn signal listener for graceful shutdown
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let cancel_trigger = cancel_token.clone();

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::signal::ctrl_c() => "SIGINT",
            _ = sigterm.recv() => "SIGTERM",
        };
        info!(component = "shutdown", "Shutdown initiated ({})", reason);
        cancel_trigger.cancel();

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!(component = "shutdown", "Shutdown already in progress (SIGINT ignored)");
                }
                _ = sigterm.recv() => {
                    info!(component = "shutdown", "Shutdown already in progress (SIGTERM ignored)");
                }
            }
        }
    });

    let listener_metrics = metrics.clone();
    let listener_runtime = Arc::clone(&brain_runtime);
    tokio::select! {
        _ = start_uds_listener(
            listener,
            listener_metrics,
            listener_runtime,
        ) => {}
        _ = cancel_token.cancelled() => {
            state = DaemonState::Draining;
            info!(component = "daemon", "Daemon state: {:?}", state);
        }
    }

    info!(component = "shutdown", "Stopping accept loop");

    let start_drain = std::time::Instant::now();
    let mut active = metrics
        .active_workers
        .load(std::sync::atomic::Ordering::Relaxed);
    while active > 0 {
        info!(
            component = "shutdown",
            "Waiting for workers ({} active)", active
        );
        if start_drain.elapsed() >= std::time::Duration::from_secs(5) {
            warn!(
                component = "shutdown",
                "Draining timed out after 5s. Forcing exit."
            );
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        active = metrics
            .active_workers
            .load(std::sync::atomic::Ordering::Relaxed);
    }

    if active == 0 {
        info!(component = "shutdown", "Workers drained");
    }

    match Arc::try_unwrap(brain_runtime) {
        Ok(runtime) => match runtime.shutdown() {
            Ok(summary) => {
                info!(
                    component = "runtime",
                    shutdown_ms = summary.duration.as_millis(),
                    "BrainRuntime shutdown complete"
                );
            }
            Err(e) => {
                error!(component = "runtime", error = %e, "BrainRuntime shutdown error");
            }
        },
        Err(_) => {
            warn!(
                component = "runtime",
                "BrainRuntime Arc had outstanding references at shutdown"
            );
        }
    }

    state = DaemonState::Stopped;
    info!(component = "daemon", "Daemon state: {:?}", state);

    info!(component = "shutdown", "Removing socket");
    if let Some(path) = cleanup_guard.socket_path.take() {
        if let Err(e) = fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    component = "shutdown",
                    "Failed to remove UDS socket file: {}", e
                );
            }
        }
    }

    info!(component = "shutdown", "Removing PID");
    if let Some(path) = cleanup_guard.pid_path.take() {
        if let Err(e) = fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(component = "shutdown", "Failed to remove PID file: {}", e);
            }
        }
    }

    cleanup_guard.disarm();
    info!(component = "shutdown", "Shutdown complete");

    Ok(())
}
