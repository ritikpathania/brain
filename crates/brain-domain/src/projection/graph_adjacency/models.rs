//! Data models for Graph Adjacency Projection.

use crate::bkf::*;
use crate::identifiers::*;
use serde::{Deserialize, Serialize};

/// Graph node identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphNodeId(pub EntityId);

/// Graph edge identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphEdgeId(pub FactVersionId);

/// Edge record containing normalized edge payload and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeRecord {
    /// Unique edge ID.
    pub id: GraphEdgeId,
    /// Source node ID.
    pub source: GraphNodeId,
    /// Target node ID.
    pub target: GraphNodeId,
    /// Predicate ID.
    pub predicate: PredicateId,
    /// Confidence score.
    pub confidence: Confidence,
    /// Temporal validity window.
    pub temporal: TemporalWindow,
}

/// Cached degree stats per node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NodeDegree {
    /// Incoming edge count.
    pub in_degree: usize,
    /// Outgoing edge count.
    pub out_degree: usize,
}
