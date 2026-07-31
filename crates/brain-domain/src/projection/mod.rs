//! Phase 3 & 4 Projection Runtime domain models, traits, and value objects.

/// Immutable projection checkpoint value object.
pub mod checkpoint;
/// Automated projection conformance testing framework.
pub mod conformance;
/// Entity Statistics Projection models, state, and reducer.
pub mod entity_statistics;
/// Typed projection error hierarchy.
pub mod errors;
/// Graph Adjacency Projection models, state, and reducer.
pub mod graph_adjacency;
/// Projection identifier and version models.
pub mod id;
/// Pure domain projection reducer contract.
pub mod reducer;
/// Search Index Projection models, state, and reducer.
pub mod search_index;
/// Temporal State Projection models, state, and reducer.
pub mod temporal_state;
/// Event stream sequence watermark.
pub mod watermark;

pub use checkpoint::*;
pub use conformance::*;
pub use entity_statistics::{EntityStatistics, EntityStatisticsReducer, EntityStatisticsState};
pub use errors::*;
pub use graph_adjacency::{
    EdgeRecord, GraphAdjacencyReducer, GraphAdjacencyState, GraphEdgeId, GraphNodeId, NodeDegree,
};
pub use id::*;
pub use reducer::*;
pub use search_index::{SearchIndexReducer, SearchIndexState, SearchToken};
pub use temporal_state::{TemporalFactId, TemporalRecord, TemporalState, TemporalStateReducer};
pub use watermark::*;
