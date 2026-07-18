//! Reflection engine contracts.
//!
//! Reflection is the process of examining entities that were created or mutated by
//! canonicalization and emitting a structured event summarising what was observed.
//!
//! Sprint 3 scope: emit-only. No graph mutations, no storage writes.
//! Relationship strengthening, edge weighting, and decay policy belong to a later sprint.

use crate::events::{CorrelationId, RuntimeEvent};
use brain_domain::{EpochId, NodeId};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// The set of entities to reflect over, derived from a `CanonicalizationResult`.
///
/// This type is the sole input to the reflection engine. It deliberately carries
/// no storage handles or repository references — reflection observes the target
/// and emits events; it does not mutate state.
#[derive(Debug, Clone)]
pub struct ReflectionTarget {
    /// Entities that were created or mutated by the preceding canonicalization.
    pub affected_entities: Vec<NodeId>,
    /// Monotonic epoch of the graph after canonicalization committed.
    pub epoch: EpochId,
    /// Causal tracing identifier inherited from the originating observation.
    pub correlation_id: CorrelationId,
}

/// Immutable domain event emitted when a reflection pass completes.
///
/// This event is a durable business fact: it records which entities were examined
/// and at which epoch. Downstream subscribers (projectors, observability, etc.)
/// use it to invalidate caches or update timelines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionCompletedEvent {
    /// Monotonic epoch at which reflection ran.
    pub epoch: EpochId,
    /// Entities that were examined during this reflection pass.
    pub entities_reflected: Vec<NodeId>,
    /// Causal tracing identifier.
    pub correlation_id: CorrelationId,
    /// Timestamp when reflection completed.
    pub timestamp: SystemTime,
}

impl RuntimeEvent for ReflectionCompletedEvent {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Contract for the reflection engine.
///
/// Implementations receive a `ReflectionTarget` derived from canonicalization output
/// and return a `ReflectionCompletedEvent`. The engine must not accept repository
/// references or perform storage mutations — any data it needs for future richer
/// implementations should be obtained through injected abstractions, not embedded
/// storage access.
pub trait ReflectionEngine: Send + Sync + 'static {
    /// Associated error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Performs a reflection pass over the target entities and returns a completion event.
    fn reflect(&self, target: ReflectionTarget) -> Result<ReflectionCompletedEvent, Self::Error>;
}
