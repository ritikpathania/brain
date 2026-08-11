//! Reflection and Memory Stewardship domain model.
//!
//! This module defines presentation-agnostic domain models for observing,
//! recommending, and resolving contradictions, staleness, duplicates, and incompleteness.
//!
//! **Domain Invariants**:
//! - The domain is presentation-agnostic. No ratatui or UI dependencies allowed.
//! - Findings (observations) are strictly separated from Resolutions (actions).

/// Reflection analysis engine.
pub mod engine;
/// Stewardship finding observations.
pub mod finding;
/// Action recommendations.
pub mod recommendation;
/// Stewardship report aggregate.
pub mod report;
/// Applied action resolutions.
pub mod resolution;

/// Legacy reflection pass and findings models.
pub mod legacy;

pub use engine::{KnowledgeFactInput, ReflectionEngine};
pub use finding::{FindingId, FindingKind, StewardshipFinding};
pub use legacy::{
    FindingEvidence, LegacyFindingKind, ReflectionDomainCommand, ReflectionDomainEvent,
    ReflectionFinding, ReflectionPassId, ReflectionPlan, ReflectionPolicy,
    ReflectionRecommendation,
};
pub use recommendation::{RecommendationId, RecommendationKind, StewardshipRecommendation};
pub use report::StewardshipReport;
pub use resolution::{
    AutoResolutionPolicy, ResolutionId, ResolutionStatus, ResolutionStrategy, StewardshipResolution,
};
