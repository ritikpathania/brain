//! Pure domain entities, strongly-typed IDs, DTOs, and API models for the Brain relational memory engine.
//!
//! This crate contains only data structures. They are independent of storage,
//! network, and execution runtime layers.

#![deny(missing_docs)]

/// Data Transfer Objects (DTOs) for API and UI boundary isolation.
pub mod dtos;
/// Core domain entities representing graph memory, embeddings, messages, and tools.
pub mod entities;
/// Strongly-typed identifiers for system resources.
pub mod identifiers;

pub use dtos::*;
pub use entities::*;
pub use identifiers::*;
