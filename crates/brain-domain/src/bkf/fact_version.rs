//! Core entities, assertions, temporal windows, and versioned fact structures for Reflection Engine v2.

use crate::bkf::value_objects::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Strongly-typed identifier for domain entities in Reflection v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KnowledgeEntityId(pub Uuid);

/// Strongly-typed identifier for semantic assertions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssertionId(pub Uuid);

/// Strongly-typed identifier for immutable fact versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FactVersionId(pub Uuid);

/// Strongly-typed identifier for predicate definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PredicateId(pub Uuid);

/// Strongly-typed entity category/type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KnowledgeEntityKind(String);

impl KnowledgeEntityKind {
    /// Creates a new KnowledgeEntityKind from a string slice.
    pub fn new(kind: &str) -> Result<Self, String> {
        let trimmed = kind.trim();
        if trimmed.is_empty() {
            return Err("KnowledgeEntityKind cannot be empty".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Returns a string slice of the entity kind.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Domain entity representing a stable subject or object identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KnowledgeEntity {
    /// Stable entity identifier.
    pub id: KnowledgeEntityId,
    /// Canonical entity name.
    pub name: EntityName,
    /// Entity category kind.
    pub kind: KnowledgeEntityKind,
}

/// Category of semantic claim for targeted reflection rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionKind {
    /// Attribute property assertion.
    Attribute,
    /// Subject-predicate-object relationship.
    Relationship,
    /// IsA classification hierarchy claim.
    Classification,
    /// Event occurrence claim.
    Event,
    /// Observation measurement.
    Observation,
}

/// Target of a semantic claim: either another Entity or a Literal Value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssertionTarget {
    /// Target is another Entity.
    Entity(KnowledgeEntityId),
    /// Target is a literal value.
    Value(LiteralValue),
}

/// Predicate definition expressing structural and temporal constraints.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Predicate {
    /// Unique predicate identifier.
    pub id: PredicateId,
    /// Canonical predicate name.
    pub name: PredicateName,
    /// Exclusivity / cardinality constraint.
    pub cardinality: PredicateCardinality,
    /// Flag indicating whether value changes invalidate prior temporal windows.
    pub is_temporal: bool,
    /// Optional reciprocal inverse predicate.
    pub inverse: Option<PredicateId>,
}

/// Semantic assertion claiming a relationship or property.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticAssertion {
    /// Assertion identifier.
    pub id: AssertionId,
    /// Assertion category.
    pub kind: AssertionKind,
    /// Subject entity identifier.
    pub subject: KnowledgeEntityId,
    /// Predicate ID.
    pub predicate: PredicateId,
    /// Assertion object target.
    pub object: AssertionTarget,
}

/// Validated temporal window enforcing statement, ingestion, and reality bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporalWindow {
    /// Timestamp when the claim was stated.
    pub asserted_at: Timestamp,
    /// Timestamp when Brain learned/ingested the claim.
    pub observed_at: Timestamp,
    /// Timestamp when the claim became true in reality.
    pub valid_from: Timestamp,
    /// Optional timestamp when the claim stopped being true (None = active).
    pub valid_to: Option<Timestamp>,
}

impl TemporalWindow {
    /// Validates and constructs a TemporalWindow.
    pub fn new(
        asserted_at: Timestamp,
        observed_at: Timestamp,
        valid_from: Timestamp,
        valid_to: Option<Timestamp>,
    ) -> Result<Self, String> {
        if asserted_at > observed_at {
            return Err("asserted_at cannot be later than observed_at".to_string());
        }
        if let Some(vt) = valid_to {
            if valid_from > vt {
                return Err("valid_from cannot be later than valid_to".to_string());
            }
        }
        Ok(Self {
            asserted_at,
            observed_at,
            valid_from,
            valid_to,
        })
    }

    /// Derives whether this temporal window represents a historical fact.
    pub fn is_historical(&self) -> bool {
        self.valid_to.is_some()
    }
}

/// Source origin of a fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FactProvenanceSource {
    /// Ingested from a conversation session.
    Conversation {
        /// Session identifier.
        session_id: String,
        /// Message identifier.
        message_id: String,
    },
    /// Extracted from a document.
    Document {
        /// Source URI.
        source_uri: String,
    },
    /// Generated by a plugin.
    Plugin {
        /// Plugin identifier.
        plugin_id: String,
    },
    /// Extracted from bulk data import.
    Import {
        /// Import format.
        format: String,
        /// Batch identifier.
        batch_id: String,
    },
    /// Derived by an inference or reflection pass.
    Inference {
        /// Pass identifier.
        pass_id: String,
        /// Explanation rationale.
        rationale: String,
    },
    /// Manually asserted by a user.
    Manual {
        /// User identifier.
        user_id: String,
    },
}

/// Provenance tracking source origin and input version lineage DAG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactProvenance {
    /// Source origin.
    pub source: FactProvenanceSource,
    /// Parent version lineage DAG.
    pub derived_from: Vec<FactVersionId>,
}

/// Explicit lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactLifecycle {
    /// Unverified candidate fact.
    Candidate,
    /// Verified active fact.
    Verified,
    /// Archived cold-storage fact.
    Archived,
}

/// Immutable versioned assertion record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactVersion {
    /// Fact version identifier.
    pub id: FactVersionId,
    /// Assertion identifier.
    pub assertion_id: AssertionId,
    /// Explicit lifecycle state.
    pub lifecycle: FactLifecycle,
    /// Bounded confidence score.
    pub confidence: Confidence,
    /// Temporal window boundaries.
    pub temporal: TemporalWindow,
    /// Single-sided predecessor link.
    pub supersedes: Option<FactVersionId>,
    /// Lineage and source origin.
    pub provenance: FactProvenance,
}
