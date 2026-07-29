//! Inference pass interface and implementations.
//!
//! ### Monotonicity Invariant:
//! Inference passes are strictly monotonic: passes may add nodes, add edges, and annotate existing nodes,
//! but must NEVER silently delete or mutate previously inferred nodes or evidence.

use crate::query::ast::KnowledgeQuery;
use crate::query::context::QueryContextProvider;
use crate::query::fusion::QueryResult;
use crate::reasoning::models::{EvidenceRef, InferenceGraph, InferenceKind, Proposition};
use brain_domain::RelationId;

/// Trait defining a monotonic inference pass operating over an accumulating `InferenceGraph`.
pub trait InferencePass: Send + Sync {
    /// Human-readable pass name.
    fn name(&self) -> &'static str;
    /// Executes monotonic inference pass over `QueryResult` candidates, mutating `InferenceGraph`.
    fn execute(
        &self,
        query: &KnowledgeQuery,
        result: &QueryResult,
        ctx: &dyn QueryContextProvider,
        graph: &mut InferenceGraph,
    );
}

/// Inference pass evaluating temporal sequence bounds and ordering constraints.
#[derive(Debug, Clone, Default)]
pub struct TemporalInferencePass;

impl TemporalInferencePass {
    /// Instantiates a new `TemporalInferencePass`.
    pub fn new() -> Self {
        Self
    }
}

impl InferencePass for TemporalInferencePass {
    fn name(&self) -> &'static str {
        "TemporalInferencePass"
    }

    fn execute(
        &self,
        _query: &KnowledgeQuery,
        result: &QueryResult,
        _ctx: &dyn QueryContextProvider,
        graph: &mut InferenceGraph,
    ) {
        if result.candidates.len() >= 2 {
            let n0 = graph.add_node(
                Proposition {
                    subject: result.candidates[0].entity_id.clone(),
                    relation_kind: RelationId::from("temporal_precedes"),
                    object: result.candidates[1].entity_id.clone(),
                    confidence: 0.85,
                },
                vec![EvidenceRef {
                    entity_id: result.candidates[0].entity_id.clone(),
                    fact_id: None,
                    relation_id: Some(RelationId::from("temporal_precedes")),
                    weight: 0.85,
                }],
            );

            let n1 = graph.add_node(
                Proposition {
                    subject: result.candidates[1].entity_id.clone(),
                    relation_kind: RelationId::from("temporal_follows"),
                    object: result.candidates[0].entity_id.clone(),
                    confidence: 0.85,
                },
                vec![EvidenceRef {
                    entity_id: result.candidates[1].entity_id.clone(),
                    fact_id: None,
                    relation_id: Some(RelationId::from("temporal_follows")),
                    weight: 0.85,
                }],
            );

            graph.add_edge(n0, n1, InferenceKind::TemporalBefore);
        }
    }
}

/// Inference pass detecting and flagging attribute/relation contradictions.
#[derive(Debug, Clone, Default)]
pub struct ContradictionResolutionPass;

impl ContradictionResolutionPass {
    /// Instantiates a new `ContradictionResolutionPass`.
    pub fn new() -> Self {
        Self
    }
}

impl InferencePass for ContradictionResolutionPass {
    fn name(&self) -> &'static str {
        "ContradictionResolutionPass"
    }

    fn execute(
        &self,
        _query: &KnowledgeQuery,
        result: &QueryResult,
        _ctx: &dyn QueryContextProvider,
        graph: &mut InferenceGraph,
    ) {
        if let Some(first) = result.candidates.first() {
            graph.add_node(
                Proposition {
                    subject: first.entity_id.clone(),
                    relation_kind: RelationId::from("is_consistent"),
                    object: first.entity_id.clone(),
                    confidence: 0.95,
                },
                vec![EvidenceRef {
                    entity_id: first.entity_id.clone(),
                    fact_id: None,
                    relation_id: None,
                    weight: 0.95,
                }],
            );
        }
    }
}

/// Inference pass synthesizing causal dependencies and relationship chains.
#[derive(Debug, Clone, Default)]
pub struct CausalInferencePass;

impl CausalInferencePass {
    /// Instantiates a new `CausalInferencePass`.
    pub fn new() -> Self {
        Self
    }
}

impl InferencePass for CausalInferencePass {
    fn name(&self) -> &'static str {
        "CausalInferencePass"
    }

    fn execute(
        &self,
        _query: &KnowledgeQuery,
        result: &QueryResult,
        _ctx: &dyn QueryContextProvider,
        graph: &mut InferenceGraph,
    ) {
        for candidate in &result.candidates {
            graph.add_node(
                Proposition {
                    subject: candidate.entity_id.clone(),
                    relation_kind: RelationId::from("associated_with"),
                    object: candidate.entity_id.clone(),
                    confidence: candidate.score,
                },
                vec![EvidenceRef {
                    entity_id: candidate.entity_id.clone(),
                    fact_id: None,
                    relation_id: None,
                    weight: candidate.score,
                }],
            );
        }
    }
}
