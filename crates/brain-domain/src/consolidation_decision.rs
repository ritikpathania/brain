//! Domain models for Consolidation Decisions: DuplicateProbability, ContradictionProbability, AssessmentMetrics, AssessmentExplanation, ConsolidationAssessment, ConsolidationDecision, ConsolidationOutcome, and ConsolidationReport.

use crate::candidate::{CandidateConfidence, KnowledgeCandidateId};
use crate::errors::DomainError;
use crate::evolution::DomainEntityId;
use crate::execution::ExecutionId;
use crate::match_domain::GraphMatchReport;
use std::fmt;
use uuid::Uuid;

/// Opaque, invariant-checked numerical duplicate probability bounded between 0.0 and 1.0.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct DuplicateProbability(f32);

impl DuplicateProbability {
    /// Certain duplicate constant (1.0).
    pub const CERTAIN: Self = Self(1.0);
    /// High duplicate probability constant (0.8).
    pub const HIGH: Self = Self(0.8);
    /// Moderate duplicate probability constant (0.5).
    pub const MODERATE: Self = Self(0.5);
    /// Unlikely duplicate probability constant (0.2).
    pub const UNLIKELY: Self = Self(0.2);
    /// Zero duplicate probability constant (0.0).
    pub const NONE: Self = Self(0.0);

    /// Instantiates a new validated `DuplicateProbability`.
    pub fn new(val: f32) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&val) {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Duplicate probability must be between 0.0 and 1.0, got {}",
                    val
                ),
                rule_id: Some("VAL-PROB-001".to_string()),
            });
        }
        Ok(Self(val))
    }

    /// Returns raw probability score float value.
    pub fn value(&self) -> f32 {
        self.0
    }
}

impl Eq for DuplicateProbability {}

#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for DuplicateProbability {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Default for DuplicateProbability {
    fn default() -> Self {
        Self::NONE
    }
}

impl fmt::Display for DuplicateProbability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// Opaque, invariant-checked numerical contradiction probability bounded between 0.0 and 1.0.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct ContradictionProbability(f32);

impl ContradictionProbability {
    /// Certain contradiction constant (1.0).
    pub const CERTAIN: Self = Self(1.0);
    /// High contradiction probability constant (0.8).
    pub const HIGH: Self = Self(0.8);
    /// Moderate contradiction probability constant (0.5).
    pub const MODERATE: Self = Self(0.5);
    /// Unlikely contradiction probability constant (0.2).
    pub const UNLIKELY: Self = Self(0.2);
    /// Zero contradiction probability constant (0.0).
    pub const NONE: Self = Self(0.0);

    /// Instantiates a new validated `ContradictionProbability`.
    pub fn new(val: f32) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&val) {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Contradiction probability must be between 0.0 and 1.0, got {}",
                    val
                ),
                rule_id: Some("VAL-PROB-002".to_string()),
            });
        }
        Ok(Self(val))
    }

    /// Returns raw probability score float value.
    pub fn value(&self) -> f32 {
        self.0
    }
}

impl Eq for ContradictionProbability {}

#[allow(clippy::derive_ord_xor_partial_ord)]
impl Ord for ContradictionProbability {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl Default for ContradictionProbability {
    fn default() -> Self {
        Self::NONE
    }
}

impl fmt::Display for ContradictionProbability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

/// Grouped metrics derived during candidate match assessment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssessmentMetrics {
    /// Evaluated duplicate probability score.
    pub duplicate_probability: DuplicateProbability,
    /// Evaluated contradiction probability score.
    pub contradiction_probability: ContradictionProbability,
}

impl AssessmentMetrics {
    /// Instantiates a new `AssessmentMetrics`.
    pub fn new(
        duplicate_probability: DuplicateProbability,
        contradiction_probability: ContradictionProbability,
    ) -> Self {
        Self {
            duplicate_probability,
            contradiction_probability,
        }
    }
}

/// Explanation domain object decoupling metrics from underlying justification evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AssessmentExplanation {
    /// Observational graph match report justifying assessment metrics.
    pub matching_report: Option<GraphMatchReport>,
}

