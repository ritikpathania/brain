//! Filter expressions for query evaluation.

use crate::bkf::*;
use serde::{Deserialize, Serialize};

/// Filter expressions over entities and facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryFilter {
    /// Filter by entity kind string.
    EntityKind(String),
    /// Filter by minimum confidence score.
    MinConfidence(Confidence),
    /// Filter by active fact temporal validity.
    ActiveOnly,
}
