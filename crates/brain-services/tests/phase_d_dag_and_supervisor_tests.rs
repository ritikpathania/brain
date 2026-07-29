//! Integration tests for Phase D Task DAG, Context, SchedulePolicy, Supervisor, and Checkpointing.

use brain_domain::{CanonicalEntity, EntityId, KnowledgeEvidence, KnowledgeState};
use brain_services::planning::reflection_supervisor::{
    Checkpoint, CheckpointRuntime, RecoveryRuntime, ReflectionSupervisor,
    CURRENT_CHECKPOINT_SCHEMA_VERSION,
};
use brain_services::reflection::{
    CentralityTask, EmbeddingRefreshTask, ReflectionExecutionMode, RepairTask, SchedulePolicy,
    ScheduleTrigger, StrengthenTask, SummarizeTask, SystemMetricsSnapshot, TaskDag,
    TaskReflectionContext,
};
use tokio_util::sync::CancellationToken;

#[test]
fn test_task_dag_topological_levelization_and_cycle_detection() {
    let mut dag = TaskDag::new();
    dag.add_node("repair", RepairTask::new(), vec![]);
    dag.add_node(
        "strengthen",
        StrengthenTask::new(),
        vec!["repair".to_string()],
    );
    dag.add_node(
        "refresh",
        EmbeddingRefreshTask::new(),
        vec!["repair".to_string()],
    );
    dag.add_node(
        "centrality",
        CentralityTask::new(),
        vec!["strengthen".to_string()],
    );
    dag.add_node(
        "summarize",
        SummarizeTask::new(),
        vec!["centrality".to_string()],
    );

    let stages = dag
        .compute_stages()
        .expect("Topological levelization failed");

    assert_eq!(stages.len(), 4);
    assert_eq!(stages[0], vec!["repair"]);
    assert_eq!(stages[1], vec!["refresh", "strengthen"]); // Alphabetically sorted stage levelization
    assert_eq!(stages[2], vec!["centrality"]);
    assert_eq!(stages[3], vec!["summarize"]);

    // Test cycle detection
    let mut cyclic_dag = TaskDag::new();
    cyclic_dag.add_node("node1", RepairTask::new(), vec!["node2".to_string()]);
    cyclic_dag.add_node("node2", StrengthenTask::new(), vec!["node1".to_string()]);

    let cycle_result = cyclic_dag.compute_stages();
    assert!(cycle_result.is_err());
    assert!(cycle_result.unwrap_err().contains("cycle detected"));
}

#[test]
fn test_schedule_policy_composable_triggers() {
    let policy = SchedulePolicy::new()
        .add_trigger(ScheduleTrigger::ElapsedTime {
            duration_ms: 3_600_000,
        })
        .add_trigger(ScheduleTrigger::ObservationCount { threshold: 100 })
        .add_trigger(ScheduleTrigger::PendingMerges { count: 5 });

    let metrics_none = SystemMetricsSnapshot::default();
    assert_eq!(policy.should_trigger(&metrics_none), None);

    let metrics_obs = SystemMetricsSnapshot {
        pending_observation_count: 150,
        ..Default::default()
    };
    assert_eq!(
        policy.should_trigger(&metrics_obs),
        Some(ReflectionExecutionMode::Idle)
    );

    let metrics_time = SystemMetricsSnapshot {
        elapsed_since_last_run_ms: 4_000_000,
        ..Default::default()
    };
    assert_eq!(
        policy.should_trigger(&metrics_time),
        Some(ReflectionExecutionMode::Periodic)
    );
}

#[test]
fn test_supervisor_execution_checkpointing_and_resumption() {
    let id1 = EntityId::new();
    let mut entities = vec![CanonicalEntity {
        id: id1,
        preferred_name: "  STALE DATA  ".to_string(),
        aliases: vec![],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    }];

    let mut dag = TaskDag::new();
    dag.add_node("repair", RepairTask::new(), vec![]);
    dag.add_node(
        "strengthen",
        StrengthenTask::new(),
        vec!["repair".to_string()],
    );

    let context = TaskReflectionContext::new("snap_001", 1000);
    let mut supervisor = ReflectionSupervisor::default();

    let result = supervisor
        .execute_dag(
            "plan_test_01",
            ReflectionExecutionMode::Manual,
            &dag,
            &context,
            &mut entities,
        )
        .expect("Supervisor execution failed");

    assert_eq!(result.plan_id, "plan_test_01");
    assert_eq!(result.modified_entity_ids.len(), 1);
    assert_eq!(entities[0].preferred_name, "stale data");

    // Checkpoint saved in runtime
    let loaded_cp = supervisor
        .checkpoint_runtime()
        .load_checkpoint("plan_test_01")
        .expect("Load checkpoint failed")
        .expect("Checkpoint missing");

    assert_eq!(loaded_cp.schema_version, CURRENT_CHECKPOINT_SCHEMA_VERSION);
    assert_eq!(loaded_cp.completed_stage_index, 1);
    assert_eq!(loaded_cp.completed_task_ids.len(), 2);
}

#[test]
fn test_checkpoint_schema_version_compatibility_and_recovery() {
    let cp_future = Checkpoint::new("plan_future", 0, vec!["repair".to_string()], vec![], 2000);
    let mut cp_future_unsupported = cp_future.clone();
    cp_future_unsupported.schema_version = 999; // Future version

    let mut runtime = CheckpointRuntime::new();
    runtime.save_checkpoint(cp_future_unsupported);

    let err = runtime.load_checkpoint("plan_future");
    assert!(err.is_err());
    assert!(err
        .unwrap_err()
        .contains("Unsupported checkpoint schema version"));

    // Verify recovery runtime stage computation
    let cp_valid = Checkpoint::new("plan_valid", 2, vec!["task1".to_string()], vec![], 1000);
    let next_stage = RecoveryRuntime::prepare_resumption(&cp_valid).expect("Recovery failed");
    assert_eq!(next_stage, 3); // Resumes at stage index 3 (2 + 1)
}

#[test]
fn test_supervisor_cancellation_token_abort() {
    let mut dag = TaskDag::new();
    dag.add_node("repair", RepairTask::new(), vec![]);

    let token = CancellationToken::new();
    token.cancel(); // Pre-cancelled

    let context = TaskReflectionContext::new("snap_cancel", 1000).with_cancellation_token(token);
    let mut supervisor = ReflectionSupervisor::default();
    let mut entities = Vec::new();

    let err = supervisor.execute_dag(
        "plan_cancel",
        ReflectionExecutionMode::Manual,
        &dag,
        &context,
        &mut entities,
    );
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("cancellation token"));
}
