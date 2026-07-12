//! Transport-agnostic publisher interface and concrete system event bus mappings.

use std::sync::Arc;
use brain_core::errors::BrainError;
use brain_domain::DomainEvent;
use brain_events::{EventEnvelope, EventPublisher, EventLog};
use brain_storage::SqliteEventLog;

/// Interface for publishing core domain events to downstream subscribers.
pub trait DomainEventPublisher: Send + Sync {
    /// Publish a core domain event.
    fn publish(&self, event: DomainEvent);
}

/// Publisher that wraps the system-wide EventPublisher, translating domain events into EventEnvelopes.
pub struct SystemDomainEventPublisher {
    inner: Arc<dyn EventPublisher>,
    source: String,
}

impl SystemDomainEventPublisher {
    /// Creates a new `SystemDomainEventPublisher` instance.
    pub fn new(inner: Arc<dyn EventPublisher>, source: String) -> Self {
        Self { inner, source }
    }
}

impl DomainEventPublisher for SystemDomainEventPublisher {
    fn publish(&self, event: DomainEvent) {
        let payload = brain_events::DomainEvent::Core(event);
        let envelope = EventEnvelope::new(self.source.clone(), payload);
        self.inner.publish(envelope);
    }
}

/// Bridge between the brain-events EventLog trait and the brain-storage SqliteEventLog backend.
pub struct SystemEventLog {
    inner: Arc<SqliteEventLog>,
}

impl SystemEventLog {
    /// Creates a new `SystemEventLog` wrapping a `SqliteEventLog` instance.
    pub fn new(inner: Arc<SqliteEventLog>) -> Self {
        Self { inner }
    }
}

impl EventLog for SystemEventLog {
    fn append(&self, envelope: &EventEnvelope) -> Result<u64, BrainError> {
        let payload_json = serde_json::to_string(&envelope.payload).map_err(|e| BrainError::Storage {
            message: format!("Failed to serialize event payload: {}", e),
            source: Some(Box::new(e)),
        })?;

        // Extract topic as topic name for diagnostics
        let topic_str = match &envelope.payload {
            brain_events::DomainEvent::System(_) => "system",
            brain_events::DomainEvent::Session(_) => "session",
            brain_events::DomainEvent::Agent(_) => "agent",
            brain_events::DomainEvent::Storage(_) => "storage",
            brain_events::DomainEvent::Plugin(_) => "plugin",
            brain_events::DomainEvent::UI(_) => "ui",
            brain_events::DomainEvent::Core(_) => "core",
        };

        self.inner.append(
            envelope.event_id,
            envelope.correlation_id,
            envelope.timestamp_ms,
            &envelope.version,
            &envelope.source,
            topic_str,
            &payload_json,
        )
    }

    fn read_from(&self, start_sequence: u64, limit: usize) -> Result<Vec<EventEnvelope>, BrainError> {
        let stored = self.inner.read_from(start_sequence, limit)?;
        let mut results = Vec::new();
        for s in stored {
            let payload: brain_events::DomainEvent = serde_json::from_str(&s.payload_json).map_err(|e| BrainError::Storage {
                message: format!("Failed to deserialize event payload: {}", e),
                source: Some(Box::new(e)),
            })?;

            results.push(EventEnvelope {
                sequence: Some(s.sequence),
                event_id: s.event_id,
                correlation_id: s.correlation_id,
                timestamp_ms: s.timestamp_ms,
                version: s.version,
                source: s.source,
                payload,
            });
        }
        Ok(results)
    }

    fn latest_sequence(&self) -> Result<u64, BrainError> {
        self.inner.latest_sequence()
    }
}

/// Publisher that persists domain events to the EventLog before publishing them to the system event bus.
pub struct PersistentDomainEventPublisher {
    event_log: Arc<dyn EventLog>,
    bus: Arc<dyn EventPublisher>,
    source: String,
}

impl PersistentDomainEventPublisher {
    /// Creates a new `PersistentDomainEventPublisher` instance.
    pub fn new(
        event_log: Arc<dyn EventLog>,
        bus: Arc<dyn EventPublisher>,
        source: String,
    ) -> Self {
        Self { event_log, bus, source }
    }
}

impl DomainEventPublisher for PersistentDomainEventPublisher {
    fn publish(&self, event: DomainEvent) {
        let payload = brain_events::DomainEvent::Core(event);
        let mut envelope = EventEnvelope::new(self.source.clone(), payload);

        // 1. Persist the event to the Event Log
        match self.event_log.append(&envelope) {
            Ok(seq) => {
                // 2. Set the assigned sequence ID on success
                envelope.sequence = Some(seq);
                // 3. Publish to the system event bus
                self.bus.publish(envelope);
            }
            Err(e) => {
                // Discard publication on storage failure to prevent partial state skew
                tracing::error!("Event log persistence failed, skipping event publication: {:?}", e);
            }
        }
    }
}
