//! Pure, deterministic `GoalValidator` evaluating `TaskPlan` safety and graph invariants (Phase 7 Milestone 7.1).

use crate::planning::models::{
    PlanningValidationError, PlanningValidationKind, PlanningValidationReport, TaskPlan,
};
use std::collections::HashSet;

/// Pure validator evaluating compiled `TaskPlan` safety invariants.
#[derive(Debug, Clone, Default)]
pub struct GoalValidator;

impl GoalValidator {
    /// Instantiates a new `GoalValidator`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates `TaskPlan` safety without side-effects, returning a `PlanningValidationReport`.
    pub fn validate(&self, plan: &TaskPlan) -> PlanningValidationReport {
        let mut errors = Vec::new();

        // 1. Dependency cycle validation
        if let Err(err_msg) = plan.task_graph.topological_sort() {
            errors.push(PlanningValidationError {
                kind: PlanningValidationKind::DependencyCycle,
                task_id: None,
                details: err_msg,
            });
        }

        // 2. Duplicate task ID check
        let mut seen_ids = HashSet::new();
        for node in &plan.task_graph.nodes {
            if !seen_ids.insert(node.task_id) {
                errors.push(PlanningValidationError {
                    kind: PlanningValidationKind::DuplicateTask,
                    task_id: Some(node.task_id),
                    details: format!("Duplicate task ID '{}' in TaskGraph", node.task_id),
                });
            }
        }

        let is_valid = errors.is_empty();

        PlanningValidationReport { is_valid, errors }
    }
}
