//! Concrete business services coordinating configuration, cache contexts, and storage.

#![deny(missing_docs)]

/// Data Transfer Object (DTO) mapping functions.
pub mod mapper;
/// Core memory retrieval services, DTO mapper, and orchestration pipeline.
pub mod retrieval;
pub use retrieval::ranking;
mod session;
mod stub;

pub use retrieval::RetrievalServiceImpl;
pub use retrieval::{pipeline, source, cache, calibration, active_weights, feature_extractor, evaluator, experiment, model_resolver, eval_harness};
pub use session::SessionServiceImpl;
pub use stub::{StubRetrievalService, StubSessionService, StubDomainEventPublisher};

/// Unified application runtime lifecycle, builder, and composition locator.
pub mod runtime;
pub use runtime::{
    ApplicationRuntime, HealthCheck, HealthReport, HealthStatus, RuntimeBuilder, RuntimeObserver,
    RuntimeState, StartupReport,
};

/// Agent execution pipeline and orchestration loop.
pub mod agent;

/// Conversation and memory life cycle management.
pub mod conversation;

/// Memory consolidation services.
pub mod consolidation;
pub use consolidation::MemoryConsolidationService;

/// Background jobs scheduling and execution engine.
pub mod jobs;
pub use jobs::*;

/// Query services and DTO layer.
pub mod query;

/// Unified stateful projections engine.
pub mod projections;
pub use projections::{
    StateReducer, ProjectionRunner, ReducerRegistry, JobProjectionReducer, SessionProjectionReducer, SearchProjectionReducer,
    ProjectionId, ProjectionNotificationBus
};

/// Reference implementations for Sprint 1 event dispatch.
pub mod event_dispatcher;
/// Reference implementations for Sprint 1 knowledge evolution validator and canonicalizer.
pub mod evolution_service;
/// Reference implementations for Sprint 1 memory list projection structures.
pub mod memory_list_projection;
/// Reference implementations for Sprint 1 projection coordinator manager.
pub mod projection_manager;
/// Reference implementations for Sprint 2 SQLite persistent evolution.
pub mod sqlite_evolution;
/// Reference implementations for Sprint 2 SQLite persistent projections.
pub mod sqlite_projection;
/// Reference implementation for Sprint 3 reflection engine (emit-only).
pub mod reflection_engine;
/// SQLite production implementation of reflection engine.
pub mod sqlite_reflection;

pub use event_dispatcher::InMemoryEventDispatcher;
pub use evolution_service::{StandardIngestionValidator, InMemoryCanonicalizer};
pub use memory_list_projection::{MemoryListQuery, MemoryListProjection, MemoryListProjector};
pub use projection_manager::ProjectionManager;
pub use sqlite_evolution::SqliteCanonicalizer;
pub use sqlite_projection::{SqliteProjector, SqliteProjectionManager};
pub use reflection_engine::InMemoryReflectionEngine;
pub use sqlite_reflection::SqliteReflectionEngine;
