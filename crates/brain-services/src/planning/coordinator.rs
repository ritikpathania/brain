//! Pluggable `LeaderElectionStrategy`, Explicit `LeadershipState`, and `CoordinatorElectionEngine` (Phase 10 Milestone 10.2).
//!
//! ### Architectural Invariants:
//! 1. Policy Separation: `LeaderElectionStrategy` trait owns election algorithm (`select_leader`); `CoordinatorElectionEngine` orchestrates state machine and epoch handoff.
//! 2. Explicit Leadership State Machine: `LeadershipState` transitions strictly (`Follower` -> `Candidate` -> `Leader` -> `Follower`).
//! 3. Identity Separation: A `ClusterNode` with `ClusterNodeRole::Coordinator` is distinct from the active `CoordinatorLeader`.
//! 4. Single Leader Per Epoch: Exactly one leader exists per cluster epoch.

use crate::planning::cluster::{
    ClusterError, ClusterManager, ClusterNode, ClusterNodeRole, EpochId, NodeId,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Explicit state machine of coordinator node leadership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LeadershipState {
    /// Node is a follower coordinator.
    #[default]
    Follower,
    /// Node is a candidate for leadership.
    Candidate,
    /// Node is the active elected coordinator leader.
    Leader,
}

/// Active elected coordinator leader model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinatorLeader {
    /// Node ID of the elected leader.
    pub node_id: NodeId,
    /// Epoch ID during which leadership was granted.
    pub epoch: EpochId,
    /// Current leadership state.
    pub state: LeadershipState,
    /// Timestamp when leadership was granted in milliseconds.
    pub elected_at_ms: u64,
}

/// Trait defining leader selection strategies.
pub trait LeaderElectionStrategy: Send + Sync {
    /// Selects a leader NodeId from a slice of candidate coordinator nodes.
    fn select_leader(&self, candidates: &[&ClusterNode]) -> Result<NodeId, ClusterError>;
}

/// Strategy picking the first active coordinator node.
#[derive(Debug, Default)]
pub struct SingleCoordinatorStrategy;

impl LeaderElectionStrategy for SingleCoordinatorStrategy {
    fn select_leader(&self, candidates: &[&ClusterNode]) -> Result<NodeId, ClusterError> {
        candidates
            .first()
            .map(|n| n.node_id)
            .ok_or(ClusterError::CoordinatorUnavailable)
    }
}

/// Strategy picking a specific static leader NodeId.
#[derive(Debug, Clone)]
pub struct StaticLeaderStrategy {
    /// Preferred static leader NodeId.
    pub target_leader: NodeId,
}

impl LeaderElectionStrategy for StaticLeaderStrategy {
    fn select_leader(&self, candidates: &[&ClusterNode]) -> Result<NodeId, ClusterError> {
        candidates
            .iter()
            .find(|n| n.node_id == self.target_leader)
            .map(|n| n.node_id)
            .ok_or(ClusterError::CoordinatorUnavailable)
    }
}

/// Current schema version constant for `LeadershipEvent`.
pub const LEADERSHIP_EVENT_SCHEMA_VERSION: u16 = 1;

/// Strongly-typed leadership event identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LeadershipEventId(pub Uuid);

impl std::fmt::Display for LeadershipEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lead_ev_{}", self.0)
    }
}

/// Structured event kind classification for coordinator leadership events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeadershipEventKind {
    /// Leader election process initiated.
    LeaderElectionStarted {
        /// Number of eligible candidate coordinator nodes.
        candidates_count: usize,
    },
    /// Leader successfully elected for an epoch.
    LeaderElected {
        /// Elected leader NodeId.
        leader_id: NodeId,
        /// Epoch ID of granted leadership.
        epoch: EpochId,
    },
    /// Leadership transferred to a new coordinator node.
    LeadershipTransferred {
        /// Former leader NodeId.
        from_leader_id: NodeId,
        /// New leader NodeId.
        to_leader_id: NodeId,
        /// New epoch ID.
        epoch: EpochId,
    },
    /// Leadership lost due to node failure or network partition.
    LeadershipLost {
        /// Former leader NodeId.
        former_leader_id: NodeId,
        /// Epoch ID of lost leadership.
        epoch: EpochId,
    },
    /// Leadership recovered by an active coordinator node.
    LeadershipRecovered {
        /// Recovered leader NodeId.
        leader_id: NodeId,
        /// Epoch ID of recovered leadership.
        epoch: EpochId,
    },
    /// Cluster quorum established (reserved consensus extension).
    QuorumEstablished {
        /// Number of active coordinator nodes.
        active_coordinators: usize,
    },
    /// Cluster quorum lost (reserved consensus extension).
    QuorumLost {
        /// Number of active coordinator nodes remaining.
        active_coordinators: usize,
    },
}

