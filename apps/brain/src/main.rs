mod frontend_artifact;
mod host;

use clap::{Parser, Subcommand};
use host::CLIHost;
use std::io::{BufRead, IsTerminal, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "brain",
    author,
    version = env!("CARGO_PKG_VERSION"),
    about = "brain: AI Coding Companion Relational Memory Engine CLI",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Optional path override to Unix Domain Socket
    #[arg(short, long)]
    socket_path: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch interactive console TUI (default)
    Ui,
    /// Launch interactive console TUI (alias for ui)
    Tui,
    /// Control local background daemon
    Daemon {
        #[command(subcommand)]
        action: Option<DaemonAction>,
    },
    /// Query relational memory graph
    Query {
        /// Text query string
        text: Option<String>,
    },
    /// Ingest sentence or observation
    Ingest {
        /// Content text to ingest
        content: Option<String>,
    },
    /// Check if local daemon is running and healthy
    Health,
    /// Print path resolutions and active configuration
    Config,
    /// Print version information
    Version,
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start daemon process in the background
    Start,
    /// Stop running background daemon process
    Stop,
    /// Check if daemon process is currently running
    Status,
    /// Run daemon process in the foreground
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionOrigin {
    CurrentExe,
    InstallBundle,
    Path,
}

pub fn resolve_daemon_executable() -> Result<(PathBuf, ResolutionOrigin), String> {
    static CACHED: std::sync::OnceLock<Result<(PathBuf, ResolutionOrigin), String>> =
        std::sync::OnceLock::new();

    CACHED
        .get_or_init(|| {
            // 1. Sibling of current_exe()
            if let Ok(exe_path) = std::env::current_exe().and_then(|path| path.canonicalize()) {
                if let Some(parent) = exe_path.parent() {
                    let sibling = parent.join("brain-daemon");
                    if sibling.exists() && sibling.is_file() {
                        return Ok((sibling, ResolutionOrigin::CurrentExe));
                    }
                }
            }

            // 2. Installed bundle directory. This deliberately uses the same
            // effective configuration root as the daemon rather than a
            // second, hard-coded ~/.brain interpretation.
            let user_bundle = brain_config::resolve_runtime_paths()
                .config_dir
                .join("bin")
                .join("brain-daemon");
            if user_bundle.exists() && user_bundle.is_file() {
                return Ok((user_bundle, ResolutionOrigin::InstallBundle));
            }
            let sys_bundle = PathBuf::from("/usr/local/bin/brain-daemon");
            if sys_bundle.exists() && sys_bundle.is_file() {
                return Ok((sys_bundle, ResolutionOrigin::InstallBundle));
            }

            // 3. System PATH check
            if let Ok(path_var) = std::env::var("PATH") {
                for dir in std::env::split_paths(&path_var) {
                    let candidate = dir.join("brain-daemon");
                    if candidate.exists() && candidate.is_file() {
                        return Ok((candidate, ResolutionOrigin::Path));
                    }
                }
            }

            Err("brain-daemon executable could not be found. Make sure brain-daemon is built and located alongside 'brain', in the effective Brain configuration directory's bin/, or in your PATH.".to_string())
        })
        .clone()
}

fn socket_is_alive() -> bool {
    UnixStream::connect(CLIHost::resolve_socket_path()).is_ok()
}

fn try_start_daemon() -> bool {
    if std::env::var("BRAIN_DEBUG").is_ok() {
        eprintln!("[daemon resolver] Starting background daemon...");
    }
    match resolve_daemon_executable() {
        Ok((bin_path, origin)) => {
            if std::env::var("BRAIN_DEBUG").is_ok() {
                eprintln!(
                    "[daemon resolver] Resolved daemon binary from {:?} ({})",
                    origin,
                    bin_path.display()
                );
            }
            let result = std::process::Command::new(bin_path)
                .arg("daemon")
                .arg("start")
                .status();
            if result.is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(800));
                socket_is_alive()
            } else {
                false
            }
        }
        Err(err) => {
            eprintln!("{}", err);
            false
        }
    }
}

