//! Entity Statistics Projection models, state, and reducer.

/// Data models for Entity Statistics Projection.
pub mod models;
pub use models::EntityStatistics;
/// In-memory entity statistics state.
pub mod state;
pub use state::EntityStatisticsState;
