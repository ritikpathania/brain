//! Runtime observability for the Brain engine.
//!
//! This crate provides structured operation tracing, correlation tracking, and
//! timeline recording. It depends only on `brain-core` \u2014 it has zero dependency
//! on `brain-services` or `brain-storage`.
//!
//! # Components
//!
//! - [`timeline`]: `OperationSpan` and `OperationTimeline` for append-only span recording.
//! - [`correlation`]: `CorrelationIndex` mapping `CorrelationId` to accumulated spans.
//! - [`subscriber`]: `ObservabilitySubscriber`, a `std::thread`-based blocking consumer
//!   that ingests `TaskProgress` events from a `std::sync::mpsc::Receiver`.

/// Append-only operation timeline types.
pub mod timeline;

/// Cross-correlation span index.
pub mod correlation;

/// Background thread subscriber feeding the correlation index.
pub mod subscriber;

pub use timeline::{OperationSpan, OperationTimeline};
pub use correlation::CorrelationIndex;
pub use subscriber::ObservabilitySubscriber;
