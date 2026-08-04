//! ExecutionCursor service helper for dynamic plan execution tracking and scheduling.

use crate::reasoning::{ExecutionPlan, PlanStepId, ReasoningPlanStep};
use std::collections::HashSet;

/// State-only tracking helper for monitoring plan progress and resolving executable steps.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionCursor {
    /// Set of plan step IDs that have completed successfully.
    pub completed: HashSet<PlanStepId>,
    /// Set of plan step IDs that failed during execution.
    pub failed: HashSet<PlanStepId>,
    /// Set of plan step IDs that were skipped.
    pub skipped: HashSet<PlanStepId>,
    /// Set of plan step IDs currently in-flight.
    pub in_flight: HashSet<PlanStepId>,
}

impl ExecutionCursor {
    /// Instantiates a new, empty `ExecutionCursor`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a plan step as currently in-flight.
    pub fn mark_in_flight(&mut self, id: PlanStepId) {
        self.in_flight.insert(id);
    }

    /// Marks a plan step as successfully completed.
    pub fn mark_completed(&mut self, id: PlanStepId) {
        self.in_flight.remove(&id);
        self.completed.insert(id);
    }

    /// Marks a plan step as failed.
    pub fn mark_failed(&mut self, id: PlanStepId) {
        self.in_flight.remove(&id);
        self.failed.insert(id);
    }

    /// Marks a plan step as skipped.
    pub fn mark_skipped(&mut self, id: PlanStepId) {
        self.in_flight.remove(&id);
        self.skipped.insert(id);
    }

    /// Resolves all steps from the immutable `ExecutionPlan` that are currently ready for evaluation.
    /// A step is executable if it is not finished (completed, failed, or skipped) nor in-flight,
    /// AND all of its dependencies are finished (present in completed, failed, or skipped).
    pub fn next_executable_steps<'a>(&self, plan: &'a ExecutionPlan) -> Vec<&'a ReasoningPlanStep> {
        plan.steps
            .iter()
            .filter(|step| {
                !self.completed.contains(&step.id)
                    && !self.failed.contains(&step.id)
                    && !self.skipped.contains(&step.id)
                    && !self.in_flight.contains(&step.id)
                    && step.depends_on.iter().all(|dep_id| {
                        self.completed.contains(dep_id)
                            || self.failed.contains(dep_id)
                            || self.skipped.contains(dep_id)
                    })
            })
            .collect()
    }

    /// Returns true if all steps in the `ExecutionPlan` are finished (completed, failed, or skipped).
    pub fn is_finished(&self, plan: &ExecutionPlan) -> bool {
        plan.steps.iter().all(|step| {
            self.completed.contains(&step.id)
                || self.failed.contains(&step.id)
                || self.skipped.contains(&step.id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::{PlanStepComplexity, ReasoningPlanStepKind};

    #[test]
    fn test_multiple_independent_roots_are_executable() {
        let step1 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::Search {
                query: "retrieval".to_string(),
            },
            "Search for retrieval engine",
            vec![],
            Some(PlanStepComplexity::Low),
        );
        let step2 = ReasoningPlanStep::new(
            PlanStepId::new(2),
            ReasoningPlanStepKind::QueryMemory {
                filter: crate::memory::MemoryFilter::Pinned,
            },
            "Query pinned memory",
            vec![],
            Some(PlanStepComplexity::Low),
        );
        let step3 = ReasoningPlanStep::new(
            PlanStepId::new(3),
            ReasoningPlanStepKind::CollectEvidence {
                step_ids: vec![PlanStepId::new(1), PlanStepId::new(2)],
            },
            "Collect evidence from search and memory",
            vec![PlanStepId::new(1), PlanStepId::new(2)],
            Some(PlanStepComplexity::Medium),
        );
        let step4 = ReasoningPlanStep::new(
            PlanStepId::new(4),
            ReasoningPlanStepKind::SynthesizeResponse,
            "Synthesize response",
            vec![PlanStepId::new(3)],
            Some(PlanStepComplexity::High),
        );

        let plan =
            ExecutionPlan::new("plan_roots", "query", vec![step1, step2, step3, step4]).unwrap();
        let cursor = ExecutionCursor::new();

        // Initially both independent root steps (Step 1 and Step 2) should be executable simultaneously
        let next_steps = cursor.next_executable_steps(&plan);
        assert_eq!(next_steps.len(), 2);
        let step_ids: HashSet<PlanStepId> = next_steps.iter().map(|s| s.id).collect();
        assert!(step_ids.contains(&PlanStepId::new(1)));
        assert!(step_ids.contains(&PlanStepId::new(2)));
    }

    #[test]
    fn test_execution_cursor_step_advancement() {
        let step1 = ReasoningPlanStep::new(
            PlanStepId::new(1),
            ReasoningPlanStepKind::Search {
                query: "a".to_string(),
            },
            "Step 1",
            vec![],
            None,
        );
        let step2 = ReasoningPlanStep::new(
            PlanStepId::new(2),
            ReasoningPlanStepKind::SynthesizeResponse,
            "Step 2",
            vec![PlanStepId::new(1)],
            None,
        );

        let plan = ExecutionPlan::new("plan_adv", "query", vec![step1, step2]).unwrap();
        let mut cursor = ExecutionCursor::new();

        assert_eq!(cursor.next_executable_steps(&plan).len(), 1);
        assert_eq!(
            cursor.next_executable_steps(&plan)[0].id,
            PlanStepId::new(1)
        );

        cursor.mark_in_flight(PlanStepId::new(1));
        assert_eq!(cursor.next_executable_steps(&plan).len(), 0);

        cursor.mark_completed(PlanStepId::new(1));
        let next_steps = cursor.next_executable_steps(&plan);
        assert_eq!(next_steps.len(), 1);
        assert_eq!(next_steps[0].id, PlanStepId::new(2));

        cursor.mark_completed(PlanStepId::new(2));
        assert!(cursor.is_finished(&plan));
    }
}
