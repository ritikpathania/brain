//! Stateful projection engine managing catch-up, checkpoints, and rebuild reduction loops.

/// The projection runner execution logic.
pub mod runner;
/// The reducer registry.
pub mod registry;
/// Background job state reducers.
pub mod jobs;
/// Session state reducers.
pub mod sessions;
/// Search indexing state reducers.
pub mod search;
/// Best-effort advance notifications and bus.
pub mod notification;

use brain_core::errors::BrainError;
use brain_events::EventEnvelope;

pub use notification::{ProjectionId, ProjectionNotification, ProjectionNotificationBus};

/// Contract for stateful projections reducing event envelopes sequentially.
pub trait StateReducer: Send {
    /// Unique identifier identifying this projection.
    fn id(&self) -> ProjectionId;
    /// Reduces a single event envelope to update internal read-model state.
    fn reduce(&mut self, envelope: &EventEnvelope) -> Result<(), BrainError>;
    /// Resets the reducer state back to initial/empty conditions.
    fn reset(&mut self) -> Result<(), BrainError>;
}

pub use runner::ProjectionRunner;
pub use registry::ReducerRegistry;
pub use jobs::JobProjectionReducer;
pub use sessions::SessionProjectionReducer;
pub use search::SearchProjectionReducer;
