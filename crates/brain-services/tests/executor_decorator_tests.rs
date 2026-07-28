use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use brain_services::worker::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_timeout_and_retry_executor_decorator_composition() {
    let inner = Arc::new(InProcessExecutor::new());
    let timeout_exec = TimeoutExecutor::new(inner, Duration::from_millis(500));
    let retry_exec = RetryExecutor::new(Arc::new(timeout_exec), 2);

    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://ref".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "w1".to_string(),
            lease_until: 1000,
        },
    };

    let ctx = TaskExecutionContext {
        cancellation_token: CancellationToken::new(),
        started_at: Instant::now(),
    };

    let result = retry_exec.execute(&assignment, &ctx).await.unwrap();
    assert_eq!(result.task_id, task_id);
}
