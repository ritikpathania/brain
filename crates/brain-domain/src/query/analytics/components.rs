use crate::identifiers::NodeId;
use crate::query::analytics::{AnalyticsAlgorithm, Complexity, GraphAnalyticsContext, ordering::sort_components_canonically};
use std::collections::{HashSet, VecDeque};

/// Configuration settings for connected components analysis.
#[derive(Debug, Clone, Default)]
pub struct ConnectedComponentsConfig {}

/// Solver computing connected components of a graph treated as undirected.
pub struct ConnectedComponents<'a, 'b> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: ConnectedComponentsConfig,
}

impl<'a, 'b> ConnectedComponents<'a, 'b> {
    /// Creates a new `ConnectedComponents` solver.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: ConnectedComponentsConfig) -> Self {
        Self { context, config }
    }
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for ConnectedComponents<'a, 'b> {
    type Output = Vec<Vec<NodeId>>;

    fn algorithm_id(&self) -> &'static str {
        "connected_components"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Linear
    }

    fn compute(&self) -> Self::Output {
        let mut visited = HashSet::new();
        let mut components = Vec::new();

        let adjacency = self.context.adjacency();
        let reverse_adjacency = self.context.reverse_adjacency();

        // Sort graph nodes to traverse in deterministic order
        let mut nodes: Vec<NodeId> = self.context.graph().nodes.keys().cloned().collect();
        nodes.sort();

        for node in nodes {
            if visited.contains(&node) {
                continue;
            }
            let mut component = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(node);
            visited.insert(node);

            while let Some(curr) = queue.pop_front() {
                component.push(curr);

                // Collect forward and reverse neighbors
                let f_neighbors = adjacency.neighbors(curr);
                let r_neighbors = reverse_adjacency.predecessors(curr);

                // Combine and sort canonically
                let mut combined_neighbors = Vec::with_capacity(f_neighbors.len() + r_neighbors.len());
                combined_neighbors.extend_from_slice(f_neighbors);
                combined_neighbors.extend_from_slice(r_neighbors);
                combined_neighbors.sort();
                combined_neighbors.dedup();

                for neighbor in combined_neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
            components.push(component);
        }

        sort_components_canonically(&mut components);
        components
    }
}
