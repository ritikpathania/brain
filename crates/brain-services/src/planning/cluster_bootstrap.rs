//! Distributed Deployment, Config Validation & Operational Controller (Phase 15 Milestone 15.4).
//!
//! ### Architectural Invariants:
//! 1. Separated Config Validation: `ClusterConfigValidator` validates configuration invariants into an immutable `ValidatedClusterConfig` prior to bootstrap.
//! 2. Pure Bootstrap Orchestration: `ClusterBootstrapEngine` initializes state machines and returns a `BootstrapReport` artifact without absorbing consensus logic.
//! 3. Compiler-Style Controller Plans: `CliClusterController` compiles immutable operation plans (`MembershipChangePlan`, `SnapshotTriggerPlan`, `ClusterStatusReport`) for management APIs.

use crate::planning::cluster::NodeId;
use crate::planning::cluster_configuration::{
    ConfigurationPlanner, ConfigurationTransition, ConfigurationVersion, MembershipView,
};
use crate::planning::consensus::{ConsensusEngine, ConsensusRole, TermId};
use crate::planning::durable_event_store::SequenceNumber;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Raw node configuration specification for distributed deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterNodeConfig {
    /// Target node identifier.
    pub node_id: NodeId,
    /// Network address to listen for incoming cluster RPCs.
    pub listen_address: String,
    /// List of peer node network addresses in cluster topology.
    pub peer_addresses: Vec<String>,
    /// Chunk size in bytes for snapshot transfers.
    pub snapshot_chunk_size: usize,
    /// Periodic heartbeat transmission interval in milliseconds.
    pub heartbeat_interval_ms: u64,
    /// Leader election timeout bound in milliseconds.
    pub election_timeout_ms: u64,
}

/// Errors occurring during cluster node configuration validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusterConfigError {
    /// Listen or peer network address format is invalid.
    InvalidAddress(String),
    /// Duplicate peer address detected in peer list.
    DuplicatePeer(String),
    /// Heartbeat timeout must be strictly smaller than election timeout.
    InvalidTimeoutRelationship {
        /// Heartbeat interval in milliseconds.
        heartbeat_ms: u64,
        /// Election timeout in milliseconds.
        election_ms: u64,
    },
    /// Snapshot chunk size is outside allowed bounds (1KB to 16MB).
    InvalidChunkSize(usize),
}

impl std::fmt::Display for ClusterConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAddress(addr) => write!(f, "Invalid network address syntax: {}", addr),
            Self::DuplicatePeer(addr) => {
                write!(f, "Duplicate peer address in configuration: {}", addr)
            }
            Self::InvalidTimeoutRelationship {
                heartbeat_ms,
                election_ms,
            } => {
                write!(
                    f,
                    "Invalid timeout relationship: heartbeat ({}ms) must be < election ({}ms)",
                    heartbeat_ms, election_ms
                )
            }
            Self::InvalidChunkSize(size) => {
                write!(f, "Invalid snapshot chunk size: {} bytes", size)
            }
        }
    }
}

impl std::error::Error for ClusterConfigError {}

/// Immutable validated node configuration artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedClusterConfig {
    /// Validated node configuration.
    pub config: ClusterNodeConfig,
}

/// Pure validator ensuring cluster configuration invariants.
pub struct ClusterConfigValidator;

impl ClusterConfigValidator {
    /// Validates raw `ClusterNodeConfig` into a `ValidatedClusterConfig`.
    pub fn validate(
        config: ClusterNodeConfig,
    ) -> Result<ValidatedClusterConfig, ClusterConfigError> {
        if config.listen_address.trim().is_empty() {
            return Err(ClusterConfigError::InvalidAddress(
                config.listen_address.clone(),
            ));
        }

        let mut seen_peers = HashSet::new();
        for peer in &config.peer_addresses {
            if peer.trim().is_empty() {
                return Err(ClusterConfigError::InvalidAddress(peer.clone()));
            }
            if !seen_peers.insert(peer.clone()) {
                return Err(ClusterConfigError::DuplicatePeer(peer.clone()));
            }
        }

        if config.heartbeat_interval_ms == 0
            || config.election_timeout_ms == 0
            || config.heartbeat_interval_ms >= config.election_timeout_ms
        {
            return Err(ClusterConfigError::InvalidTimeoutRelationship {
                heartbeat_ms: config.heartbeat_interval_ms,
                election_ms: config.election_timeout_ms,
            });
        }

        if config.snapshot_chunk_size < 1024 || config.snapshot_chunk_size > 16 * 1024 * 1024 {
            return Err(ClusterConfigError::InvalidChunkSize(
                config.snapshot_chunk_size,
            ));
        }

        Ok(ValidatedClusterConfig { config })
    }
}

