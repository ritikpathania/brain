//! Configuration precedence, schema models, and loader engines for the Brain engine.
//!
//! This crate implements versioned configuration schemas, custom provider types,
//! dynamic ConfigSource layers, and precedence resolvers.

#![deny(missing_docs)]

/// Settings schemas (both full and partial configurations).
pub mod schema;

/// Decoupled ConfigSource loaders and resolving logic.
pub mod loader;

/// Semantic configuration validator routines.
pub mod validation;

/// Config migration interfaces.
pub mod migration;

/// Immutable Copy-on-Write Versioned Configuration Engine (Phase G.5).
pub mod immutable_config;

pub use immutable_config::*;
pub use loader::*;
pub use migration::*;
pub use schema::*;
pub use validation::*;
