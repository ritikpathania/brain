//! Knowledge lifecycle state domain representations.

use serde::{Deserialize, Serialize};

/// The lifecycle state of a domain fact or entity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum KnowledgeState {
    /// Initial state when a fact/entity is first observed.
    #[default]
    Observed,
    /// Verified by external source or system validation.
    Verified,
    /// Repeatedly observed and strengthened across sessions.
    Reinforced,
    /// Weakened due to elapsed time or lack of reinforcement.
    Weak,
    /// Marked as outdated or superseded.
    Deprecated,
    /// Retained only for historical audit, hidden from default queries.
    Archived,
}
