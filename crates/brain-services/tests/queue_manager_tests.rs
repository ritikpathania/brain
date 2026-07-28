use brain_domain::jobs::JobId;
use brain_services::coordinator::*;
use brain_services::runtime::*;

#[test]
fn test_queue_manager_enqueue_and_priority_sorting() {
    let mut manager = QueueManager::new(2);

    let t1 = TaskId::new();
    let t2 = TaskId::new();
    let t3 = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    assert!(manager.enqueue(t1, exec_id, job_id, 1).is_ok());
    assert!(manager.enqueue(t2, exec_id, job_id, 10).is_ok());

    // Exceed max queue depth
    assert!(manager.enqueue(t3, exec_id, job_id, 5).is_err());

    let snapshot = manager.snapshot();
    assert_eq!(snapshot.ready_tasks.len(), 2);
    assert_eq!(snapshot.ready_tasks[0].task_id, t2); // Higher priority first
}
