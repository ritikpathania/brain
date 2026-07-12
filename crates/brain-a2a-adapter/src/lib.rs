//! A2A protocol adapter mapping JSON-RPC 2.0 messages between autonomous agents to Brain capabilities.

#![deny(missing_docs)]

/// JSON-RPC 2.0 protocol structures.
pub mod protocol;
/// Symmetric mapping for events and errors.
pub mod mapping;
/// Capability-based registry.
pub mod registry;
/// Main A2A Adapter.
pub mod adapter;

pub use adapter::A2aAdapter;
pub use protocol::{A2aRequest, A2aResponse, A2aNotification, A2aError};
pub use registry::CapabilityRegistry;
pub use mapping::{A2aErrorMapper, A2aEventMapper};
pub use brain_adapter_core::{Capability, ErasedCapability};
