//! EventStore abstraction and thread-safe reference implementation for event stream persistence.

use crate::reflection_events::ReflectionEventEnvelope;
use std::sync::Mutex;

/// Trait defining persistent storage and querying for event stream envelopes.
pub trait EventStore: Send + Sync {
    /// Appends an event envelope to the store.
    fn append(&self, envelope: ReflectionEventEnvelope) -> Result<(), String>;

    /// Queries recorded event envelopes matching a plan ID.
    fn query(&self, plan_id: &str) -> Vec<ReflectionEventEnvelope>;

    /// Streams all recorded event envelopes in append order.
    fn stream(&self) -> Vec<ReflectionEventEnvelope>;

    /// Compacts old events prior to a specified timestamp cut-off, returning the count of removed events.
    fn compact(&self, before_timestamp_ms: u64) -> usize;
}

/// Thread-safe in-memory reference implementation of `EventStore`.
#[derive(Default)]
pub struct InMemoryEventStore {
    envelopes: Mutex<Vec<ReflectionEventEnvelope>>,
}

impl InMemoryEventStore {
    /// Creates a new `InMemoryEventStore`.
    pub fn new() -> Self {
        Self {
            envelopes: Mutex::new(Vec::new()),
        }
    }
}

impl EventStore for InMemoryEventStore {
    fn append(&self, envelope: ReflectionEventEnvelope) -> Result<(), String> {
        let mut envs = self
            .envelopes
            .lock()
            .map_err(|e| format!("Lock error: {}", e))?;
        envs.push(envelope);
        Ok(())
    }

    fn query(&self, plan_id: &str) -> Vec<ReflectionEventEnvelope> {
        let envs = self.envelopes.lock().expect("Lock error");
        envs.iter()
            .filter(|e| e.plan_id == plan_id)
            .cloned()
            .collect()
    }

    fn stream(&self) -> Vec<ReflectionEventEnvelope> {
        let envs = self.envelopes.lock().expect("Lock error");
        envs.clone()
    }

    fn compact(&self, before_timestamp_ms: u64) -> usize {
        let mut envs = self.envelopes.lock().expect("Lock error");
        let initial_len = envs.len();
        envs.retain(|e| e.timestamp_ms >= before_timestamp_ms);
        initial_len - envs.len()
    }
}
