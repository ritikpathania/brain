//! Event-Sourced `SchedulingMetricsProjection` (Phase 9 Milestone 9.2).
//!
//! ### Architectural Invariants:
//! 1. Pure Event Projection: `SchedulingMetricsProjection` contains ZERO mutable runtime counters; metrics are derived strictly by replaying `SchedulingEvent` streams.
//! 2. Replay Invariant: `Replay(events) == Replay(events)`.

use crate::planning::scheduler::{SchedulingEvent, SchedulingEventKind};
use serde::{Deserialize, Serialize};

/// Metric projection derived strictly from replaying an append-only `SchedulingEvent` log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SchedulingMetricsProjection {
    /// Total scheduling events processed.
    pub total_events: usize,
    /// Tasks requested placement count.
    pub placements_requested_count: usize,
    /// Workers selected count.
    pub workers_selected_count: usize,
    /// Leases granted count.
    pub leases_granted_count: usize,
    /// Leases renewed count.
    pub leases_renewed_count: usize,
    /// Leases released count.
    pub leases_released_count: usize,
    /// Leases expired count.
    pub leases_expired_count: usize,
}

impl SchedulingMetricsProjection {
    /// Instantiates a new `SchedulingMetricsProjection`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Projects an append-only `SchedulingEvent` stream deterministically.
    pub fn project_events(&mut self, events: &[SchedulingEvent]) {
        for event in events {
            self.total_events += 1;
            match event.kind {
                SchedulingEventKind::TaskScheduled => {
                    self.placements_requested_count += 1;
                }
                SchedulingEventKind::WorkerSelected => {
                    self.workers_selected_count += 1;
                }
                SchedulingEventKind::LeaseGranted => {
                    self.leases_granted_count += 1;
                }
                SchedulingEventKind::LeaseRenewed => {
                    self.leases_renewed_count += 1;
                }
                SchedulingEventKind::LeaseReleased => {
                    self.leases_released_count += 1;
                }
                SchedulingEventKind::LeaseExpired => {
                    self.leases_expired_count += 1;
                }
            }
        }
    }
}
