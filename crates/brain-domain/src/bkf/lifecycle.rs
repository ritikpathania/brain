use serde::{Deserialize, Serialize};

/// Indicates the phase of a knowledge element's processing lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum KnowledgeLifecycle {
    /// Newly parsed or ingested from an external input, before compile verification.
    Observed,
    /// Successfully processed, normalized, and validated by the compiler pipeline.
    Compiled,
    /// Formulated as delta updates and pushed to storage engines.
    Projected,
}

/// Indicates the validity and correctness tier of the knowledge element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum KnowledgeValidity {
    /// Raw un-critiqued knowledge structure.
    Unverified,
    /// Critiqued and verified by the Reflection Engine.
    Verified,
    /// Confirmed invalid, untrustworthy, or incorrect.
    Rejected,
}

/// Indicates the version and progression state of the knowledge element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum KnowledgeVersionState {
    /// Active, current version of the semantic record.
    Current,
    /// Superseded by a newer knowledge node/relationship, but kept for audit trails.
    Deprecated,
    /// Explicitly replaced by a revised compilation pass or rewrite plan.
    Superseded,
}
