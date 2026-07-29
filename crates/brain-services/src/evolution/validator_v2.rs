//! Pure, side-effect free `PlanValidatorV2` verifying plan safety and dependency integrity (Phase 6 Milestone 6.2).

use crate::evolution::models_v2::{
    EvolutionActionKind, KnowledgeEvolutionPlan, ValidationError, ValidationReport,
};

/// Pure validator executing structural and safety invariant checks over a `KnowledgeEvolutionPlan`.
#[derive(Debug, Clone, Default)]
pub struct PlanValidatorV2;

impl PlanValidatorV2 {
    /// Instantiates a new `PlanValidatorV2`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates `KnowledgeEvolutionPlan` safety without side-effects, returning a `ValidationReport`.
    pub fn validate(&self, plan: &KnowledgeEvolutionPlan) -> ValidationReport {
        let mut errors = Vec::new();

        // 1. Dependency cycle validation
        if let Err(err_msg) = plan.dependency_graph.topological_sort() {
            errors.push(ValidationError {
                code: "CYCLE_DETECTED".to_string(),
                message: err_msg,
            });
        }

        // 2. Identity sanity validation
        for prop in &plan.proposals {
            if let EvolutionActionKind::MergeEntities {
                target_id,
                source_id,
            } = &prop.action
            {
                if target_id == source_id {
                    errors.push(ValidationError {
                        code: "SELF_MERGE_FORBIDDEN".to_string(),
                        message: format!(
                            "Proposal {} attempts to merge entity '{}' into itself",
                            prop.id, target_id
                        ),
                    });
                }
            }
        }

        let is_valid = errors.is_empty();

        ValidationReport { is_valid, errors }
    }
}
