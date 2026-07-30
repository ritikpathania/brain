//! Stateless evaluator for compound hybrid queries.

use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for compound hybrid queries.
pub struct HybridEvaluator;

impl HybridEvaluator {
    /// Evaluates hybrid query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &HybridSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        Ok(QueryFacadeResult {
            matches: vec![],
            total_matched: 0,
            metadata: QueryResponseMetadata {
                execution_duration_us: 0,
                snapshot_watermark: snapshot.watermark().0,
            },
        })
    }
}
