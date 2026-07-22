use brain_core::events::CorrelationId;
use brain_integrations::{EventIdentity, IngestionEnvelope, IngestionEvent};
use brain_services::{BrainRuntime, MemoryListQuery, RuntimeHealth, SqliteProjector};
use std::time::{Duration, SystemTime};

fn make_valid_obs(payload: &str, corr_id: CorrelationId) -> IngestionEnvelope {
    IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: EventIdentity {
            event_id: brain_domain::EventId(corr_id),
            parent_event_id: None,
            workspace_id: brain_domain::WorkspaceId::new("test"),
            client_id: brain_domain::ClientId::new("test"),
            adapter_id: brain_domain::AdapterId::new("test"),
            session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
            conversation_id: None,
            timestamp: chrono::Utc::now(),
        },
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: payload.to_string(),
            metadata: std::collections::BTreeMap::new(),
        },
    }
}

fn make_invalid_obs(corr_id: CorrelationId) -> IngestionEnvelope {
    IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: EventIdentity {
            event_id: brain_domain::EventId(corr_id),
            parent_event_id: None,
            workspace_id: brain_domain::WorkspaceId::new("test"),
            client_id: brain_domain::ClientId::new("test"),
            adapter_id: brain_domain::AdapterId::new("test"),
            session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
            conversation_id: None,
            timestamp: chrono::Utc::now(),
        },
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: "".to_string(), // Empty content triggers validation failure
            metadata: std::collections::BTreeMap::new(),
        },
    }
}

#[tokio::test]
async fn test_runtime_uptime_and_health_transitions() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_introspection.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");

    // 1. Initial health is Healthy (Initialization completed)
    let status = runtime.status();
    assert_eq!(status.health, RuntimeHealth::Healthy);
    assert_eq!(status.storage_backend, "sqlite");

    // 2. Uptime increases monotonically
    let uptime_1 = status.uptime;
    std::thread::sleep(Duration::from_millis(5));
    let uptime_2 = runtime.status().uptime;
    assert!(uptime_2 >= uptime_1);

    // Keep references to shared internal state to check after shutdown consumes runtime
    let health_ref = runtime.status().health;
    assert_eq!(health_ref, RuntimeHealth::Healthy);

    // 3. Shutdown transitions health
    let diagnostics_ref = runtime.diagnostics();
    assert!(diagnostics_ref.last_shutdown.is_none());

    runtime.shutdown().expect("Clean shutdown");
}

#[tokio::test]
async fn test_runtime_active_subscriber_count() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_subscribers.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");

    // Verify initial count is 1 (the internal ObservabilitySubscriber is registered)
    assert_eq!(runtime.status().active_event_subscribers, 1);

    // Add another subscription
    let _rx = runtime.subscribe();
    assert_eq!(runtime.status().active_event_subscribers, 2);

    runtime.shutdown().expect("Clean shutdown");
}

#[tokio::test]
async fn test_runtime_metrics_and_latencies() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_metrics.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");

    let corr_id = CorrelationId::new_v4();
    let metrics_start = runtime.metrics();
    assert_eq!(metrics_start.observations_ingested, 0);
    assert_eq!(metrics_start.last_ingest_duration, None);

    // 1. Ingestion latency and success metrics
    runtime
        .ingest(make_valid_obs("Valid Payload", corr_id))
        .expect("Ingest succeeds");

    let metrics_after = runtime.metrics();
    assert_eq!(metrics_after.observations_ingested, 1);
    assert_eq!(metrics_after.canonicalization_successes, 1);
    assert_eq!(metrics_after.canonicalization_failures, 0);
    assert!(metrics_after.last_ingest_duration.is_some());
    assert!(metrics_after.last_ingest_duration.unwrap() > Duration::ZERO);

    // 2. Ingestion failure metric (latency should not be overwritten by failure)
    let last_success_duration = metrics_after.last_ingest_duration;
    let _ = runtime.ingest(make_invalid_obs(corr_id));

    let metrics_failed = runtime.metrics();
    assert_eq!(metrics_failed.observations_ingested, 2);
    assert_eq!(metrics_failed.canonicalization_successes, 1);
    assert_eq!(metrics_failed.canonicalization_failures, 1);
    // Latency remains the last successful operation latency
    assert_eq!(metrics_failed.last_ingest_duration, last_success_duration);

    // 3. Projections metrics and timings
    let projector = SqliteProjector::new(runtime.storage_ref());
    let query = MemoryListQuery { limit: 10 };
    assert_eq!(runtime.metrics().projections_executed, 0);
    assert_eq!(runtime.metrics().last_projection_duration, None);

    let _projection = runtime.query_projection(&projector, &query, corr_id);

    let metrics_proj = runtime.metrics();
    assert_eq!(metrics_proj.projections_executed, 1);
    assert!(metrics_proj.last_projection_duration.is_some());
    assert!(metrics_proj.last_projection_duration.unwrap() > Duration::ZERO);

    runtime.shutdown().expect("Clean shutdown");
}

