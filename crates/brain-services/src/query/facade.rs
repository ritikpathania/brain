//! Single thread-safe read query composition facade over atomic projection snapshots.

use crate::query::errors::*;
use crate::query::evaluators::*;
use crate::query::models::*;
use crate::query::snapshot::ProjectionSnapshot;
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::time::Instant;

/// Single thread-safe read query composition facade over atomic projection snapshots.
#[derive(Debug)]
pub struct KnowledgeQueryFacade {
    snapshot: ArcSwap<ProjectionSnapshot>,
}

impl KnowledgeQueryFacade {
    /// Constructs a KnowledgeQueryFacade initialized with a projection snapshot.
    pub fn new(snapshot: Arc<ProjectionSnapshot>) -> Self {
        Self {
            snapshot: ArcSwap::new(snapshot),
        }
    }

    /// Atomically updates the active snapshot.
    pub fn update_snapshot(&self, new_snapshot: Arc<ProjectionSnapshot>) {
        self.snapshot.store(new_snapshot);
    }

    /// Obtains an immutable reference to the active projection snapshot.
    pub fn active_snapshot(&self) -> Arc<ProjectionSnapshot> {
        self.snapshot.load_full()
    }

    /// Evaluates a node neighborhood graph traversal query.
    pub fn query_neighborhood(
        &self,
        query: &NeighborhoodQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let start = Instant::now();
        let snapshot = self.active_snapshot();
        let mut result = NeighborhoodEvaluator::evaluate(&snapshot, query)?;
        result.metadata.execution_duration_us = start.elapsed().as_micros() as u64;
        Ok(result)
    }

    /// Evaluates a point-in-time entity state query.
    pub fn query_point_in_time(
        &self,
        query: &PointInTimeQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let start = Instant::now();
        let snapshot = self.active_snapshot();
        let mut result = TemporalEvaluator::evaluate(&snapshot, query)?;
        result.metadata.execution_duration_us = start.elapsed().as_micros() as u64;
        Ok(result)
    }

    /// Evaluates a lexical search query.
    pub fn query_search(
        &self,
        query: &LexicalSearchQuery,
    ) -> Result<QueryFacadeResult, QueryError> {
        let start = Instant::now();
        let snapshot = self.active_snapshot();
        let mut result = SearchEvaluator::evaluate(&snapshot, query)?;
        result.metadata.execution_duration_us = start.elapsed().as_micros() as u64;
        Ok(result)
    }

    /// Evaluates a compound hybrid query.
    pub fn query_hybrid(&self, query: &HybridSearchQuery) -> Result<QueryFacadeResult, QueryError> {
        let start = Instant::now();
        let snapshot = self.active_snapshot();
        let mut result = HybridEvaluator::evaluate(&snapshot, query)?;
        result.metadata.execution_duration_us = start.elapsed().as_micros() as u64;
        Ok(result)
    }
}
