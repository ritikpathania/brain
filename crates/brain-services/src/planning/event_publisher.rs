//! Generic Fallible `EventPublisher<E>` Trait & `EventPublishError` (Phase 10 Milestone 10.4).
//!
//! ### Architectural Invariants:
//! 1. Fallible Interface: `EventPublisher::publish` returns `Result<(), EventPublishError>`, decoupling event emission from storage/transport mechanics cleanly.
//! 2. Generic Scope: `EventPublisher<E>` provides a shared foundation for all control plane event streams (`LeadershipEvent`, `ClusterEvent`, `SchedulingEvent`).
//! 3. Publisher Failure Propagation: Fallible publisher errors propagate back to orchestrators without silent failure or state corruption.

use serde::{Deserialize, Serialize};

/// Strongly-typed error classification for fallible event publishing operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventPublishError {
    /// Underlying storage engine encountered an error.
    StorageError(String),
    /// Transport channel closed unexpectedly.
    ChannelClosed,
    /// Event schema version mismatch or unsupported.
    SchemaVersionMismatch {
        /// Supported maximum schema version.
        expected: u16,
        /// Provided event schema version.
        found: u16,
    },
}

impl std::fmt::Display for EventPublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StorageError(msg) => write!(f, "Event storage failure: {}", msg),
            Self::ChannelClosed => write!(f, "Event transport channel closed"),
            Self::SchemaVersionMismatch { expected, found } => {
                write!(
                    f,
                    "Schema version mismatch: found {}, expected <={}",
                    found, expected
                )
            }
        }
    }
}

impl std::error::Error for EventPublishError {}

/// Generic trait defining fallible event publication contract.
pub trait EventPublisher<E>: Send + Sync {
    /// Publishes an event item, returning `Result<(), EventPublishError>`.
    fn publish(&mut self, event: E) -> Result<(), EventPublishError>;
}

/// In-memory reference implementation buffering published event items.
#[derive(Debug, Clone)]
pub struct InMemoryEventPublisher<E> {
    events: Vec<E>,
}

impl<E> Default for InMemoryEventPublisher<E> {
    fn default() -> Self {
        Self { events: Vec::new() }
    }
}

impl<E> InMemoryEventPublisher<E> {
    /// Instantiates a new `InMemoryEventPublisher`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a slice of all published events.
    pub fn events(&self) -> &[E] {
        &self.events
    }
}

impl<E: Clone + Send + Sync> EventPublisher<E> for InMemoryEventPublisher<E> {
    fn publish(&mut self, event: E) -> Result<(), EventPublishError> {
        self.events.push(event);
        Ok(())
    }
}
