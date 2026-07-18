//! MCP protocol adapter mapping standard JSON-RPC requests to application capabilities.

#![deny(missing_docs)]

/// Main MCP Adapter.
pub mod adapter;
/// Symmetric mapping for events and errors.
pub mod mapping;
/// JSON-RPC 2.0 protocol structures.
pub mod protocol;
/// Capability-based registry.
pub mod registry;

pub use adapter::McpAdapter;
pub use brain_adapter_core::{Capability, ErasedCapability};
pub use mapping::{McpErrorMapper, McpEventMapper};
pub use protocol::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
pub use registry::CapabilityRegistry;
