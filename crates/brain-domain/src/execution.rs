//! Execution state models, state machine transitions, and lightweight event facts.

use crate::cursor::ExecutionCursor;
use crate::errors::DomainError;
use crate::reasoning::PlanStepId;
use crate::value::StructuredValue;
use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;
use uuid::Uuid;

/// Strongly-typed identifier for an execution run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExecutionId(pub Uuid);

impl ExecutionId {
    /// Instantiates a new unique `ExecutionId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exec-{}", self.0.simple())
    }
}

/// Strongly-typed timestamp for execution events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct ExecutionTimestamp(pub SystemTime);

impl ExecutionTimestamp {
    /// Returns the current system timestamp.
    pub fn now() -> Self {
        Self(SystemTime::now())
    }

    /// Returns elapsed Unix timestamp in milliseconds.
    pub fn as_unix_millis(&self) -> u64 {
        self.0
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

impl Default for ExecutionTimestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for ExecutionTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_unix_millis())
    }
}

/// Descriptive classification for why a plan step was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SkippedReason {
    /// Prerequisite step failed.
    UpstreamFailure,
    /// Execution run was cancelled.
    Cancelled,
    /// Conditional branch was not taken.
    ConditionalBranch,
}

impl fmt::Display for SkippedReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpstreamFailure => write!(f, "Upstream Failure"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::ConditionalBranch => write!(f, "Conditional Branch"),
        }
    }
}

/// Discrete lifecycle state of a plan step during execution.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StepStatus {
    /// Prerequisite dependencies are incomplete.
    Pending,
    /// Prerequisite dependencies complete; step is runnable.
    Ready,
    /// Step execution worker task is running.
    Running,
    /// Step completed successfully.
    Completed,
    /// Step failed during execution.
    Failed,
    /// Step was skipped due to upstream conditions.
    Skipped(SkippedReason),
}

/// Capability-neutral step execution output payload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StepOutput {
    /// Canonical domain structured data.
    pub value: StructuredValue,
}

impl StepOutput {
    /// Creates a new `StepOutput` wrapping a `StructuredValue`.
    pub fn new(value: StructuredValue) -> Self {
        Self { value }
    }
}

/// Prerequisite artifact references passed to a step executor as input context.
#[derive(Debug, Clone, Default)]
pub struct StepInputs {
    /// List of prerequisite artifact IDs produced by prerequisite steps.
    pub prerequisite_ids: Vec<crate::artifact::EvidenceArtifactId>,
}

impl StepInputs {
    /// Creates empty `StepInputs`.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Lightweight, append-only immutable execution facts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExecutionEvent {
    /// Step execution worker task started.
    StepStarted {
        /// Execution run ID.
        execution_id: ExecutionId,
        /// Plan step ID.
        step_id: PlanStepId,
        /// Event occurrence timestamp.
        occurred_at: ExecutionTimestamp,
    },
    /// Step execution completed.
    StepCompleted {
        /// Execution run ID.
        execution_id: ExecutionId,
        /// Plan step ID.
        step_id: PlanStepId,
        /// Event occurrence timestamp.
        occurred_at: ExecutionTimestamp,
    },
    /// Step execution failed.
    StepFailed {
        /// Execution run ID.
        execution_id: ExecutionId,
        /// Plan step ID.
        step_id: PlanStepId,
        /// Diagnostic domain error.
        error: DomainError,
        /// Event occurrence timestamp.
        occurred_at: ExecutionTimestamp,
    },
    /// Step execution skipped.
    StepSkipped {
        /// Execution run ID.
        execution_id: ExecutionId,
        /// Plan step ID.
        step_id: PlanStepId,
        /// Reason for skipping.
        reason: SkippedReason,
        /// Event occurrence timestamp.
        occurred_at: ExecutionTimestamp,
    },
    /// Execution plan completed successfully.
    PlanCompleted {
        /// Execution run ID.
        execution_id: ExecutionId,
        /// Event occurrence timestamp.
        occurred_at: ExecutionTimestamp,
    },
    /// Execution plan failed.
    PlanFailed {
        /// Execution run ID.
        execution_id: ExecutionId,
        /// Diagnostic domain error.
        error: DomainError,
        /// Event occurrence timestamp.
        occurred_at: ExecutionTimestamp,
    },
}

/// Dynamic runtime state container tracking cursor, statuses, outputs, and errors.
#[derive(Debug, Clone, Default)]
pub struct ExecutionState {
    /// Progress scheduler cursor.
    pub cursor: ExecutionCursor,
    /// Encapsulated evidence artifact store and provenance graph.
    pub artifact_store: crate::artifact_store::ArtifactStore,
    /// Map of step IDs to lifecycle statuses.
    statuses: HashMap<PlanStepId, StepStatus>,
    /// Map of completed step IDs to output payloads (only populated for Completed steps).
    outputs: HashMap<PlanStepId, StepOutput>,
    /// Map of failed step IDs to domain errors.
    errors: HashMap<PlanStepId, DomainError>,
}

impl ExecutionState {
    /// Instantiates a new `ExecutionState`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the lifecycle status for a step, defaulting to `StepStatus::Pending`.
    pub fn status(&self, step_id: PlanStepId) -> StepStatus {
        self.statuses.get(&step_id).cloned().unwrap_or(StepStatus::Pending)
    }

