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
    /// Aggregated confidence score [0.0..1.0].
    pub confidence: f64,
    /// Primary provenance tracking data.
    pub provenance: ProvenanceIR,
    /// Additive provenance chain tracking all merged observation origins without data loss.
    pub provenance_chain: Vec<ProvenanceIR>,
}

impl EntityIR {
    /// Instantiates a new EntityIR with initial provenance chain containing primary provenance.
    pub fn new(
        id: EntityId,
        canonical_name: impl Into<String>,
        kind: impl Into<String>,
        confidence: f64,
        provenance: ProvenanceIR,
    ) -> Self {
        let chain = vec![provenance.clone()];
        Self {
            id,
            canonical_name: canonical_name.into(),
            kind: kind.into(),
            aliases: Vec::new(),
            properties: BTreeMap::new(),
            confidence,
            provenance,
            provenance_chain: chain,
        }
    }

    /// Merges secondary entity into self deterministically, preserving additive provenance.
    pub fn merge_from(&mut self, secondary: EntityIR) {
        // Merge aliases
        for alias in secondary.aliases {
            if !self.aliases.contains(&alias) && alias != self.canonical_name {
                self.aliases.push(alias);
            }
        }
        if !self.aliases.contains(&secondary.canonical_name)
            && secondary.canonical_name != self.canonical_name
        {
            self.aliases.push(secondary.canonical_name);
        }

        // Merge properties additively (primary properties take precedence)
        for (k, v) in secondary.properties {
            self.properties.entry(k).or_insert(v);
        }

        // Additive provenance preservation
        self.provenance_chain.push(secondary.provenance);
        self.provenance_chain.extend(secondary.provenance_chain);

        // Bayesian confidence aggregation: 1 - (1 - c1)(1 - c2)
        let c1 = self.confidence.clamp(0.0, 1.0);
        let c2 = secondary.confidence.clamp(0.0, 1.0);
        self.confidence = (1.0 - (1.0 - c1) * (1.0 - c2)).clamp(0.0, 1.0);
    }
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
    /// Aggregated confidence score [0.0..1.0].
    pub confidence: f64,
    /// Optional start of temporal validity window in milliseconds.
    pub valid_from_ms: Option<u64>,
    /// Optional end of temporal validity window in milliseconds.
    pub valid_until_ms: Option<u64>,
    /// Whether this fact is the selected active canonical fact.
    pub is_canonical: bool,
    /// Fact ID of the canonical fact that superseded this fact, if any.
    pub superseded_by: Option<FactId>,
    /// Primary provenance tracking data.
    pub provenance: ProvenanceIR,
    /// Additive provenance chain tracking all merged evidence origins.
    pub provenance_chain: Vec<ProvenanceIR>,
}

impl FactIR {
    /// Instantiates a new FactIR with initial provenance chain containing primary provenance.
    pub fn new(
        id: FactId,
        subject_id: EntityId,
        predicate: impl Into<String>,
        object_value: impl Into<String>,
        confidence: f64,
        provenance: ProvenanceIR,
    ) -> Self {
        let chain = vec![provenance.clone()];
        Self {
            id,
            subject_id,
            predicate: predicate.into(),
            object_value: object_value.into(),
            confidence,
            valid_from_ms: None,
            valid_until_ms: None,
            is_canonical: true,
            superseded_by: None,
            provenance,
            provenance_chain: chain,
        }
    }

    /// Merges secondary fact evidence additively into self.
    pub fn merge_from(&mut self, secondary: FactIR) {
        self.provenance_chain.push(secondary.provenance);
        self.provenance_chain.extend(secondary.provenance_chain);

        let c1 = self.confidence.clamp(0.0, 1.0);
        let c2 = secondary.confidence.clamp(0.0, 1.0);
        self.confidence = (1.0 - (1.0 - c1) * (1.0 - c2)).clamp(0.0, 1.0);
    }
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
    /// Primary provenance tracking data.
    pub provenance: ProvenanceIR,
    /// Additive provenance chain tracking all merged relation origins.
    pub provenance_chain: Vec<ProvenanceIR>,
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
