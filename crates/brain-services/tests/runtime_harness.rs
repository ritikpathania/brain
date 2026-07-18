//! Runtime harness integration test.
//!
//! Validates the complete `BrainRuntime` lifecycle as a single cohesive unit:
//! construction → ingestion → projection → observability → status/metrics → shutdown.

use brain_core::{events::CorrelationId, evolution::Observation};
use brain_services::{BrainRuntime, MemoryListQuery, RuntimeHealth, SqliteProjector};
use std::time::SystemTime;

fn make_obs(payload: &str, corr_id: CorrelationId) -> Observation {
    Observation {
        payload: payload.as_bytes().to_vec(),
        media_type: "text/plain".to_string(),
        provenance: brain_core::evolution::Provenance {
            source_adapter: "runtime-harness".to_string(),
            timestamp: SystemTime::now(),
            correlation_id: corr_id,
        },
    }
}

// --- S5-RT-1: Full runtime lifecycle ---
//
// Exercises: new() → ingest() → query_projection() → observability → status() → metrics() → shutdown()
// Key assertions:
// - status() reflects Healthy during operation, ShuttingDown/Stopped transitions via health field
// - metrics() accounts for each ingest and projection call
// - ShutdownSummary is returned to the caller (terminal information belongs to the caller)
// - channel is Disconnected (not Empty) after shutdown — proves dispatcher released its senders
#[test]
fn test_runtime_harness_lifecycle() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_harness.db");
    let db_str = db_path.to_str().expect("Valid path string");

    // 1. Construction — dependency graph fully assembled
    let runtime = BrainRuntime::new(db_str).expect("BrainRuntime::new() must succeed");

    let mut rx = runtime.subscribe();

    // --- Status: Healthy immediately after construction ---
    {
        let status = runtime.status();
        assert_eq!(
            status.health,
            RuntimeHealth::Healthy,
            "Status must be Healthy after construction"
        );
        assert_eq!(status.storage_backend, "sqlite");
        // At least the sync observability subscriber is registered
        assert!(
            status.active_event_subscribers >= 1,
            "At least one subscriber (sync observability) must be active"
        );
        assert!(
            status.uptime.as_nanos() > 0,
            "Uptime must be non-zero after construction"
        );
    }

    // --- Metrics: zero before any ingestion ---
    {
        let m = runtime.metrics();
        assert_eq!(m.observations_ingested, 0);
        assert_eq!(m.canonicalization_successes, 0);
        assert_eq!(m.projections_executed, 0);
        assert!(m.last_ingest_duration.is_none());
        assert!(m.last_projection_duration.is_none());
    }

    // 2. Ingestion — observe the full pipeline: canonicalize → reflect → events
    let corr_id = CorrelationId::new_v4();
    let result = runtime
        .ingest(make_obs("Ingested from Harness", corr_id))
        .expect("ingest() must succeed for a valid observation");

    assert_eq!(result.epoch.0, 1);
    assert_eq!(result.affected_entities.len(), 1);

    // --- Metrics: ingestion counters must advance ---
    {
        let m = runtime.metrics();
        assert_eq!(m.observations_ingested, 1);
        assert_eq!(m.canonicalization_successes, 1);
        assert_eq!(m.canonicalization_failures, 0);
        assert_eq!(
            m.reflections_executed, 1,
            "Reflection must fire once per ingest"
        );
        assert!(
            m.last_ingest_duration.is_some(),
            "last_ingest_duration must be recorded after a successful ingest"
        );
    }

    // 3. Projection — on-demand read from persisted state
    let projector = SqliteProjector::new(runtime.storage_ref());
    let projection = runtime.query_projection(&projector, &MemoryListQuery { limit: 10 }, corr_id);
    assert_eq!(projection.items.len(), 1);
    assert_eq!(projection.items[0].label, "Ingested from Harness");

    // --- Metrics: projection counter must advance ---
    {
        let m = runtime.metrics();
        assert_eq!(m.projections_executed, 1);
        assert!(m.last_projection_duration.is_some());
    }

    // 4. Observability — wait deterministically for the background thread to drain all events.
    //
    // is_complete() returns true only after TaskState::Completed is ingested by the subscriber
    // thread. Poll with a 1s hard cap so the test fails loudly instead of giving a false negative.
    {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            if runtime.is_complete(corr_id) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Timed out waiting for ObservabilitySubscriber to record Completed event"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    let spans = runtime.spans_for(corr_id).expect("Spans must be recorded");
    assert!(!spans.is_empty());

    // 5. Shutdown — deterministic teardown; terminal information returned to the caller
    let summary = runtime.shutdown().expect("shutdown() must not error");

    // ShutdownSummary is the caller's property — the daemon decides what to do with it.
    assert!(
        summary.duration.as_millis() < 5_000,
        "Shutdown must complete within 5 seconds, took {:?}",
        summary.duration
    );

    // Drain buffered events, then assert Disconnected (not Empty).
    // Disconnected proves the dispatcher released its senders; Empty would mean
    // the channel is quiet but senders are still alive.
    while rx.try_recv().is_ok() {}
    assert!(matches!(
        rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected)
    ));
}

// --- S5-RT-2: Repeated lifecycle (epoch persistence) ---
//
// Exercises: shutdown + re-open same DB. Verifies epoch advances from persisted state
// and projection returns items from both lifecycles.
#[test]
fn test_runtime_repeated_lifecycle() {
    let dir = tempfile::tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("test_repeated.db");
    let db_str = db_path.to_str().expect("Valid path string");

    // Cycle 1
    {
        let runtime = BrainRuntime::new(db_str).expect("Failed to build runtime 1");
        let corr_id = CorrelationId::new_v4();
        let result = runtime
            .ingest(make_obs("Ingestion 1", corr_id))
            .expect("Failed to ingest 1");
        assert_eq!(result.epoch.0, 1);

        let summary = runtime.shutdown().expect("Failed shutdown 1");
        assert!(summary.duration.as_millis() < 5_000);
    }

    // Cycle 2 — re-open the same DB
    {
        let runtime = BrainRuntime::new(db_str).expect("Failed to build runtime 2");
        let corr_id = CorrelationId::new_v4();
        let result = runtime
            .ingest(make_obs("Ingestion 2", corr_id))
            .expect("Failed to ingest 2");

        // Epoch must advance from persisted state
        assert_eq!(result.epoch.0, 2);

        // Both items must be visible
        let projector = SqliteProjector::new(runtime.storage_ref());
        let projection =
            runtime.query_projection(&projector, &MemoryListQuery { limit: 10 }, corr_id);
        assert_eq!(projection.items.len(), 2);
        assert_eq!(projection.items[0].label, "Ingestion 1");
        assert_eq!(projection.items[1].label, "Ingestion 2");

        runtime.shutdown().expect("Failed shutdown 2");
    }
}
