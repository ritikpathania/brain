//! A2A protocol adapter mapping JSON-RPC 2.0 messages between autonomous agents to Brain capabilities.

#![deny(missing_docs)]

/// Main A2A Adapter.
pub mod adapter;
/// Symmetric mapping for events and errors.
pub mod mapping;
/// JSON-RPC 2.0 protocol structures.
pub mod protocol;
/// Capability-based registry.
pub mod registry;

pub use adapter::A2aAdapter;
pub use brain_adapter_core::{Capability, ErasedCapability};
pub use mapping::{A2aErrorMapper, A2aEventMapper};
pub use protocol::{A2aError, A2aNotification, A2aRequest, A2aResponse};
pub use registry::CapabilityRegistry;
