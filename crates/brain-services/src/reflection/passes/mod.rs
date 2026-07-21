/// Duplicate concept node detection pass.
pub mod duplicate;
pub use duplicate::DuplicateDetectionPass;

/// Property contradiction detection pass.
pub mod contradiction;
pub use contradiction::ContradictionPass;

/// Link suggestion (transitive inference) pass.
pub mod link_suggestion;
pub use link_suggestion::LinkSuggestionPass;

/// Synthesis cluster consolidation pass.
pub mod synthesis;
pub use synthesis::SynthesisPass;
