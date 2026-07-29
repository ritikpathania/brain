//! Query planner compiling `KnowledgeQuery` intent AST into an `ExecutionPlan`.

use crate::query::ast::KnowledgeQuery;
use crate::query::plan::{
    ExecutionPlan, ExecutionStep, GraphStep, SemanticStep, TemporalStep, TextStep,
};
use uuid::Uuid;

/// Query planner translating `KnowledgeQuery` AST into an `ExecutionPlan`.
#[derive(Debug, Clone, Default)]
pub struct QueryPlanner;

impl QueryPlanner {
    /// Instantiates a new `QueryPlanner`.
    pub fn new() -> Self {
        Self
    }

    /// Compiles a declarative `KnowledgeQuery` AST into an `ExecutionPlan`.
    pub fn create_plan(&self, query: &KnowledgeQuery) -> ExecutionPlan {
        let mut plan = ExecutionPlan::new(Uuid::new_v4());

        if let Some(ref text) = query.text {
            plan.add_step(ExecutionStep::Text(TextStep {
                pattern: text.clone(),
            }));
        }

        if let Some(ref prompt) = query.semantic_prompt {
            plan.add_step(ExecutionStep::Semantic(SemanticStep {
                prompt: prompt.clone(),
            }));
        }

        for filter in &query.relation_filters {
            plan.add_step(ExecutionStep::Graph(GraphStep {
                relation_kind: filter.relation_kind.clone(),
                target_id: filter.target_id.clone(),
            }));
        }

        if let Some(range) = query.temporal_range {
            plan.add_step(ExecutionStep::Temporal(TemporalStep { range }));
        }

        plan
    }
}
