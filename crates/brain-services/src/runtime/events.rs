#![allow(missing_docs)]

use crate::runtime::models::*;
use brain_domain::jobs::JobId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNo(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExecutionVersion(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionEventPayload {
    ExecutionCreated {
        header: ExecutionHeader,
    },
    ExecutionEnqueued,
    ExecutionBegan,
    ExecutionCheckpointCreated {
        checkpoint_id: String,
        journal_sequence: SequenceNo,
    },
    ExecutionPaused,
    ExecutionResumed,
    ExecutionRecovering,
    ExecutionRecovered,
    ExecutionCompleted,
    ExecutionFailed {
        reason: String,
    },
    ExecutionCancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEventPayload {
    TaskCreated {
        task_id: TaskId,
        job_id: JobId,
        priority: u32,
    },
    TaskDependencySatisfied {
        task_id: TaskId,
    },
    TaskBecameReady {
        task_id: TaskId,
    },
    TaskLeased {
        task_id: TaskId,
        worker_id: String,
        lease_until: u64,
    },
    LeaseExpired {
        task_id: TaskId,
        worker_id: String,
    },
    TaskBegan {
        task_id: TaskId,
        worker_id: String,
    },
    TaskHeartbeat {
        task_id: TaskId,
        timestamp: u64,
    },
    TaskCheckpointCreated {
        task_id: TaskId,
        checkpoint_id: String,
    },
    TaskCompleted {
        task_id: TaskId,
        output_ref: String,
    },
    TaskSkipped {
        task_id: TaskId,
        reason: String,
    },
    TaskRetryScheduled {
        task_id: TaskId,
        attempt: u32,
        retry_at: u64,
    },
    TaskFailed {
        task_id: TaskId,
        error: String,
    },
    TaskCancelled {
        task_id: TaskId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JournalPayload {
    Execution(ExecutionEventPayload),
    Task(TaskEventPayload),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEvent {
    pub sequence_no: SequenceNo,
    pub execution_id: ExecutionId,
    pub version: ExecutionVersion,
    pub occurred_at: u64,
    pub payload: JournalPayload,
}
