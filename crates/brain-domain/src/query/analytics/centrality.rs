use crate::query::analytics::{AnalyticsAlgorithm, Complexity, DegreeCentrality, GraphAnalyticsContext, ordering::sort_centrality_canonically};

/// Configuration settings for degree centrality calculations.
#[derive(Debug, Clone, Default)]
pub struct CentralityConfig {}

/// Solver calculating degree centrality score for all nodes in the graph.
pub struct Centrality<'a, 'b> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: CentralityConfig,
}

impl<'a, 'b> Centrality<'a, 'b> {
    /// Creates a new `Centrality` solver.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: CentralityConfig) -> Self {
        Self { context, config }
    }
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for Centrality<'a, 'b> {
    type Output = Vec<DegreeCentrality>;

    fn algorithm_id(&self) -> &'static str {
        "degree_centrality"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Linear
    }

    fn compute(&self) -> Self::Output {
        let degrees = self.context.degrees();
        let mut results: Vec<DegreeCentrality> = self.context
            .graph()
            .nodes
            .keys()
            .map(|node| DegreeCentrality {
                node: *node,
                score: degrees.total_degree(*node),
            })
            .collect();

        sort_centrality_canonically(&mut results);
        results
    }
}
