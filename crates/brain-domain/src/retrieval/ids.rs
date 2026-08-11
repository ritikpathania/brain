pub use crate::identifiers::{DocumentId, SourceId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Opaque newtype identifier for an incoming retrieval query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QueryId(pub Uuid);

impl Default for QueryId {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryId {
    /// Generates a new random QueryId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for QueryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "query-{}", self.0)
    }
}

/// Opaque newtype identifier for an individual evidence item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceId(pub Uuid);

impl Default for EvidenceId {
    fn default() -> Self {
        Self::new()
    }
}

impl EvidenceId {
    /// Generates a new random EvidenceId.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for EvidenceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evidence-{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opaque_ids_uniqueness_and_serialization() {
        let q1 = QueryId::new();
        let q2 = QueryId::new();
        assert_ne!(q1, q2);

        let json = serde_json::to_string(&q1).unwrap();
        let deserialized: QueryId = serde_json::from_str(&json).unwrap();
        assert_eq!(q1, deserialized);
    }
}
