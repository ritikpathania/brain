//! Typed domain findings and `ReflectionReport` models for active reflection (Phase 6 Milestone 6.1).
//!
//! ### Reflection Invariants:
//! 1. **Read-Only**: Reflection passes inspect, score, and recommend without mutating knowledge state.
//! 2. **Determinism**: Given identical `ReflectionInput` and pass configuration, `ReflectionReport` must be 100% identical.

use crate::compiler::{EntityId, FactId};
use crate::reflection::input::SnapshotId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed identifier for a reflection finding item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FindingId(pub Uuid);

impl std::fmt::Display for FindingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "finding_{}", self.0)
    }
}

/// Payload details for duplicate entity detection finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DuplicateEntityDetails {
    /// Candidate duplicate entity IDs.
    pub entity_ids: Vec<EntityId>,
    /// Lexical or semantic similarity score [0.0..1.0].
    pub similarity_score: f32,
}

/// Payload details for attribute contradiction finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContradictionDetails {
    /// Target entity ID.
    pub entity_id: EntityId,
    /// Conflicting fact IDs.
    pub conflicting_fact_ids: Vec<FactId>,
    /// Detailed description of contradiction.
    pub description: String,
}

/// Payload details for orphan entity finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrphanEntityDetails {
    /// Target orphan entity ID with no incoming/outgoing relation edges.
    pub entity_id: EntityId,
}

/// Payload details for confidence decay finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceDecayDetails {
    /// Target entity ID.
    pub entity_id: EntityId,
    /// Previous confidence score.
    pub old_confidence: f32,
    /// Recalibrated confidence score after decay.
    pub new_confidence: f32,
}

/// Strongly-typed payload enum for domain reflection findings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReflectionFindingKind {
    /// Duplicate entity candidates discovered.
    DuplicateEntity(DuplicateEntityDetails),
    /// Fact attribute contradiction discovered.
    AttributeContradiction(ContradictionDetails),
    /// Unconnected orphan entity discovered.
    OrphanEntity(OrphanEntityDetails),
    /// Confidence score decay recommendation.
    ConfidenceDecay(ConfidenceDecayDetails),
}

/// Domain finding item emitted by a read-only reflection pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionFindingV2 {
    /// Unique finding identifier.
    pub id: FindingId,
    /// Strongly-typed payload kind.
    pub kind: ReflectionFindingKind,
    /// Finding confidence score [0.0..1.0].
    pub confidence: f32,
}

/// Domain report recording observations produced during a reflection execution cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionReportV2 {
    /// Unique report execution UUID.
    pub report_id: Uuid,
    /// Versioned input snapshot ID inspected.
    pub snapshot_id: SnapshotId,
    /// List of discovered domain findings.
    pub findings: Vec<ReflectionFindingV2>,
    /// Total count of canonical entities evaluated.
    pub evaluated_entities_count: usize,
    /// Report generation timestamp in milliseconds.
    pub timestamp_ms: u64,
}
