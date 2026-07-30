//! Atomic ProjectionSnapshot container for Phase 5 Query Facade.

use brain_domain::projection::entity_statistics::EntityStatisticsState;
use brain_domain::projection::graph_adjacency::GraphAdjacencyState;
use brain_domain::projection::search_index::SearchIndexState;
use brain_domain::projection::temporal_state::TemporalState;
use brain_domain::projection::Watermark;
use std::sync::Arc;

/// Atomic, immutable snapshot of all four domain read models and stream watermark.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionSnapshot {
    graph_adjacency: Arc<GraphAdjacencyState>,
    temporal_state: Arc<TemporalState>,
    entity_statistics: Arc<EntityStatisticsState>,
    search_index: Arc<SearchIndexState>,
    watermark: Watermark,
}

impl ProjectionSnapshot {
    /// Constructs a new immutable ProjectionSnapshot wrapping shared read model references and watermark.
    pub fn new(
        graph_adjacency: Arc<GraphAdjacencyState>,
        temporal_state: Arc<TemporalState>,
        entity_statistics: Arc<EntityStatisticsState>,
        search_index: Arc<SearchIndexState>,
        watermark: Watermark,
    ) -> Self {
        Self {
            graph_adjacency,
            temporal_state,
            entity_statistics,
            search_index,
            watermark,
        }
    }

    /// Constructs an empty bootstrap ProjectionSnapshot.
    pub fn empty(watermark: Watermark) -> Self {
        Self {
            graph_adjacency: Arc::new(GraphAdjacencyState::default()),
            temporal_state: Arc::new(TemporalState::default()),
            entity_statistics: Arc::new(EntityStatisticsState::default()),
            search_index: Arc::new(SearchIndexState::default()),
            watermark,
        }
    }

    /// Accessor for Graph Adjacency read model state.
    pub fn graph(&self) -> &GraphAdjacencyState {
        self.graph_adjacency.as_ref()
    }

    /// Accessor for Temporal State read model state.
    pub fn temporal(&self) -> &TemporalState {
        self.temporal_state.as_ref()
    }

    /// Accessor for Entity Statistics read model state.
    pub fn statistics(&self) -> &EntityStatisticsState {
        self.entity_statistics.as_ref()
    }

    /// Accessor for Search Index read model state.
    pub fn search(&self) -> &SearchIndexState {
        self.search_index.as_ref()
    }

    /// Accessor for Snapshot Watermark.
    pub fn watermark(&self) -> Watermark {
        self.watermark
    }
}
