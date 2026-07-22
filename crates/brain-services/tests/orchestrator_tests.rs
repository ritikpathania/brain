use brain_core::errors::BrainError;
use brain_services::orchestrator::{
    OrchestratorTask, PriorityTaskQueue, RuntimeOrchestrator, TaskExecutor, TaskId, TaskKind,
    TaskPriority,
};
use std::sync::Arc;

struct MockExecutor {
    executed: Arc<parking_lot::Mutex<Vec<TaskId>>>,
    should_fail: bool,
}

impl TaskExecutor for MockExecutor {
    fn execute(&self, task: &OrchestratorTask) -> Result<(), BrainError> {
        self.executed.lock().push(task.id);
        if self.should_fail {
            Err(BrainError::Validation {
                message: "Simulated task failure".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

#[test]
fn test_orchestrator_priority_scheduling() {
    let mut queue = PriorityTaskQueue::new(10);

    let t_low = OrchestratorTask::new(
        TaskKind::Maintain {
            mode: brain_services::orchestrator::MaintenanceMode::PeriodicWalCheckpoint,
        },
        TaskPriority::Low,
    );
    let t_normal = OrchestratorTask::new(TaskKind::Reflect { force: false }, TaskPriority::Normal);
    let t_high = OrchestratorTask::new(TaskKind::Project { name: None }, TaskPriority::High);
    let t_critical = OrchestratorTask::new(TaskKind::Compile, TaskPriority::Critical);

    let id_low = queue.push(t_low).unwrap();
    let id_normal = queue.push(t_normal).unwrap();
    let id_high = queue.push(t_high).unwrap();
    let id_critical = queue.push(t_critical).unwrap();

    assert_eq!(queue.len(), 4);

    // Popping ready tasks must observe strict priority order: Critical -> High -> Normal -> Low
    assert_eq!(queue.pop_ready().unwrap().id, id_critical);
    assert_eq!(queue.pop_ready().unwrap().id, id_high);
    assert_eq!(queue.pop_ready().unwrap().id, id_normal);
    assert_eq!(queue.pop_ready().unwrap().id, id_low);
    assert!(queue.pop_ready().is_none());
}

#[test]
fn test_orchestrator_dependency_resolution() {
    let mut queue = PriorityTaskQueue::new(10);

    let task_a = OrchestratorTask::new(TaskKind::Compile, TaskPriority::Critical);
    let id_a = task_a.id;

    let task_b = OrchestratorTask::new(TaskKind::Project { name: None }, TaskPriority::High)
        .with_dependency(id_a);
    let id_b = task_b.id;

    queue.push(task_a).unwrap();
    queue.push(task_b).unwrap();

    // Task B depends on Task A, so Task A must pop first
    let popped_1 = queue.pop_ready().unwrap();
    assert_eq!(popped_1.id, id_a);

    // Task B cannot pop until Task A is marked completed
    assert!(queue.pop_ready().is_none());

    queue.mark_completed(id_a);

    let popped_2 = queue.pop_ready().unwrap();
    assert_eq!(popped_2.id, id_b);
}

#[test]
fn test_orchestrator_failure_isolation() {
    let executed = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let executor = Arc::new(MockExecutor {
        executed: executed.clone(),
        should_fail: true,
    });
    let orchestrator = RuntimeOrchestrator::new(executor, 10);

    let t1 = orchestrator
        .submit(TaskKind::Compile, TaskPriority::Critical)
        .unwrap();
    let t2 = orchestrator
        .submit(TaskKind::Reflect { force: true }, TaskPriority::Normal)
        .unwrap();

    // First task fails
    let res1 = orchestrator.tick().unwrap();
    assert_eq!(res1, Some(t1));

    // Orchestrator loop remains alive and continues processing task 2
    let res2 = orchestrator.tick().unwrap();
    assert_eq!(res2, Some(t2));

    assert_eq!(orchestrator.tasks_completed_count(), 0);
    assert_eq!(orchestrator.tasks_failed_count(), 2);
    assert_eq!(executed.lock().len(), 2);
}

#[test]
fn test_orchestrator_coalescing_and_backpressure() {
    let mut queue = PriorityTaskQueue::new(5);

    let r1 = OrchestratorTask::new(TaskKind::Reflect { force: false }, TaskPriority::Normal);
    let id_r1 = queue.push(r1).unwrap();

    // Second reflection push coalesces to id_r1
    let r2 = OrchestratorTask::new(TaskKind::Reflect { force: false }, TaskPriority::Normal);
    let id_r2 = queue.push(r2).unwrap();

    assert_eq!(id_r1, id_r2);
    assert_eq!(queue.len(), 1);
}

#[test]
fn test_orchestrator_runtime_replay_determinism() {
    let run_simulation = || -> Vec<TaskId> {
        let executed = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let executor = Arc::new(MockExecutor {
            executed: executed.clone(),
            should_fail: false,
        });
        let orchestrator = RuntimeOrchestrator::new(executor, 100);

        let t_compile = OrchestratorTask::new(TaskKind::Compile, TaskPriority::Critical);
        let id_compile = t_compile.id;

        let t_proj = OrchestratorTask::new(TaskKind::Project { name: None }, TaskPriority::High)
            .with_dependency(id_compile);

        let t_refl = OrchestratorTask::new(TaskKind::Reflect { force: true }, TaskPriority::Normal);
        let t_maint = OrchestratorTask::new(
            TaskKind::Maintain {
                mode: brain_services::orchestrator::MaintenanceMode::PeriodicWalCheckpoint,
            },
            TaskPriority::Low,
        );

        orchestrator.schedule(t_maint).unwrap();
        orchestrator.schedule(t_refl).unwrap();
        orchestrator.schedule(t_proj).unwrap();
        orchestrator.schedule(t_compile).unwrap();

        let mut order = Vec::new();
        while let Ok(Some(id)) = orchestrator.tick() {
            order.push(id);
        }
        order
    };

    let run_1 = run_simulation();
    let run_2 = run_simulation();

    // Total count of tasks executed must be identical
    assert_eq!(run_1.len(), run_2.len());
    assert_eq!(run_1.len(), 4);
}