/// Execution report produced by cluster bootstrapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapReport {
    /// Node identifier initialized.
    pub node_id: NodeId,
    /// Number of peer node addresses discovered.
    pub peer_count: usize,
    /// Timestamp in milliseconds when bootstrap occurred.
    pub bootstrapped_at_ms: u64,
    /// `true` if bootstrap initialized consensus state successfully.
    pub is_success: bool,
}

/// Lifecycle orchestrator for node startup and initial state initialization.
pub struct ClusterBootstrapEngine;

impl ClusterBootstrapEngine {
    /// Bootstraps cluster consensus state machine from a `ValidatedClusterConfig`.
    pub fn bootstrap(
        validated: &ValidatedClusterConfig,
        now_ms: u64,
    ) -> (ConsensusEngine, BootstrapReport) {
        let engine = ConsensusEngine::new();
        let report = BootstrapReport {
            node_id: validated.config.node_id,
            peer_count: validated.config.peer_addresses.len(),
            bootstrapped_at_ms: now_ms,
            is_success: true,
        };
        (engine, report)
    }
}

/// Immutable plan compiled for joint consensus membership changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipChangePlan {
    /// Action description ("add_node" or "remove_node").
    pub action: String,
    /// Target node ID involved in transition.
    pub target_node: NodeId,
    /// Target joint consensus transition configuration.
    pub transition: ConfigurationTransition,
}

/// Immutable plan compiled for manual snapshot trigger requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotTriggerPlan {
    /// Target node ID to receive snapshot.
    pub target_node: NodeId,
    /// Sequence index for snapshot compilation.
    pub snapshot_sequence: SequenceNumber,
}

/// Domain-level operational status model for management APIs and CLI controllers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterStatusReport {
    /// Local node ID.
    pub node_id: NodeId,
    /// Current consensus role.
    pub role: ConsensusRole,
    /// Current consensus term.
    pub term: TermId,
    /// Count of configured peer nodes.
    pub active_peers: usize,
}

/// Operational controller facade compiling immutable plans and querying status.
pub struct CliClusterController;

impl CliClusterController {
    /// Queries current domain-level status report from a `ConsensusEngine`.
    pub fn get_cluster_status(
        node_id: NodeId,
        engine: &ConsensusEngine,
        peer_count: usize,
    ) -> ClusterStatusReport {
        let state = engine.current_state();
        ClusterStatusReport {
            node_id,
            role: state.role,
            term: state.current_term,
            active_peers: peer_count,
        }
    }

    /// Compiles an immutable `MembershipChangePlan` to add a new node to joint consensus.
    pub fn plan_add_node(current_members: &[NodeId], new_node: NodeId) -> MembershipChangePlan {
        let current_view = MembershipView::new(
            ConfigurationVersion(1),
            current_members.to_vec(),
            Vec::new(),
        );
        let mut target_voters = current_members.to_vec();
        if !target_voters.contains(&new_node) {
            target_voters.push(new_node);
        }

        let transition =
            ConfigurationPlanner::plan_transition(&current_view, target_voters, Vec::new());
        MembershipChangePlan {
            action: "add_node".to_string(),
            target_node: new_node,
            transition,
        }
    }

    /// Compiles an immutable `MembershipChangePlan` to remove a node from joint consensus.
    pub fn plan_remove_node(
        current_members: &[NodeId],
        target_node: NodeId,
    ) -> MembershipChangePlan {
        let current_view = MembershipView::new(
            ConfigurationVersion(1),
            current_members.to_vec(),
            Vec::new(),
        );
        let updated_voters: Vec<NodeId> = current_members
            .iter()
            .copied()
            .filter(|&id| id != target_node)
            .collect();

        let transition =
            ConfigurationPlanner::plan_transition(&current_view, updated_voters, Vec::new());
        MembershipChangePlan {
            action: "remove_node".to_string(),
            target_node,
            transition,
        }
    }

    /// Compiles an immutable `SnapshotTriggerPlan` for manual snapshot execution.
    pub fn plan_snapshot_trigger(
        target_node: NodeId,
        snapshot_sequence: SequenceNumber,
    ) -> SnapshotTriggerPlan {
        SnapshotTriggerPlan {
            target_node,
            snapshot_sequence,
        }
    }
}
