use crate::identifiers::NodeId;
use crate::query::analytics::{
    AnalyticsAlgorithm, Complexity, GraphAnalyticsContext, EdgeWeightProvider,
    ClosenessResult, ordering::sort_closeness_canonically
};
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;

/// Supported closeness centrality measurement variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ClosenessVariant {
    /// Classic Bavelas closeness centrality: (V - 1) / sum(d(u, v)). Returns 0 if any node is unreachable.
    Classic,
    /// Harmonic closeness centrality: sum(1 / d(u, v)) / (V - 1). Naturally handles disconnected components.
    Harmonic,
    /// Wasserman-Faust normalization: classic closeness scaled by (reachable_count - 1) / (V - 1).
    WassermanFaust,
}

impl Default for ClosenessVariant {
    fn default() -> Self {
        Self::Harmonic
    }
}

/// Configuration settings for Closeness Centrality.
#[derive(Debug, Clone, Default)]
pub struct ClosenessConfig {
    /// The formula variant to compute.
    pub variant: ClosenessVariant,
}

/// Closeness centrality scorer executing shortest-path exploration across all nodes.
pub struct Closeness<'a, 'b, W> {
    context: &'b GraphAnalyticsContext<'a>,
    config: ClosenessConfig,
    weight_provider: W,
}

impl<'a, 'b, W: EdgeWeightProvider> Closeness<'a, 'b, W> {
    /// Creates a new `Closeness` scoring solver.
    pub fn new(
        context: &'b GraphAnalyticsContext<'a>,
        config: ClosenessConfig,
        weight_provider: W,
    ) -> Self {
        Self {
            context,
            config,
            weight_provider,
        }
    }

    /// Computes the single-source shortest path distances from `source` node to all other nodes.
    fn dijkstra_distances(&self, source: NodeId) -> HashMap<NodeId, f64> {
        let mut distances = HashMap::new();
        let mut min_heap = BinaryHeap::new();

        #[derive(PartialEq)]
        struct State {
            node: NodeId,
            cost: f64,
        }
        impl Eq for State {}
        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                other.cost.partial_cmp(&self.cost)
            }
        }
        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
            }
        }

        distances.insert(source, 0.0);
        min_heap.push(State { node: source, cost: 0.0 });

        let graph = self.context.graph();
        let adjacency = self.context.adjacency();

        while let Some(State { node, cost }) = min_heap.pop() {
            if cost > *distances.get(&node).unwrap_or(&f64::INFINITY) {
                continue;
            }

            for &neighbor in adjacency.neighbors(node) {
                if let Some(edge) = graph.edges.values().find(|e| e.source == node && e.target == neighbor) {
                    let next_cost = cost + self.weight_provider.weight(edge);
                    if next_cost < *distances.get(&neighbor).unwrap_or(&f64::INFINITY) {
                        distances.insert(neighbor, next_cost);
                        min_heap.push(State { node: neighbor, cost: next_cost });
                    }
                }
            }
        }

        distances
    }
}

impl<'a, 'b, W: EdgeWeightProvider> AnalyticsAlgorithm<'a, 'b> for Closeness<'a, 'b, W> {
    type Output = Vec<ClosenessResult>;

    fn algorithm_id(&self) -> &'static str {
        "closeness_centrality"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Quadratic // O(V * (E + V log V))
    }

    fn compute(&self) -> Self::Output {
        let graph = self.context.graph();
        let n = graph.nodes.len();
        if n <= 1 {
            return graph.nodes.keys().map(|&node| ClosenessResult { node, score: 0.0 }).collect();
        }

        let mut results = Vec::with_capacity(n);

        for &node in graph.nodes.keys() {
            let dists = self.dijkstra_distances(node);

            let score = match self.config.variant {
                ClosenessVariant::Classic => {
                    let mut sum_dist = 0.0;
                    let mut all_reachable = true;
                    for &other in graph.nodes.keys() {
                        if other != node {
                            if let Some(&d) = dists.get(&other) {
                                sum_dist += d;
                            } else {
                                all_reachable = false;
                                break;
                            }
                        }
                    }
                    if all_reachable && sum_dist > 0.0 {
                        (n - 1) as f64 / sum_dist
                    } else {
                        0.0
                    }
                }
                ClosenessVariant::Harmonic => {
                    let mut sum_reciprocal = 0.0;
                    for &other in graph.nodes.keys() {
                        if other != node {
                            if let Some(&d) = dists.get(&other) {
                                if d > 0.0 {
                                    sum_reciprocal += 1.0 / d;
                                }
                            }
                        }
                    }
                    sum_reciprocal / (n - 1) as f64
                }
                ClosenessVariant::WassermanFaust => {
                    let mut sum_dist = 0.0;
                    let mut reachable_count = 1;
                    for &other in graph.nodes.keys() {
                        if other != node {
                            if let Some(&d) = dists.get(&other) {
                                sum_dist += d;
                                reachable_count += 1;
                            }
                        }
                    }
                    if reachable_count > 1 && sum_dist > 0.0 {
                        let multiplier = (reachable_count - 1) as f64 / (n - 1) as f64;
                        multiplier * ((reachable_count - 1) as f64 / sum_dist)
                    } else {
                        0.0
                    }
                }
            };

            results.push(ClosenessResult { node, score });
        }

        sort_closeness_canonically(&mut results);
        results
    }
}
