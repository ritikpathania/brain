use brain_core::errors::BrainError;
use brain_services::health_evaluator::{DerivedRuntimeHealth, HealthEvaluator};
use brain_services::orchestrator::{
    OrchestratorTask, RuntimeOrchestrator, TaskExecutor, TaskKind, TaskPriority, TaskStatus,
};
use std::sync::Arc;

struct MockExecutor;

impl TaskExecutor for MockExecutor {
    fn execute(&self, _task: &OrchestratorTask) -> Result<(), BrainError> {
        Ok(())
    }
}

#[test]
fn test_derived_health_evaluation_rules() {
    let evaluator = HealthEvaluator {
        max_projection_lag_threshold: 100,
        max_failed_tasks_threshold: 5,
    };

    // 1. Healthy state
    let health_1 = evaluator.evaluate(10, 0, 50);
    assert_eq!(health_1, DerivedRuntimeHealth::Healthy);

    // 2. Projection lag breach triggers Degraded
    let health_2 = evaluator.evaluate(10, 0, 150);
    assert!(
        matches!(health_2, DerivedRuntimeHealth::Degraded { ref subsystem, .. } if subsystem == "projections")
    );

    // 3. Task failure threshold breach triggers Degraded
    let health_3 = evaluator.evaluate(10, 8, 50);
    assert!(
        matches!(health_3, DerivedRuntimeHealth::Degraded { ref subsystem, .. } if subsystem == "orchestrator")
    );
}

#[test]
fn test_task_history_ring_buffer_bounds() {
    let executor = Arc::new(MockExecutor);
    // Capacity 200, history max capacity 10
    let orchestrator = RuntimeOrchestrator::with_history_capacity(executor, 200, 10);

    for i in 0..25 {
        let task = OrchestratorTask::new(
            TaskKind::Project {
                name: Some(format!("test_{}", i)),
            },
            TaskPriority::Normal,
        );
        orchestrator.schedule(task).unwrap();
        orchestrator.tick().unwrap();
    }

    let snap = orchestrator.diagnostics_snapshot();
    assert_eq!(snap.tasks_completed, 25);
    // Ring buffer must hold exactly 10 entries
    assert_eq!(snap.task_history.len(), 10);

    // Verify task statuses in history are Succeeded
    for trace in &snap.task_history {
        assert!(matches!(trace.status, TaskStatus::Succeeded { .. }));
    }
}

#[test]
fn test_snapshot_immutability_and_monotonicity() {
    let executor = Arc::new(MockExecutor);
    let orchestrator = RuntimeOrchestrator::new(executor, 100);

    let t1 = OrchestratorTask::new(TaskKind::Compile, TaskPriority::Critical);
    orchestrator.schedule(t1).unwrap();
    orchestrator.tick().unwrap();

    let snap_1 = orchestrator.diagnostics_snapshot();
    assert_eq!(snap_1.tasks_completed, 1);
    assert_eq!(snap_1.task_history.len(), 1);

    // Execute 3 additional tasks
    for _ in 0..3 {
        let task = OrchestratorTask::new(TaskKind::Reflect { force: true }, TaskPriority::Normal);
        orchestrator.schedule(task).unwrap();
        orchestrator.tick().unwrap();
    }

    let snap_2 = orchestrator.diagnostics_snapshot();

    // snap_1 must remain completely unchanged (immutable value object)
    assert_eq!(snap_1.tasks_completed, 1);
    assert_eq!(snap_1.task_history.len(), 1);

    // snap_2 reflects the new state
    assert_eq!(snap_2.tasks_completed, 4);
    assert_eq!(snap_2.task_history.len(), 4);
}
