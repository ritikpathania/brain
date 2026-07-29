//! Domain-oriented Intermediate Representation (InferenceGraph IR) and response models for Knowledge Reasoning (Phase 5 Milestone 5.2).

use crate::compiler::{EntityId, FactId};
use crate::query::executor::Candidate;
use brain_domain::RelationId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed 0-indexed identifier for an inference node within an `InferenceGraph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InferenceNodeId(pub usize);

impl std::fmt::Display for InferenceNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "inf_node_{}", self.0)
    }
}

/// Explicit pointer to origin evidence backing an inferred proposition or reasoning claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Strongly-typed target entity identifier.
    pub entity_id: EntityId,
    /// Optional underlying fact identifier.
    pub fact_id: Option<FactId>,
    /// Optional underlying relation identifier.
    pub relation_id: Option<RelationId>,
    /// Evidence relevance or confidence weight [0.0..1.0].
    pub weight: f32,
}

/// Strongly-typed domain proposition subject-predicate-object tuple.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposition {
    /// Strongly-typed subject entity identifier.
    pub subject: EntityId,
    /// Strongly-typed relation predicate identifier.
    pub relation_kind: RelationId,
    /// Strongly-typed object entity identifier.
    pub object: EntityId,
    /// Proposition extraction or inference confidence [0.0..1.0].
    pub confidence: f32,
}

/// Classification kind for directed edges in an `InferenceGraph`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InferenceKind {
    /// Evidence supports target proposition.
    Supports,
    /// Evidence contradicts target proposition.
    Contradicts,
    /// Causal dependency (source causes or enables target).
    Causes,
    /// Temporal sequence ordering (source occurred before target).
    TemporalBefore,
    /// Temporal sequence ordering (source occurred after target).
    TemporalAfter,
    /// Logical derivation (target is derived from source proposition).
    DerivedFrom,
}

/// Domain node in an `InferenceGraph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceNode {
    /// Unique node identifier.
    pub id: InferenceNodeId,
    /// Domain proposition represented by this node.
    pub proposition: Proposition,
    /// List of explicit evidence references supporting or justifying this node.
    pub evidence: Vec<EvidenceRef>,
}

/// Directed semantic edge in an `InferenceGraph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceEdge {
    /// Source node identifier.
    pub source: InferenceNodeId,
    /// Target node identifier.
    pub target: InferenceNodeId,
    /// Directed inference classification kind.
    pub kind: InferenceKind,
}

/// Domain-oriented Intermediate Representation (IR) for inferred knowledge graphs.
///
/// ### Invariants:
/// 1. Node IDs are strictly unique within the graph.
/// 2. Edges reference existing valid `InferenceNodeId` nodes.
/// 3. Evidence references are immutable once attached.
/// 4. Inferences are deterministic: identical inputs produce identical `InferenceGraph` outputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InferenceGraph {
    /// List of domain inference nodes.
    pub nodes: Vec<InferenceNode>,
    /// List of directed inference edges.
    pub edges: Vec<InferenceEdge>,
}

impl InferenceGraph {
    /// Instantiates a new empty `InferenceGraph`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a domain proposition node and returns its `InferenceNodeId`.
    pub fn add_node(
        &mut self,
        proposition: Proposition,
        evidence: Vec<EvidenceRef>,
    ) -> InferenceNodeId {
        let node_id = InferenceNodeId(self.nodes.len());
        self.nodes.push(InferenceNode {
            id: node_id,
            proposition,
            evidence,
        });
        node_id
    }

    /// Appends a directed edge between two inference nodes.
    pub fn add_edge(
        &mut self,
        source: InferenceNodeId,
        target: InferenceNodeId,
        kind: InferenceKind,
    ) {
        self.edges.push(InferenceEdge {
            source,
            target,
            kind,
        });
    }
}

/// Transparent, evidence-derived confidence score metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceMetrics {
    /// Proportion of query intent supported by evidence [0.0..1.0].
    pub coverage_score: f32,
    /// Agreement level across retrieved candidate sets [0.0..1.0].
    pub agreement_score: f32,
    /// Penalty deduction applied due to active contradictions [0.0..1.0].
    pub contradiction_penalty: f32,
    /// Monotonic temporal consistency score [0.0..1.0].
    pub temporal_consistency_score: f32,
    /// Transparently computed composite confidence [0.0..1.0].
    pub composite_confidence: f32,
}

impl Default for ConfidenceMetrics {
    fn default() -> Self {
        Self {
            coverage_score: 1.0,
            agreement_score: 1.0,
            contradiction_penalty: 0.0,
            temporal_consistency_score: 1.0,
            composite_confidence: 1.0,
        }
    }
}

/// Human-readable step-by-step reasoning trace formatted from domain `InferenceGraph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReasoningTraceStep {
    /// 1-based step order sequence number.
    pub step_index: usize,
    /// Human-readable synthesized claim statement.
    pub claim: String,
    /// Supporting evidence references.
    pub evidence: Vec<EvidenceRef>,
    /// Step confidence score.
    pub confidence: f32,
}

/// Final synthesized knowledge reasoning response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeResponse {
    /// Query execution UUID.
    pub query_id: Uuid,
    /// Synthesized natural language answer summary.
    pub answer_summary: String,
    /// Step-by-step reasoning trace.
    pub reasoning_trace: Vec<ReasoningTraceStep>,
    /// Primary candidate entity matches.
    pub primary_candidates: Vec<Candidate>,
    /// Transparent evidence-derived confidence metrics.
    pub confidence: ConfidenceMetrics,
}
