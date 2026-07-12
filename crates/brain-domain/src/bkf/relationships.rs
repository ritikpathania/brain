use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use crate::bkf::ids::BkfRelationshipId;
use crate::bkf::provenance::Provenance;
use crate::bkf::facts::BKFTargetRef;

/// Stable extension point for relationship kind classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationshipType(pub Cow<'static, str>);

impl RelationshipType {
    /// A depends on B.
    pub const DEPENDS_ON: Self = Self(Cow::Borrowed("depends_on"));
    /// A calls B.
    pub const CALLS: Self = Self(Cow::Borrowed("calls"));
    /// A imports B.
    pub const IMPORTS: Self = Self(Cow::Borrowed("imports"));
    /// A owns B.
    pub const OWNS: Self = Self(Cow::Borrowed("owns"));
    /// A contains B.
    pub const CONTAINS: Self = Self(Cow::Borrowed("contains"));
    /// A references B.
    pub const REFERENCES: Self = Self(Cow::Borrowed("references"));
    /// A extends B.
    pub const EXTENDS: Self = Self(Cow::Borrowed("extends"));
    /// A implements B.
    pub const IMPLEMENTS: Self = Self(Cow::Borrowed("implements"));
    /// A mentions B.
    pub const MENTIONS: Self = Self(Cow::Borrowed("mentions"));
    /// A generic relation type.
    pub const RELATED_TO: Self = Self(Cow::Borrowed("related_to"));

    /// Generates a custom relationship kind.
    pub fn custom(name: String) -> Self {
        Self(Cow::Owned(name))
    }

    /// Returns the string slice representation of the relationship type.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A semantic, directed relationship between any two BKF elements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    /// Relationship ID.
    pub id: BkfRelationshipId,
    /// Source reference.
    pub source: BKFTargetRef,
    /// Target reference.
    pub target: BKFTargetRef,
    /// Type/directionality classification.
    pub relationship_type: RelationshipType,
    /// Extraction confidence (0.0 to 1.0).
    pub confidence: f32,
    /// Chain of provenance origins.
    pub provenance: Vec<Provenance>,
}
