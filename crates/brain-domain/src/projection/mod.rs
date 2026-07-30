//! Phase 3 Projection Runtime domain models, traits, and value objects.

/// Projection identifier and version models.
pub mod id;
/// Event stream sequence watermark.
pub mod watermark;

pub use id::*;
pub use watermark::*;
