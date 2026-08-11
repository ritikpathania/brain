//! Stateful execution records and transactional rollback domain models.

use super::proposal::ProposalId;
use serde::{Deserialize, Serialize};

/// Structured result outcome of an evolution proposal execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionResult {
    /// All actions executed cleanly.
    Success,
    /// Partial completion with action metrics.
    PartialSuccess {
        /// Count of actions completed.
        completed_actions: usize,
        /// Count of actions failed.
        failed_actions: usize,
    },
    /// Complete execution failure.
    Failed {
        /// Narrative failure reason.
        reason: String,
    },
    /// Transactional rollback executed.
    RolledBack,
}

/// Opaque newtype identifier for an evolution execution record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvolutionExecutionId(pub uuid::Uuid);

impl Default for EvolutionExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl EvolutionExecutionId {
    /// Generates a new random EvolutionExecutionId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for EvolutionExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exec-{}", self.0)
    }
}

/// Transactional execution record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionExecution {
    /// Unique execution identifier.
    pub id: EvolutionExecutionId,
    /// Associated proposal identifier.
    pub proposal_id: ProposalId,
    /// Execution result outcome.
    pub result: ExecutionResult,
    /// Execution event log entries.
    pub event_log: Vec<String>,
}

impl EvolutionExecution {
    /// Creates a new successful EvolutionExecution.
    pub fn new_success(proposal_id: ProposalId) -> Self {
        Self {
            id: EvolutionExecutionId::new(),
            proposal_id,
            result: ExecutionResult::Success,
            event_log: vec![
                "Execution started".to_string(),
                "All actions applied cleanly".to_string(),
            ],
        }
    }

    /// Reverts the execution via transactional rollback.
    pub fn rollback(&mut self) {
        self.result = ExecutionResult::RolledBack;
        self.event_log
            .push("Transactional rollback executed".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_rollback_lifecycle() {
        let prop_id = ProposalId::new();
        let mut exec = EvolutionExecution::new_success(prop_id);
        assert_eq!(exec.result, ExecutionResult::Success);

        exec.rollback();
        assert_eq!(exec.result, ExecutionResult::RolledBack);
        assert!(exec
            .event_log
            .contains(&"Transactional rollback executed".to_string()));
    }
}
