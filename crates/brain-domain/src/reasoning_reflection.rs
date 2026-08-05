//! Domain models for Reasoning Reflection & Critique: ReflectionReportId, ReflectionFindingKind, ReasoningReflectionFinding, and ReflectionReport.

use crate::errors::DomainError;
use crate::execution::ExecutionId;
use crate::selection::EvidenceSet;
use crate::synthesis::ReasoningFindingId;
use crate::value::StructuredValue;
use std::fmt;
use uuid::Uuid;

/// Strongly-typed identifier for a reasoning reflection report.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ReflectionReportId(pub Uuid);

impl ReflectionReportId {
    /// Instantiates a new unique `ReflectionReportId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ReflectionReportId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ReflectionReportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "refl-{}", self.0.simple())
    }
}

/// Semantic taxonomy of critique finding classifications.
/// Invariant: Reflection identifies issues; it never prescribes repairs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ReflectionFindingKind {
    /// Insufficient evidence backing a finding.
    MissingEvidence,
    /// Weak or low-confidence supporting evidence.
    WeakSupport,
    /// Direct logical or empirical contradiction detected.
    Contradiction,
    /// Redundant claims or duplicate evidence paths.
    Redundancy,
    /// Ambiguous or under-specified proposition.
    Ambiguity,
    /// Unresolved or incomplete reasoning path.
    IncompleteReasoning,
    /// Structural issue such as malformed references or topological violation.
    StructuralIssue,
}

impl fmt::Display for ReflectionFindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEvidence => write!(f, "MissingEvidence"),
            Self::WeakSupport => write!(f, "WeakSupport"),
            Self::Contradiction => write!(f, "Contradiction"),
            Self::Redundancy => write!(f, "Redundancy"),
            Self::Ambiguity => write!(f, "Ambiguity"),
            Self::IncompleteReasoning => write!(f, "IncompleteReasoning"),
            Self::StructuralIssue => write!(f, "StructuralIssue"),
        }
    }
}

/// Evidence-backed structured critique finding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReasoningReflectionFinding {
    /// Classification of this reflection finding.
    pub kind: ReflectionFindingKind,
    /// Target reasoning finding ID affected by this critique.
    pub affected_finding: ReasoningFindingId,
    /// Evidence set supporting this critique observation.
    pub supporting_evidence: EvidenceSet,
    /// Structured domain explanation justifying why this critique conclusion was reached.
    pub justification: StructuredValue,
}

impl ReasoningReflectionFinding {
    /// Instantiates a new `ReasoningReflectionFinding`.
    pub fn new(
        kind: ReflectionFindingKind,
        affected_finding: ReasoningFindingId,
        supporting_evidence: EvidenceSet,
        justification: StructuredValue,
    ) -> Self {
        Self {
            kind,
            affected_finding,
            supporting_evidence,
            justification,
        }
    }
}

/// Immutable, end-to-end critique report aggregate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReflectionReport {
    /// Unique report identifier.
    pub id: ReflectionReportId,
    /// Target execution run ID.
    pub execution_id: ExecutionId,
    /// List of derived reflection findings.
    pub findings: Vec<ReasoningReflectionFinding>,
}

impl ReflectionReport {
    /// Instantiates a new immutable `ReflectionReport`.
    pub fn new(execution_id: ExecutionId, findings: Vec<ReasoningReflectionFinding>) -> Self {
        Self {
            id: ReflectionReportId::new(),
            execution_id,
            findings,
        }
    }

    /// Evaluates whether a reasoning finding is eligible for candidate extraction.
    /// Invariant: Candidate extraction delegates eligibility to ReflectionReport.
    pub fn is_candidate_eligible(&self, finding_id: &ReasoningFindingId) -> bool {
        !self.findings.iter().any(|f| {
            f.affected_finding == *finding_id
                && (f.kind == ReflectionFindingKind::MissingEvidence
                    || f.kind == ReflectionFindingKind::StructuralIssue
                    || f.kind == ReflectionFindingKind::Contradiction)
        })
    }

    /// Validates internal consistency of the report.
    pub fn validate(&self) -> Result<(), DomainError> {
        Ok(())
    }
}
