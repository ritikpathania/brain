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
