use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use brain_services::ApplicationRuntime;
use brain_integrations::{IngestionEnvelope, IngestionEvent, EventIdentity};
use brain_services::query::SearchQuery;
use brain_application::{
    BrainApplication, ExecutionContext, ApplicationError, ApplicationEvent, ApplicationEventSink, ApplicationRequestId
};
use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{BrainSettings, PartialBrainSettings, PartialDatabaseSettings};

// ----------------------------------------------------------------------
// Test Event Sink implementation
// ----------------------------------------------------------------------
struct TestEventSink {
    events: Arc<Mutex<Vec<ApplicationEvent>>>,
}

impl ApplicationEventSink for TestEventSink {
    fn emit(&self, event: ApplicationEvent) {
        let mut list = self.events.lock().unwrap();
        list.push(event);
    }
}

// ----------------------------------------------------------------------
// Mock CLI Adapter implementation (Adapter Simplicity Rule)
// ----------------------------------------------------------------------
struct MockCliAdapter {
    app: Arc<BrainApplication>,
}

impl MockCliAdapter {
    async fn handle_command(&self, cmd: &str) -> String {
        let context = ExecutionContext::default();
        if cmd == "ingest" {
            let envelope = create_test_envelope();
            match self.app.ingest(envelope, &context).await {
                Ok(res) => format!("CLI_SUCCESS: processed={}", res.processed),
                Err(e) => format!("CLI_ERROR: {}", e),
            }
        } else {
            "CLI_UNKNOWN_COMMAND".to_string()
        }
    }
}

// ----------------------------------------------------------------------
// Mock UDS Adapter simulating SDK transport serialization/framing
// ----------------------------------------------------------------------
struct MockUdsAdapter {
    app: Arc<BrainApplication>,
}

impl MockUdsAdapter {
    async fn handle_uds_frame(&self, frame: &str) -> Result<String, String> {
        let decoded: IngestionEnvelope = serde_json::from_str(frame.trim())
            .map_err(|e| format!("Serialization error: {}", e))?;
        
        let context = ExecutionContext::default();
        
        let response = self.app.ingest(decoded, &context).await
            .map_err(|e| format!("Application error: {}", e))?;
        
        let encoded = serde_json::to_string(&response)
            .map_err(|e| format!("Serialization error: {}", e))? + "\n";
            
        Ok(encoded)
    }
}

// Helper to construct a valid test DTO
fn create_test_envelope() -> IngestionEnvelope {
    IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: EventIdentity {
            event_id: "a4a7541f-8239-44d4-95e2-b91c0683072c".parse().unwrap(),
            parent_event_id: None,
            workspace_id: brain_domain::WorkspaceId::new("proj-123"),
            client_id: brain_domain::ClientId::new("cursor-1.0"),
            adapter_id: brain_domain::AdapterId::new("vscode-ext"),
            session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
            conversation_id: None,
            timestamp: chrono::Utc::now(),
        },
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: "Ping".to_string(),
            metadata: std::collections::BTreeMap::new(),
        },
    }
}

fn get_temp_db_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    std::env::temp_dir()
        .join(format!("brain_test_app_{}.db", uuid_str))
        .to_string_lossy()
        .to_string()
}

fn get_temp_plugins_path() -> String {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    let path = std::env::temp_dir().join(format!("brain_test_app_plugins_{}", uuid_str));
    std::fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().to_string()
}

fn create_valid_test_config(db_path: &str, plugins_path: &str) -> BrainSettings {
    let defaults_src = DefaultsSource;
    let partial = PartialBrainSettings {
        database: Some(PartialDatabaseSettings {
            path: Some(db_path.to_string()),
            pool_size: Some(2),
            enable_wal: Some(false),
        }),
        plugins_directory: Some(plugins_path.to_string()),
        ..Default::default()
    };
    let override_src = OverrideSource::new(partial);
    resolve(&[Box::new(defaults_src), Box::new(override_src)]).unwrap()
}

fn setup_app() -> Arc<BrainApplication> {
    pyo3::prepare_freethreaded_python();
    let db_path = get_temp_db_path();
    let plugins_path = get_temp_plugins_path();
    let config = create_valid_test_config(&db_path, &plugins_path);

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .build()
        .unwrap();
    runtime.start().unwrap();
    Arc::new(BrainApplication::new(Arc::new(runtime)))
}

// ----------------------------------------------------------------------
// Core Integration Test cases
// ----------------------------------------------------------------------

#[tokio::test]
async fn test_direct_rust_caller_capabilities() {
    let app = setup_app();

    // 1. Test Ingest
    let context = ExecutionContext::default();
    let envelope = create_test_envelope();
    let res = app.ingest(envelope, &context).await.unwrap();
    assert_eq!(res.status, "success");
    assert!(res.processed);

    // 2. Test Search (Empty DB search summary)
    let query = SearchQuery {
        text: "relational memory".to_string(),
        kinds: None,
        pagination: None,
    };
    let search_res = app.search(query, &context).await.unwrap();
    assert!(search_res.is_empty());

    // 3. Test Workflow and Admin
    let workflow_res = app.workflow("memory_consolidation".to_string(), &context).await.unwrap();
    assert_eq!(workflow_res, vec!["step1", "step2"]);

    let admin_res = app.administration("clear_caches".to_string(), &context).await.unwrap();
    assert!(admin_res.contains("completed successfully"));
}

#[tokio::test]
async fn test_cancellation_and_progress_emission() {
    let app = setup_app();

    let progress_events = Arc::new(Mutex::new(Vec::new()));
    let token = CancellationToken::new();
    
    let context = ExecutionContext::default()
        .with_request_id(ApplicationRequestId::new())
        .with_cancellation(token.clone())
        .with_event_sink(TestEventSink {
            events: progress_events.clone(),
        })
        .with_deadline(Instant::now() + std::time::Duration::from_secs(5));

    token.cancel();

    let envelope = create_test_envelope();
    let err = app.ingest(envelope, &context).await.unwrap_err();
    assert!(matches!(err, ApplicationError::Cancelled(_)));

    let token_fresh = CancellationToken::new();
    let context_fresh = ExecutionContext::default()
        .with_cancellation(token_fresh)
        .with_event_sink(TestEventSink {
            events: progress_events.clone(),
        });

    let envelope_fresh = create_test_envelope();
    app.ingest(envelope_fresh, &context_fresh).await.unwrap();

    let events = progress_events.lock().unwrap();
    assert_eq!(events.len(), 3);
    if let ApplicationEvent::Progress(ref p) = events[0] {
        assert_eq!(p.message, "Validating ingestion envelope DTO");
    } else {
        panic!("Expected Progress event");
    }
    if let ApplicationEvent::Progress(ref p) = events[2] {
        assert_eq!(p.message, "Ingestion completed successfully");
    } else {
        panic!("Expected Progress event");
    }
}

#[tokio::test]
async fn test_mock_cli_adapter_integration() {
    let app = setup_app();
    let cli = MockCliAdapter { app };

    let output = cli.handle_command("ingest").await;
    assert_eq!(output, "CLI_SUCCESS: processed=true");

    let output_err = cli.handle_command("unknown").await;
    assert_eq!(output_err, "CLI_UNKNOWN_COMMAND");
}

#[tokio::test]
async fn test_mock_uds_transport_integration() {
    let app = setup_app();
    let uds = MockUdsAdapter { app };

    let envelope = create_test_envelope();
    let request_json = serde_json::to_string(&envelope).unwrap() + "\n";

    let response_json = uds.handle_uds_frame(&request_json).await.unwrap();
    assert!(response_json.contains("processed"));
    assert!(response_json.ends_with("\n"));
}
