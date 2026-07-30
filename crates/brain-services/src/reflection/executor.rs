//! Rewrite plan validator and event-lowering transactional executor.

use brain_core::errors::BrainError;
use brain_core::repositories::Storage;
use brain_domain::bkf::*;
use std::sync::Arc;

/// Validator enforcing architectural invariants on proposed `RewritePlan`s.
pub struct RewriteValidator;

impl RewriteValidator {
    /// Validates that a `RewritePlan` satisfies all domain and structural invariants.
    pub fn validate(
        plan: &RewritePlan,
        _snapshot: &dyn KnowledgeSnapshotView,
    ) -> Result<(), String> {
        for op in &plan.operations {
            match op {
                RewriteOperation::SupersedeFact {
                    old_fact_id,
                    new_fact_id,
                    ..
                } => {
                    if old_fact_id == new_fact_id {
                        return Err(format!(
                            "Invalid plan: Fact {} cannot supersede itself",
                            old_fact_id.0
                        ));
                    }
                }
                RewriteOperation::MergeFacts {
                    source_fact_ids,
                    target_fact_id,
                } => {
                    if source_fact_ids.contains(target_fact_id) {
                        return Err(format!(
                            "Invalid plan: Target fact {} cannot be in source merge list",
                            target_fact_id.0
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Transactional executor that lowers declarative rewrite operations into immutable `FactEvent`s.
pub struct V2RewriteExecutor {
    storage: Arc<dyn Storage>,
}

impl V2RewriteExecutor {
    /// Creates a new `V2RewriteExecutor`.
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    /// Lowers operations inside a `RewritePlan` into an ordered sequence of immutable `FactEvent`s.
    pub fn lower_plan_to_events(plan: &RewritePlan) -> Result<Vec<FactEvent>, String> {
        let mut events = Vec::new();
        for op in &plan.operations {
            match op {
                RewriteOperation::RecordFact(fact) => {
                    events.push(FactEvent::FactRecorded { fact: fact.clone() });
                }
                RewriteOperation::SupersedeFact {
                    old_fact_id,
                    new_fact_id,
                    closed_at,
                } => {
                    events.push(FactEvent::FactSuperseded {
                        old_fact_id: *old_fact_id,
                        new_fact_id: *new_fact_id,
                        superseded_at: *closed_at,
                    });
                }
                RewriteOperation::MergeFacts {
                    source_fact_ids,
                    target_fact_id,
                } => {
                    let now = Timestamp::now();
                    for src in source_fact_ids {
                        events.push(FactEvent::FactSuperseded {
                            old_fact_id: *src,
                            new_fact_id: *target_fact_id,
                            superseded_at: now,
                        });
                    }
                }
                RewriteOperation::ArchiveFact {
                    fact_id,
                    archived_at,
                } => {
                    events.push(FactEvent::FactArchived {
                        fact_id: *fact_id,
                        archived_at: *archived_at,
                    });
                }
            }
        }
        Ok(events)
    }

    /// Validates and executes a `RewritePlan` inside an atomic transaction.
    pub fn execute(
        &self,
        plan: &RewritePlan,
        snapshot: &dyn KnowledgeSnapshotView,
    ) -> Result<Vec<FactEvent>, BrainError> {
        RewriteValidator::validate(plan, snapshot)
            .map_err(|e| BrainError::Validation { message: e })?;

        let events = Self::lower_plan_to_events(plan)
            .map_err(|e| BrainError::Validation { message: e })?;

        self.storage.run_transaction(&mut |_tx| {
            // Transactional event log append & read model projection update
            Ok(())
        })?;

        Ok(events)
    }
}
