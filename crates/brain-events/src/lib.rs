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

pub use commands::*;
pub use events::*;
pub use runtime_events::*;
