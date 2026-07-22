use brain_core::projection::{ProjectionContext, ProjectionQuery, Projector};
use brain_domain::{Edge, EdgeId, Node, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ─── Neighborhood Projection ─────────────────────────────────────────────────

/// Query parameters for Neighborhood Projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborhoodQuery {
    /// The center node ID to start neighborhood traversal from.
    pub center_node_id: NodeId,
    /// Maximum depth of the neighborhood (number of hops).
    pub depth: usize,
}
impl ProjectionQuery for NeighborhoodQuery {}

/// The result containing nodes and edges in the neighborhood.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborhoodProjectionResult {
    /// Nodes within the N-hop neighborhood.
    pub nodes: Vec<Node>,
    /// Edges within the N-hop neighborhood.
    pub edges: Vec<Edge>,
}

/// Projector that builds the NeighborhoodProjectionResult.
pub struct NeighborhoodProjector;

impl Projector<NeighborhoodProjectionResult, NeighborhoodQuery> for NeighborhoodProjector {
    fn project(
        &self,
        context: &ProjectionContext<NeighborhoodQuery>,
    ) -> NeighborhoodProjectionResult {
        let center_id = context.query.center_node_id;
        let max_depth = context.query.depth;

        if !context.graph.nodes.contains_key(&center_id) {
            return NeighborhoodProjectionResult {
                nodes: Vec::new(),
                edges: Vec::new(),
            };
        }

        let mut visited = HashSet::new();
        let mut visited_edge_ids = HashSet::new();
        let mut edges = Vec::new();
        let mut queue = VecDeque::new();

        queue.push_back((center_id, 0));
        visited.insert(center_id);

        // Pre-build index of node connections for fast lookups
        let mut adjacency: HashMap<NodeId, Vec<&Edge>> = HashMap::new();
        for edge in context.graph.edges.values() {
            adjacency.entry(edge.source).or_default().push(edge);
            adjacency.entry(edge.target).or_default().push(edge);
        }

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            if let Some(connected_edges) = adjacency.get(&current_id) {
                for edge in connected_edges {
                    let edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());
                    if visited_edge_ids.insert(edge_id) {
                        edges.push((*edge).clone());
                    }
                    let neighbor_id = if edge.source == current_id {
                        edge.target
                    } else {
                        edge.source
                    };
                    if visited.insert(neighbor_id) {
                        queue.push_back((neighbor_id, depth + 1));
                    }
                }
            }
        }

        let nodes = visited
            .into_iter()
            .filter_map(|id| context.graph.nodes.get(&id).cloned())
            .collect();

        NeighborhoodProjectionResult { nodes, edges }
    }
}

// ─── Path Projection ─────────────────────────────────────────────────────────

/// Query parameters for Path Projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathQuery {
    /// The starting node ID.
    pub source_node_id: NodeId,
    /// The target node ID.
    pub target_node_id: NodeId,
}
impl ProjectionQuery for PathQuery {}

/// The result containing the shortest path of nodes and the connecting edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathProjectionResult {
    /// Shortest path of nodes from source to target (inclusive), or None if unreachable.
    pub path: Option<Vec<Node>>,
    /// Connecting edges along the path.
    pub edges: Vec<Edge>,
}

/// Projector that builds the PathProjectionResult.
pub struct PathProjector;

