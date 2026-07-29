//! Integration test suite for Task Execution Runtime (Phase 7 Milestone 7.4).

use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::planning::{
    ExecutionFailure, ExecutionFailureKind, ExecutionState, GoalId, GoalIntent, PlanningRuntime,
    Priority, TaskExecutionEventKind, TaskExecutionRuntime, TaskExecutionStatus, TaskExecutor,
    TaskStep,
};
use brain_services::query::{InMemoryQueryContext, KnowledgeQuery};
use uuid::Uuid;

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "execution_runtime_origin".to_string(),
        evidence_ids: vec!["ev_1111".to_string()],
        confidence: 0.95,
        timestamp_ms: 10000,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_worker_pool".to_string()),
        EntityIR {
            id: EntityId("entity_worker_pool".to_string()),
            canonical_name: "Worker Pool Service".to_string(),
            kind: "service".to_string(),
            aliases: vec!["worker-pool".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

struct FailingTaskExecutor;

impl TaskExecutor for FailingTaskExecutor {
    fn execute_task(&self, task: &TaskStep) -> Result<(), ExecutionFailure> {
        Err(ExecutionFailure {
            kind: ExecutionFailureKind::TaskFailure,
            task_id: Some(task.task_id),
            message: format!("Simulated failure on task '{}'", task.task_id),
        })
    }
}

#[test]
fn test_task_execution_runtime_causal_event_ordering() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Deploy Worker Pool".to_string(),
        context_query: KnowledgeQuery::new().with_text("Worker"),
        constraints: vec![],
        priority: Priority::High,
    };

    let runtime = PlanningRuntime::new();
    let (_task_plan, exec_plan) = runtime.create_execution_plan(&goal, &ctx).unwrap();

    let exec_runtime = TaskExecutionRuntime::default();
    let report = exec_runtime.execute_plan(&exec_plan).unwrap();

    assert_eq!(report.state, ExecutionState::Completed);
    assert!(!report.records.is_empty());
    assert!(!report.events.is_empty());

    // Verify causal event ordering: ExecutionStarted comes first, ExecutionCompleted comes last
    assert_eq!(
        report.events[0].kind,
        TaskExecutionEventKind::ExecutionStarted
    );
    assert_eq!(
        report.events.last().unwrap().kind,
        TaskExecutionEventKind::ExecutionCompleted
    );

    // Verify all records succeeded
    for rec in &report.records {
        assert_eq!(rec.status, TaskExecutionStatus::Succeeded);
    }
}

#[test]
fn test_task_execution_runtime_strongly_typed_failure() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Test task failure handling".to_string(),
        context_query: KnowledgeQuery::new().with_text("Worker"),
        constraints: vec![],
        priority: Priority::Normal,
    };

    let runtime = PlanningRuntime::new();
    let (_task_plan, exec_plan) = runtime.create_execution_plan(&goal, &ctx).unwrap();

    let exec_runtime = TaskExecutionRuntime::new(Box::new(FailingTaskExecutor));
    let report = exec_runtime.execute_plan(&exec_plan).unwrap();

    if let ExecutionState::Failed(ref fail) = report.state {
        assert_eq!(fail.kind, ExecutionFailureKind::TaskFailure);
        assert!(fail.message.contains("Simulated failure"));
    } else {
        panic!("Expected ExecutionState::Failed variant");
    }
}

#[test]
fn test_planning_runtime_execute_goal_end_to_end() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Execute end-to-end goal".to_string(),
        context_query: KnowledgeQuery::new().with_text("Worker"),
        constraints: vec![],
        priority: Priority::Critical,
    };

    let runtime = PlanningRuntime::new();
    let report = runtime.execute_goal(&goal, &ctx).unwrap();

    assert_eq!(report.state, ExecutionState::Completed);
    assert!(report.duration_ms > 0);
}
