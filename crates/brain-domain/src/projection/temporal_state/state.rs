//! In-memory temporal state.

use crate::bkf::*;
use crate::projection::temporal_state::models::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Materialized temporal state over entity timelines and validity intervals.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemporalState {
    records: HashMap<TemporalFactId, TemporalRecord>,
    entity_timelines: HashMap<KnowledgeEntityId, Vec<TemporalFactId>>,
    active: HashMap<KnowledgeEntityId, Vec<TemporalFactId>>,
}

impl TemporalState {
    /// Returns all currently active fact IDs for an entity.
    pub fn active_facts(&self, entity: &KnowledgeEntityId) -> &[TemporalFactId] {
        self.active.get(entity).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns the full chronological timeline of fact IDs for an entity.
    pub fn timeline(&self, entity: &KnowledgeEntityId) -> &[TemporalFactId] {
        self.entity_timelines.get(entity).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns a reference to a specific temporal record by FactId.
    pub fn record(&self, id: &TemporalFactId) -> Option<&TemporalRecord> {
        self.records.get(id)
    }

    /// Returns true if the fact ID is currently active.
    pub fn is_active(&self, id: &TemporalFactId) -> bool {
        self.record(id).map_or(false, TemporalRecord::is_active)
    }

    /// Returns references to facts for an entity that were valid at `at_timestamp`.
    pub fn facts_at(&self, entity: &KnowledgeEntityId, at_timestamp: Timestamp) -> Vec<&TemporalRecord> {
        let mut result = Vec::new();
        if let Some(fact_ids) = self.entity_timelines.get(entity) {
            for fact_id in fact_ids {
                if let Some(rec) = self.records.get(fact_id) {
                    if rec.valid_from <= at_timestamp {
                        match rec.valid_until {
                            None => result.push(rec),
                            Some(until) if until > at_timestamp => result.push(rec),
                            _ => {}
                        }
                    }
                }
            }
        }
        result
    }

    /// Inserts a new temporal record atomically. Idempotent on duplicate `record.id`.
    pub fn insert_record(&mut self, record: TemporalRecord) {
        if self.records.contains_key(&record.id) {
            return;
        }
        let fact_id = record.id.clone();
        let entity = record.entity_id.clone();
        let is_active = record.is_active();

        self.records.insert(fact_id.clone(), record);
        self.entity_timelines.entry(entity.clone()).or_default().push(fact_id.clone());
        if is_active {
            self.active.entry(entity).or_default().push(fact_id);
        }
    }

    /// Closes an open validity interval at `closed_at` timestamp with updated lifecycle.
    pub fn close_interval(&mut self, id: &TemporalFactId, closed_at: Timestamp, new_lifecycle: FactLifecycle) {
        if let Some(record) = self.records.get_mut(id) {
            if record.valid_until.is_some() {
                return;
            }
            debug_assert!(closed_at >= record.valid_from, "closed_at must be monotonic with valid_from");
            record.valid_until = Some(closed_at);
            record.lifecycle = new_lifecycle;

            let entity = record.entity_id.clone();
            if let Some(active_list) = self.active.get_mut(&entity) {
                active_list.retain(|active_id| active_id != id);
                if active_list.is_empty() {
                    self.active.remove(&entity);
                }
            }
        }
    }
}
