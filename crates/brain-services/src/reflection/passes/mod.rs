/// Canonicalization text normalization pass.
pub mod canonicalization;
pub use canonicalization::CanonicalizationPass;

/// Duplicate concept node detection pass.
pub mod duplicate;
pub use duplicate::DuplicateDetectionPass;
/// Duplicate fact version consolidation pass v2.
pub mod duplicate_consolidation;
pub use duplicate_consolidation::*;


/// Property contradiction detection pass.
pub mod contradiction;
pub use contradiction::{ContradictionPass, V2ContradictionPass};


/// Link suggestion (transitive inference) pass.
pub mod link_suggestion;
pub use link_suggestion::LinkSuggestionPass;

/// Synthesis cluster consolidation pass.
pub mod synthesis;
pub use synthesis::SynthesisPass;

