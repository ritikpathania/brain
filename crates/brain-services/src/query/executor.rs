//! Query plan executor running `ExecutionPlan` steps against a `QueryContextProvider`.

use crate::compiler::EntityId;
use crate::query::context::QueryContextProvider;
use crate::query::plan::{ExecutionPlan, ExecutionStep, ExecutionStepId};
use serde::{Deserialize, Serialize};

/// Candidate entity match item with assigned confidence/relevance score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// Strongly-typed canonical entity identifier.
    pub entity_id: EntityId,
    /// Confidence or relevance score [0.0..1.0].
    pub score: f32,
}

/// Raw candidate set produced by evaluating a single `ExecutionStep`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawCandidateSet {
    /// Identifier of the execution step that produced this candidate set.
    pub source_step_id: ExecutionStepId,
    /// List of matched candidate entities.
    pub candidates: Vec<Candidate>,
}

/// Query executor evaluating plan steps against an abstract `QueryContextProvider`.
#[derive(Debug, Clone, Default)]
pub struct QueryExecutor;

impl QueryExecutor {
    /// Instantiates a new `QueryExecutor`.
    pub fn new() -> Self {
        Self
    }

    /// Executes an `ExecutionPlan` against a `QueryContextProvider` and returns candidate sets per step.
    pub fn execute_plan(
        &self,
        plan: &ExecutionPlan,
        ctx: &dyn QueryContextProvider,
    ) -> Vec<RawCandidateSet> {
        let mut results = Vec::new();

        for (step_id, step) in &plan.steps {
            let candidates = match step {
                ExecutionStep::Text(s) => ctx.evaluate_text(s),
                ExecutionStep::Semantic(s) => ctx.evaluate_semantic(s),
                ExecutionStep::Graph(s) => ctx.evaluate_graph(s),
                ExecutionStep::Temporal(s) => ctx.evaluate_temporal(s),
            };

            results.push(RawCandidateSet {
                source_step_id: *step_id,
                candidates,
            });
        }

        results
    }
}
