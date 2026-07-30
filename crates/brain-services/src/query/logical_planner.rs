//! Logical planner building LogicalPlan from BoundQuery.

use brain_domain::query::*;

/// Logical planner converting BoundQuery into immutable LogicalPlan algebra tree.
pub struct LogicalPlanner;

impl LogicalPlanner {
    /// Generates a LogicalPlan from a BoundQuery.
    pub fn plan(bound: &BoundQuery) -> Result<LogicalPlan, QueryError> {
        let mut curr = LogicalPlan::Scan {
            target: ScanTarget::ActiveFacts,
        };

        for filter in &bound.ast.filters {
            curr = LogicalPlan::Filter {
                condition: filter.clone(),
                input: Box::new(curr),
            };
        }

        if let Some(count) = bound.ast.limit {
            curr = LogicalPlan::Limit {
                count,
                input: Box::new(curr),
            };
        }

        Ok(curr)
    }
}
