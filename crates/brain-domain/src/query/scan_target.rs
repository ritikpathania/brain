//! Typed domain scan target enum for snapshot sources.

use serde::{Deserialize, Serialize};

/// Typed targets for snapshot scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanTarget {
    /// Active fact versions.
    ActiveFacts,
    /// Historical fact versions.
    HistoricalFacts,
    /// Entities.
    Entities,
    /// Semantic assertions.
    Assertions,
    /// Predicates.
    Predicates,
}
