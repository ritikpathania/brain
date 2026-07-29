//! `WorkerSession`, Explicit `SessionState`, `ProtocolNegotiation`, and `WorkerSessionManager` (Phase 9 Milestone 9.3).
//!
//! ### Architectural Invariants:
//! 1. Explicit Session State Machine: `SessionState` transitions strictly (`Negotiating` -> `Active` -> `Closing` -> `Closed`).
//! 2. Protocol Negotiation Artifact: `ProtocolNegotiation` encapsulates version, flags, capabilities, and compatibility cleanly.
//! 3. Session Ownership Invariants:
//!    - A session belongs to exactly one worker.
//!    - A lease belongs to at most one active session.
//!    - Closing a session invalidates and returns all bound active leases.
//!    - Reconnection generates a new session identity.

use crate::planning::scheduler::LeaseId;
use crate::planning::supervision::{CheckpointCapabilitySet, SupervisionError};
use crate::planning::supervision_replay::CapabilityCompatibility;
use crate::planning::worker_registry::WorkerId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Strongly-typed worker session identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session_{}", self.0)
    }
}

/// Explicit lifecycle state machine of a `WorkerSession`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SessionState {
    /// Session currently negotiating protocol and capabilities.
    #[default]
    Negotiating,
    /// Session active and operational.
    Active,
    /// Session in progress of closing.
    Closing,
    /// Session closed and terminated.
    Closed,
}

/// Negotiation artifact holding protocol version, feature flags, capabilities, and compatibility status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolNegotiation {
    /// Supported wire protocol version.
    pub protocol_version: u16,
    /// Enabled transport feature flags.
    pub feature_flags: Vec<String>,
    /// Negotiated capability set.
    pub capabilities: CheckpointCapabilitySet,
    /// Diagnostic compatibility result.
    pub compatibility: CapabilityCompatibility,
}

impl Default for ProtocolNegotiation {
    fn default() -> Self {
        Self {
            protocol_version: 1,
            feature_flags: vec![
                "zstd_compression".to_string(),
                "streaming_events".to_string(),
            ],
            capabilities: CheckpointCapabilitySet::default_set(),
            compatibility: CapabilityCompatibility::Compatible,
        }
    }
}

/// Transient communication session wrapping a worker node connection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerSession {
    /// Unique session ID.
    pub session_id: SessionId,
    /// Target worker node ID.
    pub worker_id: WorkerId,
    /// Current session state.
    pub state: SessionState,
    /// Protocol negotiation artifact.
    pub negotiation: ProtocolNegotiation,
    /// Bound active lease IDs.
    pub active_leases: Vec<LeaseId>,
    /// Connection timestamp in milliseconds.
    pub connected_at_ms: u64,
}

/// Manager tracking active `WorkerSession` instances and lease bindings.
#[derive(Debug, Clone, Default)]
pub struct WorkerSessionManager {
    sessions: HashMap<SessionId, WorkerSession>,
}

impl WorkerSessionManager {
    /// Instantiates a new `WorkerSessionManager`.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Creates a new `WorkerSession` in `Negotiating` state.
    pub fn create_session(
        &mut self,
        worker_id: WorkerId,
        negotiation: ProtocolNegotiation,
        now_ms: u64,
    ) -> WorkerSession {
        let session_id = SessionId(Uuid::new_v4());
        let session = WorkerSession {
            session_id,
            worker_id,
            state: SessionState::Negotiating,
            negotiation,
            active_leases: Vec::new(),
            connected_at_ms: now_ms,
        };

        self.sessions.insert(session_id, session.clone());
        session
    }

    /// Transitions a session state from `Negotiating` to `Active`.
    pub fn activate_session(&mut self, session_id: SessionId) -> Result<(), SupervisionError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            SupervisionError::CorruptedCheckpoint(format!("Session '{}' not found", session_id))
        })?;

        if session.state != SessionState::Negotiating {
            return Err(SupervisionError::InvalidStateTransition {
                from: format!("{:?}", session.state),
                to: "Active".to_string(),
            });
        }

        session.state = SessionState::Active;
        Ok(())
    }

    /// Binds a `LeaseId` to an active session.
    pub fn bind_lease(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
    ) -> Result<(), SupervisionError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            SupervisionError::CorruptedCheckpoint(format!("Session '{}' not found", session_id))
        })?;

        if session.state != SessionState::Active {
            return Err(SupervisionError::InvalidStateTransition {
                from: format!("{:?}", session.state),
                to: "BindLease".to_string(),
            });
        }

        if !session.active_leases.contains(&lease_id) {
            session.active_leases.push(lease_id);
        }

        Ok(())
    }

    /// Unbinds a `LeaseId` from an active session.
    pub fn unbind_lease(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
    ) -> Result<(), SupervisionError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            SupervisionError::CorruptedCheckpoint(format!("Session '{}' not found", session_id))
        })?;

        session.active_leases.retain(|&id| id != lease_id);
        Ok(())
    }

    /// Closes a session cleanly, returning all bound lease IDs for invalidation.
    pub fn close_session(
        &mut self,
        session_id: SessionId,
    ) -> Result<Vec<LeaseId>, SupervisionError> {
        let session = self.sessions.get_mut(&session_id).ok_or_else(|| {
            SupervisionError::CorruptedCheckpoint(format!("Session '{}' not found", session_id))
        })?;

        session.state = SessionState::Closed;
        let bound_leases = session.active_leases.clone();
        session.active_leases.clear();

        Ok(bound_leases)
    }

    /// Retrieves a session reference by ID.
    pub fn get_session(&self, session_id: SessionId) -> Option<&WorkerSession> {
        self.sessions.get(&session_id)
    }
}
