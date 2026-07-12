use crate::identifiers::{NodeId, RelationId};

/// Degree centrality score for a node in the graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DegreeCentrality {
    /// The node identifier.
    pub node: NodeId,
    /// The degree score (incoming + outgoing edge count).
    pub score: usize,
}

/// Occurrences of a relation kind in the graph.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct RelationDistribution {
    /// The relation type identifier.
    pub relation: RelationId,
    /// The count of occurrences.
    pub count: usize,
}

/// Aggregated count of edge provenance sources.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProvenanceStats {
    /// Count of extracted edges.
    pub total_extracted: usize,
    /// Count of inferred edges.
    pub total_inferred: usize,
    /// Count of user-authored edges.
    pub total_user_authored: usize,
    /// Count of imported edges.
    pub total_imported: usize,
}

/// PageRank centrality score for a node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct PageRankResult {
    /// The node identifier.
    pub node: NodeId,
    /// The PageRank score.
    pub score: f64,
}

/// Strongly connected component cluster.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StronglyConnectedComponent {
    /// List of canonically ordered node identifiers in the component.
    pub nodes: Vec<NodeId>,
}

/// Closeness centrality score for a node.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ClosenessResult {
    /// The node identifier.
    pub node: NodeId,
    /// The Closeness centrality score.
    pub score: f64,
}

/// Consolidated connectivity diagnostic report.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ConnectivityReport {
    /// Critical cut nodes whose removal disconnects the graph, sorted canonically.
    pub articulation_points: Vec<NodeId>,
    /// Critical bridge edges whose removal increases the number of connected components.
    /// Each bridge is represented as a canonical tuple `(min(u, v), max(u, v))` sorted lexicographically.
    pub bridges: Vec<(NodeId, NodeId)>,
}
