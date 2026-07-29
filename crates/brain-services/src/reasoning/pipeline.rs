//! Composable `InferencePipeline` executing sequential `InferencePass` passes over an accumulating `InferenceGraph`.

use crate::query::ast::KnowledgeQuery;
use crate::query::context::QueryContextProvider;
use crate::query::fusion::QueryResult;
use crate::reasoning::models::InferenceGraph;
use crate::reasoning::pass::{
    CausalInferencePass, ContradictionResolutionPass, InferencePass, TemporalInferencePass,
};

/// Composable pipeline orchestrating monotonic inference passes.
pub struct InferencePipeline {
    passes: Vec<Box<dyn InferencePass>>,
}

impl Default for InferencePipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl InferencePipeline {
    /// Instantiates a default `InferencePipeline` with standard passes.
    pub fn new() -> Self {
        Self {
            passes: vec![
                Box::new(TemporalInferencePass::new()),
                Box::new(ContradictionResolutionPass::new()),
                Box::new(CausalInferencePass::new()),
            ],
        }
    }

    /// Appends a custom `InferencePass` to the pipeline.
    pub fn add_pass(&mut self, pass: Box<dyn InferencePass>) {
        self.passes.push(pass);
    }

    /// Executes all configured inference passes monotonically over `QueryResult` to build `InferenceGraph`.
    pub fn run(
        &self,
        query: &KnowledgeQuery,
        result: &QueryResult,
        ctx: &dyn QueryContextProvider,
    ) -> InferenceGraph {
        let mut graph = InferenceGraph::new();

        for pass in &self.passes {
            pass.execute(query, result, ctx, &mut graph);
        }

        graph
    }
}
