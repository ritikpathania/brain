use brain_domain::{
    ArtifactId, ArtifactKind, DomainEvent, Job, JobCapability, JobDescription, JobError,
    JobFailureReason, JobId, JobKind, JobOwner, JobPriority, JobProgress, JobState, JobTimestamp,
    LogEntryId, ProgressUnit,
};
use std::collections::BTreeSet;
use std::num::NonZeroU64;
use uuid::Uuid;

fn create_test_job() -> Job {
    let id = JobId(Uuid::new_v4());
    let mut capabilities = BTreeSet::new();
    capabilities.insert(JobCapability::Cancelable);
    capabilities.insert(JobCapability::ProducesLogs);

    Job::new(
        id,
        JobKind::Indexing,
        JobPriority::Normal,
        JobOwner::System,
        JobDescription("Test Indexing Job".to_string()),
        JobTimestamp(100),
        capabilities,
    )
}

#[test]
fn test_successful_creation() {
    let mut job = create_test_job();
    assert_eq!(job.state(), JobState::Pending);
    assert_eq!(job.created_at(), JobTimestamp(100));
    assert_eq!(job.started_at(), None);
    assert_eq!(job.finished_at(), None);

    let events = job.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], DomainEvent::JobCreated { .. }));
}

#[test]
fn test_valid_transitions_and_event_emissions() {
    let mut job = create_test_job();
    job.drain_events(); // Clear creation event

    // 1. Pending -> Running
    assert!(job.start(JobTimestamp(101)).is_ok());
    assert_eq!(job.state(), JobState::Running);
    assert_eq!(job.started_at(), Some(JobTimestamp(101)));
    let events = job.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], DomainEvent::JobStarted { .. }));

    // 2. Running -> Waiting
    assert!(job.wait("Approval".to_string()).is_ok());
    assert_eq!(job.state(), JobState::Waiting);
    let events = job.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], DomainEvent::JobWaiting { .. }));

    // 3. Waiting -> Running
    assert!(job.start(JobTimestamp(102)).is_ok());
    assert_eq!(job.state(), JobState::Running);
    let events = job.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], DomainEvent::JobStarted { .. }));

    // 4. Running -> Completed
    assert!(job.complete(JobTimestamp(103)).is_ok());
    assert_eq!(job.state(), JobState::Completed);
    assert_eq!(job.finished_at(), Some(JobTimestamp(103)));
    let events = job.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], DomainEvent::JobCompleted { .. }));
}

#[test]
fn test_invalid_transitions() {
    let mut job = create_test_job();

    // Pending -> Completed (Invalid)
    let res = job.complete(JobTimestamp(105));
    assert_eq!(
        res,
        Err(JobError::InvalidTransition {
            from: JobState::Pending,
            to: JobState::Completed
        })
    );

    // Pending -> Running (Valid)
    assert!(job.start(JobTimestamp(105)).is_ok());

    // Transition to self (Running -> Running is Invalid)
    let res = job.start(JobTimestamp(106));
    assert_eq!(
        res,
        Err(JobError::InvalidTransition {
            from: JobState::Running,
            to: JobState::Running
        })
    );
}

#[test]
fn test_terminal_state_mutations_rejected() {
    let mut job = create_test_job();
    assert!(job.start(JobTimestamp(101)).is_ok());
    assert!(job.complete(JobTimestamp(102)).is_ok());
    assert_eq!(job.state(), JobState::Completed);

    // 1. Any transition from completed is rejected
    assert_eq!(
        job.start(JobTimestamp(103)),
        Err(JobError::InvalidTransition {
            from: JobState::Completed,
            to: JobState::Running
        })
    );

    // 2. Logging on completed is rejected
    assert_eq!(
        job.append_log(JobTimestamp(103), "Failed log".to_string()),
        Err(JobError::TerminalStateMutation {
            state: JobState::Completed
        })
    );

    // 3. Artifact production on completed is rejected
    assert_eq!(
        job.produce_artifact(ArtifactId(Uuid::new_v4()), ArtifactKind::Json, vec![]),
        Err(JobError::TerminalStateMutation {
            state: JobState::Completed
        })
    );

    // 4. Progress update on completed is rejected
    assert_eq!(
        job.update_progress(JobProgress::Indeterminate),
        Err(JobError::TerminalStateMutation {
            state: JobState::Completed
        })
    );
}

#[test]
fn test_append_only_logs_and_artifacts() {
    let mut job = create_test_job();
    assert!(job.start(JobTimestamp(101)).is_ok());

    // Log append
    assert!(job
        .append_log(JobTimestamp(102), "Step 1".to_string())
        .is_ok());
    assert_eq!(job.logs().len(), 1);
    assert_eq!(job.logs()[0].id, LogEntryId(NonZeroU64::new(1).unwrap()));
    assert_eq!(job.logs()[0].message, "Step 1");

    assert!(job
        .append_log(JobTimestamp(103), "Step 2".to_string())
        .is_ok());
    assert_eq!(job.logs().len(), 2);
    assert_eq!(job.logs()[1].id, LogEntryId(NonZeroU64::new(2).unwrap()));

    // Artifact production
    let art_id = ArtifactId(Uuid::new_v4());
    assert!(job
        .produce_artifact(art_id, ArtifactKind::Json, b"{}".to_vec())
        .is_ok());
    assert_eq!(job.artifacts().len(), 1);
    assert_eq!(job.artifacts()[0].id, art_id);
    assert_eq!(job.artifacts()[0].kind, ArtifactKind::Json);
}

#[test]
fn test_determinate_progress() {
    let mut job = create_test_job();
    assert!(job.start(JobTimestamp(101)).is_ok());

    let progress = JobProgress::Determinate {
        completed: 5,
        total: 10,
        unit: ProgressUnit::Files,
    };
    assert!(job.update_progress(progress).is_ok());
    assert_eq!(job.progress(), progress);

    let events = job.drain_events();
    assert!(events
        .iter()
        .any(|e| matches!(e, DomainEvent::JobProgressed { .. })));
}

#[test]
fn test_job_failure_transition() {
    let mut job = create_test_job();
    assert!(job.start(JobTimestamp(101)).is_ok());

    let reason = JobFailureReason("Out of disk space".to_string());
    assert!(job.fail(reason.clone(), JobTimestamp(102)).is_ok());
    assert_eq!(job.state(), JobState::Failed);
    assert_eq!(job.finished_at(), Some(JobTimestamp(102)));

    let events = job.drain_events();
    assert!(events.iter().any(|e| match e {
        DomainEvent::JobFailed { reason: r, .. } => r == &reason,
        _ => false,
    }));
}
