//! Pure domain reasoning planner service for decomposing user prompts into dependency-linked ExecutionPlans.

use crate::errors::DomainError;
use crate::memory::MemoryFilter;
use crate::reasoning::{
    ExecutionPlan, PlanStepComplexity, PlanStepId, ReasoningPlanStep, ReasoningPlanStepKind,
};
use uuid::Uuid;

/// Pure domain service for producing DAG-validated `ExecutionPlan` aggregates from user prompts.
///
/// Invariant: The planner guarantees structural validity (`validate()`), not execution optimization.
pub struct ReasoningPlannerService;

impl ReasoningPlannerService {
    /// Decomposes a user query or command prompt into a structured, dependency-linked `ExecutionPlan`.
    pub fn plan_reasoning(user_query: &str) -> Result<ExecutionPlan, DomainError> {
        let trimmed = user_query.trim();
        let plan_id = format!("plan_{}", Uuid::new_v4().simple());

        let mut steps = Vec::new();

        if let Some(stripped) = trimmed.strip_prefix("/inspect ") {
            let entity_id = stripped.trim().to_string();

            let inspect_step = ReasoningPlanStep::new(
                PlanStepId::new(1),
                ReasoningPlanStepKind::InspectEntity {
                    entity_id: entity_id.clone(),
                },
                format!("Inspect domain entity '{}'", entity_id),
                vec![],
                Some(PlanStepComplexity::Low),
            );

            let traverse_step = ReasoningPlanStep::new(
                PlanStepId::new(2),
                ReasoningPlanStepKind::TraverseRelationships {
                    entity_id: entity_id.clone(),
                },
                format!("Traverse relationship connections for '{}'", entity_id),
                vec![PlanStepId::new(1)],
                Some(PlanStepComplexity::Medium),
            );

            let collect_step = ReasoningPlanStep::new(
                PlanStepId::new(3),
                ReasoningPlanStepKind::CollectEvidence {
                    step_ids: vec![PlanStepId::new(1), PlanStepId::new(2)],
                },
                "Collect inspection and relationship evidence",
                vec![PlanStepId::new(1), PlanStepId::new(2)],
                Some(PlanStepComplexity::Medium),
            );

            let synth_step = ReasoningPlanStep::new(
                PlanStepId::new(4),
                ReasoningPlanStepKind::SynthesizeResponse,
                "Synthesize inspection findings response",
                vec![PlanStepId::new(3)],
                Some(PlanStepComplexity::High),
            );

            steps.push(inspect_step);
            steps.push(traverse_step);
            steps.push(collect_step);
            steps.push(synth_step);
        } else {
            // General query or search prompt
            let search_step = ReasoningPlanStep::new(
                PlanStepId::new(1),
                ReasoningPlanStepKind::Search {
                    query: trimmed.to_string(),
                },
                format!("Search knowledge base for '{}'", trimmed),
                vec![],
                Some(PlanStepComplexity::Low),
            );

            let memory_step = ReasoningPlanStep::new(
                PlanStepId::new(2),
                ReasoningPlanStepKind::QueryMemory {
                    filter: MemoryFilter::Pinned,
                },
                "Query pinned runtime context memories",
                vec![],
                Some(PlanStepComplexity::Low),
            );

            let collect_step = ReasoningPlanStep::new(
                PlanStepId::new(3),
                ReasoningPlanStepKind::CollectEvidence {
                    step_ids: vec![PlanStepId::new(1), PlanStepId::new(2)],
                },
                "Collect and aggregate evidence from search and memory",
                vec![PlanStepId::new(1), PlanStepId::new(2)],
                Some(PlanStepComplexity::Medium),
            );

            let synth_step = ReasoningPlanStep::new(
                PlanStepId::new(4),
                ReasoningPlanStepKind::SynthesizeResponse,
                "Synthesize reasoning evidence into final response",
                vec![PlanStepId::new(3)],
                Some(PlanStepComplexity::High),
            );

            steps.push(search_step);
            steps.push(memory_step);
            steps.push(collect_step);
            steps.push(synth_step);
        }

        ExecutionPlan::new(plan_id, user_query, steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_planner_service_decomposition() {
        let plan = ReasoningPlannerService::plan_reasoning("How does retrieval work?").unwrap();

        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[0].id, PlanStepId::new(1));
        assert_eq!(plan.steps[1].id, PlanStepId::new(2));
        assert_eq!(plan.steps[2].id, PlanStepId::new(3));
        assert_eq!(plan.steps[3].id, PlanStepId::new(4));

        // Verify DAG invariants
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_reasoning_planner_inspect_command_decomposition() {
        let plan =
            ReasoningPlannerService::plan_reasoning("/inspect entity_rust_ownership").unwrap();

        assert_eq!(plan.steps.len(), 4);
        assert!(matches!(
            plan.steps[0].kind,
            ReasoningPlanStepKind::InspectEntity { .. }
        ));
        assert!(plan.validate().is_ok());
    }
}
