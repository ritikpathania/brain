//! Event Codec Abstraction for Storage-Agnostic Event Serialization (Phase 12 Milestone 12.1).
//!
//! ### Architectural Invariants:
//! 1. Codec Isolation: `EventLog` backends operate on raw byte payloads, completely decoupled from specific serialization formats (JSON, CBOR, Protobuf).
//! 2. Error Safety: Codec errors are mapped into strongly-typed `EventPublishError::StorageError`.

use crate::planning::event_publisher::EventPublishError;
use serde::{de::DeserializeOwned, Serialize};

/// Abstract codec interface for encoding and decoding domain event payloads.
pub trait EventCodec<E>: Send + Sync {
    /// Encodes a domain event reference into binary bytes.
    fn encode(&self, event: &E) -> Result<Vec<u8>, EventPublishError>;

    /// Decodes binary bytes into a strongly-typed domain event instance.
    fn decode(&self, bytes: &[u8]) -> Result<E, EventPublishError>;
}

/// Reference JSON event codec implementation using `serde_json`.
#[derive(Debug, Clone, Default)]
pub struct JsonEventCodec<E> {
    _marker: std::marker::PhantomData<E>,
}

impl<E> JsonEventCodec<E> {
    /// Instantiates a new `JsonEventCodec`.
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<E: Serialize + DeserializeOwned + Send + Sync> EventCodec<E> for JsonEventCodec<E> {
    fn encode(&self, event: &E) -> Result<Vec<u8>, EventPublishError> {
        serde_json::to_vec(event)
            .map_err(|e| EventPublishError::StorageError(format!("JSON encoding error: {}", e)))
    }

    fn decode(&self, bytes: &[u8]) -> Result<E, EventPublishError> {
        serde_json::from_slice(bytes)
            .map_err(|e| EventPublishError::StorageError(format!("JSON decoding error: {}", e)))
    }
}
