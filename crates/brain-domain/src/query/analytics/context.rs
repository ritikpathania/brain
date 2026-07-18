use crate::entities::KnowledgeGraph;
use crate::identifiers::NodeId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// Pre-computed adjacency index mapping nodes to outgoing neighbor nodes.
pub struct AdjacencyIndex {
    map: HashMap<NodeId, Vec<NodeId>>,
}

impl AdjacencyIndex {
    /// Returns slice of canonically sorted outgoing neighbor node IDs.
    pub fn neighbors(&self, node: NodeId) -> &[NodeId] {
        self.map.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Checks if a node is present in the adjacency index.
    pub fn contains(&self, node: NodeId) -> bool {
        self.map.contains_key(&node)
    }
}

/// Pre-computed reverse adjacency index mapping nodes to incoming predecessor nodes.
pub struct ReverseAdjacencyIndex {
    map: HashMap<NodeId, Vec<NodeId>>,
}

impl ReverseAdjacencyIndex {
    /// Returns slice of canonically sorted incoming predecessor node IDs.
    pub fn predecessors(&self, node: NodeId) -> &[NodeId] {
        self.map.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Checks if a node is present in the reverse adjacency index.
    pub fn contains(&self, node: NodeId) -> bool {
        self.map.contains_key(&node)
    }
}

/// Cache mapping nodes to incoming and outgoing degrees.
pub struct DegreeIndex {
    out_degrees: HashMap<NodeId, usize>,
    in_degrees: HashMap<NodeId, usize>,
}

impl DegreeIndex {
    /// Returns outgoing degree of a node.
    pub fn out_degree(&self, node: NodeId) -> usize {
        self.out_degrees.get(&node).copied().unwrap_or(0)
    }

    /// Returns incoming degree of a node.
    pub fn in_degree(&self, node: NodeId) -> usize {
        self.in_degrees.get(&node).copied().unwrap_or(0)
    }

    /// Returns total degree (incoming + outgoing) of a node.
    pub fn total_degree(&self, node: NodeId) -> usize {
        self.out_degree(node) + self.in_degree(node)
    }
}

/// Diagnostic metadata regarding which analytical indices are initialized.
#[derive(Debug, Clone)]
pub struct AnalyticsDiagnostics {
    /// Whether adjacency index has been evaluated.
    pub adjacency_initialized: bool,
    /// Whether reverse adjacency index has been evaluated.
    pub reverse_adjacency_initialized: bool,
    /// Whether degree index has been evaluated.
    pub degrees_initialized: bool,
    /// Count of adjacency index evaluations.
    pub adjacency_builds: usize,
    /// Count of reverse adjacency index evaluations.
    pub reverse_adjacency_builds: usize,
    /// Count of degree index evaluations.
    pub degrees_builds: usize,
}

/// Immutable, thread-safe, and lazily evaluated shared analytics index context.
pub struct GraphAnalyticsContext<'a> {
    graph: &'a KnowledgeGraph,
    adjacency: OnceLock<AdjacencyIndex>,
    reverse_adjacency: OnceLock<ReverseAdjacencyIndex>,
    degrees: OnceLock<DegreeIndex>,
    adjacency_builds: AtomicUsize,
    reverse_adjacency_builds: AtomicUsize,
    degrees_builds: AtomicUsize,
}

impl<'a> GraphAnalyticsContext<'a> {
    /// Creates a new, lazy `GraphAnalyticsContext` wrapping the given graph.
    pub fn new(graph: &'a KnowledgeGraph) -> Self {
        Self {
            graph,
            adjacency: OnceLock::new(),
            reverse_adjacency: OnceLock::new(),
            degrees: OnceLock::new(),
            adjacency_builds: AtomicUsize::new(0),
            reverse_adjacency_builds: AtomicUsize::new(0),
            degrees_builds: AtomicUsize::new(0),
        }
    }

    /// Accesses the underlying knowledge graph.
    pub fn graph(&self) -> &'a KnowledgeGraph {
        self.graph
    }

    /// Lazily constructs and accesses the forward adjacency index.
    pub fn adjacency(&self) -> &AdjacencyIndex {
        self.adjacency.get_or_init(|| {
            self.adjacency_builds.fetch_add(1, Ordering::SeqCst);
            let mut map = HashMap::new();
            for edge in self.graph.edges.values() {
                map.entry(edge.source)
                    .or_insert_with(Vec::new)
                    .push(edge.target);
            }
            for neighbors in map.values_mut() {
                neighbors.sort();
            }
            AdjacencyIndex { map }
        })
    }

