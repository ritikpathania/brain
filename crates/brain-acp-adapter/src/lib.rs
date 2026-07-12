//! ACP protocol adapter mapping standard JSON-RPC 2.0 requests to application capabilities.

#![deny(missing_docs)]

/// JSON-RPC 2.0 protocol structures.
pub mod protocol;
/// Symmetric mapping for events and errors.
pub mod mapping;
/// Capability-based registry.
pub mod registry;
/// Main ACP Adapter.
pub mod adapter;

pub use adapter::AcpAdapter;
pub use protocol::{AcpRequest, AcpResponse, AcpNotification, AcpError};
pub use registry::CapabilityExposureRegistry;
pub use brain_adapter_core::{Capability, ErasedCapability};
pub use mapping::{AcpErrorMapper, AcpEventMapper};