/// Structured append-only leadership event item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeadershipEvent {
    /// Schema version for forward compatibility.
    pub schema_version: u16,
    /// Unique leadership event ID.
    pub event_id: LeadershipEventId,
    /// Structured event kind payload.
    pub kind: LeadershipEventKind,
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Engine managing coordinator leader election, state machine transitions, and epoch handoffs.
pub struct CoordinatorElectionEngine {
    strategy: Box<dyn LeaderElectionStrategy>,
    current_leader: Option<CoordinatorLeader>,
    events: Vec<LeadershipEvent>,
}

impl Default for CoordinatorElectionEngine {
    fn default() -> Self {
        Self::new(Box::new(SingleCoordinatorStrategy))
    }
}

impl CoordinatorElectionEngine {
    /// Instantiates a new `CoordinatorElectionEngine` with specified `LeaderElectionStrategy`.
    pub fn new(strategy: Box<dyn LeaderElectionStrategy>) -> Self {
        Self {
            strategy,
            current_leader: None,
            events: Vec::new(),
        }
    }

    /// Returns the active `CoordinatorLeader` if present.
    pub fn current_leader(&self) -> Option<&CoordinatorLeader> {
        self.current_leader.as_ref()
    }

    /// Returns the append-only leadership event log.
    pub fn events(&self) -> &[LeadershipEvent] {
        &self.events
    }

    fn emit_event(&mut self, kind: LeadershipEventKind, timestamp_ms: u64) {
        self.events.push(LeadershipEvent {
            schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
            event_id: LeadershipEventId(Uuid::new_v4()),
            kind,
            timestamp_ms,
        });
    }

    /// Conducts leader election among active cluster coordinator nodes.
    pub fn elect_leader(
        &mut self,
        cluster: &mut ClusterManager,
        now_ms: u64,
    ) -> Result<CoordinatorLeader, ClusterError> {
        let coordinators = cluster.get_coordinators();
        if coordinators.is_empty() {
            return Err(ClusterError::CoordinatorUnavailable);
        }

        self.emit_event(
            LeadershipEventKind::LeaderElectionStarted {
                candidates_count: coordinators.len(),
            },
            now_ms,
        );

        let selected_id = self.strategy.select_leader(&coordinators)?;
        let epoch = cluster.current_epoch();

        let leader = CoordinatorLeader {
            node_id: selected_id,
            epoch,
            state: LeadershipState::Leader,
            elected_at_ms: now_ms + 1,
        };

        self.current_leader = Some(leader.clone());
        self.emit_event(
            LeadershipEventKind::LeaderElected {
                leader_id: selected_id,
                epoch,
            },
            now_ms + 1,
        );

        Ok(leader)
    }

    /// Hands off leadership to a designated new leader NodeId, advancing cluster epoch.
    pub fn handoff_leadership(
        &mut self,
        cluster: &mut ClusterManager,
        new_leader_id: NodeId,
        now_ms: u64,
    ) -> Result<CoordinatorLeader, ClusterError> {
        let coordinators = cluster.get_coordinators();
        let target = coordinators
            .iter()
            .find(|n| n.node_id == new_leader_id)
            .ok_or(ClusterError::UnknownNode(new_leader_id))?;

        if target.role != ClusterNodeRole::Coordinator {
            return Err(ClusterError::InvalidStateTransition {
                from: "WorkerNode".to_string(),
                to: "Leader".to_string(),
            });
        }

        let from_leader_id = self
            .current_leader
            .as_ref()
            .map(|l| l.node_id)
            .unwrap_or(new_leader_id);

        // Advance cluster epoch on leadership handoff
        let new_epoch = cluster.advance_epoch(now_ms);

        let leader = CoordinatorLeader {
            node_id: new_leader_id,
            epoch: new_epoch,
            state: LeadershipState::Leader,
            elected_at_ms: now_ms + 1,
        };

        self.current_leader = Some(leader.clone());
        self.emit_event(
            LeadershipEventKind::LeadershipTransferred {
                from_leader_id,
                to_leader_id: new_leader_id,
                epoch: new_epoch,
            },
            now_ms + 1,
        );

        Ok(leader)
    }
}
