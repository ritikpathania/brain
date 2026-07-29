//! Query plan optimization pass interface and implementations.

use crate::query::plan::ExecutionPlan;

/// Trait defining an optimization transformation over an `ExecutionPlan`.
pub trait PlanOptimizer: Send + Sync {
    /// Transforms an unoptimized `ExecutionPlan` into an optimized `ExecutionPlan`.
    fn optimize(&self, plan: ExecutionPlan) -> ExecutionPlan;
}

/// Default identity pass-through plan optimizer.
#[derive(Debug, Clone, Default)]
pub struct NoOpOptimizer;

impl NoOpOptimizer {
    /// Instantiates a new `NoOpOptimizer`.
    pub fn new() -> Self {
        Self
    }
}

impl PlanOptimizer for NoOpOptimizer {
    fn optimize(&self, plan: ExecutionPlan) -> ExecutionPlan {
        plan
    }
}
