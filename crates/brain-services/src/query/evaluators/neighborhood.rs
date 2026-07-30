//! Stateless evaluator for node neighborhood graph traversal.

use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for node neighborhood graph traversal.
pub struct NeighborhoodEvaluator;

impl NeighborhoodEvaluator {
    /// Evaluates neighborhood query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &NeighborhoodQuery,
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
