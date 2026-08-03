//! Projection-Agnostic `ReplayTarget<E>` & `LogReplayEngine` (Phase 11 Milestone 11.1).
//!
//! ### Architectural Invariants:
//! 1. Projection Agnosticism: Replay engine operates exclusively via `ReplayTarget<E>` traits, avoiding tight coupling to specific domain projections.
//! 2. Arbitrary Sequence Offset Support: Replay can begin from `SequenceNumber(1)` or any arbitrary sequence offset to support snapshots and incremental recovery.
//! 3. Replay Idempotency & Determinism: `Replay(log, start)` produces 100% deterministic projection states regardless of chunking.

use crate::planning::cluster::ClusterEvent;
use crate::planning::cluster_projection::ClusterTopologyProjection;
use crate::planning::coordinator::LeadershipEvent;
use crate::planning::durable_event_store::{EventEnvelope, EventLog, SequenceNumber};
use crate::planning::event_publisher::EventPublishError;
use crate::planning::leadership_projection::LeadershipProjection;
use crate::planning::scheduler::SchedulingEvent;
use crate::planning::scheduling_metrics::SchedulingMetricsProjection;

/// Projection-agnostic target contract for applying event log envelopes.
pub trait ReplayTarget<E> {
    /// Applies a canonical event envelope to update projection metrics.
    fn apply_envelope(&mut self, envelope: &EventEnvelope<E>);
}

impl ReplayTarget<ClusterEvent> for ClusterTopologyProjection {
    fn apply_envelope(&mut self, envelope: &EventEnvelope<ClusterEvent>) {
        self.project_events(std::slice::from_ref(&envelope.payload));
    }
}

impl ReplayTarget<LeadershipEvent> for LeadershipProjection {
    fn apply_envelope(&mut self, envelope: &EventEnvelope<LeadershipEvent>) {
        self.project_events(std::slice::from_ref(&envelope.payload));
    }
}

impl ReplayTarget<SchedulingEvent> for SchedulingMetricsProjection {
    fn apply_envelope(&mut self, envelope: &EventEnvelope<SchedulingEvent>) {
        self.project_events(std::slice::from_ref(&envelope.payload));
    }
}

/// Generic log replay engine for state recovery and projection reconstruction.
pub struct LogReplayEngine;

impl LogReplayEngine {
    /// Replays event log envelopes from an arbitrary `start` sequence offset into a `ReplayTarget<E>`.
    pub fn replay_from_offset<E, T: ReplayTarget<E>, L: EventLog<E>>(
        log: &L,
        target: &mut T,
        start: SequenceNumber,
        batch_size: usize,
    ) -> Result<usize, EventPublishError> {
        let mut current_offset = start;
        let mut total_replayed = 0;

        loop {
            let envelopes = log.read_range(current_offset, batch_size)?;
            if envelopes.is_empty() {
                break;
            }

            let batch_count = envelopes.len();
            for envelope in &envelopes {
                target.apply_envelope(envelope);
            }

            total_replayed += batch_count;
            current_offset = SequenceNumber(current_offset.0 + batch_count as u64);
        }

        Ok(total_replayed)
    }

    /// Replays event log envelopes starting from `start` sequence up to `limit` total envelopes into a `ReplayTarget<E>`.
    pub fn replay_range<E, T: ReplayTarget<E>, L: EventLog<E>>(
        log: &L,
        target: &mut T,
        start: SequenceNumber,
        limit: usize,
    ) -> Result<usize, EventPublishError> {
        let envelopes = log.read_range(start, limit)?;
        let count = envelopes.len();
        for envelope in &envelopes {
            target.apply_envelope(envelope);
        }
        Ok(count)
    }
}
