//! ConsolidationPolicy pure strategy trait and DefaultConsolidationPolicy implementation.

use brain_domain::{
    CandidateConfidence, ConsolidationAssessment, ConsolidationDecision, DuplicateProbability,
    KnowledgeCandidate,
};

/// Pure strategy interface defining decision rules for knowledge candidates given an assessment.
/// Invariants:
/// - A ConsolidationPolicy is a pure strategy object containing zero mutable execution state between invocations.
/// - Decision boundary remains solely inside ConsolidationPolicy.
pub trait ConsolidationPolicy: Send + Sync + std::fmt::Debug {
    /// Derives a declarative `ConsolidationDecision` from a candidate and its assessment.
    fn decide(
        &self,
        candidate: &KnowledgeCandidate,
        assessment: &ConsolidationAssessment,
    ) -> ConsolidationDecision;
}

/// Default stateless consolidation policy.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultConsolidationPolicy;

impl DefaultConsolidationPolicy {
    /// Instantiates a new `DefaultConsolidationPolicy`.
    pub fn new() -> Self {
        Self
    }
}

impl ConsolidationPolicy for DefaultConsolidationPolicy {
    fn decide(
        &self,
        _candidate: &KnowledgeCandidate,
        assessment: &ConsolidationAssessment,
    ) -> ConsolidationDecision {
        if assessment.metrics().duplicate_probability >= DuplicateProbability::HIGH {
            ConsolidationDecision::RejectDuplicate
        } else if assessment.confidence() >= CandidateConfidence::HIGH {
            ConsolidationDecision::PromoteToLongTerm
        } else if assessment.confidence() >= CandidateConfidence::MEDIUM {
            ConsolidationDecision::KeepEphemeral
        } else {
            ConsolidationDecision::RejectLowConfidence
        }
    }
}
