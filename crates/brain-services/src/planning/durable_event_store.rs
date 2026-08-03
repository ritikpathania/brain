//! Concurrent `EventLog<E>` Abstraction & Canonical `EventEnvelope<E>` (Phase 11 Milestone 11.1).
//!
//! ### Architectural Invariants:
//! 1. Monotonic & Gap-Free Sequence Numbers: Sequence numbers strictly increment by 1 (`1, 2, 3...`), never decrease, and are never reused.
//! 2. Thread-Safe Concurrent Interface: `EventLog::append` uses `&self` rather than `&mut self` to allow shared log access behind `Arc<dyn EventLog<E>>`.
//! 3. Canonical Envelope Metadata: `EventEnvelope<E>` encapsulates monotonic `sequence`, `timestamp_ms`, `schema_version`, and `payload` without mutating domain models.

use crate::planning::event_publisher::EventPublishError;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Strongly-typed 1-based monotonic sequence number for event log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SequenceNumber(pub u64);

impl std::fmt::Display for SequenceNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "seq_{}", self.0)
    }
}

/// Canonical metadata envelope wrapping control plane domain events for durable log persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope<E> {
    /// Monotonic 1-based log sequence number.
    pub sequence: SequenceNumber,
    /// Event creation timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Schema version for forward-compatible evolution.
    pub schema_version: u16,
    /// Domain event payload.
    pub payload: E,
}

/// Abstract concurrent interface for append-only durable event logs.
pub trait EventLog<E>: Send + Sync {
    /// Appends a domain event to the log, returning its assigned `SequenceNumber`.
    fn append(
        &self,
        event: E,
        timestamp_ms: u64,
        schema_version: u16,
    ) -> Result<SequenceNumber, EventPublishError>;

    /// Reads a range of event envelopes starting at `start` sequence offset, up to `limit` items.
    fn read_range(
        &self,
        start: SequenceNumber,
        limit: usize,
    ) -> Result<Vec<EventEnvelope<E>>, EventPublishError>;

    /// Returns the last sequence number appended to the log, or `SequenceNumber(0)` if empty.
    fn last_sequence_number(&self) -> SequenceNumber;
}

/// Thread-safe in-memory reference implementation of `EventLog<E>`.
#[derive(Debug)]
pub struct InMemoryEventLog<E> {
    envelopes: Mutex<Vec<EventEnvelope<E>>>,
}

impl<E> Default for InMemoryEventLog<E> {
    fn default() -> Self {
        Self {
            envelopes: Mutex::new(Vec::new()),
        }
    }
}

impl<E> InMemoryEventLog<E> {
    /// Instantiates a new `InMemoryEventLog`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<E: Clone + Send + Sync> EventLog<E> for InMemoryEventLog<E> {
    fn append(
        &self,
        event: E,
        timestamp_ms: u64,
        schema_version: u16,
    ) -> Result<SequenceNumber, EventPublishError> {
        let mut guard = self
            .envelopes
            .lock()
            .map_err(|e| EventPublishError::StorageError(format!("Lock poisoning error: {}", e)))?;

        let seq_val = (guard.len() + 1) as u64;
        let sequence = SequenceNumber(seq_val);

        let envelope = EventEnvelope {
            sequence,
            timestamp_ms,
            schema_version,
            payload: event,
        };

        guard.push(envelope);
        Ok(sequence)
    }

    fn read_range(
        &self,
        start: SequenceNumber,
        limit: usize,
    ) -> Result<Vec<EventEnvelope<E>>, EventPublishError> {
        let guard = self
            .envelopes
            .lock()
            .map_err(|e| EventPublishError::StorageError(format!("Lock poisoning error: {}", e)))?;

        if start.0 == 0 || limit == 0 {
            return Ok(Vec::new());
        }

        let start_idx = (start.0 - 1) as usize;
        if start_idx >= guard.len() {
            return Ok(Vec::new());
        }

        let end_idx = std::cmp::min(start_idx + limit, guard.len());
        Ok(guard[start_idx..end_idx].to_vec())
    }

    fn last_sequence_number(&self) -> SequenceNumber {
        let guard = match self.envelopes.lock() {
            Ok(g) => g,
            Err(_) => return SequenceNumber(0),
        };
        SequenceNumber(guard.len() as u64)
    }
}