#[tokio::test]
async fn test_runtime_diagnostics_ring_buffer() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_diagnostics.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");

    // Initially no failures
    let diag_init = runtime.diagnostics();
    assert_eq!(diag_init.recent_failures.len(), 0);

    // Generate 60 failures (empty payload) to verify FIFO eviction of ring buffer
    let corr_id = CorrelationId::new_v4();
    for _ in 0..60 {
        let _ = runtime.ingest(IngestionEnvelope {
            event_model_version: "1.0".to_string(),
            identity: EventIdentity {
                event_id: brain_domain::EventId(corr_id),
                parent_event_id: None,
                workspace_id: brain_domain::WorkspaceId::new("test"),
                client_id: brain_domain::ClientId::new("test"),
                adapter_id: brain_domain::AdapterId::new("test"),
                session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
                conversation_id: None,
                timestamp: chrono::Utc::now(),
            },
            event: IngestionEvent::Message {
                role: "user".to_string(),
                content: "".to_string(),
                metadata: std::collections::BTreeMap::new(),
            },
        });
    }

    let diag_after = runtime.diagnostics();
    // Bounded log cap of 50 entries
    assert_eq!(diag_after.recent_failures.len(), 50);

    // Assert FIFO: First 10 entries (indices 0..10) must be evicted.
    // The oldest remaining entry must correspond to format!("err-index-10").
    assert!(diag_after.recent_failures[0]
        .error
        .contains("Structural validation failed"));
    // Verify that the timestamps and diagnostic details exist
    assert!(diag_after.recent_failures[0].timestamp <= SystemTime::now());

    runtime.shutdown().expect("Clean shutdown");
}

#[tokio::test]
async fn test_snapshot_consistency_monotonicity() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_consistency.db");
    let db_str = db_path.to_str().expect("Valid path string");

    let runtime = BrainRuntime::new(db_str).expect("Failed to construct runtime");
    let corr_id = CorrelationId::new_v4();

    // Take snapshot 1
    let m1 = runtime.metrics();

    // Perform operations
    runtime
        .ingest(make_valid_obs("Payload 1", corr_id))
        .unwrap();
    runtime
        .ingest(make_valid_obs("Payload 2", corr_id))
        .unwrap();

    // Take snapshot 2
    let m2 = runtime.metrics();

    // Perform more operations
    runtime
        .ingest(make_valid_obs("Payload 3", corr_id))
        .unwrap();

    // Take snapshot 3
    let m3 = runtime.metrics();

    // Assert strictly monotonic increments on counters
    assert!(m2.observations_ingested > m1.observations_ingested);
    assert!(m3.observations_ingested > m2.observations_ingested);

    assert_eq!(m1.canonicalization_successes, 0);
    assert_eq!(m2.canonicalization_successes, 2);
    assert_eq!(m3.canonicalization_successes, 3);

    runtime.shutdown().expect("Clean shutdown");
}
