//! Stateless evaluator for point-in-time entity state lookups.

use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for point-in-time entity state lookups.
pub struct TemporalEvaluator;

impl TemporalEvaluator {
    /// Evaluates point-in-time query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &PointInTimeQuery,
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
