//! Event-Sourced Domain Events & Publisher for Linearizable Read Observability (Phase 14 Milestone 14.2).
//!
//! ### Architectural Invariants:
//! 1. Decoupled Publisher Boundary: `ReadEventPublisher` isolates read event emission from `LinearizableReadEngine` execution.
//! 2. Immutable Event Artifacts: `ReadEvent` envelopes carry strongly-typed `ReadEventId(pub Uuid)` and schema version headers.
//! 3. Stage Latency Decomposition: `ReadServed` records stage latency breakdown (`planning_latency_us`, `validation_latency_us`, `execution_latency_us`).

use crate::planning::cluster::NodeId;
use crate::planning::consensus::{ReadValidationResult, TermId};
use crate::planning::durable_event_store::{EventEnvelope, SequenceNumber};
use crate::planning::linearizable_read_engine::ReadPlanKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Constant schema version header for `ReadEvent`.
pub const READ_EVENT_SCHEMA_VERSION: u16 = 1;

/// Strongly-typed identifier for linearizable read events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReadEventId(pub Uuid);

impl std::fmt::Display for ReadEventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "read_evt_{}", self.0)
    }
}

/// Domain event classification variants for linearizable read observability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadEventKind {
    /// Zero-log-append read request received.
    ReadRequested {
        /// Target sequence read index.
        target_read_index: SequenceNumber,
    },
    /// Read plan compiled by `ReadPlanner`.
    ReadPlanCompiled {
        /// Compiled plan kind.
        kind: ReadPlanKind,
        /// Target sequence read index.
        target_read_index: SequenceNumber,
    },
    /// Leader lease validated for read.
    LeaseValidated {
        /// Timestamp in milliseconds when lease was granted.
        granted_at_ms: u64,
        /// Lease time-to-live bound in milliseconds.
        ttl_ms: u64,
    },
    /// Read index confirmed via heartbeat quorum ping.
    QuorumValidated,
    /// Linearizable read successfully served to client.
    ReadServed {
        /// Sequence read index at which query was served.
        target_read_index: SequenceNumber,
        /// Duration in microseconds spent compiling read plan.
        planning_latency_us: u64,
        /// Duration in microseconds spent validating lease/quorum.
        validation_latency_us: u64,
        /// Duration in microseconds spent executing projection query.
        execution_latency_us: u64,
    },
    /// Linearizable read rejected with diagnostic reason.
    ReadRejected {
        /// Diagnostic rejection reason.
        reason: ReadValidationResult,
    },
}

/// Immutable event-sourced domain event artifact for linearizable reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadEvent {
    /// Schema version header.
    pub schema_version: u16,
    /// Unique event identifier.
    pub event_id: ReadEventId,
    /// Timestamp in milliseconds when event occurred.
    pub timestamp_ms: u64,
    /// Associated read query identifier.
    pub read_id: Uuid,
    /// Leader node ID servicing read.
    pub leader_id: NodeId,
    /// Leader term when event occurred.
    pub term: TermId,
    /// Event classification kind.
    pub kind: ReadEventKind,
}

impl ReadEvent {
    /// Instantiates a new `ReadEvent`.
    pub fn new(
        timestamp_ms: u64,
        read_id: Uuid,
        leader_id: NodeId,
        term: TermId,
        kind: ReadEventKind,
    ) -> Self {
        Self {
            schema_version: READ_EVENT_SCHEMA_VERSION,
            event_id: ReadEventId(Uuid::new_v4()),
            timestamp_ms,
            read_id,
            leader_id,
            term,
            kind,
        }
    }
}

/// Helper publishing boundary wrapping `ReadEvent` into sequence-allocated envelopes.
pub struct ReadEventPublisher;

impl ReadEventPublisher {
    /// Wraps a `ReadEvent` payload into a sequence-allocated `EventEnvelope`.
    pub fn create_envelope(sequence: SequenceNumber, event: ReadEvent) -> EventEnvelope<ReadEvent> {
        EventEnvelope {
            sequence,
            timestamp_ms: event.timestamp_ms,
            schema_version: event.schema_version,
            payload: event,
        }
    }
}
