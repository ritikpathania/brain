//! Domain models for knowledge evidence containers.

use crate::observation::RetentionTier;
use serde::{Deserialize, Serialize};

/// Provenance and evidence container for facts and entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeEvidence {
    /// Tiered observation history.
    pub retention: RetentionTier,
    /// Source reliability coefficient (0.0 to 1.0).
    pub source_reliability: f32,
}

impl Default for KnowledgeEvidence {
    fn default() -> Self {
        Self {
            retention: RetentionTier::default(),
            source_reliability: 1.0,
        }
    }
}
