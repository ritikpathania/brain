//! Command and Event schemas, envelopes, and message bus contracts for the Brain engine.
//!
//! This crate defines query objects, Command patterns for synchronous operations,
//! and Domain Events for asynchronous event bus publication and subscription.

#![deny(missing_docs)]

/// Query objects, synchronous commands, and dispatcher traits.
pub mod commands;

/// Asynchronous domain events, metadata envelopes, and pub/sub bus traits.
pub mod events;

/// In-process event vocabulary, pub/sub bus, and subscriber isolation contracts.
pub mod runtime_events;

/// Reflection runtime event stream, payload vocabulary, and pub/sub bus.
pub mod reflection_events;

/// Persistent event storage interface and in-memory implementation.
pub mod event_store;

pub use commands::*;
pub use event_store::*;
pub use events::*;
pub use reflection_events::*;
pub use runtime_events::*;
