//! Traits defining the contracts for producing and consuming ingestion events.

use crate::envelope::IngestionEnvelope;
use crate::replay::ReplayPosition;
use brain_domain::{AdapterId, EventId};

/// Trait implemented by adapter clients that produce events.
pub trait EventSource: Send + Sync {
    /// Returns the adapter identifier.
    fn adapter_id(&self) -> &AdapterId;

    /// Returns the current replay position.
    fn replay_position(&self) -> ReplayPosition;
}

/// Trait implemented by the daemon to consume events.
pub trait EventSink: Send + Sync {
    /// Accepts an ingestion envelope. Returns the assigned sequence number.
    /// The implementation must durably persist the event before returning.
    fn accept(&self, envelope: IngestionEnvelope) -> Result<u64, EventSinkError>;

    /// Accepts a batch of envelopes atomically.
    fn accept_batch(&self, envelopes: Vec<IngestionEnvelope>) -> Result<Vec<u64>, EventSinkError>;

    /// Acknowledges replay position for an adapter.
    fn acknowledge(&self, position: ReplayPosition) -> Result<(), EventSinkError>;
}

/// Errors returned by the EventSink during ingestion.
#[derive(Debug, thiserror::Error)]
pub enum EventSinkError {
    /// The event has already been ingested.
    #[error("duplicate event: {event_id}")]
    DuplicateEvent {
        /// Unique event ID of the duplicate.
        event_id: EventId,
    },

    /// The client's event model version is not supported.
    #[error("schema version mismatch: expected {expected}, got {actual}")]
    SchemaVersionMismatch {
        /// Expected Event Model Version.
        expected: String,
        /// Actual Event Model Version sent.
        actual: String,
    },

    /// A storage system failure.
    #[error("storage error: {0}")]
    Storage(String),

    /// Backpressure triggered when processing queue is full.
    #[error("backpressure: queue full, retry after {retry_after_ms}ms")]
    Backpressure {
        /// Retry delay parameter.
        retry_after_ms: u64,
    },
}
