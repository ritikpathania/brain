//! Dynamic Cluster Membership, Joint Consensus & Configuration Transitions (Phase 12 Milestone 12.3).
//!
//! ### Architectural Invariants:
//! 1. Immutable `MembershipView`: Node membership snapshots are immutable artifacts carrying a strongly-typed `ConfigurationVersion(pub u64)`.
//! 2. Joint Consensus ($C_{old,new}$): Configuration changes transition through explicit joint consensus states requiring majorities from BOTH $C_{old}$ and $C_{new}$ ($Q_{old} \land Q_{new}$).
//! 3. Decoupled Planner/Applier Architecture: `ConfigurationPlanner` constructs `ConfigurationTransition`; `ConfigurationApplier` commits transitions into a new `MembershipView`.

use crate::planning::cluster::NodeId;
use crate::planning::consensus::QuorumEvaluator;
use serde::{Deserialize, Serialize};

/// Strongly-typed 1-based monotonic configuration version identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConfigurationVersion(pub u64);

impl std::fmt::Display for ConfigurationVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "config_v{}", self.0)
    }
}

/// Immutable snapshot of cluster node roles and voting privileges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipView {
    /// Strongly-typed configuration version.
    pub version: ConfigurationVersion,
    /// Active voting coordinator node IDs ($C$).
    pub voters: Vec<NodeId>,
    /// Non-voting learner node IDs ($L$).
    pub learners: Vec<NodeId>,
}

impl MembershipView {
    /// Instantiates a new `MembershipView`.
    pub fn new(version: ConfigurationVersion, voters: Vec<NodeId>, learners: Vec<NodeId>) -> Self {
        Self {
            version,
            voters,
            learners,
        }
    }

    /// Evaluates if a given node is an active voting member.
    pub fn is_voter(&self, node_id: &NodeId) -> bool {
        self.voters.contains(node_id)
    }
}

/// Dynamic joint consensus configuration transition lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigurationTransition {
    /// Single stable configuration state ($C_{old}$ or $C_{new}$).
    Stable(MembershipView),
    /// Joint consensus transition state ($C_{old,new}$) requiring dual-quorum approval.
    Joint {
        /// Former active membership configuration ($C_{old}$).
        old: MembershipView,
        /// Target new membership configuration ($C_{new}$).
        new: MembershipView,
    },
}

/// Pure planner constructing `ConfigurationTransition` artifacts.
pub struct ConfigurationPlanner;

impl ConfigurationPlanner {
    /// Plans a joint consensus transition ($C_{old} \rightarrow C_{old,new}$) to update active voters and learners.
    pub fn plan_transition(
        current: &MembershipView,
        target_voters: Vec<NodeId>,
        target_learners: Vec<NodeId>,
    ) -> ConfigurationTransition {
        let next_version = ConfigurationVersion(current.version.0 + 1);
        let new_view = MembershipView::new(next_version, target_voters, target_learners);

        ConfigurationTransition::Joint {
            old: current.clone(),
            new: new_view,
        }
    }
}

/// Applier committing `ConfigurationTransition` states into new `MembershipView` snapshots.
pub struct ConfigurationApplier;

impl ConfigurationApplier {
    /// Evaluates whether a vote count satisfies joint consensus requirements ($Q_{old} \land Q_{new}$).
    pub fn evaluate_joint_quorum(
        old_votes: usize,
        old_total: usize,
        new_votes: usize,
        new_total: usize,
    ) -> bool {
        let old_ok = QuorumEvaluator::evaluate_quorum(old_votes, old_total);
        let new_ok = QuorumEvaluator::evaluate_quorum(new_votes, new_total);
        old_ok && new_ok
    }

    /// Commits a `ConfigurationTransition::Joint` state into a final stable `MembershipView` ($C_{new}$).
    pub fn finalize_transition(transition: &ConfigurationTransition) -> MembershipView {
        match transition {
            ConfigurationTransition::Stable(view) => view.clone(),
            ConfigurationTransition::Joint { new, .. } => {
                let final_version = ConfigurationVersion(new.version.0 + 1);
                MembershipView::new(final_version, new.voters.clone(), new.learners.clone())
            }
        }
    }
}
