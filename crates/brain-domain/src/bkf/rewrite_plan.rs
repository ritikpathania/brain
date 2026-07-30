//! Declarative rewrite intent models for Reflection Engine v2.

use crate::bkf::fact_version::*;
use crate::bkf::value_objects::*;
use serde::{Deserialize, Serialize};

/// High-level rationale categories for why a rewrite plan was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteReason {
    /// Contradiction detected between active exclusive facts.
    Contradiction,
    /// Duplicate entities or assertions identified for consolidation.
    Duplicate,
    /// Lineage corroboration resulted in a confidence increase.
    ConfidenceIncrease,
    /// Source decay or conflict resulted in a confidence decrease.
    ConfidenceDecrease,
    /// Temporal window expiration.
    TemporalExpiration,
    /// Text casing, whitespace, or alias canonicalization.
    Canonicalization,
}

/// Atomic declarative rewrite operation intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RewriteOperation {
    /// Record a newly constructed fact version.
    RecordFact(FactVersion),
    /// Close an existing fact version's temporal window and mark supercedence link.
    SupersedeFact {
        /// Identifier of superseded fact version.
        old_fact_id: FactVersionId,
        /// Identifier of successor fact version.
        new_fact_id: FactVersionId,
        /// Timestamp when supercedence window closed.
        closed_at: Timestamp,
    },
    /// Consolidate multiple redundant source fact versions into a single canonical target fact.
    MergeFacts {
        /// Identifiers of source redundant fact versions.
        source_fact_ids: Vec<FactVersionId>,
        /// Identifier of canonical target fact version.
        target_fact_id: FactVersionId,
    },
    /// Move a fact version into archived cold storage.
    ArchiveFact {
        /// Identifier of archived fact version.
        fact_id: FactVersionId,
        /// Timestamp when archiving occurred.
        archived_at: Timestamp,
    },
}

/// Declarative plan emitted by a reflection pass containing proposed operations and cost estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RewritePlan {
    /// Identifier of the producing reflection pass.
    pub pass_id: PassId,
    /// Category reason for the rewrite.
    pub reason: RewriteReason,
    /// Human-readable explanation rationale.
    pub rationale: String,
    /// Estimated execution cost in operation units.
    pub execution_cost: u32,
    /// Ordered sequence of proposed rewrite operations.
    pub operations: Vec<RewriteOperation>,
}
