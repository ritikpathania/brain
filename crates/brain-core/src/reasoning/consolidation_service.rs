//! ConsolidationService orchestrating ConsolidationReports from KnowledgeCandidateSets using pure ConsolidationPolicies.

use crate::reasoning::consolidation_policy::ConsolidationPolicy;
use crate::reasoning::match_assessment_service::MatchAssessmentService;
use brain_domain::{
    ConsolidationOutcome, ConsolidationReport, DomainError, ExecutionId, KnowledgeCandidateSet,
};

/// Pure orchestration service generating ConsolidationReport aggregates.
/// Invariant: ConsolidationService performs orchestration only; assessment and decision heuristics live inside policies.
#[derive(Debug, Clone, Default)]
pub struct ConsolidationService;

impl ConsolidationService {
    /// Instantiates a new `ConsolidationService`.
    pub fn new() -> Self {
        Self
    }

    /// Evaluates a `KnowledgeCandidateSet` using a `ConsolidationPolicy` to produce an immutable `ConsolidationReport`.
    pub fn consolidate(
        &self,
        execution_id: ExecutionId,
        candidates: &KnowledgeCandidateSet,
        policy: &dyn ConsolidationPolicy,
        assessment_service: &MatchAssessmentService,
        assessment_policy: &dyn crate::reasoning::match_assessment_policy::MatchAssessmentPolicy,
    ) -> Result<ConsolidationReport, DomainError> {
        let mut outcomes = Vec::new();

        for candidate in candidates.iter() {
            let assessment = assessment_service.assess(candidate, None, assessment_policy);
            let decision = policy.decide(candidate, &assessment);
            outcomes.push(ConsolidationOutcome::new(
                candidate.id,
                decision,
                assessment,
            ));
        }

        Ok(ConsolidationReport::new(execution_id, outcomes))
    }
}
