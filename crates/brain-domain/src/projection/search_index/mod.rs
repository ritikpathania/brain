//! Search Index Projection models, state, and reducer.

/// Data models for Search Index Projection.
pub mod models;
pub use models::SearchToken;
/// Pure domain reducer for Search Index Projection.
pub mod reducer;
pub use reducer::SearchIndexReducer;
/// In-memory inverted search index state.
pub mod state;
pub use state::SearchIndexState;
