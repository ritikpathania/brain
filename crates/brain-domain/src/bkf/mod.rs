//! Brain Knowledge Format (BKF) Module.
//!
//! BKF defines the canonical internal representation of all knowledge inside Brain,
//! serving as the common interchange format before storage, indexing, embedding,
//! and retrieval.
//!
//! # Design Invariants
//!
//! * **Canonical Representation**: BKF is the single canonical knowledge representation.
//! * **No Storage Leakage**: BKF never stores implementation-specific storage details.
//! * **No Retrieval Leakage**: BKF never stores retrieval-specific state.
//! * **No Provider/LLM Leakage**: BKF never stores LLM-specific state.
//! * **Immutability**: BKF is immutable after construction (read-only getters, no setters).
//! * **Type Safety**: All references are strongly typed.
//! * **Validity**: Every BKF document is structurally valid by construction.
//! * **Index Projections**: Derived artifacts (embeddings, indexes, caches) are projections,
//!   not part of the canonical model.
//! * **Deterministic Serialization**: Two semantically identical BKF documents must serialize
//!   to identical byte representations (crucial for hashing, deduplication, caching, and signatures).
//! * **Runtime State Isolation**: Retrieval and execution layers must never mutate or decorate BKF
//!   documents with dynamic runtime values (e.g. search scores, context weights). State must be managed separately.

/// Import, export, and normalization traits.
pub mod adapters;
/// Structural content block types.
pub mod blocks;
/// Incremental builder for BKF documents.
pub mod builder;
/// Document capability advertisements.
pub mod capabilities;
/// Knowledge Compiler structures and pass contracts.
mod compiler;
/// Main immutable document structure.
pub mod document;
/// Extracted semantic entities.
pub mod entities;
/// Validation and structural errors.
pub mod errors;
/// RDF-like factual assertions.
pub mod facts;
/// Strongly-typed identifiers.
pub mod ids;
/// Multi-stage intermediate representations.
mod ir;
/// Knowledge element lifecycle states.
mod lifecycle;
/// Document metadata and attributes.
pub mod metadata;
/// Observation IR input representations.
mod observation_ir;
/// Semantics-preserving mechanical optimizer passes.
mod optimizer;
/// Knowledge lineage and origin tracking.
pub mod provenance;
/// Index linkages, citations, and attachments.
pub mod references;
/// Type-safe semantic relationships.
pub mod relationships;
/// User-facing context retrieval models.
pub mod retrieval;

/// Incremental storage and formatting projections.
mod projection;
/// Offline critique reflection engine and planner.
mod reflection;

pub use adapters::*;
pub use blocks::*;
pub use builder::*;
pub use capabilities::*;
pub use compiler::*;
pub use document::*;
pub use entities::*;
pub use errors::*;
pub use facts::*;
pub use ids::*;
pub use ir::*;
pub use lifecycle::*;
pub use metadata::*;
pub use observation_ir::*;
pub use optimizer::*;
pub use projection::*;
pub use provenance::*;
pub use references::*;
pub use reflection::*;
pub use relationships::*;
pub use retrieval::*;