impl AssessmentExplanation {
    /// Instantiates an empty `AssessmentExplanation`.
    pub fn empty() -> Self {
        Self {
            matching_report: None,
        }
    }

    /// Instantiates a new `AssessmentExplanation` with matching report.
    pub fn with_report(report: GraphMatchReport) -> Self {
        Self {
            matching_report: Some(report),
        }
    }
}

/// Immutable intermediate assessment object produced prior to deriving a ConsolidationDecision.
/// Invariants:
/// - Immutable value object with private fields and read-only accessors.
/// - Derives metrics but zero consolidation decisions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConsolidationAssessment {
    confidence: CandidateConfidence,
    metrics: AssessmentMetrics,
    explanation: AssessmentExplanation,
}

impl ConsolidationAssessment {
    /// Instantiates a new immutable `ConsolidationAssessment`.
    pub fn new(
        confidence: CandidateConfidence,
        metrics: AssessmentMetrics,
        explanation: AssessmentExplanation,
    ) -> Self {
        Self {
            confidence,
            metrics,
            explanation,
        }
    }

    /// Returns derived candidate confidence.
    pub fn confidence(&self) -> CandidateConfidence {
        self.confidence
    }

    /// Returns derived assessment metrics.
    pub fn metrics(&self) -> &AssessmentMetrics {
        &self.metrics
    }

    /// Returns assessment explanation.
    pub fn explanation(&self) -> &AssessmentExplanation {
        &self.explanation
    }
}

/// Strongly-typed identifier for a consolidation decision.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ConsolidationDecisionId(pub Uuid);

impl ConsolidationDecisionId {
    /// Instantiates a new unique `ConsolidationDecisionId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for ConsolidationDecisionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ConsolidationDecisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dec-{}", self.0.simple())
    }
}

/// Declarative outcome decision for a KnowledgeCandidate.
/// Invariant: ConsolidationDecision is declarative; no memory mutations occur during decision derivation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ConsolidationDecision {
    /// Promote candidate to permanent long-term memory.
    PromoteToLongTerm,
    /// Merge candidate into an existing memory domain entity.
    MergeWithExisting {
        /// Target existing memory entity ID.
        existing_entity_id: DomainEntityId,
    },
    /// Reject candidate due to duplicate detection.
    RejectDuplicate,
    /// Reject candidate due to low confidence score.
    RejectLowConfidence,
    /// Flag candidate due to detected memory contradiction.
    MarkContradiction,
    /// Maintain candidate as transient/ephemeral.
    KeepEphemeral,
}

/// Association mapping a KnowledgeCandidateId to its evaluated ConsolidationDecision and ConsolidationAssessment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConsolidationOutcome {
    /// Unique outcome decision identifier.
    pub id: ConsolidationDecisionId,
    /// Target knowledge candidate ID.
    pub candidate_id: KnowledgeCandidateId,
    /// Derived consolidation decision.
    pub decision: ConsolidationDecision,
    /// Assessment justification used to reach decision.
    pub assessment: ConsolidationAssessment,
}

impl ConsolidationOutcome {
    /// Instantiates a new `ConsolidationOutcome`.
    pub fn new(
        candidate_id: KnowledgeCandidateId,
        decision: ConsolidationDecision,
        assessment: ConsolidationAssessment,
    ) -> Self {
        Self {
            id: ConsolidationDecisionId::new(),
            candidate_id,
            decision,
            assessment,
        }
    }
}

/// Immutable collection of derived consolidation outcomes for an execution run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConsolidationReport {
    /// Target execution run ID.
    pub execution_id: ExecutionId,
    /// List of consolidation outcomes.
    pub outcomes: Vec<ConsolidationOutcome>,
}

impl ConsolidationReport {
    /// Instantiates a new immutable `ConsolidationReport`.
    pub fn new(execution_id: ExecutionId, outcomes: Vec<ConsolidationOutcome>) -> Self {
        Self {
            execution_id,
            outcomes,
        }
    }
}
