//! Application boundary facade orchestrating DTO boundaries, dispatching capability methods,
//! execution contexts, and semantic error categorization.

#![deny(missing_docs)]

/// Main Application facade entrypoint.
pub mod application;
/// Execution contexts and progress sinks.
pub mod context;
/// Semantic application error classifications.
pub mod errors;
/// Application data transfer objects.
pub mod dto;

pub use application::{BrainApplication, IngestionResponse};
pub use context::{
    ApplicationEvent, ApplicationEventSink, ApplicationRequestId, ExecutionContext, ProgressEvent,
};
pub use errors::ApplicationError;
