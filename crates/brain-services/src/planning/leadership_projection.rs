//! Event-Sourced `LeadershipProjection` (Phase 10 Milestone 10.3).
//!
//! ### Architectural Invariants:
//! 1. Pure Event Projection: `LeadershipProjection` contains ZERO mutable runtime state; metrics are derived strictly by replaying `LeadershipEvent` streams.
//! 2. Replay Invariant: `Replay(leadership_events) == Replay(leadership_events)`.

use crate::planning::coordinator::{LeadershipEvent, LeadershipEventKind};
use serde::{Deserialize, Serialize};

/// Metric projection derived strictly from replaying an append-only `LeadershipEvent` log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LeadershipProjection {
    /// Total leadership events processed.
    pub total_events: usize,
    /// Elections started count.
    pub election_started_count: usize,
    /// Leaders elected count.
    pub leaders_elected_count: usize,
    /// Leadership transfers count.
    pub transfers_count: usize,
    /// Leadership losses count.
    pub losses_count: usize,
    /// Leadership recoveries count.
    pub recoveries_count: usize,
    /// Quorum established count.
    pub quorum_established_count: usize,
    /// Quorum lost count.
    pub quorum_lost_count: usize,
}

impl LeadershipProjection {
    /// Instantiates a new `LeadershipProjection`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Projects an append-only `LeadershipEvent` stream deterministically.
    pub fn project_events(&mut self, events: &[LeadershipEvent]) {
        for event in events {
            self.total_events += 1;
            match &event.kind {
                LeadershipEventKind::LeaderElectionStarted { .. } => {
                    self.election_started_count += 1;
                }
                LeadershipEventKind::LeaderElected { .. } => {
                    self.leaders_elected_count += 1;
                }
                LeadershipEventKind::LeadershipTransferred { .. } => {
                    self.transfers_count += 1;
                }
                LeadershipEventKind::LeadershipLost { .. } => {
                    self.losses_count += 1;
                }
                LeadershipEventKind::LeadershipRecovered { .. } => {
                    self.recoveries_count += 1;
                }
                LeadershipEventKind::QuorumEstablished { .. } => {
                    self.quorum_established_count += 1;
                }
                LeadershipEventKind::QuorumLost { .. } => {
                    self.quorum_lost_count += 1;
                }
            }
        }
    }
}

impl crate::planning::recovery_engine::RestoreFromSnapshot<LeadershipProjection>
    for LeadershipProjection
{
    fn restore_snapshot(&mut self, state: &LeadershipProjection) {
        *self = state.clone();
    }
}
