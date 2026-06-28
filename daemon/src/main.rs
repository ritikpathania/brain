use pyo3::prelude::PyAnyMethods;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::sync::RwLock;
use tracing::{error, info};

use daemon_bridge::config::{self, BrainPaths};
use daemon_bridge::plugins::{self, BuiltinPythonExtractor};
use daemon_bridge::retrieval::{DefaultRanking, FuzzyRetrieval};
use daemon_bridge::server::{start_health_server, start_uds_listener};
use daemon_bridge::storage::duckdb::AnalyticsDatabase;
use daemon_bridge::storage::sqlite::LtmDatabase;
use daemon_bridge::workers::{start_analytics_worker, start_cleanup_worker};
use daemon_bridge::{DaemonMetrics, GlobalState};

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

    let pid = child.id();
    fs::write(&paths.pid_path, pid.to_string())?;
    println!("Daemon started successfully (PID: {}).", pid);
    Ok(())
}

fn daemonize_stop() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    if !paths.pid_path.exists() {
        println!("Daemon is not running.");
        return Ok(());
    }

    let pid_str = fs::read_to_string(&paths.pid_path)?;
    let pid = pid_str.trim().parse::<i32>()?;

    if is_pid_running(pid) {
        println!("Stopping daemon (PID: {})...", pid);
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }

        for _ in 0..50 {
            if !is_pid_running(pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if is_pid_running(pid) {
            println!("Daemon did not exit. Force killing...");
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        } else {
            println!("Daemon stopped.");
        }
    } else {
        println!("Daemon was not running, cleaning up stale PID file.");
    }

    let _ = fs::remove_file(&paths.pid_path);
    let _ = fs::remove_file(&paths.socket_path);
    Ok(())
}

fn daemonize_status() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    if !paths.pid_path.exists() {
        println!("Daemon Status: Stopped");
        return Ok(());
    }

    let pid_str = fs::read_to_string(&paths.pid_path)?;
    let pid = pid_str.trim().parse::<i32>()?;

    if is_pid_running(pid) {
        println!("Daemon Status: Running (PID: {})", pid);
    } else {
        println!("Daemon Status: Stopped (Stale PID file exists)");
    }
    Ok(())
}

async fn query_http_endpoint(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect("127.0.0.1:8080").await?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:8080\r\nConnection: close\r\n\r\n",
        path
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;

    if let Some(body_start) = response.find("\r\n\r\n") {
        Ok(response[body_start + 4..].to_string())
    } else {
        Ok(response)
    }
}

async fn check_health() -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking brain daemon health...");
    match query_http_endpoint("/health").await {
        Ok(health) => {
            println!("Daemon /health: {}", health.trim());
            match query_http_endpoint("/ready").await {
                Ok(ready) => {
                    println!("Daemon /ready: {}", ready.trim());
                }
                Err(e) => {
                    println!("Daemon /ready failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Daemon is unreachable: {}. Is it running?", e);
        }
    }
    Ok(())
}

async fn run_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    println!("=== brain Diagnostics ===");
    println!("OS: {} {}", std::env::consts::OS, std::env::consts::ARCH);
    println!("Config/Data Directory: {}", paths.config_dir.display());
    println!("SQLite DB Path: {}", paths.db_path.display());
    println!("DuckDB Path: {}", paths.analytics_db_path.display());
    println!("UDS Socket Path: {}", paths.socket_path.display());

    let python_version = pyo3::Python::with_gil(|py| -> pyo3::PyResult<String> {
        let sys = py.import_bound("sys")?;
        let version: String = sys.getattr("version")?.extract()?;
        Ok(version)
    });
    match python_version {
        Ok(ver) => println!("Python Version (embedded): {}", ver.replace('\n', " ")),
        Err(e) => println!("Python Check Failed: {}", e),
    }

    if paths.pid_path.exists() {
        if let Ok(pid_str) = fs::read_to_string(&paths.pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if is_pid_running(pid) {
                    println!("Daemon Status: Running (PID: {})", pid);

                    match query_http_endpoint("/metrics").await {
                        Ok(metrics) => {
                            println!("\nDaemon Telemetry Metrics:");
                            println!("{}", metrics.trim());
                        }
                        Err(e) => {
                            println!("Daemon Metrics Unreachable: {}", e);
                        }
                    }
                } else {
                    println!("Daemon Status: Stopped (Stale PID file)");
                }
            }
        }
    } else {
        println!("Daemon Status: Stopped");
    }

    Ok(())
}

