//! Reusable Query Processing Primitives for Phase 5 Query Evaluators.

/// Confidence threshold filtering primitive.
pub mod confidence;
/// Deterministic sorting primitive with tie-breaking.
pub mod ordering;
/// Safe offset/limit candidate slicing primitive.
pub mod pagination;
/// Read-model query filter DTO specifications.
pub mod specs;
/// Half-open temporal validity interval filtering primitive.
pub mod temporal;

pub use confidence::filter_by_confidence;
pub use ordering::sort_matches;
pub use pagination::paginate_matches;
pub use specs::*;
pub use temporal::is_valid_at;
