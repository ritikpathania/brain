//! In-memory reflection engine \u2014 Sprint 3 reference implementation.
//!
//! This engine is emit-only: it receives a `ReflectionTarget` derived from
//! canonicalization output, builds a `ReflectionCompletedEvent`, dispatches it
//! through the runtime event bus, and invalidates the affected projection instances.
//!
//! No graph mutations are performed. Edge strengthening, weighting, and decay
//! policy belong to a future sprint once those semantics are defined.

use brain_core::{
    events::{EventSource, ProjectionInstanceInvalidatedEvent, RuntimeEventDispatcher},
    reflection::{ReflectionCompletedEvent, ReflectionEngine, ReflectionTarget},
};
use std::sync::Arc;
use std::time::SystemTime;

/// In-memory reference implementation of the `ReflectionEngine` contract.
///
/// Sprint 3 responsibility: examine the affected entity set, emit
/// `ReflectionCompletedEvent`, and signal projection invalidation.
/// No storage interaction, no graph mutations.
pub struct InMemoryReflectionEngine {
    event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
}

impl InMemoryReflectionEngine {
    /// Creates a new `InMemoryReflectionEngine`.
    pub fn new(event_dispatcher: Arc<dyn RuntimeEventDispatcher>) -> Self {
        Self { event_dispatcher }
    }
}

impl ReflectionEngine for InMemoryReflectionEngine {
    type Error = brain_core::errors::BrainError;

    fn reflect(&self, target: ReflectionTarget) -> Result<ReflectionCompletedEvent, Self::Error> {
        let event = ReflectionCompletedEvent {
            epoch: target.epoch,
            entities_reflected: target.affected_entities.clone(),
            correlation_id: target.correlation_id,
            timestamp: SystemTime::now(),
        };

        // Dispatch the durable reflection completion event
        self.event_dispatcher.dispatch(Arc::new(event.clone()));

        // Signal projection invalidation for each affected entity
        for entity_id in &target.affected_entities {
            self.event_dispatcher
                .dispatch(Arc::new(ProjectionInstanceInvalidatedEvent {
                    projection_type: format!("entity:{}", entity_id),
                    epoch: target.epoch,
                    source: EventSource::Reflection,
                    correlation_id: target.correlation_id,
                }));
        }

        Ok(event)
    }
}
