//! Domain-neutral ReasoningFinding, ReasoningFindingKind, and immutable ReasoningResult models.

use crate::execution::ExecutionId;
use crate::selection::EvidenceSet;
use crate::value::StructuredValue;
use std::fmt;
use uuid::Uuid;

/// Strongly-typed identifier for a reasoning finding.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ReasoningFindingId(pub Uuid);

impl ReasoningFindingId {
    /// Instantiates a new unique `ReasoningFindingId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ReasoningFindingId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ReasoningFindingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "find-{}", self.0.simple())
    }
}

/// Classification of domain synthesis findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReasoningFindingKind {
    /// Direct empirical observation.
    Observation,
    /// Derived factual proposition or claim.
    Claim,
    /// High-level logical synthesis conclusion.
    Conclusion,
    /// Actionable recommendation.
    Recommendation,
}

impl fmt::Display for ReasoningFindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation => write!(f, "Observation"),
            Self::Claim => write!(f, "Claim"),
            Self::Conclusion => write!(f, "Conclusion"),
            Self::Recommendation => write!(f, "Recommendation"),
        }
    }
}

/// Structured finding associating a structured domain value payload with supporting evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningFinding {
    /// Strongly-typed finding identifier.
    pub id: ReasoningFindingId,
    /// Semantic classification of this finding.
    pub kind: ReasoningFindingKind,
    /// Structured domain payload.
    pub value: StructuredValue,
    /// Opaque supporting evidence set.
    pub supporting_evidence: EvidenceSet,
}

impl ReasoningFinding {
    /// Instantiates a new `ReasoningFinding`.
    pub fn new(
        kind: ReasoningFindingKind,
        value: StructuredValue,
        supporting_evidence: EvidenceSet,
    ) -> Self {
        Self {
            id: ReasoningFindingId::new(),
            kind,
            value,
            supporting_evidence,
        }
    }
}

/// Immutable, end-to-end reasoning synthesis result aggregate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningResult {
    /// Execution run identifier.
    pub execution_id: ExecutionId,
    /// Contextual query string.
    pub query: String,
    /// Derived structured findings.
    pub findings: Vec<ReasoningFinding>,
    /// Combined overall evidence set backing the synthesis findings.
    pub evidence_set: EvidenceSet,
}

impl ReasoningResult {
    /// Instantiates a new `ReasoningResult`.
    pub fn new(
        execution_id: ExecutionId,
        query: String,
        findings: Vec<ReasoningFinding>,
        evidence_set: EvidenceSet,
    ) -> Self {
        Self {
            execution_id,
            query,
            findings,
            evidence_set,
        }
    }
}
