//! Stateful projection engine managing catch-up, checkpoints, and rebuild reduction loops.

/// Background job state reducers.
pub mod jobs;
/// Best-effort advance notifications and bus.
pub mod notification;
/// The reducer registry.
pub mod registry;
/// The projection runner execution logic.
pub mod runner;
/// Projection background execution scheduler.
pub mod scheduler;
/// Search indexing state reducers.
pub mod search;
/// Session state reducers.
pub mod sessions;

use brain_core::errors::BrainError;
use brain_events::EventEnvelope;

pub use notification::{ProjectionId, ProjectionNotification, ProjectionNotificationBus};

/// Contract for stateful projections reducing event envelopes sequentially.
pub trait StateReducer: Send + Sync {
    /// Unique identifier identifying this projection.
    fn id(&self) -> ProjectionId;
    /// Current logical schema/code version of this projection logic.
    fn version(&self) -> u32;
    /// Reduces a single event envelope to update internal read-model state.
    /// Runs atomically inside the active transaction on the provided connection.
    fn reduce(
        &self,
        conn: &rusqlite::Connection,
        envelope: &EventEnvelope,
    ) -> Result<(), BrainError>;
    /// Resets the reducer state back to initial/empty conditions.
    /// Runs atomically inside the active transaction on the provided connection.
    fn reset(&self, conn: &rusqlite::Connection) -> Result<(), BrainError>;
}

pub use jobs::JobProjectionReducer;
pub use registry::ReducerRegistry;
pub use runner::ProjectionRunner;
pub use search::SearchProjectionReducer;
pub use sessions::SessionProjectionReducer;

/// Configuration for projection execution batching and queue capacity.
#[derive(Debug, Clone)]
pub struct ProjectionConfig {
    /// Maximum number of events to process in a single transaction batch.
    pub batch_size: usize,
    /// Maximum duration to await batch completion (unused for sync execution).
    pub max_batch_duration_ms: u64,
    /// Capacity of the signal channel.
    pub queue_capacity: usize,
}

/// Metadata representation of a projection's status and health.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProjectionMetadata {
    /// Unique name of the projection.
    pub name: String,
    /// Schema/logic version of the projection.
    pub version: u32,
    /// Last successfully processed sequence number.
    pub last_sequence: u64,
    /// Current health status.
    pub status: brain_storage::ProjectionStatus,
    /// Last error detail, if status is Failed.
    pub last_error: Option<String>,
    /// Epoch timestamp of the last status update in seconds.
    pub updated_at: u64,
}

/// Command interface for executing and managing stateful projections.
pub trait ProjectionScheduler: Send + Sync {
    /// Catches up all registered projections to the latest WAL event sequence.
    fn catch_up_all(&self) -> Result<(), BrainError>;

    /// Catches up a specific projection.
    fn catch_up_projection(&self, id: ProjectionId) -> Result<(), BrainError>;

    /// Rebuilds a specific projection from sequence 0.
    fn rebuild_projection(&self, id: ProjectionId) -> Result<(), BrainError>;

    /// Rebuilds all registered projections from sequence 0.
    fn rebuild_all(&self) -> Result<(), BrainError>;

    /// Lists metadata status for all registered projections.
    fn list_metadata(&self) -> Result<Vec<ProjectionMetadata>, BrainError>;
}

pub use scheduler::{SchedulerRuntime, SequentialScheduler};
