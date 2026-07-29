//! Event-Sourced `ClusterTopologyProjection` (Phase 10 Milestone 10.2).
//!
//! ### Architectural Invariants:
//! 1. Pure Event Projection: `ClusterTopologyProjection` contains ZERO mutable runtime state; metrics are derived strictly by replaying `ClusterEvent` streams.
//! 2. Replay Invariant: `Replay(cluster_events) == Replay(cluster_events)`.

use crate::planning::cluster::{ClusterEvent, ClusterEventKind};
use serde::{Deserialize, Serialize};

/// Metric projection derived strictly from replaying an append-only `ClusterEvent` log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClusterTopologyProjection {
    /// Total cluster events processed.
    pub total_events: usize,
    /// Nodes joined count.
    pub nodes_joined_count: usize,
    /// Nodes activated count.
    pub nodes_activated_count: usize,
    /// Nodes suspected count.
    pub nodes_suspected_count: usize,
    /// Nodes recovered count.
    pub nodes_recovered_count: usize,
    /// Nodes left count.
    pub nodes_left_count: usize,
    /// Epoch advancements count.
    pub epoch_advancements_count: usize,
    /// Fence rejections count.
    pub fence_rejections_count: usize,
}

impl ClusterTopologyProjection {
    /// Instantiates a new `ClusterTopologyProjection`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Projects an append-only `ClusterEvent` stream deterministically.
    pub fn project_events(&mut self, events: &[ClusterEvent]) {
        for event in events {
            self.total_events += 1;
            match event.kind {
                ClusterEventKind::NodeJoined => {
                    self.nodes_joined_count += 1;
                }
                ClusterEventKind::NodeActivated => {
                    self.nodes_activated_count += 1;
                }
                ClusterEventKind::NodeSuspected => {
                    self.nodes_suspected_count += 1;
                }
                ClusterEventKind::NodeRecovered => {
                    self.nodes_recovered_count += 1;
                }
                ClusterEventKind::NodeLeft => {
                    self.nodes_left_count += 1;
                }
                ClusterEventKind::EpochAdvanced => {
                    self.epoch_advancements_count += 1;
                }
                ClusterEventKind::FenceRejected => {
                    self.fence_rejections_count += 1;
                }
            }
        }
    }
}
