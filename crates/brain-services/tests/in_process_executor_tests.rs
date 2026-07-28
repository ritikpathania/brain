use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use brain_services::worker::*;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_in_process_executor_execution_and_cancellation() {
    let executor = InProcessExecutor::new();
    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://inputs/sample.txt".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "worker-1".to_string(),
            lease_until: 2000,
        },
    };

    let token = CancellationToken::new();
    let ctx = TaskExecutionContext {
        cancellation_token: token.clone(),
        started_at: Instant::now(),
    };

    let result = executor.execute(&assignment, &ctx).await.unwrap();
    assert_eq!(result.task_id, task_id);

    // Verify cancellation
    token.cancel();
    let err = executor.execute(&assignment, &ctx).await.unwrap_err();
    assert!(matches!(err, TaskExecutionError::Cancelled));
}
