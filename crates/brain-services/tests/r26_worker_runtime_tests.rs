use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use brain_services::worker::*;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_end_to_end_worker_execution_staging_and_decorators() {
    let dir = tempdir().unwrap();
    let _artifact_store = Arc::new(LocalFilesystemArtifactStore::new(dir.path().to_path_buf()));

    let inner = Arc::new(InProcessExecutor::new());
    let timeout = Arc::new(TimeoutExecutor::new(inner, Duration::from_secs(5)));
    let executor = RetryExecutor::new(timeout, 2);

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
            lease_until: 3000,
        },
    };

    let ctx = TaskExecutionContext {
        cancellation_token: CancellationToken::new(),
        started_at: Instant::now(),
    };

    let result = executor.execute(&assignment, &ctx).await.unwrap();
    assert_eq!(result.task_id, task_id);
    assert_eq!(result.metadata.get("executor").unwrap(), "in_process");
}
