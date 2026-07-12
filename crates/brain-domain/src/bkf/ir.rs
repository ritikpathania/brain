use serde::{Deserialize, Serialize};
use crate::bkf::lifecycle::{KnowledgeLifecycle, KnowledgeValidity, KnowledgeVersionState};

/// Node element in the KPP intermediate representation graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IRNode {
    /// Unique identifier for the node.
    pub id: String,
    /// Canonical label of the node.
    pub label: String,
    /// Classification type of the node.
    pub entity_type: String,
    /// Extensible semantic attributes.
    pub attributes: serde_json::Map<String, serde_json::Value>,
    /// Processing lifecycle state.
    pub lifecycle: KnowledgeLifecycle,
    /// Validity tier.
    pub validity: KnowledgeValidity,
    /// Evolution/version state.
    pub version_state: KnowledgeVersionState,
}

/// Edge element in the KPP intermediate representation graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IREdge {
    /// Unique identifier for the edge.
    pub id: String,
    /// Source node identifier.
    pub source: String,
    /// Target node identifier.
    pub target: String,
    /// Directional relationship category/label.
    pub relation: String,
    /// Numerical weight/strength of relation.
    pub weight: f32,
    /// Processing lifecycle state.
    pub lifecycle: KnowledgeLifecycle,
    /// Validity tier.
    pub validity: KnowledgeValidity,
    /// Evolution/version state.
    pub version_state: KnowledgeVersionState,
}

/// Pre-optimization intermediate representation containing raw parsed semantic constructs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeIR {
    /// Unoptimized graph nodes.
    pub nodes: Vec<IRNode>,
    /// Unoptimized graph edges.
    pub edges: Vec<IREdge>,
}

/// Post-optimization canonical graph representing verified, deduplicated, and resolved semantic knowledge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledKnowledge {
    /// Canonical optimized nodes.
    pub nodes: Vec<IRNode>,
    /// Canonical optimized edges.
    pub edges: Vec<IREdge>,
}

impl CompiledKnowledge {
    /// Converts a raw, pre-optimization `KnowledgeIR` into `CompiledKnowledge`.
    pub fn from_ir(ir: KnowledgeIR) -> Self {
        Self {
            nodes: ir.nodes,
            edges: ir.edges,
        }
    }
}

