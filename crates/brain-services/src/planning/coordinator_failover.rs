//! Decoupled Coordinator Failover Engine & State Recovery Architecture (Phase 11 Milestone 11.3).
//!
//! ### Architectural Invariants:
//! 1. Decoupled Lifecycle: Failure Detection -> Failover Planning -> Failover Plan -> Failover Execution -> Recovery Report.
//! 2. Immutable Failover Plan: `FailoverPlan` captures the plan target leader, term, epoch, and recovery strategy before execution.
//! 3. Implicit Fence Token Invalidation: Epoch advancement implicitly invalidates any lease with `epoch < current_epoch`.
//! 4. Sequence-Based Recovery Progress: `RecoveryProgress` tracks sequence ranges (`start_sequence` ..= `end_sequence`) rather than raw event counts.

use crate::planning::cluster::{ClusterError, ClusterManager, ClusterNodeStatus, EpochId, NodeId};
use crate::planning::consensus::{ConsensusEngine, TermId};
use crate::planning::coordinator::CoordinatorLeader;
use crate::planning::durable_event_store::SequenceNumber;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Failover state machine phase classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FailoverState {
    /// System operating normally.
    Idle,
    /// Failure observation detected.
    FailureDetected,
    /// Failover plan generation in progress.
    Planning,
    /// Consensus election in progress.
    Election,
    /// Log replay and state recovery in progress.
    Recovering,
    /// Recovery completed successfully.
    Recovered,
}

/// Recovery execution strategy classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Replay control plane events directly from log sequence start offset.
    ReplayOnly,
    /// Restore latest snapshot then replay log entries from snapshot sequence offset.
    SnapshotThenReplay,
    /// Cold start initial state bootstrap.
    ColdStart,
}

/// Immutable failover plan artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailoverPlan {
    /// Node ID of the former leader being replaced.
    pub former_leader: Option<NodeId>,
    /// Node ID of the promoted target leader.
    pub target_leader: NodeId,
    /// Consensus term for the promotion.
    pub target_term: TermId,
    /// Target epoch advancing cluster epoch monotonically.
    pub target_epoch: EpochId,
    /// Applied recovery strategy.
    pub recovery_strategy: RecoveryStrategy,
}

/// Sequence-based recovery progress measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryProgress {
    /// Starting log sequence number.
    pub start_sequence: SequenceNumber,
    /// Ending log sequence number.
    pub end_sequence: SequenceNumber,
    /// Recovered consensus term.
    pub recovered_term: TermId,
    /// Recovered cluster epoch.
    pub recovered_epoch: EpochId,
}

/// Audit report detailing failover execution results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRecoveryReport {
    /// Executed failover plan.
    pub plan: FailoverPlan,
    /// Measured recovery progress.
    pub progress: RecoveryProgress,
    /// Timestamp in milliseconds when recovery was finalized.
    pub timestamp_ms: u64,
}

/// External failure detector observing node health status.
pub struct FailureDetector;

impl FailureDetector {
    /// Observes cluster nodes and returns suspect active leader node ID if one exists.
    pub fn detect_suspect_leader(
        cluster: &ClusterManager,
        current_leader_id: Option<NodeId>,
    ) -> Option<NodeId> {
        let leader_id = current_leader_id?;
        let leader_node = cluster.get_node(&leader_id)?;

        if leader_node.status == ClusterNodeStatus::Suspect
            || leader_node.status == ClusterNodeStatus::Left
        {
            Some(leader_id)
        } else {
            None
        }
    }
}

/// Failover planner generating immutable `FailoverPlan` artifacts.
pub struct FailoverPlanner;

impl FailoverPlanner {
    /// Generates a `FailoverPlan` for replacing a suspect/failed leader.
    pub fn plan_failover(
        cluster: &mut ClusterManager,
        consensus_engine: &ConsensusEngine,
        suspect_leader: Option<NodeId>,
        target_leader: NodeId,
    ) -> Result<FailoverPlan, ClusterError> {
        let current_state = consensus_engine.current_state();
        let target_term = TermId(current_state.current_term.0 + 1);
        let target_epoch = EpochId(cluster.current_epoch().0 + 1);

        Ok(FailoverPlan {
            former_leader: suspect_leader,
            target_leader,
            target_term,
            target_epoch,
            recovery_strategy: RecoveryStrategy::ReplayOnly,
        })
    }
}

/// Failover executor enforcing failover plans and advancing cluster epoch.
#[derive(Debug)]
pub struct FailoverExecutor {
    state: Mutex<FailoverState>,
}

impl Default for FailoverExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl FailoverExecutor {
    /// Instantiates a new `FailoverExecutor`.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(FailoverState::Idle),
        }
    }

    /// Returns current failover state machine phase.
    pub fn current_state(&self) -> FailoverState {
        *self.state.lock().unwrap()
    }

    /// Executes a `FailoverPlan`, promoting new leader, advancing cluster epoch, and returning `StateRecoveryReport`.
    pub fn execute_plan(
        &self,
        plan: &FailoverPlan,
        cluster: &mut ClusterManager,
        timestamp_ms: u64,
        start_seq: SequenceNumber,
        end_seq: SequenceNumber,
    ) -> Result<(CoordinatorLeader, StateRecoveryReport), ClusterError> {
        *self.state.lock().unwrap() = FailoverState::Planning;
        *self.state.lock().unwrap() = FailoverState::Election;

        // Advance cluster epoch monotonically (implicitly invalidates older leases)
        let epoch = cluster.advance_epoch(timestamp_ms);

        *self.state.lock().unwrap() = FailoverState::Recovering;

        let leader = CoordinatorLeader {
            node_id: plan.target_leader,
            epoch,
            state: crate::planning::coordinator::LeadershipState::Leader,
            elected_at_ms: timestamp_ms,
        };

        let progress = RecoveryProgress {
            start_sequence: start_seq,
            end_sequence: end_seq,
            recovered_term: plan.target_term,
            recovered_epoch: epoch,
        };

        let report = StateRecoveryReport {
            plan: plan.clone(),
            progress,
            timestamp_ms,
        };

        *self.state.lock().unwrap() = FailoverState::Recovered;

        Ok((leader, report))
    }
}
