//! Graph Adjacency Projection models, state, and reducer.

/// Data models for Graph Adjacency Projection.
pub mod models;
pub use models::{EdgeRecord, GraphEdgeId, GraphNodeId, NodeDegree};
/// In-memory graph adjacency state.
pub mod state;
pub use state::GraphAdjacencyState;
/// Pure domain reducer for Graph Adjacency Projection.
pub mod reducer;
pub use reducer::GraphAdjacencyReducer;
