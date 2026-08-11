//! Runtime schema versioning and compatibility checks: RuntimeSchemaVersion.

use std::fmt;

/// Strongly-typed runtime schema version for contract stability and replay compatibility checks.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct RuntimeSchemaVersion {
    /// Major breaking version number.
    pub major: u16,
    /// Minor backwards-compatible version number.
    pub minor: u16,
}

impl RuntimeSchemaVersion {
    /// Current schema version constant (v1.0).
    pub const CURRENT: Self = Self { major: 1, minor: 0 };

    /// Instantiates a new `RuntimeSchemaVersion`.
    pub fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Evaluates compatibility against a target schema version.
    /// Invariant: Major versions must match exactly; minor version of target must be >= self.minor.
    pub fn is_compatible_with(&self, target: &Self) -> bool {
        self.major == target.major && target.minor >= self.minor
    }
}

impl Default for RuntimeSchemaVersion {
    fn default() -> Self {
        Self::CURRENT
    }
}

impl fmt::Display for RuntimeSchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}", self.major, self.minor)
    }
}
