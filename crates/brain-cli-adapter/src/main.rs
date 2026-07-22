use clap::{Parser, Subcommand};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use brain_integrations::IngestionEvent;
use brain_sdk_rs::{BrainClient, ClientConfig, RuntimeState};

#[derive(Parser)]
#[command(
    name = "brain-cli-adapter",
    about = "Brain CLI Integration Adapter Reference Client",
    version = "0.1.0"
)]
struct Cli {
    /// Override default UDS socket path (~/.brain/daemon.sock)
    #[arg(short, long, global = true)]
    socket_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a canonical ingestion event to the daemon
    Send {
        #[command(subcommand)]
        event: SendCommands,
    },
    /// Inspect local and remote replay logs
    Replay {
        /// Replay events after this sequence number
        #[arg(short, long, default_value_t = 0)]
        after: u64,
    },
    /// Verify connectivity to the daemon
    Ping,
    /// Display system and event versioning info
    Version,
    /// Get daemon status DTO
    Status,
    /// Get daemon metrics DTO
    Metrics,
    /// Get daemon diagnostics DTO
    Diagnostics,
    /// Get daemon capabilities DTO
    Capabilities,
    /// Execute a search query on the daemon
    Search {
        /// The query text
        query: String,
    },
    /// Trigger manual reflection consolidation cycle or inspect reflection subsystem state
    Reflect {
        #[command(subcommand)]
        subcommand: Option<ReflectCommands>,
    },
}

#[derive(Subcommand)]
enum ReflectCommands {
    /// Trigger manual reflection consolidation cycle and print report summary
    Run,
    /// Display background scheduler status, interval, trigger thresholds, and telemetry counters
    Status,
    /// Display the structured report of the most recent reflection run
    Report,
    /// Browse active unresolved reflection findings (duplicates, contradictions, link suggestions)
    Findings,
}

#[derive(Subcommand)]
enum SendCommands {
    /// Send a message conversation turn
    Message {
        /// Sender role (e.g. user, assistant, system)
        #[arg(long, default_value = "user")]
        role: String,
        /// Plaintext message content
        #[arg(long)]
        text: String,
    },
    /// Send unstructured text content
    Text {
        /// Read content from standard input
        #[arg(long)]
        stdin: bool,
        /// Direct text content (ignored if --stdin is set)
        #[arg(long)]
        content: Option<String>,
    },
    /// Send a file edit workspace event
    FileEdit {
        /// Workspace path of the file
        #[arg(long)]
        path: String,
        /// Unified diff or content representation of edits
        #[arg(long)]
        diff: Option<String>,
    },
    /// Send an assistant-initiated tool call
    ToolCall {
        /// Name of the called tool
        #[arg(long)]
        tool_name: String,
        /// Call identifier correlating to the request
        #[arg(long)]
        call_id: String,
        /// JSON string representation of tool arguments
        #[arg(long)]
        arguments: Option<String>,
    },
    /// Send a tool execution result
    ToolResult {
        /// Call identifier matching the ToolCall
        #[arg(long)]
        call_id: String,
        /// Flag set to true if execution failed
        #[arg(long)]
        is_error: bool,
        /// Text representation of stdout/results
        #[arg(long)]
        output: String,
    },
    /// Send a terminal command execution trace
    TerminalCommand {
        /// Full command line executed
        #[arg(long)]
        command: String,
        /// Exit code returned by the shell
        #[arg(long)]
        exit_code: Option<i32>,
        /// Summary of command output
        #[arg(long)]
        stdout_summary: Option<String>,
    },
    /// Send a git commit snapshot
    GitCommit {
        /// Commit message
        #[arg(long)]
        message: String,
        /// Commit hash
        #[arg(long)]
        hash: String,
        /// Active branch name
        #[arg(long)]
        branch: Option<String>,
        /// Comma-separated list of modified files
        #[arg(long, value_delimiter = ',')]
        files: Option<Vec<String>>,
    },
    /// Send compilation/lint diagnostics
    Diagnostic {
        /// Plaintext message details
        #[arg(long)]
        message: String,
        /// Diagnostic severity (e.g. error, warning, info)
        #[arg(long, default_value = "error")]
        severity: String,
        /// Source identifier (e.g. rustc, eslint)
        #[arg(long, default_value = "rust-cli")]
        source: String,
        /// File path where diagnostic occurred
        #[arg(long)]
        file: Option<String>,
        /// Line number
        #[arg(long)]
        line: Option<u32>,
    },
}

fn default_socket_path() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".brain").join("daemon.sock")
    } else {
        PathBuf::from("/tmp/brain-daemon.sock")
    }
}

