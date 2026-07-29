//! Internal runtime orchestration result separate from user-facing presentation DTOs.

use crate::reconciliation::PassDiagnostic;
use crate::reflection::contracts::{ReflectionExecutionMode, TaskReport};
use brain_domain::EntityId;
use std::time::Duration;

/// Internal execution result returned by the `ReflectionSupervisor` after executing a plan.
#[derive(Debug, Clone, PartialEq)]
pub struct ReflectionResult {
    /// Plan execution identifier.
    pub plan_id: String,
    /// Execution mode used for triggering.
    pub execution_mode: ReflectionExecutionMode,
    /// IDs of entities modified during execution.
    pub modified_entity_ids: Vec<EntityId>,
    /// Individual task reports generated per DAG stage.
    pub task_reports: Vec<TaskReport>,
    /// Accumulated runtime diagnostics.
    pub diagnostics: Vec<PassDiagnostic>,
    /// Total execution duration.
    pub total_duration: Duration,
}

impl ReflectionResult {
    /// Returns true if any structural or state changes were applied to the graph.
    pub fn has_changes(&self) -> bool {
        !self.modified_entity_ids.is_empty()
    }
}
