//! Integration tests for Phase F Production Readiness: Event Envelopes, EventStore, Metrics Summary & Replay Verification.

use brain_domain::{CanonicalEntity, EntityId, KnowledgeEvidence, KnowledgeState};
use brain_events::{
    EventStore, InMemoryEventStore, ReflectionEventBus, ReflectionEventEnvelope,
    CURRENT_EVENT_SCHEMA_VERSION,
};
use brain_services::planning::reflection_supervisor::ReflectionSupervisor;
use brain_services::reflection::{
    ReflectionExecutionMode, ReflectionMetricsSummary, RepairTask, ReplayEngine, StrengthenTask,
    TaskDag, TaskReflectionContext,
};
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn test_phase_f_event_envelope_correlation_and_versioning() {
    let correlation_id = Uuid::new_v4();
    let envelope = ReflectionEventEnvelope::new(
        "plan_env_01",
        Some("task_repair".to_string()),
        correlation_id,
        1000,
        brain_events::ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_env_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    );

    assert_eq!(envelope.schema_version, CURRENT_EVENT_SCHEMA_VERSION);
    assert_eq!(envelope.plan_id, "plan_env_01");
    assert_eq!(envelope.task_id, Some("task_repair".to_string()));
    assert_eq!(envelope.correlation_id, correlation_id);
}

#[test]
fn test_phase_f_event_store_persistence_and_compaction() {
    let store = InMemoryEventStore::new();
    let correlation_id = Uuid::new_v4();

    let env1 = ReflectionEventEnvelope::new(
        "plan_store_01",
        None,
        correlation_id,
        1000,
        brain_events::ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_store_01".to_string(),
            stage_index: 0,
            modified_entity_count: 2,
            timestamp_ms: 1000,
        },
    );

    let env2 = ReflectionEventEnvelope::new(
        "plan_store_01",
        None,
        correlation_id,
        2000,
        brain_events::ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_store_01".to_string(),
            stage_index: 1,
            modified_entity_count: 2,
            timestamp_ms: 2000,
        },
    );

    store.append(env1).expect("Append env1 failed");
    store.append(env2).expect("Append env2 failed");

    let queried = store.query("plan_store_01");
    assert_eq!(queried.len(), 2);

    let stream = store.stream();
    assert_eq!(stream.len(), 2);

    // Test timestamp compaction
    let removed = store.compact(1500);
    assert_eq!(removed, 1);
    assert_eq!(store.stream().len(), 1);
    assert_eq!(store.stream()[0].timestamp_ms, 2000);
}

#[test]
fn test_phase_f_metrics_summary_and_replay_verification() {
    let bus = Arc::new(ReflectionEventBus::new());
    let store = Arc::new(InMemoryEventStore::new());

    let store_clone = store.clone();
    bus.subscribe(move |envelope| {
        store_clone.append((**envelope).clone()).unwrap();
    });

    let mut dag = TaskDag::new();
    dag.add_node("repair", RepairTask::new(), vec![]);
    dag.add_node(
        "strengthen",
        StrengthenTask::new(),
        vec!["repair".to_string()],
    );

    let context = TaskReflectionContext::new("snap_prod", 1000);
    let mut supervisor = ReflectionSupervisor::default().with_event_bus(bus);

    let id1 = EntityId::new();
    let mut entities = vec![CanonicalEntity {
        id: id1,
        preferred_name: "  PROD FACT  ".to_string(),
        aliases: vec![],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    }];

    supervisor
        .execute_dag(
            "plan_prod_01",
            ReflectionExecutionMode::Manual,
            &dag,
            &context,
            &mut entities,
        )
        .expect("Supervisor execution failed");

    // Verify event store recorded envelopes
    let envelopes = store.query("plan_prod_01");
    assert!(envelopes.len() >= 4);

    // Aggregate metrics summary
    let mut summary = ReflectionMetricsSummary::new();
    for env in &envelopes {
        summary.record_event(env);
    }

    assert!(summary.checkpoint_count > 0);
    assert!(summary.total_changes_applied > 0);

    // Verify replay engine over event store
    let replay_engine = ReplayEngine::new();
    let count = replay_engine
        .verify_event_store_plan(store.as_ref(), "plan_prod_01")
        .expect("Deterministic replay failed");
    assert_eq!(count, envelopes.len());
}
