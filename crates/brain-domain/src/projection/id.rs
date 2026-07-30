//! Strongly typed projection identifier and version value objects.

use serde::{Deserialize, Serialize};

/// Unique projection identifier string wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectionId(pub String);

impl ProjectionId {
    /// Creates a new ProjectionId.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns string reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Logical schema/code version of projection logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProjectionVersion(pub u32);
