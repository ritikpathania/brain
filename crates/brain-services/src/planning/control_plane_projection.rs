//! Compositional `ClusterControlPlaneProjection` (Phase 10 Milestone 10.4).
//!
//! ### Architectural Invariants:
//! 1. Compositional Layer: `ClusterControlPlaneProjection` orchestrates independent projections (`ClusterTopologyProjection`, `LeadershipProjection`, `SchedulingMetricsProjection`); it does NOT introduce independent mutable projection state.
//! 2. Stream Isolation Invariant: Replaying one event stream does NOT mutate or corrupt projections of other streams.
//! 3. Cross-Stream Replay Determinism: `Replay(cluster_events, leadership_events, scheduling_events) == CompositeProjection`.

use crate::planning::cluster::ClusterEvent;
use crate::planning::cluster_projection::ClusterTopologyProjection;
use crate::planning::coordinator::LeadershipEvent;
use crate::planning::leadership_projection::LeadershipProjection;
use crate::planning::scheduler::SchedulingEvent;
use crate::planning::scheduling_metrics::SchedulingMetricsProjection;
use serde::{Deserialize, Serialize};

/// Composite projection derived strictly from replaying independent control plane event streams.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClusterControlPlaneProjection {
    /// Topology metric projection.
    pub topology: ClusterTopologyProjection,
    /// Leadership metric projection.
    pub leadership: LeadershipProjection,
    /// Scheduling metric projection.
    pub scheduling: SchedulingMetricsProjection,
}

impl ClusterControlPlaneProjection {
    /// Instantiates a new `ClusterControlPlaneProjection`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replays independent control plane event streams deterministically.
    pub fn project_all(
        &mut self,
        cluster_events: &[ClusterEvent],
        leadership_events: &[LeadershipEvent],
        scheduling_events: &[SchedulingEvent],
    ) {
        self.topology.project_events(cluster_events);
        self.leadership.project_events(leadership_events);
        self.scheduling.project_events(scheduling_events);
    }
}
