//! Persistence-agnostic Intermediate Representation (Knowledge IR) for the Knowledge Compiler.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable semantic identifier for a canonical entity node in Knowledge IR.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub String);

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Stable semantic identifier for a compiled fact in Knowledge IR.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FactId(pub String);

impl std::fmt::Display for FactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Provenance metadata detailing origin evidence for a compiled knowledge artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceIR {
    /// Ingestion event sequence number or source origin string.
    pub source_origin: String,
    /// Event IDs or document IDs associated with this piece of knowledge.
    pub evidence_ids: Vec<String>,
    /// Confidence score aggregated across source observations [0.0..1.0].
    pub confidence: f64,
    /// Creation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Intermediate Representation of a canonical entity node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityIR {
    /// Stable semantic entity ID.
    pub id: EntityId,
    /// Canonical primary label/name.
    pub canonical_name: String,
    /// Entity classification ("person", "concept", "organization", "project", etc.).
    pub kind: String,
    /// Discovered alias strings mapping to this canonical entity.
    pub aliases: Vec<String>,
    /// Key-value property map.
    pub properties: BTreeMap<String, String>,
    /// Aggregated confidence score.
    pub confidence: f64,
    /// Provenance tracking data.
    pub provenance: ProvenanceIR,
}

/// Intermediate Representation of a subject-predicate-object fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactIR {
    /// Unique fact identifier.
    pub id: FactId,
    /// Subject entity ID.
    pub subject_id: EntityId,
    /// Predicate relation string (e.g. "works_at", "depends_on", "authored_by").
    pub predicate: String,
    /// Object value or target entity ID.
    pub object_value: String,
    /// Aggregated confidence score.
    pub confidence: f64,
    /// Provenance tracking data.
    pub provenance: ProvenanceIR,
}

/// Intermediate Representation of a directed relation edge between entities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationIR {
    /// Source entity ID.
    pub source_id: EntityId,
    /// Target entity ID.
    pub target_id: EntityId,
    /// Relation category.
    pub relation_kind: String,
    /// Relationship strength weight [0.0..1.0].
    pub weight: f64,
    /// Provenance tracking data.
    pub provenance: ProvenanceIR,
}

/// Complete in-memory Intermediate Representation (Knowledge IR) payload.
///
/// **Invariants**:
/// - **Persistence Agnostic**: Stores only pure semantic structures; no SQLite connection handles or transport DTOs.
/// - **Deterministic Ordering**: Entities and facts are maintained in sorted `BTreeMap` structures keyed by stable `EntityId` and `FactId`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeIR {
    /// Canonical entity nodes indexed by EntityId.
    pub entities: BTreeMap<EntityId, EntityIR>,
    /// Subject-predicate-object facts indexed by FactId.
    pub facts: BTreeMap<FactId, FactIR>,
    /// Directed edge relations.
    pub relations: Vec<RelationIR>,
}

impl KnowledgeIR {
    /// Instantiates a new empty Knowledge IR payload.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a canonical entity node.
    pub fn insert_entity(&mut self, entity: EntityIR) {
        self.entities.insert(entity.id.clone(), entity);
    }

    /// Adds or replaces a compiled fact.
    pub fn insert_fact(&mut self, fact: FactIR) {
        self.facts.insert(fact.id.clone(), fact);
    }

    /// Adds a directed edge relation.
    pub fn add_relation(&mut self, relation: RelationIR) {
        self.relations.push(relation);
    }
}
