//! Integration test suite for Execution Resilience & Event-Sourced Monitoring (Phase 7 Milestone 7.5).

use brain_services::compiler::{EntityIR, EntityId, KnowledgeIR, ProvenanceIR};
use brain_services::planning::{
    BackoffStrategy, ExecutionFailure, ExecutionFailureKind, ExecutionMonitor, ExecutionState,
    GoalId, GoalIntent, PlanningRuntime, Priority, RetryPolicy, TaskExecutionRuntime, TaskExecutor,
    TaskStep,
};
use brain_services::query::{InMemoryQueryContext, KnowledgeQuery};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use uuid::Uuid;

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "execution_monitor_origin".to_string(),
        evidence_ids: vec!["ev_1212".to_string()],
        confidence: 0.95,
        timestamp_ms: 11000,
    }
}

fn sample_ir() -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    ir.entities.insert(
        EntityId("entity_resilient_pool".to_string()),
        EntityIR {
            id: EntityId("entity_resilient_pool".to_string()),
            canonical_name: "Resilient Pool Service".to_string(),
            kind: "service".to_string(),
            aliases: vec!["resilient-pool".to_string()],
            properties: std::collections::BTreeMap::new(),
            confidence: 0.95,
            provenance: sample_prov(),
            provenance_chain: vec![sample_prov()],
        },
    );

    ir
}

struct FlakyTaskExecutor {
    attempts: Arc<AtomicU32>,
    succeed_on_attempt: u32,
}

impl TaskExecutor for FlakyTaskExecutor {
    fn execute_task(&self, task: &TaskStep) -> Result<(), ExecutionFailure> {
        let current = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
        if current >= self.succeed_on_attempt {
            Ok(())
        } else {
            Err(ExecutionFailure {
                kind: ExecutionFailureKind::TaskFailure,
                task_id: Some(task.task_id),
                message: format!("Transient error on attempt {}", current),
            })
        }
    }
}

#[test]
fn test_execution_monitor_event_sourced_projection_and_idempotency() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Test monitor event projection".to_string(),
        context_query: KnowledgeQuery::new().with_text("Resilient"),
        constraints: vec![],
        priority: Priority::Normal,
    };

    let runtime = PlanningRuntime::new();
    let report = runtime.execute_goal(&goal, &ctx).unwrap();

    let monitor = ExecutionMonitor::new();
    let snapshot1 = monitor.project_events(&report.events);
    let snapshot2 = monitor.project_events(&report.events);

    // Replay idempotency invariant: Replay(events) == Replay(events)
    assert_eq!(snapshot1.total_tasks, snapshot2.total_tasks);
    assert_eq!(snapshot1.completed_tasks, snapshot2.completed_tasks);
    assert_eq!(snapshot1.failed_tasks, snapshot2.failed_tasks);
    assert!(snapshot1.total_tasks >= 1);
}

#[test]
fn test_retry_policy_recovers_transient_failure() {
    let ir = sample_ir();
    let ctx = InMemoryQueryContext::new(&ir);

    let goal = GoalIntent {
        goal_id: GoalId(Uuid::new_v4()),
        description: "Test retry recovery".to_string(),
        context_query: KnowledgeQuery::new().with_text("Resilient"),
        constraints: vec![],
        priority: Priority::High,
    };

    let planning_runtime = PlanningRuntime::new();
    let (_task_plan, exec_plan) = planning_runtime.create_execution_plan(&goal, &ctx).unwrap();

    let attempts = Arc::new(AtomicU32::new(0));
    let flaky_executor = FlakyTaskExecutor {
        attempts: Arc::clone(&attempts),
        succeed_on_attempt: 2, // Fails on attempt 1, succeeds on attempt 2
    };

    let exec_runtime = TaskExecutionRuntime::new(Box::new(flaky_executor));
    let policy = RetryPolicy::default();

    let report = exec_runtime
        .execute_plan_with_retry(&exec_plan, Some(&policy))
        .unwrap();

    assert_eq!(report.state, ExecutionState::Completed);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);

    let monitor = ExecutionMonitor::new();
    let snapshot = monitor.project_events(&report.events);
    assert_eq!(snapshot.retried_tasks, 1);
}

#[test]
fn test_backoff_strategy_exponential_calculation() {
    let strategy = BackoffStrategy::Exponential {
        initial_ms: 100,
        multiplier: 2.0,
        max_ms: 1000,
    };

    assert_eq!(strategy.calculate_delay_ms(1), 100);
    assert_eq!(strategy.calculate_delay_ms(2), 200);
    assert_eq!(strategy.calculate_delay_ms(3), 400);
    assert_eq!(strategy.calculate_delay_ms(5), 1000); // capped at max_ms
}
