//! Data models for Entity Statistics Projection.

use crate::bkf::*;
use serde::{Deserialize, Serialize};

/// Materialized operational summary metrics for a single domain entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityStatistics {
    /// Target entity identifier.
    pub entity_id: KnowledgeEntityId,
    /// Total count of fact versions recorded for this entity.
    pub total_fact_versions: u64,
    /// Count of superseded fact versions.
    pub superseded_facts_count: u64,
    /// Count of archived fact versions.
    pub archived_facts_count: u64,
    /// Count of currently active fact versions.
    pub active_facts_count: usize,
    /// Count of unique predicates associated with currently active facts.
    pub unique_predicates_count: usize,
    /// Timestamp when this entity was first observed in the fact stream.
    pub first_observed_at: Timestamp,
    /// Timestamp when this entity was last updated.
    pub last_updated_at: Timestamp,
    /// Internal sum of active fact confidence scores for exact running mean calculation.
    pub active_confidence_sum: f64,
}

impl EntityStatistics {
    /// Computes the running average confidence across active facts in O(1) time.
    pub fn average_confidence(&self) -> f32 {
        if self.active_facts_count > 0 {
            (self.active_confidence_sum / self.active_facts_count as f64) as f32
        } else {
            0.0
        }
    }
}
