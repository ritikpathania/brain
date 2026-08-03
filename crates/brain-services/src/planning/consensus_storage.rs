//! Persistent Consensus Storage Boundary & Models (Phase 12 Milestone 12.1).
//!
//! ### Architectural Invariants:
//! 1. Backward-Compatible Persistent State: `ConsensusPersistentState` captures `current_term`, `voted_for`, and `schema_version`.
//! 2. Storage Decoupling: Persistence mechanics are isolated behind the `ConsensusStorage` trait boundary, separate from the `ConsensusEngine`.

use crate::planning::cluster::NodeId;
use crate::planning::consensus::{ConsensusError, TermId};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Persistent consensus state snapshot for crash recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusPersistentState {
    /// Monotonic consensus term.
    pub current_term: TermId,
    /// Candidate node ID voted for in current term.
    pub voted_for: Option<NodeId>,
    /// Schema version for backward-compatible state evolution.
    pub schema_version: u16,
}

impl Default for ConsensusPersistentState {
    fn default() -> Self {
        Self {
            current_term: TermId(0),
            voted_for: None,
            schema_version: 1,
        }
    }
}

/// Abstract storage boundary for persisting consensus state.
pub trait ConsensusStorage: Send + Sync {
    /// Saves consensus persistent state snapshot atomically.
    fn save_state(&self, state: &ConsensusPersistentState) -> Result<(), ConsensusError>;

    /// Loads consensus persistent state snapshot.
    fn load_state(&self) -> Result<ConsensusPersistentState, ConsensusError>;
}

/// In-memory reference implementation of `ConsensusStorage`.
#[derive(Debug)]
pub struct InMemoryConsensusStorage {
    state: Mutex<ConsensusPersistentState>,
}

impl Default for InMemoryConsensusStorage {
    fn default() -> Self {
        Self {
            state: Mutex::new(ConsensusPersistentState::default()),
        }
    }
}

impl InMemoryConsensusStorage {
    /// Instantiates a new `InMemoryConsensusStorage`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ConsensusStorage for InMemoryConsensusStorage {
    fn save_state(&self, state: &ConsensusPersistentState) -> Result<(), ConsensusError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|e| ConsensusError::StorageError(format!("Lock poisoning error: {}", e)))?;
        *guard = state.clone();
        Ok(())
    }

    fn load_state(&self) -> Result<ConsensusPersistentState, ConsensusError> {
        let guard = self
            .state
            .lock()
            .map_err(|e| ConsensusError::StorageError(format!("Lock poisoning error: {}", e)))?;
        Ok(guard.clone())
    }
}
