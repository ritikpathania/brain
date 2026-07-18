//! Background jobs domain models, state machine, and invariants.

mod models;
pub use models::*;

use crate::events::DomainEvent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// Lifecycle state machine phase of a background job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Initial queued state, waiting to run.
    Pending,
    /// Actively executing.
    Running,
    /// Blocked/waiting for approval or external signal.
    Waiting,
    /// Finished successfully.
    Completed,
    /// Finished due to unrecoverable failure.
    Failed,
    /// Finished due to cancellation request.
    Cancelled,
}

impl JobState {
    /// Returns true if transitioning from `self` to `next` is allowed by invariants.
    pub fn can_transition_to(self, next: Self) -> bool {
        match (self, next) {
            (JobState::Pending, JobState::Running) => true,
            (JobState::Pending, JobState::Cancelled) => true,
            (JobState::Running, JobState::Waiting) => true,
            (JobState::Running, JobState::Completed) => true,
            (JobState::Running, JobState::Failed) => true,
            (JobState::Running, JobState::Cancelled) => true,
            (JobState::Waiting, JobState::Running) => true,
            (JobState::Waiting, JobState::Cancelled) => true,
            _ => false,
        }
    }
}

/// Domain error type representing aggregate constraint failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JobError {
    /// State machine transition invariant was violated.
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition {
        /// Starting state.
        from: JobState,
        /// Requested next state.
        to: JobState,
    },
    /// Invalid mutation attempted on a job in a terminal state.
    #[error("Cannot mutate job aggregate in terminal state {state:?}")]
    TerminalStateMutation {
        /// The terminal state of the job.
        state: JobState,
    },
}

/// Aggregate root representing a background job unit of work.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    /// Immutable unique identifier.
    id: JobId,
    /// Categorical category of task.
    kind: JobKind,
    /// Current lifecycle state.
    state: JobState,
    /// Execution precedence.
    priority: JobPriority,
    /// Scope context owner.
    owner: JobOwner,
    /// Short text description of the task.
    description: JobDescription,
    /// Creation time.
    created_at: JobTimestamp,
    /// Start execution time.
    started_at: Option<JobTimestamp>,
    /// Terminal state execution finished time.
    finished_at: Option<JobTimestamp>,
    /// Step completion progress.
    progress: JobProgress,
    /// Enabled capability flags.
    capabilities: BTreeSet<JobCapability>,
    /// Child job dependencies/composites.
    child_jobs: Vec<JobId>,
    /// Diagnostic trace log output.
    logs: Vec<LogEntry>,
    /// Output items generated.
    artifacts: Vec<Artifact>,
    /// Staged domain events.
    events: Vec<DomainEvent>,
}

impl Job {
    /// Instantiate a new `Job` in the `Pending` state.
    pub fn new(
        id: JobId,
        kind: JobKind,
        priority: JobPriority,
        owner: JobOwner,
        description: JobDescription,
        created_at: JobTimestamp,
        capabilities: BTreeSet<JobCapability>,
    ) -> Self {
        let mut job = Self {
            id,
            kind,
            state: JobState::Pending,
            priority,
            owner: owner.clone(),
            description,
            created_at,
            started_at: None,
            finished_at: None,
            progress: JobProgress::Indeterminate,
            capabilities,
            child_jobs: Vec::new(),
            logs: Vec::new(),
            artifacts: Vec::new(),
            events: Vec::new(),
        };
        job.events.push(DomainEvent::JobCreated {
            job_id: id,
            kind,
            priority,
            owner,
        });
        job
    }

    /// Read-only identifier.
    pub fn id(&self) -> JobId {
        self.id
    }

    /// Read-only category kind.
    pub fn kind(&self) -> JobKind {
        self.kind
    }

    /// Read-only lifecycle phase.
    pub fn state(&self) -> JobState {
        self.state
    }

    /// Read-only priority tier.
    pub fn priority(&self) -> JobPriority {
        self.priority
    }

    /// Read-only owner metadata.
    pub fn owner(&self) -> &JobOwner {
        &self.owner
    }

    /// Read-only descriptive summary.
    pub fn description(&self) -> &JobDescription {
        &self.description
    }

    /// Read-only creation timestamp.
    pub fn created_at(&self) -> JobTimestamp {
        self.created_at
    }

    /// Read-only start execution timestamp.
    pub fn started_at(&self) -> Option<JobTimestamp> {
        self.started_at
    }

    /// Read-only finished execution timestamp.
    pub fn finished_at(&self) -> Option<JobTimestamp> {
        self.finished_at
    }

    /// Read-only progress description.
    pub fn progress(&self) -> JobProgress {
        self.progress
    }