fn load_cli_registry() -> Result<plugins::PluginRegistry, Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    let config_path = paths.config_dir.join("config.json");
    let plugin_config = if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            serde_json::from_str::<config::PluginConfig>(&content).unwrap_or_default()
        } else {
            config::PluginConfig::default()
        }
    } else {
        config::PluginConfig::default()
    };
    let mut registry = plugins::PluginRegistry::new(plugin_config);

    // Register built-in defaults
    registry
        .embedding_providers
        .insert("noop".to_string(), Arc::new(plugins::NoopEmbeddingProvider));
    registry
        .llm_providers
        .insert("noop".to_string(), Arc::new(plugins::NoopLlmProvider));
    registry
        .retrieval_algorithms
        .insert("fuzzy".to_string(), Arc::new(FuzzyRetrieval));
    registry
        .ranking_strategies
        .insert("default".to_string(), Arc::new(DefaultRanking));

    // Load python plugins
    let plugins_dir = paths.config_dir.join("plugins");
    let _ = plugins::load_python_plugins(&mut registry, &plugins_dir);
    Ok(registry)
}

fn print_config() -> Result<(), Box<dyn std::error::Error>> {
    let paths = config::resolve_paths();
    println!("=== brain Configurations ===");
    println!("data_dir = \"{}\"", paths.config_dir.display());
    println!("sqlite_db = \"{}\"", paths.db_path.display());
    println!("duckdb = \"{}\"", paths.analytics_db_path.display());
    println!("socket_path = \"{}\"", paths.socket_path.display());
    println!("http_port = 8080");
    println!("uds_timeout_ms = 30000");

    if let Ok(registry) = load_cli_registry() {
        println!("\nActive Plugins:");
        println!(
            "  embedding_provider  = \"{}\"",
            registry.config.active_embedding_provider
        );
        println!(
            "  llm_provider        = \"{}\"",
            registry.config.active_llm_provider
        );
        println!(
            "  retrieval_algorithm = \"{}\"",
            registry.config.active_retrieval_algorithm
        );
        println!(
            "  ranking_strategy    = \"{}\"",
            registry.config.active_ranking_strategy
        );
        println!(
            "  storage_backend     = \"{}\"",
            registry.config.active_storage_backend
        );
        println!(
            "  memory_extractor    = \"{}\"",
            registry.config.active_memory_extractor
        );
        println!(
            "  exporter            = \"{}\"",
            registry.config.active_exporter
        );
    }
    Ok(())
}

fn launch_embedded_tui() -> Result<(), Box<dyn std::error::Error>> {
    println!("Launching native Ratatui TUI client...");
    let paths = config::resolve_paths();

    let mut child = std::process::Command::new("brain-v2")
        .env("BRAIN_SOCKET_PATH", &paths.socket_path)
        .spawn()?;

    let _ = child.wait();
    Ok(())
}

