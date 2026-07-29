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
    version = "0.1.0",
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

fn socket_is_alive() -> bool {
    let socket_path = dirs::home_dir()
        .map(|h| h.join(".brain").join("daemon.sock"))
        .unwrap_or_else(|| PathBuf::from("daemon.sock"));
    UnixStream::connect(socket_path).is_ok()
}

fn try_start_daemon() -> bool {
    println!("Starting background daemon...");
    let result = std::process::Command::new("brain-daemon")
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

async fn check_daemon_before_ui() -> bool {
    if socket_is_alive() {
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

    match cli.command {
        Some(Commands::Ui) | Some(Commands::Tui) | None => {
            if check_daemon_before_ui().await {
                CLIHost::run_tui().await?;
            }
        }
        Some(Commands::Daemon { action }) => match action {
            Some(DaemonAction::Start) => {
                let status = std::process::Command::new("brain-daemon")
                    .args(["daemon", "start"])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            Some(DaemonAction::Stop) => {
                let status = std::process::Command::new("brain-daemon")
                    .args(["daemon", "stop"])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            Some(DaemonAction::Status) => {
                let status = std::process::Command::new("brain-daemon")
                    .args(["daemon", "status"])
                    .status()?;
                std::process::exit(status.code().unwrap_or(0));
            }
            Some(DaemonAction::Run) => {
                CLIHost::run_daemon().await?;
            }
            None => {
                println!("brain daemon subcommands: start, stop, status, run");
            }
        },
        Some(Commands::Query { text }) => {
            if !socket_is_alive() {
                println!("Daemon is not running.\n");
                println!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
                return Ok(());
            }
            if let Some(query_str) = text {
                let response = CLIHost::run_query(&query_str).await?;
                println!("{}", response);
            } else {
                println!("Usage: brain query <text>");
            }
        }
        Some(Commands::Ingest { content }) => {
            if !socket_is_alive() {
                println!("Daemon is not running.\n");
                println!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
                return Ok(());
            }
            if let Some(content_str) = content {
                let response = CLIHost::run_ingest(&content_str).await?;
                println!("{}", response);
            } else {
                println!("Usage: brain ingest <content>");
            }
        }
        Some(Commands::Health) => {
            if socket_is_alive() {
                println!("Daemon health status: OK");
            } else {
                println!("Daemon health status: UNREACHABLE");
                println!("\nHint: Background daemon is not running.");
                println!("Start it with:\n    brain-daemon daemon start\nor\n    make dev");
            }
        }
        Some(Commands::Config) => {
            println!("=== brain Configurations ===");
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
            println!("data_dir = \"{}\"", home.join(".brain").display());
            println!(
                "sqlite_runtime_db = \"{}\"",
                home.join(".brain").join("brain_runtime.db").display()
            );
            println!(
                "socket_path = \"{}\"",
                home.join(".brain").join("daemon.sock").display()
            );
            println!("http_port = 8080");
            println!("uds_timeout_ms = 30000");
        }
        Some(Commands::Version) => {
            println!("brain version 0.1.0");
        }
    }

    Ok(())
}
