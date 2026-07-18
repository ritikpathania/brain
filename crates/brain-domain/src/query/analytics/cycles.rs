use crate::identifiers::NodeId;
use crate::query::analytics::{AnalyticsAlgorithm, Complexity, GraphAnalyticsContext};
use std::collections::HashSet;

/// Configuration settings for cycle detection query.
#[derive(Debug, Clone, Default)]
pub struct CycleDetectionConfig {
    /// Limit the maximum number of cycles to report.
    pub max_cycles: Option<usize>,
}

/// Solver finding all directed cycles in the graph canonically.
pub struct CycleDetector<'a, 'b> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: CycleDetectionConfig,
}

impl<'a, 'b> CycleDetector<'a, 'b> {
    /// Creates a new `CycleDetector`.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: CycleDetectionConfig) -> Self {
        Self { context, config }
    }

    /// Helper to canonicalize a cycle path representation.
    /// 1. Find the lexicographically smallest NodeId in the cycle.
    /// 2. Rotate the cycle path so that this node appears first.
    pub fn canonicalize_cycle(mut cycle: Vec<NodeId>) -> Vec<NodeId> {
        if cycle.is_empty() {
            return cycle;
        }
        let mut min_idx = 0;
        for i in 1..cycle.len() {
            if cycle[i] < cycle[min_idx] {
                min_idx = i;
            }
        }
        cycle.rotate_left(min_idx);
        cycle
    }

    fn dfs_cycle(
        &self,
        start: NodeId,
        curr: NodeId,
        path: &mut Vec<NodeId>,
        visited: &mut HashSet<NodeId>,
        all_cycles: &mut Vec<Vec<NodeId>>,
    ) {
        if let Some(max_c) = self.config.max_cycles {
            if all_cycles.len() >= max_c {
                return;
            }
        }

        let adjacency = self.context.adjacency();
        for &next in adjacency.neighbors(curr) {
            if next == start {
                if !path.is_empty() {
                    all_cycles.push(path.clone());
                }
            } else if next > start && !visited.contains(&next) {
                visited.insert(next);
                path.push(next);
                self.dfs_cycle(start, next, path, visited, all_cycles);
                path.pop();
                visited.remove(&next);
            }
        }
    }
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for CycleDetector<'a, 'b> {
    type Output = Vec<Vec<NodeId>>;

    fn algorithm_id(&self) -> &'static str {
        "cycle_detection"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Exponential
    }

    fn compute(&self) -> Self::Output {
        let graph = self.context.graph();
        let mut all_cycles = Vec::new();
        let mut path = Vec::new();
        let mut visited = HashSet::new();

        // Sort nodes canonically to traverse in deterministic order
        let mut nodes: Vec<NodeId> = graph.nodes.keys().cloned().collect();
        nodes.sort();

        for &start in &nodes {
            path.push(start);
            self.dfs_cycle(start, start, &mut path, &mut visited, &mut all_cycles);
            path.pop();
        }

        // Canonicalize and sort the list of cycle paths lexicographically
        let mut canonical_cycles: Vec<Vec<NodeId>> = all_cycles
            .into_iter()
            .map(Self::canonicalize_cycle)
            .collect();

        canonical_cycles.sort();
        canonical_cycles.dedup();
        canonical_cycles
    }
}
