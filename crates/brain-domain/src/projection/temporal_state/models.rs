//! Data models for Temporal State Projection.

use crate::bkf::*;
use serde::{Deserialize, Serialize};

/// Wrapper around FactVersionId for temporal projection indexing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TemporalFactId(pub FactVersionId);

/// Materialized temporal record representing a fact version's validity interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporalRecord {
    /// Unique fact version ID.
    pub id: TemporalFactId,
    /// Subject entity identifier.
    pub entity_id: KnowledgeEntityId,
    /// Predicate identifier.
    pub predicate_id: PredicateId,
    /// Inclusive beginning of the validity interval.
    pub valid_from: Timestamp,
    /// Exclusive end of the validity interval. None indicates an open/active interval ([valid_from, ∞)).
    pub valid_until: Option<Timestamp>,
    /// Explicit fact lifecycle state (e.g. Verified, Archived, Superseded).
    pub lifecycle: FactLifecycle,
    /// Bounded confidence score.
    pub confidence: Confidence,
    /// Predecessor fact version ID in the version lineage chain.
    pub previous_version: Option<FactVersionId>,
}

impl TemporalRecord {
    /// Returns true if the validity interval is open ([valid_from, ∞)).
    pub fn is_active(&self) -> bool {
        self.valid_until.is_none()
    }
}