impl Projector<PathProjectionResult, PathQuery> for PathProjector {
    fn project(&self, context: &ProjectionContext<PathQuery>) -> PathProjectionResult {
        let source_id = context.query.source_node_id;
        let target_id = context.query.target_node_id;

        if !context.graph.nodes.contains_key(&source_id)
            || !context.graph.nodes.contains_key(&target_id)
        {
            return PathProjectionResult {
                path: None,
                edges: Vec::new(),
            };
        }

        if source_id == target_id {
            let start_node = context.graph.nodes.get(&source_id).unwrap().clone();
            return PathProjectionResult {
                path: Some(vec![start_node]),
                edges: Vec::new(),
            };
        }

        // Pre-build index of node connections for fast lookups
        let mut adjacency: HashMap<NodeId, Vec<&Edge>> = HashMap::new();
        for edge in context.graph.edges.values() {
            adjacency.entry(edge.source).or_default().push(edge);
            adjacency.entry(edge.target).or_default().push(edge);
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((source_id, vec![source_id]));
        visited.insert(source_id);

        let mut shortest_path_ids = None;

        while let Some((current_id, path)) = queue.pop_front() {
            if current_id == target_id {
                shortest_path_ids = Some(path);
                break;
            }

            if let Some(connected_edges) = adjacency.get(&current_id) {
                for edge in connected_edges {
                    let neighbor_id = if edge.source == current_id {
                        edge.target
                    } else {
                        edge.source
                    };
                    if visited.insert(neighbor_id) {
                        let mut new_path = path.clone();
                        new_path.push(neighbor_id);
                        queue.push_back((neighbor_id, new_path));
                    }
                }
            }
        }

        if let Some(path_ids) = shortest_path_ids {
            let path_nodes: Vec<Node> = path_ids
                .iter()
                .filter_map(|id| context.graph.nodes.get(id).cloned())
                .collect();

            // Collect the edges that connect the path nodes
            let mut path_edges = Vec::new();
            for window in path_ids.windows(2) {
                let u = window[0];
                let v = window[1];
                if let Some(edge) =
                    context.graph.edges.values().find(|e| {
                        (e.source == u && e.target == v) || (e.source == v && e.target == u)
                    })
                {
                    path_edges.push(edge.clone());
                }
            }

            PathProjectionResult {
                path: Some(path_nodes),
                edges: path_edges,
            }
        } else {
            PathProjectionResult {
                path: None,
                edges: Vec::new(),
            }
        }
    }
}

// ─── Cluster Projection ──────────────────────────────────────────────────────

/// Query parameters for Cluster Projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterQuery {
    /// Optional minimum cluster size filter.
    pub min_cluster_size: Option<usize>,
}
impl ProjectionQuery for ClusterQuery {}

/// The result containing community/cluster membership mappings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterProjectionResult {
    /// Maps cluster ID (e.g. "cluster_0") to a list of node IDs in that cluster.
    pub clusters: HashMap<String, Vec<NodeId>>,
}

/// Projector that builds the ClusterProjectionResult.
pub struct ClusterProjector;

impl Projector<ClusterProjectionResult, ClusterQuery> for ClusterProjector {
    fn project(&self, context: &ProjectionContext<ClusterQuery>) -> ClusterProjectionResult {
        // Pre-build index of node connections for fast lookups
        let mut adjacency: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        for edge in context.graph.edges.values() {
            adjacency.entry(edge.source).or_default().push(edge.target);
            adjacency.entry(edge.target).or_default().push(edge.source);
        }

        let mut visited = HashSet::new();
        let mut clusters = HashMap::new();
        let mut cluster_counter = 0;

        for &node_id in context.graph.nodes.keys() {
            if visited.contains(&node_id) {
                continue;
            }

            // Find all reachable nodes using BFS (connected component)
            let mut cluster_nodes = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(node_id);
            visited.insert(node_id);

            while let Some(current_id) = queue.pop_front() {
                cluster_nodes.push(current_id);

                if let Some(neighbors) = adjacency.get(&current_id) {
                    for &neighbor_id in neighbors {
                        if visited.insert(neighbor_id) {
                            queue.push_back(neighbor_id);
                        }
                    }
                }
            }

            let include_cluster = match context.query.min_cluster_size {
                Some(min_size) => cluster_nodes.len() >= min_size,
                None => true,
            };

            if include_cluster {
                // Sort cluster nodes to ensure deterministic output
                cluster_nodes.sort_by_key(|a| a.0);
                let cluster_id = format!("cluster_{}", cluster_counter);
                clusters.insert(cluster_id, cluster_nodes);
                cluster_counter += 1;
            }
        }

        ClusterProjectionResult { clusters }
    }
}
