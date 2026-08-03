//! Self-Validating `LogSnapshot`, `SnapshotCodec<S>`, and `SnapshotStore` (Phase 12 Milestone 12.2).
//!
//! ### Architectural Invariants:
//! 1. Codec Isolation: `SnapshotCodec<S>` isolates projection state serialization from snapshot storage backends.
//! 2. Self-Validating Snapshots: `LogSnapshot` includes a SHA-256 `payload_hash` checksum for corruption detection prior to state restoration.
//! 3. Pure Builder Pattern: `SnapshotBuilder` constructs self-validating `LogSnapshot` instances independently from storage.

use crate::planning::consensus::TermId;
use crate::planning::durable_event_store::SequenceNumber;
use crate::planning::event_publisher::EventPublishError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use uuid::Uuid;

/// Self-validating control plane snapshot artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogSnapshot {
    /// Unique snapshot identifier.
    pub snapshot_id: Uuid,
    /// Last sequence number included in this snapshot checkpoint.
    pub snapshot_sequence: SequenceNumber,
    /// Consensus term when snapshot was captured.
    pub snapshot_term: TermId,
    /// Schema version for backward-compatible snapshot evolution.
    pub schema_version: u16,
    /// Creation timestamp in milliseconds.
    pub created_at_ms: u64,
    /// SHA-256 hash checksum of state payload for corruption validation.
    pub payload_hash: String,
    /// Encoded state payload bytes.
    pub state_payload: Vec<u8>,
}

impl LogSnapshot {
    /// Verifies that the payload hash checksum matches the binary payload contents.
    pub fn verify_checksum(&self) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(&self.state_payload);
        let hash = hasher.finalize();
        let calculated_hash: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
        calculated_hash == self.payload_hash
    }
}

/// Abstract codec interface for serializing projection state into snapshot payloads.
pub trait SnapshotCodec<S>: Send + Sync {
    /// Encodes projection state reference into binary bytes.
    fn encode(&self, state: &S) -> Result<Vec<u8>, EventPublishError>;

    /// Decodes binary bytes into projection state instance.
    fn decode(&self, bytes: &[u8]) -> Result<S, EventPublishError>;
}

/// Reference JSON snapshot codec implementation.
#[derive(Debug, Clone, Default)]
pub struct JsonSnapshotCodec<S> {
    _marker: std::marker::PhantomData<S>,
}

impl<S> JsonSnapshotCodec<S> {
    /// Instantiates a new `JsonSnapshotCodec`.
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

impl<S: Serialize + DeserializeOwned + Send + Sync> SnapshotCodec<S> for JsonSnapshotCodec<S> {
    fn encode(&self, state: &S) -> Result<Vec<u8>, EventPublishError> {
        serde_json::to_vec(state).map_err(|e| {
            EventPublishError::StorageError(format!("JSON snapshot encoding error: {}", e))
        })
    }

    fn decode(&self, bytes: &[u8]) -> Result<S, EventPublishError> {
        serde_json::from_slice(bytes).map_err(|e| {
            EventPublishError::StorageError(format!("JSON snapshot decoding error: {}", e))
        })
    }
}

/// Pure builder constructing self-validating `LogSnapshot` instances.
pub struct SnapshotBuilder;

impl SnapshotBuilder {
    /// Builds a self-validating `LogSnapshot` with calculated SHA-256 payload checksum.
    pub fn build_snapshot<S, C: SnapshotCodec<S>>(
        state: &S,
        sequence: SequenceNumber,
        term: TermId,
        codec: &C,
        schema_version: u16,
        timestamp_ms: u64,
    ) -> Result<LogSnapshot, EventPublishError> {
        let payload_bytes = codec.encode(state)?;
        let mut hasher = Sha256::new();
        hasher.update(&payload_bytes);
        let hash = hasher.finalize();
        let payload_hash: String = hash.iter().map(|b| format!("{:02x}", b)).collect();

        Ok(LogSnapshot {
            snapshot_id: Uuid::new_v4(),
            snapshot_sequence: sequence,
            snapshot_term: term,
            schema_version,
            created_at_ms: timestamp_ms,
            payload_hash,
            state_payload: payload_bytes,
        })
    }
}

/// Abstract storage boundary for persistent snapshot management.
pub trait SnapshotStore: Send + Sync {
    /// Saves a snapshot artifact atomically.
    fn save_snapshot(&self, snapshot: &LogSnapshot) -> Result<(), EventPublishError>;

    /// Loads the latest snapshot artifact, if one exists.
    fn load_latest_snapshot(&self) -> Result<Option<LogSnapshot>, EventPublishError>;
}

/// In-memory reference implementation of `SnapshotStore`.
#[derive(Debug, Default)]
pub struct InMemorySnapshotStore {
    snapshots: Mutex<Vec<LogSnapshot>>,
}

impl InMemorySnapshotStore {
    /// Instantiates a new `InMemorySnapshotStore`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SnapshotStore for InMemorySnapshotStore {
    fn save_snapshot(&self, snapshot: &LogSnapshot) -> Result<(), EventPublishError> {
        if !snapshot.verify_checksum() {
            return Err(EventPublishError::StorageError(
                "Snapshot checksum verification failed".to_string(),
            ));
        }

        let mut guard = self
            .snapshots
            .lock()
            .map_err(|e| EventPublishError::StorageError(format!("Lock poisoning error: {}", e)))?;
        guard.push(snapshot.clone());
        Ok(())
    }

    fn load_latest_snapshot(&self) -> Result<Option<LogSnapshot>, EventPublishError> {
        let guard = self
            .snapshots
            .lock()
            .map_err(|e| EventPublishError::StorageError(format!("Lock poisoning error: {}", e)))?;
        Ok(guard.last().cloned())
    }
}
