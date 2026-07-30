//! Phase 3 Projection Runtime domain models, traits, and value objects.

/// Projection identifier and version models.
pub mod id;
/// Event stream sequence watermark.
pub mod watermark;
/// Immutable projection checkpoint value object.
pub mod checkpoint;
/// Typed projection error hierarchy.
pub mod errors;
/// Pure domain projection reducer contract.
pub mod reducer;

pub use checkpoint::*;
pub use errors::*;
pub use id::*;
pub use reducer::*;
pub use watermark::*;
