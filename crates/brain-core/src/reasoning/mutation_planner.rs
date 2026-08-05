//! MemoryMutationPlannerService translating ConsolidationReports into StewardshipMemoryMutationPlans and compiled StewardshipMemoryMutationBatches.

use brain_domain::{
    ConsolidationDecision, ConsolidationReport, DomainEntityId, DomainError, MemoryMutationId,
    StewardshipMemoryMutation, StewardshipMemoryMutationBatch, StewardshipMemoryMutationPlan,
};

/// Pure domain service translating declarative ConsolidationReports into StewardshipMemoryMutationPlans.
///
/// Invariants:
/// - The planner translates consolidation decisions into mutations; it never inspects storage or database engines.
/// - Memory mutation planning converts consolidation decisions into executable operations but does not perform memory mutations directly.
/// - Given identical ConsolidationReports, produces identical StewardshipMemoryMutationPlans and compiled StewardshipMemoryMutationBatches (determinism).
#[derive(Debug, Clone, Default)]
pub struct MemoryMutationPlannerService;

impl MemoryMutationPlannerService {
    /// Instantiates a new `MemoryMutationPlannerService`.
    pub fn new() -> Self {
        Self
    }

    /// Derives a `StewardshipMemoryMutationPlan` and compiles it into an executable `StewardshipMemoryMutationBatch`.
    pub fn plan_mutations(
        &self,
        report: &ConsolidationReport,
    ) -> Result<StewardshipMemoryMutationBatch, DomainError> {
        let mut proposed_mutations = Vec::new();

        for outcome in &report.outcomes {
            match outcome.decision {
                ConsolidationDecision::PromoteToLongTerm => {
                    let target_entity = DomainEntityId::new();
                    proposed_mutations.push(StewardshipMemoryMutation::CreateEntity {
                        id: MemoryMutationId::new(),
                        target_id: target_entity,
                        candidate_id: outcome.candidate_id,
                        payload: brain_domain::StructuredValue::String(
                            "Promoted entity".to_string(),
                        ),
                    });
                }
                ConsolidationDecision::MergeWithExisting { existing_entity_id } => {
                    proposed_mutations.push(StewardshipMemoryMutation::MergeEntity {
                        id: MemoryMutationId::new(),
                        target_id: existing_entity_id,
                        candidate_id: outcome.candidate_id,
                        payload: brain_domain::StructuredValue::String(
                            "Merged payload".to_string(),
                        ),
                    });
                }
                ConsolidationDecision::MarkContradiction => {
                    // Mark contradiction does not create entity; handled as non-mutation flag
                }
                ConsolidationDecision::RejectDuplicate
                | ConsolidationDecision::RejectLowConfidence
                | ConsolidationDecision::KeepEphemeral => {}
            }
        }

        let plan = StewardshipMemoryMutationPlan::new(report.execution_id, proposed_mutations);
        StewardshipMemoryMutationBatch::compile(plan)
    }
}
