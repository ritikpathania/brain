//! Generic adapter core providing abstract capability contracts, registries, and dynamic type-erased dispatch.

#![deny(missing_docs)]

/// Generic capability definition modules.
pub mod capability;
/// Generic capability registry modules.
pub mod registry;

pub use capability::{Capability, ErasedCapability};
pub use registry::CapabilityRegistry;