    /// Returns the output payload for a completed step.
    pub fn output(&self, step_id: PlanStepId) -> Option<&StepOutput> {
        self.outputs.get(&step_id)
    }

    /// Returns the domain error for a failed step.
    pub fn error(&self, step_id: PlanStepId) -> Option<&DomainError> {
        self.errors.get(&step_id)
    }

    /// Returns all completed step IDs.
    pub fn completed_steps(&self) -> Vec<PlanStepId> {
        self.cursor.completed.iter().cloned().collect()
    }

    /// Returns all failed step IDs.
    pub fn failed_steps(&self) -> Vec<PlanStepId> {
        self.cursor.failed.iter().cloned().collect()
    }

    /// Returns all skipped step IDs.
    pub fn skipped_steps(&self) -> Vec<PlanStepId> {
        self.cursor.skipped.iter().cloned().collect()
    }

    /// Enforces state machine transition invariants.
    ///
    /// Allowed paths:
    /// - `Pending` -> `Ready`
    /// - `Pending` / `Ready` -> `Running`
    /// - `Running` -> `Completed`
    /// - `Running` -> `Failed`
    /// - `Pending` / `Ready` -> `Skipped(reason)`
    ///
    /// Illegal rewinds (e.g. `Running` -> `Ready`, `Completed` -> `Running`) are rejected.
    pub fn transition(&mut self, step_id: PlanStepId, next_status: StepStatus) -> Result<(), DomainError> {
        let current = self.status(step_id);

        let valid = match (&current, &next_status) {
            (StepStatus::Pending, StepStatus::Ready) => true,
            (StepStatus::Pending | StepStatus::Ready, StepStatus::Running) => true,
            (StepStatus::Running, StepStatus::Completed) => true,
            (StepStatus::Running, StepStatus::Failed) => true,
            (StepStatus::Pending | StepStatus::Ready, StepStatus::Skipped(_)) => true,
            // Self-transitions are idempotent
            (a, b) if a == b => true,
            _ => false,
        };

        if !valid {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Illegal state machine transition for step {}: {:?} -> {:?}",
                    step_id, current, next_status
                ),
                rule_id: Some("VAL-EXEC-001".to_string()),
            });
        }

        self.statuses.insert(step_id, next_status.clone());

        match next_status {
            StepStatus::Running => {
                self.cursor.mark_in_flight(step_id);
            }
            StepStatus::Completed => {
                self.cursor.mark_completed(step_id);
            }
            StepStatus::Failed => {
                self.cursor.mark_failed(step_id);
            }
            StepStatus::Skipped(_) => {
                self.cursor.mark_skipped(step_id);
            }
            _ => {}
        }

        Ok(())
    }

    /// Stores the output payload for a completed step.
    pub fn set_output(&mut self, step_id: PlanStepId, output: StepOutput) -> Result<(), DomainError> {
        if self.status(step_id) != StepStatus::Completed {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Cannot set output for step {} in non-Completed status {:?}",
                    step_id,
                    self.status(step_id)
                ),
                rule_id: Some("VAL-EXEC-002".to_string()),
            });
        }
        self.outputs.insert(step_id, output);
        Ok(())
    }

    /// Stores error details for a failed step.
    pub fn set_error(&mut self, step_id: PlanStepId, error: DomainError) -> Result<(), DomainError> {
        if self.status(step_id) != StepStatus::Failed {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Cannot set error for step {} in non-Failed status {:?}",
                    step_id,
                    self.status(step_id)
                ),
                rule_id: Some("VAL-EXEC-003".to_string()),
            });
        }
        self.errors.insert(step_id, error);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_state_valid_transitions() {
        let mut state = ExecutionState::new();
        let step_id = PlanStepId::new(1);

        assert_eq!(state.status(step_id), StepStatus::Pending);

        assert!(state.transition(step_id, StepStatus::Ready).is_ok());
        assert_eq!(state.status(step_id), StepStatus::Ready);

        assert!(state.transition(step_id, StepStatus::Running).is_ok());
        assert_eq!(state.status(step_id), StepStatus::Running);

        assert!(state.transition(step_id, StepStatus::Completed).is_ok());
        assert_eq!(state.status(step_id), StepStatus::Completed);

        assert!(state
            .set_output(step_id, StepOutput::new(StructuredValue::String("ok".to_string())))
            .is_ok());
        assert_eq!(
            state.output(step_id).unwrap().value,
            StructuredValue::String("ok".to_string())
        );
    }

    #[test]
    fn test_execution_state_rewind_from_running_rejected() {
        let mut state = ExecutionState::new();
        let step_id = PlanStepId::new(1);

        state.transition(step_id, StepStatus::Running).unwrap();

        // Rewinding from Running to Ready must be rejected by the state machine
        let err = state.transition(step_id, StepStatus::Ready);
        assert!(err.is_err());
    }

    #[test]
    fn test_execution_state_set_output_on_non_completed_rejected() {
        let mut state = ExecutionState::new();
        let step_id = PlanStepId::new(1);

        let err = state.set_output(step_id, StepOutput::new(StructuredValue::Null));
        assert!(err.is_err());
    }
}
