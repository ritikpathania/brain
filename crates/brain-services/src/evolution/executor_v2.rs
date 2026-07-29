//! Transactional `EvolutionExecutorV2` executing compiled `KnowledgeEvolutionPlan` artifacts (Phase 6 Milestone 6.2).

use crate::evolution::models_v2::{
    EvolutionActionKind, EvolutionExecutionReport, EvolutionMutationSet, KnowledgeEvolutionPlan,
    ProposalExecutionState, ProposalId,
};
use crate::evolution::validator_v2::PlanValidatorV2;
use std::time::Instant;
use uuid::Uuid;

/// Transactional execution engine translating validated `KnowledgeEvolutionPlan` items into intent mutation sets.
#[derive(Debug, Clone, Default)]
pub struct EvolutionExecutorV2 {
    validator: PlanValidatorV2,
}

impl EvolutionExecutorV2 {
    /// Instantiates a new `EvolutionExecutorV2`.
    pub fn new() -> Self {
        Self {
            validator: PlanValidatorV2::new(),
        }
    }

    /// Transactionally executes a `KnowledgeEvolutionPlan`.
    pub fn execute(
        &self,
        plan: &KnowledgeEvolutionPlan,
    ) -> Result<(EvolutionMutationSet, EvolutionExecutionReport), String> {
        let start = Instant::now();

        // 1. Validate plan safety before execution
        let val_report = self.validator.validate(plan);
        if !val_report.is_valid {
            let err_msg = val_report
                .errors
                .iter()
                .map(|e| e.message.clone())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(format!(
                "Execution aborted due to validation failure: {}",
                err_msg
            ));
        }

        // 2. Derive topological execution ordering from canonical ProposalGraph
        let order = plan.dependency_graph.topological_sort()?;

        let mut mutation_set = EvolutionMutationSet::default();
        let mut applied_proposals = Vec::new();
        let failed_proposals = Vec::new();
        let mut skipped_proposals = Vec::new();

        let proposal_map: std::collections::HashMap<ProposalId, _> =
            plan.proposals.iter().map(|p| (p.id, p)).collect();

        // State Machine: Pending -> Applied -> Committed
        for prop_id in order {
            if let Some(prop) = proposal_map.get(&prop_id) {
                match &prop.action {
                    EvolutionActionKind::MergeEntities {
                        target_id,
                        source_id,
                    } => {
                        mutation_set
                            .entity_merges
                            .push((target_id.clone(), source_id.clone()));
                    }
                    EvolutionActionKind::SupercedeFact {
                        target_entity_id,
                        stale_fact_id,
                    } => {
                        mutation_set
                            .fact_supercessions
                            .push((target_entity_id.clone(), stale_fact_id.clone()));
                    }
                    EvolutionActionKind::UpdateConfidence {
                        target_entity_id,
                        new_confidence,
                    } => {
                        mutation_set
                            .confidence_updates
                            .push((target_entity_id.clone(), *new_confidence));
                    }
                }
                applied_proposals.push(prop_id);
            } else {
                skipped_proposals.push(prop_id);
            }
        }

        let final_state = ProposalExecutionState::Committed;
        let duration = start.elapsed().as_millis() as u64;

        let report = EvolutionExecutionReport {
            report_id: Uuid::new_v4(),
            plan_id: plan.plan_id,
            final_state,
            applied_proposals,
            skipped_proposals,
            failed_proposals,
            rollback_occurred: false,
            execution_duration_ms: duration,
            timestamp_ms: plan.timestamp_ms,
        };

        Ok((mutation_set, report))
    }
}
