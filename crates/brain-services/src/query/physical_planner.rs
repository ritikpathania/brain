//! Physical planner translating LogicalPlan into PhysicalPlan.

use crate::query::physical_plan::*;
use brain_domain::query::*;

/// Physical planner converting LogicalPlan into PhysicalPlan.
pub struct PhysicalPlanner;

impl PhysicalPlanner {
    /// Lowers a LogicalPlan into a PhysicalPlan.
    pub fn plan(logical: &LogicalPlan) -> Result<PhysicalPlan, QueryError> {
        let root = Self::lower_node(logical);
        Ok(PhysicalPlan { root })
    }

    fn lower_node(node: &LogicalPlan) -> PhysicalPlanNode {
        match node {
            LogicalPlan::Scan { target } => PhysicalPlanNode::Scan { target: *target },
            LogicalPlan::Filter { condition, input } => PhysicalPlanNode::Filter {
                description: format!("{:?}", condition),
                input: Box::new(Self::lower_node(input)),
            },
            LogicalPlan::Limit { count, input } => PhysicalPlanNode::Limit {
                count: *count,
                input: Box::new(Self::lower_node(input)),
            },
            _ => PhysicalPlanNode::Scan {
                target: ScanTarget::ActiveFacts,
            },
        }
    }
}
