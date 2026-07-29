//! Presentation synthesizer translating domain `InferenceGraph` IR into structured `KnowledgeResponse`.

use crate::query::fusion::QueryResult;
use crate::reasoning::models::{
    ConfidenceMetrics, InferenceGraph, KnowledgeResponse, ReasoningTraceStep,
};
use uuid::Uuid;

/// Presentation synthesizer translating domain `InferenceGraph` into natural-language answers and step traces.
#[derive(Debug, Clone, Default)]
pub struct ResponseSynthesizer;

impl ResponseSynthesizer {
    /// Instantiates a new `ResponseSynthesizer`.
    pub fn new() -> Self {
        Self
    }

    /// Synthesizes a structured `KnowledgeResponse` from a domain `InferenceGraph` and `QueryResult`.
    pub fn synthesize(
        &self,
        query_id: Uuid,
        graph: &InferenceGraph,
        query_result: &QueryResult,
    ) -> KnowledgeResponse {
        let mut trace_steps = Vec::new();

        for (idx, node) in graph.nodes.iter().enumerate() {
            let claim = format!(
                "Proposition: {} --[{}]--> {} (confidence: {:.2})",
                node.proposition.subject,
                node.proposition.relation_kind,
                node.proposition.object,
                node.proposition.confidence
            );

            trace_steps.push(ReasoningTraceStep {
                step_index: idx + 1,
                claim,
                evidence: node.evidence.clone(),
                confidence: node.proposition.confidence,
            });
        }

        let answer_summary = if query_result.candidates.is_empty() {
            "No candidate knowledge entities matched the query constraints.".to_string()
        } else {
            format!(
                "Discovered {} candidate entity match(es) across {} inference nodes.",
                query_result.candidates.len(),
                graph.nodes.len()
            )
        };

        let total_conf: f32 = query_result.candidates.iter().map(|c| c.score).sum();
        let avg_score = if query_result.candidates.is_empty() {
            0.0
        } else {
            total_conf / query_result.candidates.len() as f32
        };

        let confidence_metrics = ConfidenceMetrics {
            coverage_score: if query_result.candidates.is_empty() {
                0.0
            } else {
                1.0
            },
            agreement_score: avg_score.clamp(0.0, 1.0),
            contradiction_penalty: 0.0,
            temporal_consistency_score: 1.0,
            composite_confidence: avg_score.clamp(0.0, 1.0),
        };

        KnowledgeResponse {
            query_id,
            answer_summary,
            reasoning_trace: trace_steps,
            primary_candidates: query_result.candidates.clone(),
            confidence: confidence_metrics,
        }
    }
}
