use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;

#[tokio::test]
async fn test_mock_worker_transport_dispatch_success_and_failure() {
    let transport = MockWorkerTransport::new();
    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://input-1".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "worker-1".to_string(),
            lease_until: 1000,
        },
    };

    // Success dispatch
    transport.dispatch(assignment.clone()).await.unwrap();
    assert_eq!(transport.dispatched_count(), 1);
    assert_eq!(transport.last_dispatched().unwrap(), assignment);

    // Failure emulation
    transport.set_should_fail_dispatch(true);
    assert!(transport.dispatch(assignment).await.is_err());
}
