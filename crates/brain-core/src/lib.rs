//! Trait specifications, Repository interfaces, and custom Error types for the Brain engine.
//!
//! This crate contains trait definitions that define contracts between the storage,
//! execution runtime, and service layers, along with the unified system error hierarchy.

#![deny(missing_docs)]

extern crate tracing;

/// Unified custom system error definitions.
pub mod errors;

/// Storage repository CRUD traits.
pub mod repositories;

/// High-level core business services.
pub mod services;

/// LLM agent execution traits.
pub mod agents;

/// Plugin and system tool extensibility traits.
pub mod extensibility;

/// Core retrieval abstractions and traits.
pub mod retrieval;

/// Canonical stream event models.
pub mod events;

/// Semantic views/projections contracts.
pub mod projection;

/// Ingestion and canonicalization evolution contracts.
pub mod evolution;

/// Reflection engine contracts for post-canonicalization entity examination.
pub mod reflection;

/// Graph/relationship model DTOs.
pub mod graph;

/// Phase 3 Runtime Reasoning execution engine contracts and runner.
pub mod reasoning;

pub use agents::*;
pub use errors::*;
pub use events::*;
pub use evolution::*;
pub use extensibility::*;
pub use graph::*;
pub use projection::*;
pub use reasoning::*;
pub use reflection::*;
pub use repositories::*;
pub use retrieval::*;
pub use services::*;
