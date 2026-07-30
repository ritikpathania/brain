//! Entity Statistics Projection models, state, and reducer.

/// Data models for Entity Statistics Projection.
pub mod models;
pub use models::EntityStatistics;
/// Pure domain reducer for Entity Statistics Projection.
pub mod reducer;
pub use reducer::EntityStatisticsReducer;
/// In-memory entity statistics state.
pub mod state;
pub use state::EntityStatisticsState;
