//! Immutable EvolutionPlan aggregate orchestrating planned proposals.

use super::proposal::{EvolutionProposal, Priority};
use serde::{Deserialize, Serialize};

/// Immutable plan aggregate produced by the EvolutionEngine planner.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EvolutionPlan {
    /// Ordered evolution proposals.
    pub proposals: Vec<EvolutionProposal>,
}

impl EvolutionPlan {
    /// Creates a new EvolutionPlan.
    pub fn new(proposals: Vec<EvolutionProposal>) -> Self {
        Self { proposals }
    }

    /// Returns proposals grouped by priority tier.
    pub fn proposals_by_priority(&self, priority: Priority) -> Vec<&EvolutionProposal> {
        self.proposals
            .iter()
            .filter(|p| p.priority == priority)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evolution_plan_grouping() {
        let plan = EvolutionPlan::new(vec![]);
        let critical = plan.proposals_by_priority(Priority::Critical);
        assert_eq!(critical.len(), 0);
    }
}
