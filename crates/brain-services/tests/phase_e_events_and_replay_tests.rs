//! Integration tests for Phase E Stable Task Identity, Runtime Event Stream, and Replay Tooling.

use brain_domain::{CanonicalEntity, EntityId, KnowledgeEvidence, KnowledgeState};
use brain_events::ReflectionEventBus;
use brain_services::planning::reflection_supervisor::ReflectionSupervisor;
use brain_services::reflection::{
    ExecutionTimeline, ReflectionExecutionMode, ReflectionTask, RepairTask, ReplayEngine,
    StrengthenTask, TaskDag, TaskId, TaskReflectionContext, TaskRetryPolicy,
};
use std::sync::{Arc, Mutex};

#[test]
fn test_stable_task_identity_and_retry_policy() {
    let repair_task = RepairTask::new();
    assert_eq!(
        repair_task.id(),
        TaskId::new("reflection.repair.reconciliation")
    );
    assert_eq!(repair_task.name(), "RepairTask");

    let strengthen_task = StrengthenTask::new();
    assert_eq!(
        strengthen_task.id(),
        TaskId::new("reflection.strengthen.lifecycle")
    );
    assert_eq!(strengthen_task.retry_policy(), TaskRetryPolicy::default());
}

#[test]
fn test_runtime_event_stream_and_replay_engine() {
    let bus = Arc::new(ReflectionEventBus::new());
    let recorded_events: Arc<Mutex<Vec<brain_events::ReflectionEventEnvelope>>> =
        Arc::new(Mutex::new(Vec::new()));

    let rec_clone = recorded_events.clone();
    bus.subscribe(move |evt| {
        rec_clone.lock().unwrap().push((**evt).clone());
    });

    let mut dag = TaskDag::new();
    dag.add_node("repair", RepairTask::new(), vec![]);
    dag.add_node(
        "strengthen",
        StrengthenTask::new(),
        vec!["repair".to_string()],
    );

    let context = TaskReflectionContext::new("snap_obs", 1000);
    let mut supervisor = ReflectionSupervisor::default().with_event_bus(bus);

    let id1 = EntityId::new();
    let mut entities = vec![CanonicalEntity {
        id: id1,
        preferred_name: "  TEST NODE  ".to_string(),
        aliases: vec![],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    }];

    let result = supervisor
        .execute_dag(
            "plan_obs_01",
            ReflectionExecutionMode::Manual,
            &dag,
            &context,
            &mut entities,
        )
        .expect("Execution failed");

    assert_eq!(result.plan_id, "plan_obs_01");

    let events = recorded_events.lock().unwrap();
    assert!(events.len() >= 4); // TaskStarted * 2, TaskCompleted * 2, CheckpointCreated * 2

    // Record into timeline and verify replay
    let mut timeline = ExecutionTimeline::new();
    for evt in events.iter() {
        timeline.record(evt.clone());
    }

    let replay_engine = ReplayEngine::new();
    let replayed_count = replay_engine
        .replay_timeline(&timeline)
        .expect("Replay failed");
    assert_eq!(replayed_count, events.len());
}
