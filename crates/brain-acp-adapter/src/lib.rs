//! ACP protocol adapter mapping standard JSON-RPC 2.0 requests to application capabilities.

#![deny(missing_docs)]

/// Main ACP Adapter.
pub mod adapter;
/// Symmetric mapping for events and errors.
pub mod mapping;
/// JSON-RPC 2.0 protocol structures.
pub mod protocol;
/// Capability-based registry.
pub mod registry;

pub use adapter::AcpAdapter;
pub use brain_adapter_core::{Capability, ErasedCapability};
pub use mapping::{AcpErrorMapper, AcpEventMapper};
pub use protocol::{AcpError, AcpNotification, AcpRequest, AcpResponse};
pub use registry::CapabilityExposureRegistry;
