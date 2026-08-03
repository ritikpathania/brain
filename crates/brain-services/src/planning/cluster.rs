//! Multi-Node `ClusterManager`, Explicit `ClusterNodeStatus` State Machine, and `ClusterEvent` Vocabulary (Phase 10 Milestone 10.1).
//!
//! ### Architectural Invariants:
//! 1. Multi-Node Membership: `ClusterManager` tracks cluster node membership, roles (`Coordinator` vs `WorkerNode`), and health independently from local worker discovery.
//! 2. Explicit State Machine: `ClusterNodeStatus` transitions strictly (`Joining` -> `Active` -> `Suspect` -> `Left`).
//! 3. Strongly-Typed Address & Errors: `NodeAddress(pub String)` transport-neutral endpoint newtype; operations return `ClusterError`.
//! 4. Monotonic Cluster Epochs: Cluster epochs (`EpochId(pub u64)`) advance monotonically upon topology or coordinator state changes.
//! 5. Append-Only Control Stream: `ClusterEvent` event vocabulary records node lifecycle boundaries and epoch advancements.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Strongly-typed cluster node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node_{}", self.0)
    }
}

/// Strongly-typed cluster epoch identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct EpochId(pub u64);

impl std::fmt::Display for EpochId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "epoch_{}", self.0)
    }
}

/// Strongly-typed cluster node network address or endpoint.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeAddress(pub String);

impl std::fmt::Display for NodeAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed cluster event identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ClusterEventId(pub Uuid);

impl std::fmt::Display for ClusterEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cluster_ev_{}", self.0)
    }
}

/// Role classification of a node within the cluster topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClusterNodeRole {
    /// Node acts as a supervisory cluster coordinator.
    Coordinator,
    /// Node acts as an execution worker node.
    WorkerNode,
}

/// Explicit lifecycle state machine of a `ClusterNode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ClusterNodeStatus {
    /// Node currently joining cluster topology.
    #[default]
    Joining,
    /// Node active and healthy.
    Active,
    /// Node suspected of failure due to missing heartbeats.
    Suspect,
    /// Node explicitly left cluster topology.
    Left,
}

/// Strongly-typed error classification for cluster membership and fencing operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterError {
    /// Attempted to register a duplicate NodeId.
    DuplicateNode(NodeId),
    /// Target NodeId not found in cluster membership.
    UnknownNode(NodeId),
    /// Illegal state machine transition.
    InvalidStateTransition {
        /// Current state.
        from: String,
        /// Attempted state.
        to: String,
    },
    /// No cluster coordinator is active or available.
    CoordinatorUnavailable,
    /// Invalid or stale fence token rejected.
    InvalidFenceToken {
        /// Expected minimum fence token.
        expected: u64,
        /// Provided fence token.
        found: u64,
    },
}

impl std::fmt::Display for ClusterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateNode(id) => write!(f, "Node '{}' already in cluster", id),
            Self::UnknownNode(id) => write!(f, "Node '{}' not found in cluster", id),
            Self::InvalidStateTransition { from, to } => {
                write!(
                    f,
                    "Invalid cluster node transition from '{}' to '{}'",
                    from, to
                )
            }
            Self::CoordinatorUnavailable => write!(f, "Cluster coordinator unavailable"),
            Self::InvalidFenceToken { expected, found } => {
                write!(f, "Invalid fence token {}; expected >={}", found, expected)
            }
        }
    }
}

impl std::error::Error for ClusterError {}

/// Model representing a single cluster node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterNode {
    /// Unique node ID.
    pub node_id: NodeId,
    /// Transport-neutral network address.
    pub address: NodeAddress,
    /// Assigned cluster role.
    pub role: ClusterNodeRole,
    /// Current membership status.
    pub status: ClusterNodeStatus,
}

/// Event kind classification for cluster control events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClusterEventKind {
    /// New node joined cluster.
    NodeJoined,
    /// Node state transitioned to Active.
    NodeActivated,
    /// Node state transitioned to Suspect.
    NodeSuspected,
    /// Node recovered from Suspect back to Active.
    NodeRecovered,
    /// Node explicitly left cluster.
    NodeLeft,
    /// Cluster epoch advanced monotonically.
    EpochAdvanced,
    /// Stale fence token rejected.
    FenceRejected,
}

/// Single append-only event item in the cluster control log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClusterEvent {
    /// Unique cluster event ID.
    pub event_id: ClusterEventId,
    /// Event classification kind.
    pub kind: ClusterEventKind,
    /// Descriptive message text.
    pub message: String,
    /// Timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Manager coordinating multi-node cluster membership, roles, epochs, and cluster event streams.
pub struct ClusterManager {
    current_epoch: EpochId,
    nodes: HashMap<NodeId, ClusterNode>,
    events: Vec<ClusterEvent>,
}

