//! Orchestration root composing `QueryPlanner`, `PlanOptimizer`, `QueryExecutor`, and `FusionStrategy`.

use crate::query::ast::KnowledgeQuery;
use crate::query::context::QueryContextProvider;
use crate::query::executor::QueryExecutor;
use crate::query::fusion::{FusionStrategy, QueryResult, ReciprocalRankFusion};
use crate::query::optimizer::{NoOpOptimizer, PlanOptimizer};
use crate::query::planner::QueryPlanner;

/// Composition root orchestrating the end-to-end knowledge query execution pipeline.
pub struct QueryPipeline {
    planner: QueryPlanner,
    optimizer: Box<dyn PlanOptimizer>,
    executor: QueryExecutor,
    fusion: Box<dyn FusionStrategy>,
}

impl Default for QueryPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryPipeline {
    /// Instantiates a standard `QueryPipeline` with default components.
    pub fn new() -> Self {
        Self {
            planner: QueryPlanner::new(),
            optimizer: Box::new(NoOpOptimizer::new()),
            executor: QueryExecutor::new(),
            fusion: Box::new(ReciprocalRankFusion::default()),
        }
    }

    /// Customizes the plan optimizer.
    pub fn with_optimizer(mut self, optimizer: Box<dyn PlanOptimizer>) -> Self {
        self.optimizer = optimizer;
        self
    }

    /// Customizes the fusion strategy.
    pub fn with_fusion_strategy(mut self, fusion: Box<dyn FusionStrategy>) -> Self {
        self.fusion = fusion;
        self
    }

    /// Executes a declarative `KnowledgeQuery` end-to-end through plan generation, optimization, execution, and candidate fusion.
    pub fn execute(&self, query: &KnowledgeQuery, ctx: &dyn QueryContextProvider) -> QueryResult {
        let plan = self.planner.create_plan(query);
        let optimized_plan = self.optimizer.optimize(plan);
        let candidate_sets = self.executor.execute_plan(&optimized_plan, ctx);
        self.fusion.fuse(&candidate_sets, query.limit)
    }
}
