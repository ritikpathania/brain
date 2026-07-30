//! Stateless evaluator for lexical search queries.

use crate::query::errors::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;

/// Stateless evaluator for lexical search queries.
pub struct SearchEvaluator;

impl SearchEvaluator {
    /// Evaluates search query against projection snapshot.
    pub fn evaluate(
        snapshot: &ProjectionSnapshot,
        _query: &LexicalSearchQuery,
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
