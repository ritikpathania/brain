//! Presentation ViewModel for displaying DAG-validated execution plans.

use brain_domain::reasoning::{ExecutionPlan, ReasoningPlanStepKind};

/// Presentation ViewModel for an individual reasoning plan step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningStepViewModel {
    /// Formatted step identifier badge string (e.g. "step-1").
    pub id_badge: String,
    /// Capability-oriented step kind label text.
    pub kind_label: &'static str,
    /// Human-readable step description.
    pub description: String,
    /// Formatted dependency badge strings (e.g. `["step-1", "step-2"]`).
    pub dependency_badges: Vec<String>,
    /// Advisory complexity badge text (e.g. "Low", "Medium", "High").
    pub complexity_badge: &'static str,
}

/// Presentation ViewModel for an entire execution plan DAG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReasoningPlanViewModel {
    /// Unique plan identifier.
    pub plan_id: String,
    /// User query or diagnostic prompt.
    pub user_query: String,
    /// Total step count.
    pub total_steps: usize,
    /// Sequenced presentation step view models.
    pub steps: Vec<ReasoningStepViewModel>,
}

impl ReasoningPlanViewModel {
    /// Constructs a `ReasoningPlanViewModel` from a domain `ExecutionPlan`.
    pub fn from_domain(plan: &ExecutionPlan) -> Self {
        let steps = plan
            .steps
            .iter()
            .map(|step| {
                let kind_label = match step.kind {
                    ReasoningPlanStepKind::Search { .. } => "Search",
                    ReasoningPlanStepKind::QueryMemory { .. } => "Query Memory",
                    ReasoningPlanStepKind::InspectEntity { .. } => "Inspect Entity",
                    ReasoningPlanStepKind::TraverseRelationships { .. } => "Traverse Adjacency",
                    ReasoningPlanStepKind::CollectEvidence { .. } => "Collect Evidence",
                    ReasoningPlanStepKind::SynthesizeResponse => "Synthesize Response",
                };

                let dependency_badges = step
                    .depends_on
                    .iter()
                    .map(|id| format!("step-{}", id.value()))
                    .collect();

                let complexity_badge = step
                    .complexity
                    .as_ref()
                    .map(|c| c.badge_text())
                    .unwrap_or("Standard");

                ReasoningStepViewModel {
                    id_badge: format!("step-{}", step.id.value()),
                    kind_label,
                    description: step.description.clone(),
                    dependency_badges,
                    complexity_badge,
                }
            })
            .collect();

        Self {
            plan_id: plan.id.clone(),
            user_query: plan.user_query.clone(),
            total_steps: plan.steps.len(),
            steps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brain_domain::reasoning::{PlanStepComplexity, PlanStepId, ReasoningPlanStep, ReasoningPlanStepKind};

    #[test]
    fn test_reasoning_plan_view_model_conversion() {
        let step1 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::Search {
                query: "q".to_string(),
            },
            "Search step",
            vec![],
            Some(PlanStepComplexity::Low),
        );
        let step2 = ReasoningPlanStep::new(
            PlanStepId::new(2),
            ReasoningPlanStepKind::SynthesizeResponse,
            "Synthesize step",
            vec![PlanStepId::new(1)],
            Some(PlanStepComplexity::High),
        );

        let plan = ExecutionPlan::new("plan_vm", "Test Query", vec![step1, step2]).unwrap();
        let vm = ReasoningPlanViewModel::from_domain(&plan);

        assert_eq!(vm.total_steps, 2);
        assert_eq!(vm.steps[0].id_badge, "step-1");
        assert_eq!(vm.steps[0].kind_label, "Search");
        assert_eq!(vm.steps[1].dependency_badges, vec!["step-1"]);
        assert_eq!(vm.steps[1].complexity_badge, "High");
    }
}
