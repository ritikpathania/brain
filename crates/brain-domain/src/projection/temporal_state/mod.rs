//! Temporal State Projection models, state, and reducer.

/// Data models for Temporal State Projection.
pub mod models;
pub use models::{TemporalFactId, TemporalRecord};
/// In-memory temporal state.
pub mod state;
pub use state::TemporalState;
