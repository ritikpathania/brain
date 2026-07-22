use brain_domain::{Embedding, Node, NodeId, NodeType};
use brain_services::brain_runtime::BrainRuntime;
use tempfile::tempdir;
use uuid::Uuid;

fn setup_test_runtime() -> (BrainRuntime, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("brain_test.db");
    let runtime = BrainRuntime::new(db_path.to_str().unwrap()).unwrap();
    (runtime, dir)
}

#[tokio::test]
async fn test_reflection_scheduler_disabled_by_default() {
    let (runtime, _dir) = setup_test_runtime();

    // Default configuration invariant verification
    assert!(!runtime.config().reflection().background_enabled());
    assert_eq!(runtime.metrics().reflections_executed, 0);
    assert_eq!(runtime.metrics().reflection_findings_count, 0);
    assert_eq!(runtime.metrics().reflection_commands_executed, 0);
    assert_eq!(runtime.metrics().reflection_commands_skipped, 0);
    assert!(runtime.metrics().last_reflection_duration.is_none());
}

#[tokio::test]
async fn test_reflection_scheduler_manual_run_and_metrics() {
    let (runtime, _dir) = setup_test_runtime();

    let node_a_id = NodeId(Uuid::new_v4());
    let node_b_id = NodeId(Uuid::new_v4());

    let node_a = Node::new(
        node_a_id,
        "Quantum Computing".to_string(),
        NodeType::Concept,
    );
    let node_b = Node::new(
        node_b_id,
        "quantum computing".to_string(),
        NodeType::Concept,
    );

    let emb_a = Embedding::new(node_a_id, vec![1.0, 0.0, 0.0]);
    let emb_b = Embedding::new(node_b_id, vec![1.0, 0.0, 0.0]);

    let mut run_tx = |tx: &dyn brain_core::repositories::StorageTransaction| {
        tx.repositories().nodes().save(&node_a)?;
        tx.repositories().nodes().save(&node_b)?;
        tx.repositories().embeddings().save(&emb_a)?;
        tx.repositories().embeddings().save(&emb_b)?;
        Ok(())
    };
    runtime.storage_ref().run_transaction(&mut run_tx).unwrap();

    // Force run a background cycle (simulating rate-limited tick)
    runtime
        .reflection_scheduler()
        .run_cycle(true)
        .expect("Reflection cycle run failed");

    let metrics = runtime.metrics();
    assert_eq!(metrics.reflections_executed, 1);
    assert!(metrics.reflection_findings_count >= 1);
    assert!(metrics.last_reflection_duration.is_some());
}

#[tokio::test]
async fn test_reflection_scheduler_event_trigger_threshold() {
    let (runtime, _dir) = setup_test_runtime();

    // Unforced run_cycle with min_events_trigger > 0 should not execute when no new events arrived
    runtime
        .reflection_scheduler()
        .run_cycle(false)
        .expect("Unforced cycle failed");

    let metrics = runtime.metrics();
    assert_eq!(metrics.reflections_executed, 0);
}
