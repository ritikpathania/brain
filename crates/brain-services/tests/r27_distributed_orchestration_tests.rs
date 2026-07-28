use brain_domain::jobs::JobId;
use brain_services::coordinator::*;
use brain_services::distributed::*;
use brain_services::runtime::*;

#[test]
fn test_end_to_end_coordinator_orchestration_pipeline() {
    let _state = CoordinatorState::new(100);
    let mut queue_mgr = QueueManager::new(100);
    let mut failure_det = FailureDetector::new(5);

    let t1 = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    // 1. Enqueue task via ExternalEvent
    let enq_ev = CoordinatorEvent::External(ExternalEvent::TaskEnqueued {
        task_id: t1,
        execution_id: exec_id,
        job_id,
        priority: 5,
    });

    match enq_ev {
        CoordinatorEvent::External(ExternalEvent::TaskEnqueued {
            task_id,
            execution_id,
            job_id,
            priority,
        }) => {
            queue_mgr
                .enqueue(task_id, execution_id, job_id, priority)
                .unwrap();
        }
        _ => panic!("Expected TaskEnqueued"),
    }

    assert_eq!(queue_mgr.len(), 1);

    // 2. Failure detector heartbeat check
    failure_det.record_heartbeat("worker-1".to_string(), 1000);
    assert!(failure_det.check_health("worker-1".to_string(), 1002).is_none());
}
