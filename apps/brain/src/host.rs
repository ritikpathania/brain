use brain_services::runtime::{ApplicationRuntime, RuntimeObserver};
use brain_tui::client::{ExecutionClient, ExecutionOptions, ExecutionRequest, UdsClient};
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

struct LogObserver;
impl RuntimeObserver for LogObserver {
    fn on_started(&self, _runtime: &ApplicationRuntime) {
        println!("ApplicationRuntime observer: started.");
    }
    fn on_stopping(&self, _runtime: &ApplicationRuntime) {
        println!("ApplicationRuntime observer: stopping.");
    }
    fn on_stopped(&self, _runtime: &ApplicationRuntime) {
        println!("ApplicationRuntime observer: stopped.");
    }
}

pub struct CLIHost;

impl CLIHost {
    pub async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting Brain v2 Engine composition root as background daemon...");

        let defaults_src = brain_config::loader::DefaultsSource;
        let config = brain_config::loader::resolve(&[Box::new(defaults_src)])?;

        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(config)
                .with_working_dir(working_dir)
                .register_observer(Arc::new(LogObserver))
                .build()?,
        );

        let startup_report = runtime.start()?;
        println!(
            "Runtime successfully started in {}ms. Completed phases: {:?}",
            startup_report.duration().as_millis(),
            startup_report.completed_phases()
        );

        println!("Running... Press Ctrl+C to stop.");

        tokio::signal::ctrl_c().await?;

        println!("Ctrl+C received. Initiating graceful shutdown...");

        runtime.shutdown()?;
        println!("Shutdown sequence completed successfully.");

        Ok(())
    }

    pub async fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
        let client = Box::new(UdsClient::default());
        if let Err(e) = brain_tui::run(client).await {
            eprintln!("TUI Error: {}", e);
            std::process::exit(1);
        }
        Ok(())
    }

    pub async fn run_query(text: &str) -> Result<String, Box<dyn std::error::Error>> {
        let client = UdsClient::default();
        let req = ExecutionRequest {
            session_id: brain_domain::SessionId::new(),
            prompt: text.to_string(),
            options: ExecutionOptions::default(),
            cancellation_token: CancellationToken::new(),
            workspace_context: None,
        };
        let mut rx = client.execute(req).await?;
        let mut output = String::new();
        while let Some(res) = rx.recv().await {
            match res {
                Ok(ev) => match ev.kind {
                    brain_core::events::StreamEventKind::Token(text) => {
                        output.push_str(&text);
                    }
                    brain_core::events::StreamEventKind::Finished { response } => {
                        if output.is_empty() {
                            output = response;
                        }
                        break;
                    }
                    brain_core::events::StreamEventKind::Error { message } => {
                        return Err(Box::new(std::io::Error::other(message)));
                    }
                    _ => {}
                },
                Err(e) => return Err(Box::new(e)),
            }
        }
        Ok(output)
    }

    pub async fn run_ingest(content: &str) -> Result<String, Box<dyn std::error::Error>> {
        let socket_path = if let Ok(path) = std::env::var("BRAIN_SOCKET_PATH") {
            PathBuf::from(path)
        } else {
            dirs::home_dir()
                .map(|h| h.join(".brain").join("daemon.sock"))
                .unwrap_or_else(|| PathBuf::from("daemon.sock"))
        };
        let mut stream = tokio::net::UnixStream::connect(socket_path).await?;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let payload = serde_json::json!({
            "action": "ingest",
            "payload": content,
        })
        .to_string()
            + "\n";
        stream.write_all(payload.as_bytes()).await?;
        stream.flush().await?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        let trimmed = line.trim();

        // Parse response JSON for clean CLI output
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.get("status").and_then(|s| s.as_str()) == Some("ok") {
                if let Some(msg_str) = value.get("message").and_then(|m| m.as_str()) {
                    if let Ok(inner) = serde_json::from_str::<serde_json::Value>(msg_str) {
                        let event_id = inner
                            .get("event_id")
                            .and_then(|id| id.as_str())
                            .unwrap_or("unknown");
                        return Ok(format!(
                            "✔ Ingested memory successfully (Event ID: {})",
                            event_id
                        ));
                    }
                }
                return Ok("✔ Ingested memory successfully".to_string());
            } else if let Some(msg) = value.get("message").and_then(|m| m.as_str()) {
                return Ok(format!("✖ Ingest failed: {}", msg));
            }
        }

        Ok(trimmed.to_string())
    }
}
