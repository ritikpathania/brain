use crate::query::analytics::{GraphAnalyticsContext, weights::EdgeWeightProvider, RoutingAlgorithm};
use crate::NodeId;
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;


/// Configuration settings for shortest path query.
#[derive(Debug, Clone, Default)]
pub struct ShortestPathConfig {
    /// Maximum path cost threshold.
    pub max_cost: Option<f64>,
}

/// State helper for priority queue sorting (Dijkstra).
#[derive(Debug, Clone, Copy)]
struct State {
    node: NodeId,
    cost: f64,
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        (self.cost - other.cost).abs() < f64::EPSILON && self.node == other.node
    }
}

impl Eq for State {}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap (binary heap is a max-heap by default)
        let cost_cmp = other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal);
        if cost_cmp != Ordering::Equal {
            return cost_cmp;
        }
        // Canonical tie-breaker (lexicographical sorting of NodeId)
        other.node.cmp(&self.node)
    }
}

/// Solver finding the shortest path between nodes using Dijkstra's algorithm.
pub struct ShortestPath<'a, 'b, W: EdgeWeightProvider> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: ShortestPathConfig,
    /// Provider determining cost weight for each edge.
    pub weight_provider: W,
}

impl<'a, 'b, W: EdgeWeightProvider> ShortestPath<'a, 'b, W> {
    /// Creates a new `ShortestPath` Dijkstra solver.
    pub fn new(
        context: &'b GraphAnalyticsContext<'a>,
        config: ShortestPathConfig,
        weight_provider: W,
    ) -> Self {
        Self {
            context,
            config,
            weight_provider,
        }
    }
}

impl<'a, 'b, W: EdgeWeightProvider> RoutingAlgorithm<'a, 'b> for ShortestPath<'a, 'b, W> {
    type Config = ShortestPathConfig;
    type Result = Option<Vec<NodeId>>;

    fn compute(&self, source: NodeId, target: NodeId) -> Self::Result {
        if source == target {
            return Some(vec![source]);
        }

        let mut distances: HashMap<NodeId, f64> = HashMap::new();
        let mut predecessors: HashMap<NodeId, NodeId> = HashMap::new();
        let mut heap = BinaryHeap::new();

        distances.insert(source, 0.0);
        heap.push(State { node: source, cost: 0.0 });

        while let Some(State { node, cost }) = heap.pop() {
            if node == target {
                if let Some(max_c) = self.config.max_cost {
                    if cost > max_c {
                        return None;
                    }
                }
                break;
            }

            if let Some(&best_dist) = distances.get(&node) {
                if cost > best_dist + f64::EPSILON {
                    continue;
                }
            }

            // Find all outgoing edges from the node
            for edge in self.context.graph().edges.values() {
                if edge.source == node {
                    let next = edge.target;
                    let edge_cost = self.weight_provider.weight(edge);
                    if edge_cost.is_infinite() || edge_cost < 0.0 {
                        continue;
                    }
                    let next_cost = cost + edge_cost;

                    if let Some(max_c) = self.config.max_cost {
                        if next_cost > max_c {
                            continue;
                        }
                    }

                    let should_update = match distances.get(&next) {
                        Some(&current_best) => {
                            if next_cost < current_best - f64::EPSILON {
                                true
                            } else if (next_cost - current_best).abs() < f64::EPSILON {
                                // Deterministic tie-breaker: prefer lexicographically smaller predecessor node
                                if let Some(&existing_pred) = predecessors.get(&next) {
                                    node < existing_pred
                                } else {
                                    true
                                }
                            } else {
                                false
                            }
                        }
                        None => true,
                    };

                    if should_update {
                        distances.insert(next, next_cost);
                        predecessors.insert(next, node);
                        heap.push(State { node: next, cost: next_cost });
                    }
                }
            }
        }

        if !distances.contains_key(&target) {
            return None;
        }

        // Reconstruct the path backwards
        let mut path = Vec::new();
        let mut current = target;
        path.push(current);
        while let Some(&pred) = predecessors.get(&current) {
            path.push(pred);
            current = pred;
        }
        path.reverse();
        Some(path)
    }
}
