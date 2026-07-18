use brain_application::dto::v1;
use brain_application::{
    ApplicationError, ApplicationEvent, ApplicationEventSink, ApplicationRequestId,
    BrainApplication, ExecutionContext,
};
use brain_integrations::{EventIdentity, IngestionEnvelope, IngestionEvent};
use brain_services::query::SearchQuery;
use brain_services::BrainRuntime;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

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

        let response = self
            .app
            .ingest(decoded, &context)
            .await
            .map_err(|e| format!("Application error: {}", e))?;

        let encoded = serde_json::to_string(&response)
            .map_err(|e| format!("Serialization error: {}", e))?
            + "\n";

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

fn setup_app() -> (Arc<BrainApplication>, tempfile::TempDir) {
    pyo3::prepare_freethreaded_python();
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("brain_test_app.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to initialize runtime");
    (Arc::new(BrainApplication::new(Arc::new(runtime))), dir)
}

// ----------------------------------------------------------------------
// Core Integration Test cases
// ----------------------------------------------------------------------

#[tokio::test]
async fn test_direct_rust_caller_capabilities() {
    let (app, _dir) = setup_app();

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
    let workflow_res = app
        .workflow("memory_consolidation".to_string(), &context)
        .await
        .unwrap();
    assert_eq!(workflow_res, vec!["step1", "step2"]);

    let admin_res = app
        .administration("clear_caches".to_string(), &context)
        .await
        .unwrap();
    assert!(admin_res.contains("completed successfully"));
}

#[tokio::test]
async fn test_cancellation_and_progress_emission() {
    let (app, _dir) = setup_app();

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
    let (app, _dir) = setup_app();
    let cli = MockCliAdapter { app };

    let output = cli.handle_command("ingest").await;
    assert_eq!(output, "CLI_SUCCESS: processed=true");

    let output_err = cli.handle_command("unknown").await;
    assert_eq!(output_err, "CLI_UNKNOWN_COMMAND");
}

#[tokio::test]
async fn test_mock_uds_transport_integration() {
    let (app, _dir) = setup_app();
    let uds = MockUdsAdapter { app };

    let envelope = create_test_envelope();
    let request_json = serde_json::to_string(&envelope).unwrap() + "\n";

    let response_json = uds.handle_uds_frame(&request_json).await.unwrap();
    assert!(response_json.contains("processed"));
    assert!(response_json.ends_with("\n"));
}

#[test]
fn test_api_contract_stability_dto_serialization() {
    use std::collections::BTreeMap;

    // 1. Status DTO
    let status = v1::Status {
        uptime_secs: 100,
        storage_backend: "sqlite".to_string(),
        active_event_subscribers: 2,
        health: "healthy".to_string(),
    };
    let serialized_status = serde_json::to_string(&status).unwrap();
    let deserialized_status: v1::Status = serde_json::from_str(&serialized_status).unwrap();
    assert_eq!(status, deserialized_status);

    // 2. Metrics DTO
    let metrics = v1::Metrics {
        observations_ingested: 10,
        canonicalization_successes: 8,
        canonicalization_failures: 2,
        reflections_executed: 5,
        projections_executed: 4,
        retrieval_queries: 3,
        last_ingest_duration_ms: Some(120),
        last_projection_duration_ms: Some(45),
        avg_canonicalization_duration_ms: Some(80),
        avg_reflection_duration_ms: Some(25),
        avg_dispatch_duration_ms: Some(15),
    };
    let serialized_metrics = serde_json::to_string(&metrics).unwrap();
    let deserialized_metrics: v1::Metrics = serde_json::from_str(&serialized_metrics).unwrap();
    assert_eq!(metrics, deserialized_metrics);

    // 3. Diagnostics DTO
    let failure = v1::Failure {
        operation: "ingest".to_string(),
        error: "Database lock conflict".to_string(),
        timestamp_ms: 1718000000000,
    };
    let diagnostics = v1::Diagnostics {
        recent_failures: vec![failure],
        last_shutdown_duration_ms: Some(250),
    };
    let serialized_diag = serde_json::to_string(&diagnostics).unwrap();
    let deserialized_diag: v1::Diagnostics = serde_json::from_str(&serialized_diag).unwrap();
    assert_eq!(diagnostics, deserialized_diag);

    // 4. Capability DTO
    let capability = v1::Capability {
        name: "storage".to_string(),
        version: 1,
        description: "Durable SQLite relational storage".to_string(),
        state: "active".to_string(),
        is_enabled: true,
        is_experimental: false,
    };
    let serialized_cap = serde_json::to_string(&capability).unwrap();
    let deserialized_cap: v1::Capability = serde_json::from_str(&serialized_cap).unwrap();
    assert_eq!(capability, deserialized_cap);

    // 5. SearchSummary DTO
    let mut metadata = BTreeMap::new();
    metadata.insert("author".to_string(), "brain-system".to_string());
    let summary = v1::SearchSummary {
        id: "doc-123".to_string(),
        kind: "Message".to_string(),
        title: "Initial Observation".to_string(),
        body: "Hello relational memory".to_string(),
        metadata,
    };
    let serialized_sum = serde_json::to_string(&summary).unwrap();
    let deserialized_sum: v1::SearchSummary = serde_json::from_str(&serialized_sum).unwrap();
    assert_eq!(summary, deserialized_sum);

    // 6. Event DTO (TaskProgress)
    let progress_event = v1::Event::TaskProgress {
        operation_id: "op-123".to_string(),
        correlation_id: "corr-123".to_string(),
        state: "Started".to_string(),
        source: "evolution".to_string(),
        sequence: 1,
    };
    let serialized_prog = serde_json::to_string(&progress_event).unwrap();
    let deserialized_prog: v1::Event = serde_json::from_str(&serialized_prog).unwrap();
    assert_eq!(progress_event, deserialized_prog);

    // 7. Event DTO (ProjectionInvalidated)
    let invalidation_event = v1::Event::ProjectionInvalidated {
        projection_type: "sessions".to_string(),
        epoch: 5,
        correlation_id: "corr-456".to_string(),
    };
    let serialized_inv = serde_json::to_string(&invalidation_event).unwrap();
    let deserialized_inv: v1::Event = serde_json::from_str(&serialized_inv).unwrap();
    assert_eq!(invalidation_event, deserialized_inv);
}
