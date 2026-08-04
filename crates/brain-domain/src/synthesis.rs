//! Domain-neutral ReasoningFinding, ReasoningFindingKind, and immutable ReasoningResult models.

use crate::execution::ExecutionId;
use crate::selection::EvidenceSet;
use crate::value::StructuredValue;
use std::fmt;

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
            kind,
            value,
            supporting_evidence,
        }
    }
}

/// Immutable, end-to-end reasoning synthesis result aggregate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningResult {
    /// Execution run ID.
    pub execution_id: ExecutionId,
    /// Original user query.
    pub user_query: String,
    /// Derived structured findings.
    pub findings: Vec<ReasoningFinding>,
    /// Global selected evidence set.
    pub evidence_set: EvidenceSet,
}

impl ReasoningResult {
    /// Instantiates a new immutable `ReasoningResult`.
    pub fn new(
        execution_id: ExecutionId,
        user_query: String,
        findings: Vec<ReasoningFinding>,
        evidence_set: EvidenceSet,
    ) -> Self {
        Self {
            execution_id,
            user_query,
            findings,
            evidence_set,
        }
    }
}