async fn wait_for_ready(client: &BrainClient, timeout: Duration) -> Result<(), &'static str> {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match client.state() {
            RuntimeState::Ready => return Ok(()),
            RuntimeState::Disconnected if start.elapsed() > Duration::from_millis(500) => {
                return Err("Failed to connect to daemon (connection refused or socket missing)");
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    Err("Connection timeout")
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let socket = cli.socket_path.unwrap_or_else(default_socket_path);
    let mut config = ClientConfig::default_for_socket(socket);
    config.flush_interval = Duration::from_millis(5); // Flush quickly for CLI response

    match cli.command {
        Commands::Ping => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            match wait_for_ready(&client, Duration::from_secs(2)).await {
                Ok(()) => {
                    println!("Ping successful: connected to daemon.");
                    client.shutdown().await;
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Ping failed: {}", e);
                    client.shutdown().await;
                    std::process::exit(1);
                }
            }
        }
        Commands::Version => {
            println!("SDK version: 0.1.0");
            println!("Event Model version: 1.0");
            println!("Supported serializations: json");
        }
        Commands::Status => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = wait_for_ready(&client, Duration::from_secs(2)).await {
                eprintln!("Error connecting to daemon: {}", e);
                client.shutdown().await;
                std::process::exit(1);
            }
            match client.status().await {
                Ok(dto) => println!("{}", serde_json::to_string_pretty(&dto).unwrap()),
                Err(e) => eprintln!("Error: {:?}", e),
            }
            client.shutdown().await;
        }
        Commands::Metrics => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = wait_for_ready(&client, Duration::from_secs(2)).await {
                eprintln!("Error connecting to daemon: {}", e);
                client.shutdown().await;
                std::process::exit(1);
            }
            match client.metrics().await {
                Ok(dto) => println!("{}", serde_json::to_string_pretty(&dto).unwrap()),
                Err(e) => eprintln!("Error: {:?}", e),
            }
            client.shutdown().await;
        }
        Commands::Diagnostics => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = wait_for_ready(&client, Duration::from_secs(2)).await {
                eprintln!("Error connecting to daemon: {}", e);
                client.shutdown().await;
                std::process::exit(1);
            }
            match client.diagnostics().await {
                Ok(dto) => println!("{}", serde_json::to_string_pretty(&dto).unwrap()),
                Err(e) => eprintln!("Error: {:?}", e),
            }
            client.shutdown().await;
        }
        Commands::Capabilities => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = wait_for_ready(&client, Duration::from_secs(2)).await {
                eprintln!("Error connecting to daemon: {}", e);
                client.shutdown().await;
                std::process::exit(1);
            }
            match client.capabilities().await {
                Ok(dto) => println!("{}", serde_json::to_string_pretty(&dto).unwrap()),
                Err(e) => eprintln!("Error: {:?}", e),
            }
            client.shutdown().await;
        }
        Commands::Search { query } => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = wait_for_ready(&client, Duration::from_secs(2)).await {
                eprintln!("Error connecting to daemon: {}", e);
                client.shutdown().await;
                std::process::exit(1);
            }
            let search_query = brain_integrations::dto::v1::SearchQuery {
                text: query,
                kinds: None,
                pagination: None,
            };
            match client.search(search_query).await {
                Ok(dto) => println!("{}", serde_json::to_string_pretty(&dto).unwrap()),
                Err(e) => eprintln!("Error: {:?}", e),
            }
            client.shutdown().await;
        }
        Commands::Reflect { subcommand } => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = wait_for_ready(&client, Duration::from_secs(2)).await {
                eprintln!("Error connecting to daemon: {}", e);
                client.shutdown().await;
                std::process::exit(1);
            }
            match subcommand.unwrap_or(ReflectCommands::Run) {
                ReflectCommands::Run => match client.reflect().await {
                    Ok(report) => {
                        println!("=== Reflection Consolidation Report ===");
                        println!("Execution ID:       {}", report.execution_id);
                        println!("Duration:           {} ms", report.duration_ms);
                        println!("Findings Processed: {}", report.findings_processed);
                        println!("Commands Executed:  {}", report.commands_executed);
                        println!("Skipped Findings:   {}", report.skipped_findings.len());
                        if !report.executed_commands.is_empty() {
                            println!("\n[Executed Commands]");
                            for cmd in &report.executed_commands {
                                println!("  - {}", cmd);
                            }
                        }
                    }
                    Err(e) => eprintln!("Error: {:?}", e),
                },
                ReflectCommands::Status => match client.reflect_status().await {
                    Ok(status) => {
                        println!("=== Background Reflection Scheduler Status ===");
                        println!("Background Enabled:   {}", status.background_enabled);
                        println!("Interval:             {} s", status.interval_secs);
                        println!("Min Events Trigger:   {}", status.min_events_trigger);
                        println!("Max Nodes per Cycle:  {}", status.max_nodes_per_cycle);
                        println!("Cycle Time Budget:    {} ms", status.cycle_time_budget_ms);
                        println!("Total Cycles Run:     {}", status.reflections_executed);
                        println!("Total Findings:       {}", status.reflection_findings_count);
                        println!(
                            "Commands Executed:    {}",
                            status.reflection_commands_executed
                        );
                        println!(
                            "Findings Skipped:     {}",
                            status.reflection_commands_skipped
                        );
                        println!(
                            "Last Duration:        {} ms",
                            status.last_reflection_duration_ms.unwrap_or(0)
                        );
                    }
                    Err(e) => eprintln!("Error retrieving reflection status: {:?}", e),
                },
                ReflectCommands::Report => match client.last_reflection_report().await {
                    Ok(Some(report)) => {
                        println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    }
                    Ok(None) => println!("No previous reflection report found."),
                    Err(e) => eprintln!("Error retrieving reflection report: {:?}", e),
                },
                ReflectCommands::Findings => match client.active_reflection_findings().await {
                    Ok(findings) => {
                        if findings.is_empty() {
                            println!("No active unresolved findings detected.");
                        } else {
                            println!("=== Active Reflection Findings ({}) ===", findings.len());
                            for (i, f) in findings.iter().enumerate() {
                                println!(
                                    "  [{}] Kind: {} | Confidence: {:.2} | Targets: {:?}",
                                    i + 1,
                                    f.kind,
                                    f.confidence,
                                    f.target_ids
                                );
                                println!("      Details: {}", f.details);
                            }
                        }
                    }
                    Err(e) => eprintln!("Error retrieving active findings: {:?}", e),
                },
            }
            client.shutdown().await;
        }
        Commands::Replay { after } => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            match wait_for_ready(&client, Duration::from_secs(2)).await {
                Ok(()) => {
                    let last_seq = client.last_sequence();
                    let pending = client.get_unacknowledged_events().await;
                    let replay_res = client.request_replay(after).await;

                    println!("Last Sequence: {}", last_seq);
                    println!("Pending Events: {}", pending.len());
                    match replay_res {
                        Ok(events) => {
                            println!("Replay Result: {} events found", events.len());
                            for (i, ev) in events.iter().enumerate() {
                                println!(
                                    "  [{}] ID: {} | Kind: {:?}",
                                    i,
                                    ev.identity.event_id,
                                    ev.event.kind()
                                );
                            }
                        }
                        Err(e) => {
                            println!("Replay Result: Failed to retrieve ({:?})", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error connecting: {}", e);
                    client.shutdown().await;
                    std::process::exit(1);
                }
            }
            client.shutdown().await;
        }
        Commands::Send { event: send_cmd } => {
            let client = match BrainClient::connect(config).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect client: {:?}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = wait_for_ready(&client, Duration::from_secs(2)).await {
                eprintln!("Error connecting to daemon: {}", e);
                client.shutdown().await;
                std::process::exit(1);
            }

            let event = match send_cmd {
                SendCommands::Message { role, text } => IngestionEvent::Message {
                    role,
                    content: text,
                    metadata: BTreeMap::new(),
                },
                SendCommands::Text { stdin, content } => {
                    let text_content = if stdin {
                        std::io::read_to_string(std::io::stdin()).unwrap_or_default()
                    } else {
                        content.unwrap_or_default()
                    };
                    IngestionEvent::Text {
                        content: text_content,
                        metadata: BTreeMap::new(),
                    }
                }
                SendCommands::FileEdit { path, diff } => IngestionEvent::FileEdit {
                    path,
                    diff,
                    metadata: BTreeMap::new(),
                },
                SendCommands::ToolCall {
                    tool_name,
                    call_id,
                    arguments,
                } => {
                    let args_value = if let Some(arg_str) = arguments {
                        serde_json::from_str(&arg_str).unwrap_or(brain_integrations::Value::Null)
                    } else {
                        brain_integrations::Value::Null
                    };
                    IngestionEvent::ToolCall {
                        tool_name,
                        call_id,
                        arguments: args_value,
                        metadata: BTreeMap::new(),
                    }
                }
                SendCommands::ToolResult {
                    call_id,
                    is_error,
                    output,
                } => IngestionEvent::ToolResult {
                    call_id,
                    is_error,
                    output,
                    metadata: BTreeMap::new(),
                },
                SendCommands::TerminalCommand {
                    command,
                    exit_code,
                    stdout_summary,
                } => IngestionEvent::TerminalCommand {
                    command,
                    exit_code,
                    stdout_summary,
                    metadata: BTreeMap::new(),
                },
                SendCommands::GitCommit {
                    message,
                    hash,
                    branch,
                    files,
                } => IngestionEvent::GitCommit {
                    message,
                    hash,
                    branch,
                    files_changed: files.unwrap_or_default(),
                    metadata: BTreeMap::new(),
                },
                SendCommands::Diagnostic {
                    message,
                    severity,
                    source,
                    file,
                    line,
                } => IngestionEvent::Diagnostic {
                    message,
                    severity,
                    source,
                    file,
                    line,
                    metadata: BTreeMap::new(),
                },
            };

            match client.send(event).await {
                Ok(ack) => {
                    println!("Event ingested successfully.");
                    println!("Sequence: {}", ack.sequence);
                    println!("Event ID: {}", ack.event_id);
                }
                Err(e) => {
                    eprintln!("Failed to send event: {:?}", e);
                    client.shutdown().await;
                    std::process::exit(1);
                }
            }
            client.shutdown().await;
        }
    }
}
