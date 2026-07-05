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
pub use retrieval::{pipeline, source, cache, calibration, active_weights, feature_extractor};
pub use session::SessionServiceImpl;
pub use stub::{StubRetrievalService, StubSessionService};

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
