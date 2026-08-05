//! SynthesizerService producing structured ReasoningResult objects using EvidenceSelectors, EvidenceResolvers, and SynthesisPolicies.

use crate::reasoning::evidence_resolver::EvidenceResolver;
use crate::reasoning::evidence_selector::EvidenceSelector;
use crate::reasoning::synthesis_policy::SynthesisPolicy;
use brain_domain::{
    DomainError, EvidenceQuery, ExecutionPlan, ExecutionState, ReasoningResult, SelectionContext,
    SelectionStrategy,
};

/// Service orchestrating response synthesis across selected evidence sets.
/// Invariant: SynthesizerService does not perform graph traversal itself; it delegates to EvidenceSelector and EvidenceResolver.
#[derive(Debug, Clone, Default)]
pub struct SynthesizerService {
    resolver: EvidenceResolver,
}

impl SynthesizerService {
    /// Instantiates a new `SynthesizerService`.
    pub fn new() -> Self {
        Self {
            resolver: EvidenceResolver::new(),
        }
    }

    /// Synthesizes an end-to-end immutable `ReasoningResult` from plan execution state.
    pub fn synthesize(
        &self,
        execution_id: brain_domain::ExecutionId,
        plan: &ExecutionPlan,
        state: &ExecutionState,
        selector: &EvidenceSelector,
        policy: &dyn SynthesisPolicy,
    ) -> Result<ReasoningResult, DomainError> {
        let context = SelectionContext::new(execution_id);
        let query = EvidenceQuery::new(SelectionStrategy::All, context);
        let evidence_set = selector.select(&state.artifact_store, &query);

        let views = self.resolver.resolve(&evidence_set, &state.artifact_store);
        let findings = policy.interpret(&evidence_set, &views);

        Ok(ReasoningResult::new(
            execution_id,
            plan.user_query.clone(),
            findings,
            evidence_set,
        ))
    }
}
