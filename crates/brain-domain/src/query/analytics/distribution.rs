use crate::identifiers::RelationId;
use crate::query::analytics::{
    ordering::sort_distribution_canonically, AnalyticsAlgorithm, Complexity, GraphAnalyticsContext,
    RelationDistribution,
};
use std::collections::HashMap;

/// Configuration settings for relation distribution analysis.
#[derive(Debug, Clone, Default)]
pub struct DistributionConfig {}

/// Solver calculating relation kind occurrence statistics.
pub struct Distribution<'a, 'b> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: DistributionConfig,
}

impl<'a, 'b> Distribution<'a, 'b> {
    /// Creates a new `Distribution` solver.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: DistributionConfig) -> Self {
        Self { context, config }
    }
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for Distribution<'a, 'b> {
    type Output = Vec<RelationDistribution>;

    fn algorithm_id(&self) -> &'static str {
        "relation_distribution"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Linear
    }

    fn compute(&self) -> Self::Output {
        let mut counts: HashMap<RelationId, usize> = HashMap::new();
        for edge in self.context.graph().edges.values() {
            *counts.entry(edge.relation.id()).or_default() += 1;
        }

        let mut results: Vec<RelationDistribution> = counts
            .into_iter()
            .map(|(relation, count)| RelationDistribution { relation, count })
            .collect();

        sort_distribution_canonically(&mut results);
        results
    }
}
