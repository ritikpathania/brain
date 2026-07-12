use serde::{Deserialize, Serialize};
use crate::bkf::ids::*;
use crate::bkf::provenance::Provenance;

/// Target reference to any BKF element.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", content = "id", rename_all = "lowercase")]
pub enum BKFTargetRef {
    /// Reference to a BKFDocument.
    Document(BkfDocumentId),
    /// Reference to a Section.
    Section(BkfSectionId),
    /// Reference to a Block.
    Block(BkfBlockId),
    /// Reference to an Entity.
    Entity(BkfEntityId),
    /// Reference to a Relationship.
    Relationship(BkfRelationshipId),
    /// Reference to a Fact.
    Fact(BkfFactId),
    /// Reference to a Citation.
    Citation(BkfCitationId),
    /// Reference to an Attachment.
    Attachment(BkfAttachmentId),
}

/// Rich RDF-like factual object representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum FactObject {
    /// Object is another entity.
    Entity(BkfEntityId),
    /// Object is a string literal.
    Literal(String),
    /// Object is a numeric literal.
    Number(f64),
    /// Object is a boolean literal.
    Boolean(bool),
    /// Object is a date/timestamp.
    Date(u64),
}

/// A standalone RDF-like factual assertion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fact {
    /// Fact ID.
    pub id: BkfFactId,
    /// The subject of the statement.
    pub subject: BKFTargetRef,
    /// The predicate describing the assertion.
    pub predicate: String,
    /// The target object of the statement.
    pub object: FactObject,
    /// Extraction confidence (0.0 to 1.0).
    pub confidence: f32,
    /// Chain of provenance origins.
    pub provenance: Vec<Provenance>,
}
