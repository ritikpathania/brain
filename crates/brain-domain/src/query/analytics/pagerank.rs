use crate::identifiers::NodeId;
use crate::query::analytics::{
    AnalyticsAlgorithm, Complexity, GraphAnalyticsContext, PageRankResult,
};
use std::collections::HashMap;

/// Configuration settings for PageRank calculation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PageRankConfig {
    /// Damping factor (typically 0.85).
    pub damping: f64,
    /// Maximum number of iterations.
    pub max_iterations: usize,
    /// Convergence tolerance.
    pub tolerance: f64,
}

impl Default for PageRankConfig {
    fn default() -> Self {
        Self {
            damping: 0.85,
            max_iterations: 100,
            tolerance: 1e-6,
        }
    }
}

/// Solver calculating PageRank centrality score for all nodes in the graph.
pub struct PageRank<'a, 'b> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: PageRankConfig,
}

impl<'a, 'b> PageRank<'a, 'b> {
    /// Creates a new `PageRank` solver.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: PageRankConfig) -> Self {
        Self { context, config }
    }
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for PageRank<'a, 'b> {
    type Output = Vec<PageRankResult>;

    fn algorithm_id(&self) -> &'static str {
        "pagerank"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Quadratic
    }

    fn compute(&self) -> Self::Output {
        let graph = self.context.graph();
        let num_nodes = graph.nodes.len();
        if num_nodes == 0 {
            return Vec::new();
        }

        let n_f64 = num_nodes as f64;
        let mut pr: HashMap<NodeId, f64> = HashMap::new();
        for &node in graph.nodes.keys() {
            pr.insert(node, 1.0 / n_f64);
        }

        let damping = self.config.damping;
        let base_score = (1.0 - damping) / n_f64;

        let adjacency = self.context.adjacency();
        let degrees = self.context.degrees();

        // Get sorted nodes to iterate deterministically
        let mut sorted_nodes: Vec<NodeId> = graph.nodes.keys().cloned().collect();
        sorted_nodes.sort();

        for _ in 0..self.config.max_iterations {
            let mut next_pr = HashMap::new();
            for &node in &sorted_nodes {
                next_pr.insert(node, base_score);
            }

            // Distribute dangling node score contribution
            let mut dangling_sum = 0.0;
            for &node in &sorted_nodes {
                if degrees.out_degree(node) == 0 {
                    dangling_sum += pr[&node];
                }
            }
            let dangling_share = (damping * dangling_sum) / n_f64;
            for &node in &sorted_nodes {
                *next_pr.get_mut(&node).unwrap() += dangling_share;
            }

            // Distribute regular edge contributions
            for &node in &sorted_nodes {
                let out_deg = degrees.out_degree(node);
                if out_deg > 0 {
                    let score_share = (damping * pr[&node]) / (out_deg as f64);
                    for &target in adjacency.neighbors(node) {
                        if let Some(val) = next_pr.get_mut(&target) {
                            *val += score_share;
                        }
                    }
                }
            }

            // Check convergence (L1 norm distance)
            let mut diff = 0.0;
            for &node in &sorted_nodes {
                diff += (next_pr[&node] - pr[&node]).abs();
            }

            pr = next_pr;

            if diff < self.config.tolerance {
                break;
            }
        }

        // Convert to PageRankResult list
        let mut results: Vec<PageRankResult> = pr
            .into_iter()
            .map(|(node, score)| PageRankResult { node, score })
            .collect();

        // Sort descending by score, tie-break lexicographically by node ID
        results.sort_by(|r1, r2| {
            let score_cmp = r2
                .score
                .partial_cmp(&r1.score)
                .unwrap_or(std::cmp::Ordering::Equal);
            if score_cmp != std::cmp::Ordering::Equal {
                return score_cmp;
            }
            r1.node.cmp(&r2.node)
        });

        results
    }
}
