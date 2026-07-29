//! `KnowledgeReasoningEngine` orchestration root for end-to-end retrieval, inference, and response synthesis.

use crate::query::ast::KnowledgeQuery;
use crate::query::context::QueryContextProvider;
use crate::query::pipeline::QueryPipeline;
use crate::reasoning::models::KnowledgeResponse;
use crate::reasoning::pipeline::InferencePipeline;
use crate::reasoning::synthesizer::ResponseSynthesizer;
use uuid::Uuid;

/// Orchestration root executing query retrieval, monotonic knowledge inference, and answer synthesis.
pub struct KnowledgeReasoningEngine {
    query_pipeline: QueryPipeline,
    inference_pipeline: InferencePipeline,
    synthesizer: ResponseSynthesizer,
}

impl Default for KnowledgeReasoningEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeReasoningEngine {
    /// Instantiates a standard `KnowledgeReasoningEngine`.
    pub fn new() -> Self {
        Self {
            query_pipeline: QueryPipeline::new(),
            inference_pipeline: InferencePipeline::new(),
            synthesizer: ResponseSynthesizer::new(),
        }
    }

    /// Executes end-to-end reasoning over a declarative `KnowledgeQuery` and `QueryContextProvider`.
    pub fn execute(
        &self,
        query: &KnowledgeQuery,
        ctx: &dyn QueryContextProvider,
    ) -> KnowledgeResponse {
        let query_id = Uuid::new_v4();
        let query_result = self.query_pipeline.execute(query, ctx);
        let inference_graph = self.inference_pipeline.run(query, &query_result, ctx);
        self.synthesizer
            .synthesize(query_id, &inference_graph, &query_result)
    }
}
