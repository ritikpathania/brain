use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use parking_lot::Mutex;
use uuid::Uuid;
use async_trait::async_trait;
use brain_domain::{
    Job, JobId, JobKind, JobPriority, JobOwner, JobDescription,
    JobTimestamp, JobState, Artifact, ArtifactId, ArtifactKind
};
use brain_services::jobs::{
    JobScheduler, JobExecutorRegistry, JobExecutor, JobExecutionContext,
    JobExecutionResult, JobExecutionFailure, SchedulerError, DomainEventPublisher
};

#[derive(Clone, Default)]
struct MockExecutor {
    run_count: Arc<AtomicUsize>,
    executed_ids: Arc<Mutex<Vec<JobId>>>,
}

#[async_trait]
impl JobExecutor for MockExecutor {
    async fn execute(
        &self,
        id: JobId,
        ctx: &JobExecutionContext,
    ) -> Result<JobExecutionResult, JobExecutionFailure> {
        self.run_count.fetch_add(1, Ordering::SeqCst);
        self.executed_ids.lock().push(id);

        // Simulate some async work while checking token
        for _ in 0..10 {
            if ctx.cancellation_token.is_cancelled() {
                return Err(JobExecutionFailure("Cancelled cooperatively".to_string()));
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let art_id = ArtifactId(Uuid::new_v4());
        Ok(JobExecutionResult {
            artifacts: vec![Artifact::new(art_id, ArtifactKind::Json, b"{}".to_vec())],
            logs: vec![(JobTimestamp(100), "Execution successful".to_string())],
        })
    }
}

struct MockDomainEventPublisher;
impl DomainEventPublisher for MockDomainEventPublisher {
    fn publish(&self, _event: brain_domain::DomainEvent) {}
}

fn create_job(prio: JobPriority) -> Job {
    let id = JobId(Uuid::new_v4());
    Job::new(
        id,
        JobKind::Tool,
        prio,
        JobOwner::System,
        JobDescription("Test task".to_string()),
        JobTimestamp(0),
        BTreeSet::new(),
    )
}

#[tokio::test]
async fn test_priority_fifo_ordering() {
    let mock = MockExecutor::default();
    let mut registry = JobExecutorRegistry::new();
    registry.register(JobKind::Tool, Arc::new(mock.clone()));

    // Max concurrency = 1 to enforce sequential ordering in queue
    let scheduler = JobScheduler::new(registry, Arc::new(MockDomainEventPublisher), 1);

    // Create jobs with distinct priorities
    let j_normal1 = create_job(JobPriority::Normal);
    let j_high = create_job(JobPriority::High);
    let j_critical = create_job(JobPriority::Critical);
    let j_normal2 = create_job(JobPriority::Normal);

    let id_normal1 = j_normal1.id();
    let id_high = j_high.id();
    let id_critical = j_critical.id();
    let id_normal2 = j_normal2.id();

    // Submit in order: Normal1, High, Critical, Normal2
    assert!(scheduler.submit(j_normal1, BTreeSet::new()).await.is_ok());
    assert!(scheduler.submit(j_high, BTreeSet::new()).await.is_ok());
    assert!(scheduler.submit(j_critical, BTreeSet::new()).await.is_ok());
    assert!(scheduler.submit(j_normal2, BTreeSet::new()).await.is_ok());

    // Wait for all to complete
    for _ in 0..100 {
        if scheduler.get_job_state(id_normal2) == Some(JobState::Completed) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // Verify ordering: Critical (first from queue), High, Normal1 (first FIFO), Normal2 (second FIFO)
    // Wait, the very first job submitted (Normal1) runs immediately because the queue is empty
    // when it was submitted!
    // So the running order should be:
    // - Normal1 (starts immediately as first sub)
    // - Critical (highest in queue when Normal1 completes)
    // - High (second highest in queue)
    // - Normal2 (third in queue)
    let ran = mock.executed_ids.lock().clone();
    assert_eq!(ran.len(), 4);
    assert_eq!(ran[0], id_normal1);
    assert_eq!(ran[1], id_critical);
    assert_eq!(ran[2], id_high);
    assert_eq!(ran[3], id_normal2);
}

#[tokio::test]
async fn test_dag_dependency_resolution() {
    let mock = MockExecutor::default();
    let mut registry = JobExecutorRegistry::new();
    registry.register(JobKind::Tool, Arc::new(mock.clone()));

    let scheduler = JobScheduler::new(registry, Arc::new(MockDomainEventPublisher), 1);

    // Tree: A -> B -> C
    let job_a = create_job(JobPriority::Normal);
    let job_b = create_job(JobPriority::Normal);
    let job_c = create_job(JobPriority::Normal);

    let id_a = job_a.id();
    let id_b = job_b.id();
    let id_c = job_c.id();

    // Submit A
    assert!(scheduler.submit(job_a, BTreeSet::new()).await.is_ok());

    // Submit B depending on A
    let mut deps_b = BTreeSet::new();
    deps_b.insert(id_a);
    assert!(scheduler.submit(job_b, deps_b).await.is_ok());

    // Submit C depending on B
    let mut deps_c = BTreeSet::new();
    deps_c.insert(id_b);
    assert!(scheduler.submit(job_c, deps_c).await.is_ok());

    // Wait for all to complete
    for _ in 0..100 {
        if scheduler.get_job_state(id_c) == Some(JobState::Completed) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let ran = mock.executed_ids.lock().clone();
    assert_eq!(ran.len(), 3);
    assert_eq!(ran[0], id_a);
    assert_eq!(ran[1], id_b);
    assert_eq!(ran[2], id_c);
}

#[tokio::test]
async fn test_dependency_cycle_rejection() {
    let mock = MockExecutor::default();
    let mut registry = JobExecutorRegistry::new();
    registry.register(JobKind::Tool, Arc::new(mock.clone()));
    let scheduler = JobScheduler::new(registry, Arc::new(MockDomainEventPublisher), 1);

    let job_a = create_job(JobPriority::Normal);
    let job_b = create_job(JobPriority::Normal);
    let job_c = create_job(JobPriority::Normal);

    let id_a = job_a.id();
    let id_b = job_b.id();
    let id_c = job_c.id();

    // 1. Submit A depending on C
    let mut deps_a = BTreeSet::new();
    deps_a.insert(id_c);
    assert!(scheduler.submit(job_a, deps_a).await.is_ok());

    // 2. Submit B depending on A
    let mut deps_b = BTreeSet::new();
    deps_b.insert(id_a);
    assert!(scheduler.submit(job_b, deps_b).await.is_ok());

    // 3. Submit C depending on B (creates C -> B -> A -> C cycle)
    let mut deps_c = BTreeSet::new();
    deps_c.insert(id_b);
    let res = scheduler.submit(job_c, deps_c).await;
    assert_eq!(res, Err(SchedulerError::DependencyCycle));
}

#[tokio::test]
async fn test_cooperative_cancellation() {
    let mock = MockExecutor::default();
    let mut registry = JobExecutorRegistry::new();
    registry.register(JobKind::Tool, Arc::new(mock.clone()));

    let scheduler = JobScheduler::new(registry, Arc::new(MockDomainEventPublisher), 1);

    let job = create_job(JobPriority::Normal);
    let id = job.id();

    assert!(scheduler.submit(job, BTreeSet::new()).await.is_ok());

    // Wait for the job to start running
    for _ in 0..100 {
        if scheduler.get_job_state(id) == Some(JobState::Running) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(scheduler.get_job_state(id), Some(JobState::Running));

    // Cancel job
    assert!(scheduler.cancel(id).await.is_ok());

    // Wait for status to transition to Cancelled
    for _ in 0..100 {
        if scheduler.get_job_state(id) == Some(JobState::Cancelled) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(scheduler.get_job_state(id), Some(JobState::Cancelled));
}

#[tokio::test]
async fn test_concurrency_limit() {
    let mock = MockExecutor::default();
    let mut registry = JobExecutorRegistry::new();
    registry.register(JobKind::Tool, Arc::new(mock.clone()));

    // Concurrency limit of 2
    let scheduler = JobScheduler::new(registry, Arc::new(MockDomainEventPublisher), 2);

    let job1 = create_job(JobPriority::Normal);
    let job2 = create_job(JobPriority::Normal);
    let job3 = create_job(JobPriority::Normal);
    let job4 = create_job(JobPriority::Normal);

    let id1 = job1.id();
    let id2 = job2.id();
    let id3 = job3.id();
    let id4 = job4.id();

    assert!(scheduler.submit(job1, BTreeSet::new()).await.is_ok());
    assert!(scheduler.submit(job2, BTreeSet::new()).await.is_ok());
    assert!(scheduler.submit(job3, BTreeSet::new()).await.is_ok());
    assert!(scheduler.submit(job4, BTreeSet::new()).await.is_ok());

    // Wait a brief moment and verify that at most 2 are running/completed at this instant
    tokio::time::sleep(Duration::from_millis(5)).await;

    let mut running_or_done = 0;
    for id in &[id1, id2, id3, id4] {
        let state = scheduler.get_job_state(*id).unwrap();
        if state == JobState::Running || state == JobState::Completed {
            running_or_done += 1;
        }
    }
    assert!(running_or_done <= 2);
}