    /// Read-only capabilities list.
    pub fn capabilities(&self) -> &BTreeSet<JobCapability> {
        &self.capabilities
    }

    /// Read-only children relationships list.
    pub fn child_jobs(&self) -> &[JobId] {
        &self.child_jobs
    }

    /// Read-only log tracing list.
    pub fn logs(&self) -> &[LogEntry] {
        &self.logs
    }

    /// Read-only produced artifacts list.
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// Drains and clears staged events.
    pub fn drain_events(&mut self) -> Vec<DomainEvent> {
        std::mem::take(&mut self.events)
    }

    /// Helper to transition job state if valid.
    fn transition_to(
        &mut self,
        next: JobState,
        timestamp: Option<JobTimestamp>,
    ) -> Result<(), JobError> {
        if !self.state.can_transition_to(next) {
            return Err(JobError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        if next == JobState::Running && self.started_at.is_none() {
            self.started_at = timestamp;
        }
        if is_terminal(next) {
            self.finished_at = timestamp;
        }
        Ok(())
    }

    /// Mark the job as running.
    pub fn start(&mut self, timestamp: JobTimestamp) -> Result<(), JobError> {
        self.transition_to(JobState::Running, Some(timestamp))?;
        self.events.push(DomainEvent::JobStarted {
            job_id: self.id,
            timestamp,
        });
        Ok(())
    }

    /// Update execution progress.
    pub fn update_progress(&mut self, progress: JobProgress) -> Result<(), JobError> {
        if is_terminal(self.state) {
            return Err(JobError::TerminalStateMutation { state: self.state });
        }
        self.progress = progress;
        self.events.push(DomainEvent::JobProgressed {
            job_id: self.id,
            progress,
        });
        Ok(())
    }

    /// Transition to waiting state.
    pub fn wait(&mut self, reason: String) -> Result<(), JobError> {
        self.transition_to(JobState::Waiting, None)?;
        self.events.push(DomainEvent::JobWaiting {
            job_id: self.id,
            reason,
        });
        Ok(())
    }

    /// Transition to completed state.
    pub fn complete(&mut self, timestamp: JobTimestamp) -> Result<(), JobError> {
        self.transition_to(JobState::Completed, Some(timestamp))?;
        self.events.push(DomainEvent::JobCompleted {
            job_id: self.id,
            timestamp,
        });
        Ok(())
    }

    /// Transition to failed state.
    pub fn fail(
        &mut self,
        reason: JobFailureReason,
        timestamp: JobTimestamp,
    ) -> Result<(), JobError> {
        self.transition_to(JobState::Failed, Some(timestamp))?;
        self.events.push(DomainEvent::JobFailed {
            job_id: self.id,
            reason,
            timestamp,
        });
        Ok(())
    }

    /// Transition to cancelled state.
    pub fn cancel(&mut self, timestamp: JobTimestamp) -> Result<(), JobError> {
        self.transition_to(JobState::Cancelled, Some(timestamp))?;
        self.events.push(DomainEvent::JobCancelled {
            job_id: self.id,
            timestamp,
        });
        Ok(())
    }

    /// Append log trace to log history.
    pub fn append_log(&mut self, timestamp: JobTimestamp, message: String) -> Result<(), JobError> {
        if is_terminal(self.state) {
            return Err(JobError::TerminalStateMutation { state: self.state });
        }
        let next_idx = (self.logs.len() + 1) as u64;
        let entry_id = LogEntryId(std::num::NonZeroU64::new(next_idx).unwrap());
        self.logs
            .push(LogEntry::new(entry_id, timestamp, message.clone()));
        self.events.push(DomainEvent::LogAppended {
            job_id: self.id,
            entry_id,
            message,
        });
        Ok(())
    }

    /// Produce and write output artifact.
    pub fn produce_artifact(
        &mut self,
        id: ArtifactId,
        kind: ArtifactKind,
        payload: Vec<u8>,
    ) -> Result<(), JobError> {
        if is_terminal(self.state) {
            return Err(JobError::TerminalStateMutation { state: self.state });
        }
        self.artifacts.push(Artifact::new(id, kind, payload));
        self.events.push(DomainEvent::ArtifactProduced {
            job_id: self.id,
            artifact_id: id,
            kind,
        });
        Ok(())
    }

    /// Associate child sub-task dependencies.
    pub fn add_child_job(&mut self, child_id: JobId) -> Result<(), JobError> {
        if is_terminal(self.state) {
            return Err(JobError::TerminalStateMutation { state: self.state });
        }
        self.child_jobs.push(child_id);
        Ok(())
    }
}

fn is_terminal(state: JobState) -> bool {
    matches!(
        state,
        JobState::Completed | JobState::Failed | JobState::Cancelled
    )
}
