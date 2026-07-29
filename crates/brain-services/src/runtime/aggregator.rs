#![allow(missing_docs)]

use crate::runtime::events::*;
use crate::runtime::models::*;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AggregatorError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: ExecutionFsmState,
        to: ExecutionFsmState,
    },
    #[error("Version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u64, got: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSnapshot {
    pub task_id: TaskId,
    pub status: TaskFsmState,
    pub lease_owner: Option<String>,
    pub lease_until: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionProjection {
    pub header: ExecutionHeader,
    pub status: ExecutionFsmState,
    pub version: ExecutionVersion,
    pub last_sequence: SequenceNo,
    pub tasks: HashMap<TaskId, TaskSnapshot>,
}

pub struct ExecutionAggregator {
    projection: ExecutionProjection,
}

impl ExecutionAggregator {
    pub fn new(header: ExecutionHeader) -> Self {
        Self {
            projection: ExecutionProjection {
                header,
                status: ExecutionFsmState::Created,
                version: ExecutionVersion(0),
                last_sequence: SequenceNo(0),
                tasks: HashMap::new(),
            },
        }
    }

    pub fn apply(&mut self, event: &JournalEvent) -> Result<(), AggregatorError> {
        let expected_ver = self.projection.version.0 + 1;
        if event.version.0 != expected_ver {
            return Err(AggregatorError::VersionMismatch {
                expected: expected_ver,
                got: event.version.0,
            });
        }

        self.projection.version = event.version;
        self.projection.last_sequence = event.sequence_no;

        match &event.payload {
            JournalPayload::Execution(exec_payload) => match exec_payload {
                ExecutionEventPayload::ExecutionCreated { header } => {
                    self.projection.header = header.clone();
                    self.projection.status = ExecutionFsmState::Created;
                }
                ExecutionEventPayload::ExecutionEnqueued => {
                    self.transition_execution(ExecutionFsmState::Queued)?;
                }
                ExecutionEventPayload::ExecutionBegan => {
                    self.transition_execution(ExecutionFsmState::Running)?;
                }
                ExecutionEventPayload::ExecutionRecovering => {
                    self.transition_execution(ExecutionFsmState::Recovering)?;
                }
                ExecutionEventPayload::ExecutionRecovered => {
                    self.transition_execution(ExecutionFsmState::Running)?;
                }
                ExecutionEventPayload::ExecutionCompleted => {
                    self.transition_execution(ExecutionFsmState::Completed)?;
                }
                ExecutionEventPayload::ExecutionFailed { .. } => {
                    self.transition_execution(ExecutionFsmState::Failed)?;
                }
                ExecutionEventPayload::ExecutionCancelled => {
                    self.transition_execution(ExecutionFsmState::Cancelled)?;
                }
                _ => {}
            },
            JournalPayload::Task(task_payload) => match task_payload {
                TaskEventPayload::TaskCreated { task_id, .. } => {
                    self.projection.tasks.insert(
                        *task_id,
                        TaskSnapshot {
                            task_id: *task_id,
                            status: TaskFsmState::Created,
                            lease_owner: None,
                            lease_until: None,
                        },
                    );
                }
                TaskEventPayload::TaskBecameReady { task_id } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Ready;
                    }
                }
                TaskEventPayload::TaskLeased {
                    task_id,
                    worker_id,
                    lease_until,
                } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Leased;
                        t.lease_owner = Some(worker_id.clone());
                        t.lease_until = Some(*lease_until);
                    }
                }
                TaskEventPayload::TaskCompleted { task_id, .. } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Completed;
                    }
                }
                TaskEventPayload::TaskSkipped { task_id, .. } => {
                    if let Some(t) = self.projection.tasks.get_mut(task_id) {
                        t.status = TaskFsmState::Skipped;
                    }
                }
                _ => {}
            },
        }

        Ok(())
    }

    fn transition_execution(&mut self, next: ExecutionFsmState) -> Result<(), AggregatorError> {
        if self.projection.status.can_transition_to(next) {
            self.projection.status = next;
            Ok(())
        } else {
            Err(AggregatorError::InvalidStateTransition {
                from: self.projection.status,
                to: next,
            })
        }
    }

    pub fn projection(&self) -> &ExecutionProjection {
        &self.projection
    }
}
