use crate::identifiers::NodeId;
use crate::query::analytics::{
    GraphAnalyticsContext, EdgeWeightProvider,
    heuristic::HeuristicProvider, RoutingAlgorithm
};
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;

/// Configuration parameters for A* Search.
#[derive(Debug, Clone, Default)]
pub struct AStarConfig {}

/// Priority Queue node entry tracking standard A* path costs: $f(x) = g(x) + h(x)$.
#[derive(Debug, PartialEq)]
struct AStarNode {
    node: NodeId,
    f_score: f64,
    g_score: f64,
}

impl Eq for AStarNode {}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Minimum f_score wins; break ties by NodeId for determinism
        other.f_score.partial_cmp(&self.f_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.node.cmp(&self.node))
    }
}

/// A* Search pathfinder utilizing heuristic guidance and weight/distance providers.
pub struct AStar<'a, 'b, W, H> {
    context: &'b GraphAnalyticsContext<'a>,
    #[allow(dead_code)]
    config: AStarConfig,
    weight_provider: W,
    heuristic_provider: H,
}

impl<'a, 'b, W: EdgeWeightProvider, H: HeuristicProvider> AStar<'a, 'b, W, H> {
    /// Creates a new `AStar` pathfinding solver.
    pub fn new(
        context: &'b GraphAnalyticsContext<'a>,
        config: AStarConfig,
        weight_provider: W,
        heuristic_provider: H,
    ) -> Self {
        Self {
            context,
            config,
            weight_provider,
            heuristic_provider,
        }
    }
}

impl<'a, 'b, W: EdgeWeightProvider, H: HeuristicProvider> RoutingAlgorithm<'a, 'b> for AStar<'a, 'b, W, H> {
    type Config = AStarConfig;
    type Result = Option<Vec<NodeId>>;

    fn compute(&self, source: NodeId, target: NodeId) -> Self::Result {
        if source == target {
            return Some(vec![source]);
        }

        let graph = self.context.graph();
        if !graph.nodes.contains_key(&source) || !graph.nodes.contains_key(&target) {
            return None;
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<NodeId, NodeId> = HashMap::new();
        let mut g_scores: HashMap<NodeId, f64> = HashMap::new();

        let h_start = self.heuristic_provider.estimate(source, target, self.context);
        g_scores.insert(source, 0.0);
        open_set.push(AStarNode {
            node: source,
            f_score: h_start,
            g_score: 0.0,
        });

        let adjacency = self.context.adjacency();

        while let Some(current) = open_set.pop() {
            let u = current.node;

            if u == target {
                let mut path = vec![target];
                let mut curr = target;
                while let Some(&prev) = came_from.get(&curr) {
                    path.push(prev);
                    curr = prev;
                }
                path.reverse();
                return Some(path);
            }

            if let Some(&recorded_g) = g_scores.get(&u) {
                if current.g_score > recorded_g {
                    continue;
                }
            }

            for &neighbor in adjacency.neighbors(u) {
                if let Some(edge) = graph.edges.values().find(|e| e.source == u && e.target == neighbor) {
                    let cost = self.weight_provider.weight(edge);
                    let tentative_g = g_scores.get(&u).cloned().unwrap_or(f64::INFINITY) + cost;
                    let existing_g = *g_scores.get(&neighbor).unwrap_or(&f64::INFINITY);

                    let should_update = if tentative_g < existing_g - 1e-9 {
                        true
                    } else if (tentative_g - existing_g).abs() < 1e-9 {
                        if let Some(&existing_pred) = came_from.get(&neighbor) {
                            u < existing_pred
                        } else {
                            true
                        }
                    } else {
                        false
                    };

                    if should_update {
                        came_from.insert(neighbor, u);
                        g_scores.insert(neighbor, tentative_g);
                        
                        let h = self.heuristic_provider.estimate(neighbor, target, self.context);
                        open_set.push(AStarNode {
                            node: neighbor,
                            f_score: tentative_g + h,
                            g_score: tentative_g,
                        });
                    }
                }
            }
        }

        None
    }
}
