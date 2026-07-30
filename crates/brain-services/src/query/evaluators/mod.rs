//! Modular stateless evaluators for Phase 5 Query Facade.

/// Stateless evaluator for compound hybrid queries.
pub mod hybrid;
/// Stateless evaluator for node neighborhood graph traversal.
pub mod neighborhood;
/// Stateless evaluator for lexical search queries.
pub mod search;
/// Stateless evaluator for point-in-time entity state lookups.
pub mod temporal;

pub use hybrid::HybridEvaluator;
pub use neighborhood::NeighborhoodEvaluator;
pub use search::SearchEvaluator;
pub use temporal::TemporalEvaluator;
