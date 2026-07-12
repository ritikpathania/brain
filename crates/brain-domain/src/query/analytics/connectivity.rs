use crate::identifiers::NodeId;
use crate::query::analytics::{AnalyticsAlgorithm, Complexity, GraphAnalyticsContext, ConnectivityReport};
use std::collections::{HashSet, HashMap};

/// Configuration parameters for connectivity diagnostics.
#[derive(Debug, Clone, Default)]
pub struct ConnectivityConfig {}

/// Connectivity diagnostician locating bridges and cut vertices (articulation points).
///
/// **Undirected Semantics**:
/// This solver treats all directed edges as undirected for the purpose of articulation point
/// and bridge detection. Both forward and reverse adjacencies are combined to traverse the graph
/// as a single undirected structure.
pub struct Connectivity<'a, 'b> {
    context: &'b GraphAnalyticsContext<'a>,
    #[allow(dead_code)]
    config: ConnectivityConfig,
}

impl<'a, 'b> Connectivity<'a, 'b> {
    /// Creates a new `Connectivity` diagnostics solver.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: ConnectivityConfig) -> Self {
        Self { context, config }
    }
}

struct TarjanState {
    time: usize,
    discovery: HashMap<NodeId, usize>,
    low: HashMap<NodeId, usize>,
    articulation_points: HashSet<NodeId>,
    bridges: Vec<(NodeId, NodeId)>,
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for Connectivity<'a, 'b> {
    type Output = ConnectivityReport;

    fn algorithm_id(&self) -> &'static str {
        "connectivity_diagnostics"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Linear // O(V + E)
    }

    fn compute(&self) -> Self::Output {
        let graph = self.context.graph();
        let mut state = TarjanState {
            time: 0,
            discovery: HashMap::new(),
            low: HashMap::new(),
            articulation_points: HashSet::new(),
            bridges: Vec::new(),
        };

        // Standardize sorting of nodes to ensure deterministic DFS entry order
        let mut sorted_nodes: Vec<NodeId> = graph.nodes.keys().cloned().collect();
        sorted_nodes.sort();

        fn dfs(
            u: NodeId,
            parent: Option<NodeId>,
            context: &GraphAnalyticsContext,
            state: &mut TarjanState,
        ) {
            state.time += 1;
            state.discovery.insert(u, state.time);
            state.low.insert(u, state.time);

            let adjacency = context.adjacency();
            let reverse_adjacency = context.reverse_adjacency();

            // Collect neighbors in deterministic order
            let f_neighbors = adjacency.neighbors(u);
            let r_neighbors = reverse_adjacency.predecessors(u);
            let mut neighbors = Vec::with_capacity(f_neighbors.len() + r_neighbors.len());
            neighbors.extend_from_slice(f_neighbors);
            neighbors.extend_from_slice(r_neighbors);
            neighbors.sort();
            neighbors.dedup();

            let mut children = 0;

            for &v in &neighbors {
                if Some(v) == parent {
                    continue;
                }

                if !state.discovery.contains_key(&v) {
                    children += 1;
                    dfs(v, Some(u), context, state);

                    let low_v = *state.low.get(&v).unwrap_or(&0);
                    let low_u = state.low.entry(u).or_insert(state.time);
                    *low_u = std::cmp::min(*low_u, low_v);

                    let disc_u = *state.discovery.get(&u).unwrap_or(&0);

                    // Articulation point check (non-root)
                    if parent.is_some() && low_v >= disc_u {
                        state.articulation_points.insert(u);
                    }

                    // Bridge check
                    if low_v > disc_u {
                        let mut bridge = (u, v);
                        if bridge.0 > bridge.1 {
                            bridge = (v, u);
                        }
                        state.bridges.push(bridge);
                    }
                } else {
                    let disc_v = *state.discovery.get(&v).unwrap_or(&0);
                    let low_u = state.low.entry(u).or_insert(state.time);
                    *low_u = std::cmp::min(*low_u, disc_v);
                }
            }

            // Articulation point check (root)
            if parent.is_none() && children > 1 {
                state.articulation_points.insert(u);
            }
        }

        for node in sorted_nodes {
            if !state.discovery.contains_key(&node) {
                dfs(node, None, self.context, &mut state);
            }
        }

        let mut articulation_points: Vec<NodeId> = state.articulation_points.into_iter().collect();
        articulation_points.sort();

        let mut bridges = state.bridges;
        bridges.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        ConnectivityReport {
            articulation_points,
            bridges,
        }
    }
}
