use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use parking_lot::Mutex;
use uuid::Uuid;
use async_trait::async_trait;
use brain_domain::{
    Job, JobId, JobKind, JobPriority, JobOwner, JobDescription,
    JobTimestamp, JobState, DomainEvent, Artifact, ArtifactId, ArtifactKind
};
use brain_services::jobs::{
    JobScheduler, JobExecutorRegistry, JobExecutor, JobExecutionContext,
    JobExecutionResult, JobExecutionFailure, DomainEventPublisher
};

#[derive(Clone, Default)]
struct TestEventCollector {
    events: Arc<Mutex<Vec<DomainEvent>>>,
}

impl DomainEventPublisher for TestEventCollector {
    fn publish(&self, event: DomainEvent) {
        self.events.lock().push(event);
    }
}

#[derive(Clone, Default)]
struct TestJobExecutor {
    run_count: Arc<AtomicUsize>,
}

#[async_trait]
impl JobExecutor for TestJobExecutor {
    async fn execute(
        &self,
        _id: JobId,
        _ctx: &JobExecutionContext,
    ) -> Result<JobExecutionResult, JobExecutionFailure> {
        self.run_count.fetch_add(1, Ordering::SeqCst);

        let art_id = ArtifactId(Uuid::new_v4());
        Ok(JobExecutionResult {
            artifacts: vec![Artifact::new(art_id, ArtifactKind::Json, b"{}".to_vec())],
            logs: vec![(JobTimestamp(100), "Job events run successfully".to_string())],
        })
    }
}

#[test]
fn test_fifo_event_ordering_and_exactly_once() {
    let collector = TestEventCollector::default();
    let id = JobId(Uuid::new_v4());
    
    // 1. Stage JobCreated
    let mut job = Job::new(
        id,
        JobKind::Tool,
        JobPriority::Normal,
        JobOwner::System,
        JobDescription("Event test".to_string()),
        JobTimestamp(10),
        BTreeSet::new(),
    );

    // 2. Stage JobStarted
    let _ = job.start(JobTimestamp(20));

    // 3. Stage LogAppended
    let _ = job.append_log(JobTimestamp(30), "Logging event".to_string());

    // 4. Stage JobCompleted
    let _ = job.complete(JobTimestamp(40));

    // Assert exactly-once draining first call
    let drained_1 = job.drain_events();
    assert_eq!(drained_1.len(), 4);

    for ev in drained_1 {
        collector.publish(ev);
    }

    let published = collector.events.lock().clone();
    assert_eq!(published.len(), 4);

    // Assert strict FIFO order
    assert!(matches!(published[0], DomainEvent::JobCreated { .. }));
    assert!(matches!(published[1], DomainEvent::JobStarted { .. }));
    assert!(matches!(published[2], DomainEvent::LogAppended { .. }));
    assert!(matches!(published[3], DomainEvent::JobCompleted { .. }));

    // Assert exactly-once draining second call (should be empty)
    let drained_2 = job.drain_events();
    assert!(drained_2.is_empty());

    for ev in drained_2 {
        collector.publish(ev);
    }
    
    let published_after = collector.events.lock().clone();
    assert_eq!(published_after.len(), 4); // No new events added
}

#[tokio::test]
async fn test_scheduler_event_integration() {
    let collector = TestEventCollector::default();
    let mock_exec = TestJobExecutor::default();
    let mut registry = JobExecutorRegistry::new();
    registry.register(JobKind::Tool, Arc::new(mock_exec.clone()));

    let scheduler = JobScheduler::new(registry, Arc::new(collector.clone()), 1);

    let job = Job::new(
        JobId(Uuid::new_v4()),
        JobKind::Tool,
        JobPriority::Normal,
        JobOwner::System,
        JobDescription("Worker event".to_string()),
        JobTimestamp(10),
        BTreeSet::new(),
    );
    let job_id = job.id();

    // Submit and wait for completion
    assert!(scheduler.submit(job, BTreeSet::new()).await.is_ok());

    for _ in 0..100 {
        if scheduler.get_job_state(job_id) == Some(JobState::Completed) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(scheduler.get_job_state(job_id), Some(JobState::Completed));

    let events = collector.events.lock().clone();
    
    // Events expected:
    // - JobCreated (on Submit)
    // - JobStarted (on dispatch)
    // - LogAppended (on WorkerFinished log writing)
    // - ArtifactProduced (on WorkerFinished artifact writing)
    // - JobCompleted (on WorkerFinished completion)
    assert!(events.len() >= 5);

    // Verify ordering
    assert!(matches!(events[0], DomainEvent::JobCreated { .. }));
    assert!(matches!(events[1], DomainEvent::JobStarted { .. }));
    
    let has_log = events.iter().any(|e| matches!(e, DomainEvent::LogAppended { .. }));
    let has_art = events.iter().any(|e| matches!(e, DomainEvent::ArtifactProduced { .. }));
    assert!(has_log);
    assert!(has_art);

    assert!(matches!(events.last().unwrap(), DomainEvent::JobCompleted { .. }));
}
