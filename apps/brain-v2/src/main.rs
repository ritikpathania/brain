use brain_services::runtime::{ApplicationRuntime, RuntimeObserver};
use std::sync::Arc;
use async_trait::async_trait;
use brain_tui::client::{ExecutionClient, ExecutionRequest, EventReceiver, SessionSummary};
use brain_domain::Message;
use brain_core::errors::BrainError;

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

struct EmbeddedClient;

#[async_trait]
impl ExecutionClient for EmbeddedClient {
    async fn execute(&self, req: ExecutionRequest) -> Result<EventReceiver, BrainError> {
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok(EventReceiver::new(rx, req.cancellation_token))
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError> {
        Ok(vec![])
    }

    async fn load_session(&self, _id: brain_domain::SessionId) -> Result<Vec<Message>, BrainError> {
        Ok(vec![])
    }

    async fn delete_session(&self, _id: brain_domain::SessionId) -> Result<(), BrainError> {
        Ok(())
    }

    async fn approve_tool_call(&self, _call_id: brain_core::events::ToolCallId, _approved: bool) -> Result<(), BrainError> {
        Ok(())
    }

    async fn search_messages(&self, _query: &str) -> Result<Vec<Message>, BrainError> {
        Ok(vec![])
    }

    async fn inspect_node(&self, id: brain_domain::NodeId) -> Result<brain_domain::query::inspector::InspectorModel, BrainError> {
        let entity = brain_domain::dtos::NodeDTO::new(
            id.to_string(),
            "Embedded Node".to_string(),
            "Technology".to_string(),
            serde_json::Value::Null,
        );
        Ok(brain_domain::query::inspector::InspectorModel {
            entity,
            metadata: std::collections::HashMap::new(),
            relationships: vec![],
            provenance: brain_domain::query::inspector::ProvenanceDTO {
                source: "Embedded".to_string(),
                location: "Local Process".to_string(),
                timestamp: 0,
                extra_info: std::collections::HashMap::new(),
            },
            retrieval_explanation: None,
            recent_activity: vec![],
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Check if running in daemon service mode
    let is_daemon = args.len() > 1 && args[1] == "daemon";

    if is_daemon {
        println!("Starting Brain v2 Engine composition root as background daemon...");

        // 1. Resolve configuration
        let defaults_src = brain_config::loader::DefaultsSource;
        let config = brain_config::loader::resolve(&[Box::new(defaults_src)])?;

        // 2. Build the application runtime
        let runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(config)
                .register_observer(Arc::new(LogObserver))
                .build()?,
        );

        // 3. Start the runtime
        let startup_report = runtime.start()?;
        println!(
            "Runtime successfully started in {}ms. Completed phases: {:?}",
            startup_report.duration().as_millis(),
            startup_report.completed_phases()
        );

        println!("Running... Press Ctrl+C to stop.");

        // 4. Wait for Ctrl+C signal
        tokio::signal::ctrl_c().await?;

        println!("Ctrl+C received. Initiating graceful shutdown...");

        // 5. Shutdown the runtime
        runtime.shutdown()?;
        println!("Shutdown sequence completed successfully.");
    } else {
        // Start interactive Ratatui TUI mode by default
        let client = Box::new(brain_tui::client::UdsClient::default());
        if let Err(e) = brain_tui::run(client).await {
            eprintln!("TUI Error: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
