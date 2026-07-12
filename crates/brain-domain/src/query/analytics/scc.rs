use crate::identifiers::NodeId;
use crate::query::analytics::{AnalyticsAlgorithm, Complexity, GraphAnalyticsContext, StronglyConnectedComponent};
use std::collections::{HashMap, HashSet};
use std::cmp;

/// Configuration settings for strongly connected components query.
#[derive(Debug, Clone, Default)]
pub struct SccConfig {}

/// Solver identifying directed cyclic clusters using Tarjan's strongly connected components algorithm.
pub struct StronglyConnectedComponents<'a, 'b> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: SccConfig,
}

impl<'a, 'b> StronglyConnectedComponents<'a, 'b> {
    /// Creates a new `StronglyConnectedComponents` solver.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: SccConfig) -> Self {
        Self { context, config }
    }
}

struct TarjanState {
    index: usize,
    indices: HashMap<NodeId, usize>,
    lowlink: HashMap<NodeId, usize>,
    on_stack: HashSet<NodeId>,
    stack: Vec<NodeId>,
    sccs: Vec<StronglyConnectedComponent>,
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for StronglyConnectedComponents<'a, 'b> {
    type Output = Vec<StronglyConnectedComponent>;

    fn algorithm_id(&self) -> &'static str {
        "strongly_connected_components"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Linear
    }

    fn compute(&self) -> Self::Output {
        let graph = self.context.graph();
        let mut state = TarjanState {
            index: 0,
            indices: HashMap::new(),
            lowlink: HashMap::new(),
            on_stack: HashSet::new(),
            stack: Vec::new(),
            sccs: Vec::new(),
        };

        // Sort nodes canonically to traverse in deterministic order
        let mut sorted_nodes: Vec<NodeId> = graph.nodes.keys().cloned().collect();
        sorted_nodes.sort();

        for &node in &sorted_nodes {
            if !state.indices.contains_key(&node) {
                self.strongconnect(node, &mut state);
            }
        }

        // Canonical sort for results:
        // 1. Sort nodes inside each component canonically.
        // 2. Sort components lexicographically based on their first node.
        for scc in &mut state.sccs {
            scc.nodes.sort();
        }
        state.sccs.sort_by(|s1, s2| {
            if s1.nodes.is_empty() || s2.nodes.is_empty() {
                s1.nodes.len().cmp(&s2.nodes.len())
            } else {
                s1.nodes[0].cmp(&s2.nodes[0])
            }
        });

        state.sccs
    }
}

impl<'a, 'b> StronglyConnectedComponents<'a, 'b> {
    fn strongconnect(&self, v: NodeId, state: &mut TarjanState) {
        state.indices.insert(v, state.index);
        state.lowlink.insert(v, state.index);
        state.index += 1;
        state.stack.push(v);
        state.on_stack.insert(v);

        let adjacency = self.context.adjacency();
        for &w in adjacency.neighbors(v) {
            if !state.indices.contains_key(&w) {
                self.strongconnect(w, state);
                let w_low = state.lowlink[&w];
                let v_low = state.lowlink.get_mut(&v).unwrap();
                *v_low = cmp::min(*v_low, w_low);
            } else if state.on_stack.contains(&w) {
                let w_index = state.indices[&w];
                let v_low = state.lowlink.get_mut(&v).unwrap();
                *v_low = cmp::min(*v_low, w_index);
            }
        }

        if state.lowlink[&v] == state.indices[&v] {
            let mut component_nodes = Vec::new();
            while let Some(node) = state.stack.pop() {
                state.on_stack.remove(&node);
                component_nodes.push(node);
                if node == v {
                    break;
                }
            }
            state.sccs.push(StronglyConnectedComponent {
                nodes: component_nodes,
            });
        }
    }
}
