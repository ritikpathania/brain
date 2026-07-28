use brain_domain::jobs::JobId;
use brain_services::coordinator::*;
use brain_services::distributed::*;
use brain_services::runtime::*;

#[test]
fn test_scheduling_engine_pure_placement_evaluation() {
    let mut q_manager = QueueManager::new(10);
    let t1 = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());
    q_manager.enqueue(t1, exec_id, job_id, 5).unwrap();

    let desc1 = WorkerDescriptor {
        worker_id: "w1".to_string(),
        hostname: "node1".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "x86_64".to_string(),
        supported_capabilities: std::collections::HashSet::new(),
        labels: std::collections::HashMap::new(),
    };
    let status1 = WorkerStatus {
        current_load: 0.1,
        available_resources: Resources {
            cpu_cores: 8,
            memory_bytes: 16000,
            gpu_count: 0,
            custom_resources: std::collections::HashMap::new(),
        },
        active_lease_count: 0,
        is_healthy: true,
    };
    let c1 = WorkerCandidate {
        descriptor: &desc1,
        status: &status1,
    };

    let queue_snap = q_manager.snapshot();
    let candidates = vec![c1];
    let worker_snap = WorkerSnapshot {
        candidates: &candidates,
    };

    let engine = SchedulingEngine::new(LeastLoadedPolicy);
    let decisions = engine.schedule(&queue_snap, &worker_snap);

    assert_eq!(decisions.len(), 1);
    match &decisions[0] {
        SchedulingDecision::Assign(assignment) => {
            assert_eq!(assignment.task_id, t1);
            assert_eq!(assignment.lease.lease_owner, "w1");
        }
        _ => panic!("Expected Assign decision"),
    }
}