fn print_help() {
    println!(
        r#"brain - Standalone Relational Memory Engine Developer Tool

Usage:
  brain [command]

Available Commands:
  daemon start    Start the memory engine daemon in the background
  daemon stop     Stop the background memory engine daemon
  daemon status   Check the current status of the daemon
  version         Print the version information
  health          Check health and readiness of the running daemon
  diagnostics     Output system diagnostics and runtime metrics
  config          Show data paths and configurations
  ui              Launch the interactive React/Ink terminal interface (default)
  help, --help    Show this help message

Default:
  Running 'brain' without arguments is equivalent to 'brain ui'.
"#
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let cmd = args[1].as_str();
        match cmd {
            "daemon" => {
                if args.len() > 2 {
                    match args[2].as_str() {
                        "start" => {
                            daemonize_start()?;
                        }
                        "stop" => {
                            daemonize_stop()?;
                        }
                        "status" => {
                            daemonize_status()?;
                        }
                        "run" => {
                            let paths = config::resolve_paths();
                            run_daemon_server(paths).await?;
                        }
                        _ => {
                            eprintln!("Unknown daemon subcommand. Use: start, stop, status, run");
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Missing daemon subcommand. Use: start, stop, status, run");
                    std::process::exit(1);
                }
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
                if let Ok(registry) = load_cli_registry() {
                    if let Some(plugin) = registry.cli_plugins.get(cmd) {
                        let sub_args: Vec<String> = args.iter().skip(2).cloned().collect();
                        if let Err(e) = plugin.handle_command(&sub_args) {
                            eprintln!("CLI Plugin '{}' failed: {}", cmd, e);
                            std::process::exit(1);
                        }
                        std::process::exit(0);
                    }
                }
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

    let analytics_db = Arc::new(
        AnalyticsDatabase::new(paths.analytics_db_path.to_str().unwrap())
            .expect("Failed to initialize DuckDB"),
    );
    let (analytics_tx, analytics_rx) =
        tokio::sync::mpsc::unbounded_channel::<daemon_bridge::storage::duckdb::AnalyticsEvent>();

    let worker_analytics_db = Arc::clone(&analytics_db);
    tokio::spawn(async move {
        start_analytics_worker(analytics_rx, worker_analytics_db).await;
    });

    let health_metrics = Arc::clone(&metrics);
    let health_analytics_db = Arc::clone(&analytics_db);
    tokio::spawn(async move {
        start_health_server(health_metrics, health_analytics_db).await;
    });

    let global_state: GlobalState = Arc::new(RwLock::new(HashMap::new()));
    let ltm_db = Arc::new(
        LtmDatabase::new(paths.db_path.to_str().unwrap())
            .expect("Failed to initialize LTM Database"),
    );
    info!(component = "database", db_path = %paths.db_path.display(), "LTM Persistent Graph Database initialized");

    // Read or create config
    let config_path = paths.config_dir.join("config.json");
    let plugin_config = if config_path.exists() {
        if let Ok(content) = fs::read_to_string(&config_path) {
            serde_json::from_str::<config::PluginConfig>(&content).unwrap_or_default()
        } else {
            config::PluginConfig::default()
        }
    } else {
        let default_config = config::PluginConfig::default();
        if let Ok(content) = serde_json::to_string_pretty(&default_config) {
            let _ = fs::write(&config_path, content);
        }
        default_config
    };

    let mut plugin_registry = plugins::PluginRegistry::new(plugin_config);

    // Register built-ins
    plugin_registry
        .embedding_providers
        .insert("noop".to_string(), Arc::new(plugins::NoopEmbeddingProvider));
    plugin_registry
        .llm_providers
        .insert("noop".to_string(), Arc::new(plugins::NoopLlmProvider));
    plugin_registry
        .retrieval_algorithms
        .insert("fuzzy".to_string(), Arc::new(FuzzyRetrieval));
    plugin_registry
        .ranking_strategies
        .insert("default".to_string(), Arc::new(DefaultRanking));
    plugin_registry
        .storage_backends
        .insert("sqlite".to_string(), ltm_db.clone());
    plugin_registry
        .exporters
        .insert("duckdb".to_string(), analytics_db.clone());

    let extractor_code = include_str!("../brain/extraction/heuristics.py");
    plugin_registry.memory_extractors.insert(
        "python-default".to_string(),
        Arc::new(BuiltinPythonExtractor::new(extractor_code.to_string())),
    );

    // Load Python dynamic plugins from ~/.brain/plugins/
    let plugins_dir = paths.config_dir.join("plugins");
    if let Err(e) = plugins::load_python_plugins(&mut plugin_registry, &plugins_dir) {
        error!(
            component = "plugins",
            "Failed to load Python dynamic plugins: {}", e
        );
    } else {
        info!(
            component = "plugins",
            "Python dynamic plugins loaded successfully"
        );
    }

    let plugin_registry = Arc::new(plugin_registry);

    if paths.socket_path.exists() {
        info!(
            component = "socket",
            "Clean up old socket at: {}",
            paths.socket_path.display()
        );
        let _ = fs::remove_file(&paths.socket_path);
    }

    let listener = UnixListener::bind(&paths.socket_path)?;
    info!(component = "socket", socket_path = %paths.socket_path.display(), "Socket bound successfully");

    let consolidation_state = Arc::clone(&global_state);
    let worker_metrics = Arc::clone(&metrics);
    let consolidation_registry = Arc::clone(&plugin_registry);

    tokio::spawn(async move {
        start_cleanup_worker(consolidation_state, worker_metrics, consolidation_registry).await;
    });

    start_uds_listener(
        listener,
        global_state,
        plugin_registry,
        metrics,
        analytics_tx,
    )
    .await;

    Ok(())
}
