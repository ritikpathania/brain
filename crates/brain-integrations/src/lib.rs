//! Integration models and contracts for the Brain memory engine.
//!
//! This crate contains only data structures and traits. They are independent of storage,
//! network, and execution runtime layers.

#![deny(missing_docs)]

pub mod envelope;
pub mod events;
pub mod identity;
pub mod replay;
pub mod traits;

pub use envelope::*;
pub use events::*;
pub use identity::*;
pub use replay::*;
pub use traits::*;

/// Serializes any serializable structure to its canonical JSON representation,
/// ensuring that all object keys are sorted lexicographically regardless of their
/// declaration order in source code.
pub fn to_canonical_json<T: serde::Serialize>(val: &T) -> Result<String, serde_json::Error> {
    let value = serde_json::to_value(val)?;
    serde_json::to_string(&value)
}