impl Default for ClusterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterManager {
    /// Instantiates a new `ClusterManager`.
    pub fn new() -> Self {
        Self {
            current_epoch: EpochId(1),
            nodes: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// Returns the current cluster epoch ID.
    pub fn current_epoch(&self) -> EpochId {
        self.current_epoch
    }

    /// Returns the append-only cluster event log.
    pub fn events(&self) -> &[ClusterEvent] {
        &self.events
    }

    fn emit_event(&mut self, kind: ClusterEventKind, msg: &str, timestamp_ms: u64) {
        self.events.push(ClusterEvent {
            event_id: ClusterEventId(Uuid::new_v4()),
            kind,
            message: msg.to_string(),
            timestamp_ms,
        });
    }

    /// Advances the cluster epoch monotonically.
    pub fn advance_epoch(&mut self, now_ms: u64) -> EpochId {
        self.current_epoch = EpochId(self.current_epoch.0 + 1);
        self.emit_event(
            ClusterEventKind::EpochAdvanced,
            &format!("Cluster epoch advanced to {}", self.current_epoch),
            now_ms,
        );
        self.current_epoch
    }

    /// Looks up a cluster node reference by ID.
    pub fn get_node(&self, node_id: &NodeId) -> Option<&ClusterNode> {
        self.nodes.get(node_id)
    }

    /// Adds a new node to cluster membership in `Joining` state.
    pub fn join_cluster(&mut self, node: ClusterNode, now_ms: u64) -> Result<(), ClusterError> {
        if self.nodes.contains_key(&node.node_id) {
            return Err(ClusterError::DuplicateNode(node.node_id));
        }

        let id = node.node_id;
        self.nodes.insert(id, node);
        self.emit_event(
            ClusterEventKind::NodeJoined,
            &format!("Node '{}' joined cluster", id),
            now_ms,
        );
        Ok(())
    }

    /// Transitions a node status from `Joining` / `Suspect` to `Active`.
    pub fn activate_node(&mut self, node_id: NodeId, now_ms: u64) -> Result<(), ClusterError> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(ClusterError::UnknownNode(node_id))?;

        match node.status {
            ClusterNodeStatus::Joining => {
                node.status = ClusterNodeStatus::Active;
                self.emit_event(
                    ClusterEventKind::NodeActivated,
                    &format!("Node '{}' activated", node_id),
                    now_ms,
                );
                Ok(())
            }
            ClusterNodeStatus::Suspect => {
                node.status = ClusterNodeStatus::Active;
                self.emit_event(
                    ClusterEventKind::NodeRecovered,
                    &format!("Node '{}' recovered to Active", node_id),
                    now_ms,
                );
                Ok(())
            }
            _ => Err(ClusterError::InvalidStateTransition {
                from: format!("{:?}", node.status),
                to: "Active".to_string(),
            }),
        }
    }

    /// Transitions a node status to `Suspect`.
    pub fn suspect_node(&mut self, node_id: NodeId, now_ms: u64) -> Result<(), ClusterError> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(ClusterError::UnknownNode(node_id))?;

        if node.status == ClusterNodeStatus::Left {
            return Err(ClusterError::InvalidStateTransition {
                from: "Left".to_string(),
                to: "Suspect".to_string(),
            });
        }

        node.status = ClusterNodeStatus::Suspect;
        self.emit_event(
            ClusterEventKind::NodeSuspected,
            &format!("Node '{}' marked Suspect", node_id),
            now_ms,
        );
        Ok(())
    }

    /// Removes a node from active membership (`Left` status).
    pub fn leave_cluster(&mut self, node_id: NodeId, now_ms: u64) -> Result<(), ClusterError> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(ClusterError::UnknownNode(node_id))?;

        node.status = ClusterNodeStatus::Left;
        self.emit_event(
            ClusterEventKind::NodeLeft,
            &format!("Node '{}' left cluster", node_id),
            now_ms,
        );
        Ok(())
    }

    /// Returns active coordinator nodes sorted deterministically by NodeId.
    pub fn get_coordinators(&self) -> Vec<&ClusterNode> {
        let mut coords: Vec<&ClusterNode> = self
            .nodes
            .values()
            .filter(|n| {
                n.role == ClusterNodeRole::Coordinator && n.status == ClusterNodeStatus::Active
            })
            .collect();
        coords.sort_by_key(|n| n.node_id);
        coords
    }

    /// Returns active worker nodes sorted deterministically by NodeId.
    pub fn get_workers(&self) -> Vec<&ClusterNode> {
        let mut workers: Vec<&ClusterNode> = self
            .nodes
            .values()
            .filter(|n| {
                n.role == ClusterNodeRole::WorkerNode && n.status == ClusterNodeStatus::Active
            })
            .collect();
        workers.sort_by_key(|n| n.node_id);
        workers
    }
}
