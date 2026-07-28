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
pub use retrieval::{
    active_weights, cache, calibration, eval_harness, evaluator, experiment, feature_extractor,
    model_resolver, pipeline, relationship_expander, source,
};
pub use session::SessionServiceImpl;
pub use stub::{StubDomainEventPublisher, StubRetrievalService, StubSessionService};

/// Unified application runtime lifecycle, builder, and composition locator.
pub mod runtime;
pub use runtime::{
    ApplicationRuntime, HealthCheck, HealthReport, HealthStatus, RuntimeBuilder, RuntimeObserver,
    RuntimeState, StartupReport,
};

/// Distributed runtime architecture and worker registry.
pub mod distributed;

/// Worker runtime and execution engine.
pub mod worker;

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

/// Knowledge reasoning and inference engine (Phase 5 Milestone 5.2).
pub mod reasoning;

/// Graph projections and utilities.
pub mod graph;

/// Unified stateful projections engine.
pub mod projections;
pub use projections::{
    JobProjectionReducer, ProjectionId, ProjectionNotificationBus, ProjectionRunner,
    ReducerRegistry, SearchProjectionReducer, SessionProjectionReducer, StateReducer,
};

/// Background automation orchestration rules and scheduler.
pub mod automation;
/// Reference implementations for Sprint 1 event dispatch.
pub mod event_dispatcher;
/// Long-term knowledge governance policies and planner.
pub mod evolution;
/// Reference implementations for Sprint 1 knowledge evolution validator and canonicalizer.
pub mod evolution_service;
/// Reference implementations for Sprint 1 memory list projection structures.
pub mod memory_list_projection;
/// Reference implementations for Sprint 1 projection coordinator manager.
pub mod projection_manager;
/// Reference implementation for Sprint 3 reflection engine (emit-only).
pub mod reflection_engine;
/// Reference implementations for Sprint 2 SQLite persistent evolution.
pub mod sqlite_evolution;
/// Reference implementations for Sprint 2 SQLite persistent projections.
pub mod sqlite_projection;
/// SQLite production implementation of reflection engine.
pub mod sqlite_reflection;

pub use event_dispatcher::InMemoryEventDispatcher;
pub use evolution_service::{InMemoryCanonicalizer, StandardIngestionValidator};
pub use memory_list_projection::{
    MemoryListProjection, MemoryListProjector, MemoryListQuery, SearchProjectionQuery,
    SearchProjectionResult, SearchProjector,
};
pub use projection_manager::ProjectionManager;
pub use reflection_engine::InMemoryReflectionEngine;
pub use sqlite_evolution::SqliteCanonicalizer;
pub use sqlite_projection::{SqliteProjectionManager, SqliteProjector};
pub use sqlite_reflection::SqliteReflectionEngine;

/// Unified composition root and lifecycle owner for the Brain Relational Engine.
pub mod brain_runtime;
pub use brain_runtime::{
    BrainRuntime, CapabilityDescriptor, CapabilityRegistry, CapabilityState, RuntimeDiagnostics,
    RuntimeFailure, RuntimeHealth, RuntimeMetrics, RuntimeStatus, ShutdownSummary,
};

/// Self-reflection and memory consolidation engine.
pub mod reflection;

/// Background orchestrator and runtime task automation.
pub mod orchestrator;

/// Rule-based health evaluation module.
pub mod health_evaluator;
pub use health_evaluator::{DerivedRuntimeHealth, HealthEvaluator};

/// Knowledge Processing Pipeline (KPP) Knowledge Compiler.
pub mod compiler;
