//! In-memory entity statistics state.

use crate::bkf::*;
use crate::projection::entity_statistics::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ActiveFactMetadata {
    entity_id: KnowledgeEntityId,
    predicate_id: PredicateId,
    confidence: f64,
}

/// Materialized operational summary state mapping entity IDs to EntityStatistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityStatisticsState {
    entities: HashMap<KnowledgeEntityId, EntityStatistics>,
    active_facts: HashMap<FactVersionId, ActiveFactMetadata>,
    predicate_refcounts: HashMap<KnowledgeEntityId, HashMap<PredicateId, usize>>,
}

impl EntityStatisticsState {
    /// Returns statistical summary for entity if present.
    pub fn get(&self, entity: &KnowledgeEntityId) -> Option<&EntityStatistics> {
        self.entities.get(entity)
    }

    /// Returns total number of tracked entities.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Returns true if no entities are tracked.
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Internal helper processing FactRecorded event. Idempotent on duplicate `fact.id`.
    pub fn record_fact(&mut self, fact: &FactVersion, assertion: &SemanticAssertion) {
        if self.active_facts.contains_key(&fact.id) {
            return;
        }

        let entity_id = assertion.subject.clone();
        let predicate_id = assertion.predicate.clone();
        let confidence_val = fact.confidence.value() as f64;
        let recorded_at = fact.temporal.valid_from;

        self.active_facts.insert(
            fact.id.clone(),
            ActiveFactMetadata {
                entity_id: entity_id.clone(),
                predicate_id: predicate_id.clone(),
                confidence: confidence_val,
            },
        );

        let pred_counts = self.predicate_refcounts.entry(entity_id.clone()).or_default();
        let refcount = pred_counts.entry(predicate_id).or_default();
        let is_new_predicate = *refcount == 0;
        *refcount += 1;

        let stats = self.entities.entry(entity_id.clone()).or_insert_with(|| EntityStatistics {
            entity_id,
            total_fact_versions: 0,
            superseded_facts_count: 0,
            archived_facts_count: 0,
            active_facts_count: 0,
            unique_predicates_count: 0,
            first_observed_at: recorded_at,
            last_updated_at: recorded_at,
            active_confidence_sum: 0.0,
        });

        stats.total_fact_versions += 1;
        stats.active_facts_count += 1;
        stats.active_confidence_sum += confidence_val;
        if is_new_predicate {
            stats.unique_predicates_count += 1;
        }
        stats.last_updated_at = recorded_at;
    }

    /// Internal helper processing FactSuperseded event.
    pub fn supersede_fact(&mut self, old_fact_id: &FactVersionId, superseded_at: Timestamp) {
        if let Some(meta) = self.active_facts.remove(old_fact_id) {
            self.remove_active_metadata(meta, superseded_at, FactLifecycle::Superseded);
        }
    }

    /// Internal helper processing FactArchived event.
    pub fn archive_fact(&mut self, archived_fact_id: &FactVersionId, archived_at: Timestamp) {
        if let Some(meta) = self.active_facts.remove(archived_fact_id) {
            self.remove_active_metadata(meta, archived_at, FactLifecycle::Archived);
        }
    }

    fn remove_active_metadata(&mut self, meta: ActiveFactMetadata, event_time: Timestamp, reason: FactLifecycle) {
        if let Some(stats) = self.entities.get_mut(&meta.entity_id) {
            stats.active_facts_count = stats.active_facts_count.saturating_sub(1);
            stats.active_confidence_sum = (stats.active_confidence_sum - meta.confidence).max(0.0);
            stats.last_updated_at = event_time;

            match reason {
                FactLifecycle::Superseded => stats.superseded_facts_count += 1,
                FactLifecycle::Archived => stats.archived_facts_count += 1,
                _ => {}
            }

            let mut remove_entity_pred_map = false;
            if let Some(pred_counts) = self.predicate_refcounts.get_mut(&meta.entity_id) {
                if let Some(cnt) = pred_counts.get_mut(&meta.predicate_id) {
                    *cnt = cnt.saturating_sub(1);
                    if *cnt == 0 {
                        pred_counts.remove(&meta.predicate_id);
                        stats.unique_predicates_count = stats.unique_predicates_count.saturating_sub(1);
                    }
                }
                if pred_counts.is_empty() {
                    remove_entity_pred_map = true;
                }
            }
            if remove_entity_pred_map {
                self.predicate_refcounts.remove(&meta.entity_id);
            }
        }
    }
}
