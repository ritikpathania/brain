use crate::entities::ProvenanceSource;
use crate::query::analytics::{AnalyticsAlgorithm, Complexity, ProvenanceStats, GraphAnalyticsContext};

/// Configuration settings for provenance statistics calculations.
#[derive(Debug, Clone, Default)]
pub struct ProvenanceConfig {}

/// Solver aggregating edge counts grouped by provenance source.
pub struct ProvenanceStatistics<'a, 'b> {
    /// Reference to the shared graph analytics context.
    pub context: &'b GraphAnalyticsContext<'a>,
    /// Configuration parameter value object.
    pub config: ProvenanceConfig,
}

impl<'a, 'b> ProvenanceStatistics<'a, 'b> {
    /// Creates a new `ProvenanceStatistics` solver.
    pub fn new(context: &'b GraphAnalyticsContext<'a>, config: ProvenanceConfig) -> Self {
        Self { context, config }
    }
}

impl<'a, 'b> AnalyticsAlgorithm<'a, 'b> for ProvenanceStatistics<'a, 'b> {
    type Output = ProvenanceStats;

    fn algorithm_id(&self) -> &'static str {
        "provenance_statistics"
    }

    fn complexity(&self) -> Complexity {
        Complexity::Linear
    }

    fn compute(&self) -> Self::Output {
        let mut stats = ProvenanceStats {
            total_extracted: 0,
            total_inferred: 0,
            total_user_authored: 0,
            total_imported: 0,
        };
        for edge in self.context.graph().edges.values() {
            match edge.provenance.source {
                ProvenanceSource::Extracted => stats.total_extracted += 1,
                ProvenanceSource::Inferred => stats.total_inferred += 1,
                ProvenanceSource::UserAuthored => stats.total_user_authored += 1,
                ProvenanceSource::Imported => stats.total_imported += 1,
            }
        }
        stats
    }
}
