use crate::identifiers::NodeId;
use crate::query::analytics::GraphAnalyticsContext;

/// Trait defining search heuristic estimates to guide pathfinding algorithms.
///
/// **Contract**:
/// - The returned estimate must be **non-negative** (i.e., $h(u, v) \geq 0.0$).
/// - To guarantee optimality of shortest paths, the returned estimate must be **admissible**
///   (it never overestimates the actual cost to the destination node, i.e., $h(u, v) \leq d(u, v)$).
/// - Ideally, the heuristic should be **consistent** (monotone), satisfying the triangle inequality:
///   $h(u, target) \leq \text{weight}(u, v) + h(v, target)$.
pub trait HeuristicProvider {
    /// Returns the estimated cost/distance from the current node `from` to the target node `to`.
    fn estimate(&self, from: NodeId, to: NodeId, context: &GraphAnalyticsContext) -> f64;
}

/// Zero-heuristic mapping always returning 0.0, rendering A* equivalent to Dijkstra.
/// Always admissible and consistent.
#[derive(Debug, Clone, Default)]
pub struct ZeroHeuristic;

impl HeuristicProvider for ZeroHeuristic {
    fn estimate(&self, _from: NodeId, _to: NodeId, _context: &GraphAnalyticsContext) -> f64 {
        0.0
    }
}