async fn check_daemon_before_ui() -> bool {
    if socket_is_alive() {
        return true;
    }

    if try_start_daemon() {
        return true;
    }

    if std::io::stdin().is_terminal() {
        println!("Daemon is not running.\n");
        print!("Start it now? [Y/n] ");
        let _ = std::io::stdout().flush();

        let mut input = String::new();
        let stdin = std::io::stdin();
        if stdin.lock().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            if trimmed.is_empty() || trimmed == "y" || trimmed == "yes" {
                if try_start_daemon() {
                    return true;
                } else {
                    eprintln!("Failed to start daemon automatically.");
                    eprintln!("Please start it with: 'brain-daemon daemon start' or 'make dev'");
                    return false;
                }
            } else {
                println!("Aborted.");
                return false;
            }
        }
    } else {
        println!("Daemon is not running.\n");
        println!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
    }
    false
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if let Some(ref sp) = cli.socket_path {
        std::env::set_var("BRAIN_SOCKET_PATH", sp);
    }

    match cli.command {
        Some(Commands::Ui) | Some(Commands::Tui) | None => {
            let frontend = CLIHost::resolve_canonical_frontend()?;
            if !check_daemon_before_ui().await {
                return Err(std::io::Error::other(
                    "Brain daemon is unavailable; the canonical frontend was not launched.",
                )
                .into());
            }

            let status = CLIHost::run_tui(frontend)?;
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Some(Commands::Daemon { action }) => match action {
            Some(DaemonAction::Start) => {
                let (bin_path, origin) =
                    resolve_daemon_executable().map_err(std::io::Error::other)?;
                if std::env::var("BRAIN_DEBUG").is_ok() {
                    eprintln!(
                        "[daemon resolver] Resolved daemon binary from {:?} ({})",
                        origin,
                        bin_path.display()
                    );
                }
                let status = std::process::Command::new(bin_path)
                    .args(["daemon", "start"])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            Some(DaemonAction::Stop) => {
                let (bin_path, origin) =
                    resolve_daemon_executable().map_err(std::io::Error::other)?;
                if std::env::var("BRAIN_DEBUG").is_ok() {
                    eprintln!(
                        "[daemon resolver] Resolved daemon binary from {:?} ({})",
                        origin,
                        bin_path.display()
                    );
                }
                let status = std::process::Command::new(bin_path)
                    .args(["daemon", "stop"])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            Some(DaemonAction::Status) => {
                let (bin_path, origin) =
                    resolve_daemon_executable().map_err(std::io::Error::other)?;
                if std::env::var("BRAIN_DEBUG").is_ok() {
                    eprintln!(
                        "[daemon resolver] Resolved daemon binary from {:?} ({})",
                        origin,
                        bin_path.display()
                    );
                }
                let status = std::process::Command::new(bin_path)
                    .args(["daemon", "status"])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            Some(DaemonAction::Run) => {
                let (bin_path, origin) =
                    resolve_daemon_executable().map_err(std::io::Error::other)?;
                if std::env::var("BRAIN_DEBUG").is_ok() {
                    eprintln!(
                        "[daemon resolver] Resolved daemon binary from {:?} ({})",
                        origin,
                        bin_path.display()
                    );
                }
                let status = std::process::Command::new(bin_path)
                    .args(["daemon", "run"])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            None => {
                println!("brain daemon subcommands: start, stop, status, run");
                std::process::exit(2);
            }
        },
        Some(Commands::Query { text }) => {
            if !socket_is_alive() {
                eprintln!("Daemon is not running.\n");
                eprintln!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
                std::process::exit(1);
            }
            if let Some(query_str) = text {
                match CLIHost::run_query(&query_str).await {
                    Ok(response) => println!("{}", response),
                    Err(e) => {
                        eprintln!("Query failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Usage: brain query <text>");
                std::process::exit(2);
            }
        }
        Some(Commands::Ingest { content }) => {
            if !socket_is_alive() {
                eprintln!("Daemon is not running.\n");
                eprintln!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
                std::process::exit(1);
            }
            if let Some(content_str) = content {
                match CLIHost::run_ingest(&content_str).await {
                    Ok(response) => println!("{}", response),
                    Err(e) => {
                        eprintln!("Ingest failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Usage: brain ingest <content>");
                std::process::exit(2);
            }
        }
        Some(Commands::Health) => match CLIHost::run_status().await {
            Ok(status)
                if status.get("health").and_then(serde_json::Value::as_str) == Some("healthy") =>
            {
                println!("Daemon health status: OK");
            }
            Ok(status) => {
                eprintln!("Daemon health status: UNHEALTHY ({status})");
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("Daemon health status: UNREACHABLE ({error})");
                eprintln!("\nHint: Background daemon is not running.");
                eprintln!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
                std::process::exit(1);
            }
        },
        Some(Commands::Config) => {
            let paths = brain_config::resolve_runtime_paths();
            println!("=== brain Configurations ===");
            println!("data_dir = \"{}\"", paths.config_dir.display());
            println!("sqlite_runtime_db = \"{}\"", paths.database_path.display());
            println!("socket_path = \"{}\"", paths.socket_path.display());
            println!("pid_path = \"{}\"", paths.pid_path.display());
            println!("log_path = \"{}\"", paths.log_path.display());
            println!("http_port = {}", paths.health_port);
            println!("uds_timeout_ms = 30000");
        }
        Some(Commands::Version) => {
            println!("brain version {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
