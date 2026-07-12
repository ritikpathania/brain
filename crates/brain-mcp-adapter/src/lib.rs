//! MCP protocol adapter mapping standard JSON-RPC requests to application capabilities.

#![deny(missing_docs)]

/// JSON-RPC 2.0 protocol structures.
pub mod protocol;
/// Symmetric mapping for events and errors.
pub mod mapping;
/// Capability-based registry.
pub mod registry;
/// Main MCP Adapter.
pub mod adapter;

pub use adapter::McpAdapter;
pub use protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcNotification, JsonRpcError};
pub use registry::CapabilityRegistry;
pub use brain_adapter_core::{Capability, ErasedCapability};
pub use mapping::{McpErrorMapper, McpEventMapper};
