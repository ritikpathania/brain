//! Decoupled versioned `ReflectionInput` snapshot contract (Phase 6 Milestone 6.1).

use crate::compiler::{EntityIR, RelationIR};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed 0-indexed or UUID identifier for a `ReflectionInput` snapshot revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub Uuid);

impl std::fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "snap_{}", self.0)
    }
}

/// Immutable, versioned snapshot of compiled knowledge input for reflection passes.
///
/// Decouples reflection pass inspection from underlying raw storage or compiler pipeline structures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionInput {
    /// Unique versioned snapshot identifier.
    pub snapshot_id: SnapshotId,
    /// List of canonical entity IR representations in this snapshot.
    pub entities: Vec<EntityIR>,
    /// List of relation IR representations in this snapshot.
    pub relations: Vec<RelationIR>,
    /// Snapshot creation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

impl ReflectionInput {
    /// Instantiates a new versioned `ReflectionInput` snapshot.
    pub fn new(entities: Vec<EntityIR>, relations: Vec<RelationIR>, timestamp_ms: u64) -> Self {
        Self {
            snapshot_id: SnapshotId(Uuid::new_v4()),
            entities,
            relations,
            timestamp_ms,
        }
    }
}