    /// Lazily constructs and accesses the reverse adjacency index.
    pub fn reverse_adjacency(&self) -> &ReverseAdjacencyIndex {
        self.reverse_adjacency.get_or_init(|| {
            self.reverse_adjacency_builds.fetch_add(1, Ordering::SeqCst);
            let mut map = HashMap::new();
            for edge in self.graph.edges.values() {
                map.entry(edge.target)
                    .or_insert_with(Vec::new)
                    .push(edge.source);
            }
            for neighbors in map.values_mut() {
                neighbors.sort();
            }
            ReverseAdjacencyIndex { map }
        })
    }

    /// Lazily constructs and accesses the degree index.
    pub fn degrees(&self) -> &DegreeIndex {
        self.degrees.get_or_init(|| {
            self.degrees_builds.fetch_add(1, Ordering::SeqCst);
            let mut out_degrees = HashMap::new();
            let mut in_degrees = HashMap::new();
            for node in self.graph.nodes.keys() {
                out_degrees.insert(*node, 0);
                in_degrees.insert(*node, 0);
            }
            for edge in self.graph.edges.values() {
                *out_degrees.entry(edge.source).or_default() += 1;
                *in_degrees.entry(edge.target).or_default() += 1;
            }
            DegreeIndex {
                out_degrees,
                in_degrees,
            }
        })
    }

    /// Returns crate-private diagnostics on the initialization state of indices.
    #[allow(dead_code)]
    pub(crate) fn diagnostics(&self) -> AnalyticsDiagnostics {
        AnalyticsDiagnostics {
            adjacency_initialized: self.adjacency.get().is_some(),
            reverse_adjacency_initialized: self.reverse_adjacency.get().is_some(),
            degrees_initialized: self.degrees.get().is_some(),
            adjacency_builds: self.adjacency_builds.load(Ordering::SeqCst),
            reverse_adjacency_builds: self.reverse_adjacency_builds.load(Ordering::SeqCst),
            degrees_builds: self.degrees_builds.load(Ordering::SeqCst),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Edge, KnowledgeGraph, Node, NodeType, RelationKind};
    use crate::identifiers::NodeId;
    use crate::query::analytics::{
        AnalyticsAlgorithm, Centrality, CentralityConfig, ConnectedComponents,
        ConnectedComponentsConfig,
    };

    #[test]
    fn test_context_diagnostics_and_lazy_evaluation() {
        let mut graph = KnowledgeGraph::new();
        let node_a = NodeId::new();
        let node_b = NodeId::new();
        graph.add_node(Node::new(node_a, "NodeA".to_string(), NodeType::Concept));
        graph.add_node(Node::new(node_b, "NodeB".to_string(), NodeType::Concept));
        graph
            .add_edge(Edge::new(node_a, node_b, RelationKind::Uses, 0.9))
            .unwrap();

        let ctx = GraphAnalyticsContext::new(&graph);
        let diag_initial = ctx.diagnostics();
        assert!(!diag_initial.adjacency_initialized);
        assert!(!diag_initial.reverse_adjacency_initialized);
        assert!(!diag_initial.degrees_initialized);
        assert_eq!(diag_initial.adjacency_builds, 0);

        // Run ConnectedComponents - demands adjacency and reverse adjacency
        let _comps = ConnectedComponents::new(&ctx, ConnectedComponentsConfig::default()).compute();
        let diag_after_cc = ctx.diagnostics();
        assert!(diag_after_cc.adjacency_initialized);
        assert!(diag_after_cc.reverse_adjacency_initialized);
        assert!(!diag_after_cc.degrees_initialized);
        assert_eq!(diag_after_cc.adjacency_builds, 1);
        assert_eq!(diag_after_cc.reverse_adjacency_builds, 1);
        assert_eq!(diag_after_cc.degrees_builds, 0);

        // Run Centrality - demands degree index
        let _centrality = Centrality::new(&ctx, CentralityConfig::default()).compute();
        let diag_after_cen = ctx.diagnostics();
        assert!(diag_after_cen.degrees_initialized);
        assert_eq!(diag_after_cen.degrees_builds, 1);
    }
}
