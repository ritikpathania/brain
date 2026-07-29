//! Domain models for individual observation records and retention tiers.

use crate::identifiers::SourceId;
use serde::{Deserialize, Serialize};

/// A single atomic observation record event for a fact or entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationRecord {
    /// Identifier of the observation source (e.g. file, user, tool).
    pub source_id: SourceId,
    /// Unix timestamp when the observation occurred.
    pub timestamp: u64,
    /// Confidence score of this specific observation (0.0 to 1.0).
    pub confidence: f32,
    /// Extractor or tool metadata that generated the observation.
    pub extractor_info: String,
}

/// Aggregated summary statistics for high-volume historical observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationSummary {
    /// Total count of observations recorded.
    pub total_observations: u32,
    /// First observed timestamp.
    pub first_observed_at: u64,
    /// Most recent observed timestamp.
    pub last_observed_at: u64,
    /// Most recent reinforcement timestamp.
    pub last_reinforced_at: u64,
    /// Average confidence score across all observations.
    pub average_confidence: f32,
}

/// Tiered storage representation for observation history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetentionTier {
    /// Detailed recent individual observation records.
    Recent(Vec<ObservationRecord>),
    /// Compacted statistical summary for historical observations.
    Aggregated(ObservationSummary),
    /// Archived historical record state.
    Archived,
}

impl Default for RetentionTier {
    fn default() -> Self {
        Self::Recent(Vec::new())
    }
}
