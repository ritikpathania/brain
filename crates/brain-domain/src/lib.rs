//! Pure domain entities, strongly-typed IDs, DTOs, and API models for the Brain relational memory engine.
//!
//! This crate contains only data structures. They are independent of storage,
//! network, and execution runtime layers.

#![deny(missing_docs)]

/// Data Transfer Objects (DTOs) for API and UI boundary isolation.
pub mod dtos;
/// Core domain entities representing graph memory, embeddings, messages, and tools.
pub mod entities;
/// Domain-specific errors for validating business invariants.
pub mod errors;
/// Domain events.
pub mod events;
/// Strongly-typed identifiers for system resources.
pub mod identifiers;
/// Pure domain services for multi-entity logic.
pub mod services;
/// Domain specifications.
pub mod specification;

pub use dtos::*;
pub use entities::*;
pub use errors::*;
pub use events::*;
pub use identifiers::*;
pub use services::*;
pub use specification::*;
pub use ulid;

/// Brain Knowledge Format (BKF) canonical representation.
pub mod bkf;
pub use bkf::*;

/// Graph relation taxonomy registry.
pub mod relations;
pub use relations::*;

/// Lexical normalization and entity canonicalization.
pub mod canonical;
pub use canonical::*;

/// Rule-driven inference engine.
pub mod inference;
pub use inference::*;

/// Conflict resolution engine for filtering relationships.
pub mod suppression;
pub use suppression::*;

/// Integrity and invariant validation passes.
pub mod validation;
pub use validation::*;

/// Graph query parameters and types.
#[allow(missing_docs)]
pub mod query;
pub use query::*;

/// Persistent relational memory models and lifecycle state.
pub mod memory;
pub use memory::*;

/// Cognitive retrieval and reasoning engine.
pub mod retrieval;
pub use retrieval::*;

/// First-class temporal domain models and abstractions.
pub mod temporal;
pub use temporal::*;

/// First-class domain consolidation policies and decision engines.
pub mod consolidation;
pub use consolidation::*;

/// First-class background jobs models and state machine.
pub mod jobs;
pub use jobs::*;

/// Knowledge Graph self-reflection and consolidation models.
pub mod reflection;
pub use reflection::*;

/// Knowledge lifecycle states.
pub mod lifecycle;
pub use lifecycle::*;

/// First-class observation models and retention tiers.
pub mod observation;
pub use observation::*;

/// Evidence and provenance containers.
pub mod evidence;
pub use evidence::*;

/// Domain-wide canonical structured value type.
pub mod value;
pub use value::*;

/// Phase 3 Projection Runtime domain models.
pub mod projection;
pub use projection::*;

/// Phase 3 Runtime Reasoning and Execution Planning domain models.
pub mod reasoning;
pub use reasoning::*;

/// Execution progress cursor helper.
pub mod cursor;
pub use cursor::*;

/// Execution state models, state machine transitions, and lightweight event facts.
pub mod execution;
pub use execution::*;

/// Pure domain reasoning planner service.
pub mod planner;
pub use planner::*;

/// Execution artifacts, first-class provenance edges, and artifact views.
pub mod artifact;
pub use artifact::*;

/// Append-only store managing evidence artifacts and provenance graph relationships.
pub mod artifact_store;
pub use artifact_store::*;

/// ArtifactDescriptor payload produced by StepExecutors.
pub mod descriptor;
pub use descriptor::*;

/// Algebraic Evidence Selection Strategy, Context, Query, and EvidenceSet.
pub mod selection;
pub use selection::*;

/// Domain-neutral ReasoningFinding and immutable ReasoningResult models.
pub mod synthesis;
pub use synthesis::*;

/// Domain models for Reasoning Reflection & Critique.
pub mod reasoning_reflection;
pub use reasoning_reflection::*;

/// Knowledge Candidate models and KnowledgeCandidateSet.
pub mod candidate;
pub use candidate::*;

/// Knowledge Graph Matching domain models.
pub mod match_domain;
pub use match_domain::*;

/// Consolidation decision domain models.
pub mod consolidation_decision;
pub use consolidation_decision::*;

/// Declarative Knowledge Evolution models.
pub mod evolution;
pub use evolution::*;

/// Immutable ReasoningSession aggregate and stage transition state machine.
pub mod session;
pub use session::*;
