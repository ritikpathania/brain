use crate::dtos::NodeDTO;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Data Transfer Object representing a relationship edge in the inspector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipDTO {
    /// Stringified identifier of the target node.
    pub target_id: String,
    /// Label/name of the target node.
    pub target_label: String,
    /// NodeType of the target node.
    pub target_type: String,
    /// The relation type name.
    pub relation: String,
    /// Direction: "incoming" or "outgoing".
    pub direction: String,
    /// Confidence or strength score of this relationship.
    pub weight: f64,
}

/// Data Transfer Object representing the provenance history of an entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenanceDTO {
    /// Origin classification: e.g. "Ingested", "Inferred", "UserAuthored".
    pub source: String,
    /// Physical location reference: e.g. file path, code repository URL, etc.
    pub location: String,
    /// Ingestion timestamp.
    pub timestamp: u64,
    /// Key-value metadata annotations.
    pub extra_info: HashMap<String, String>,
}

/// Data Transfer Object explaining the retrieval scoring and boosts for this entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetrievalExplanationDTO {
    /// Integrated score used for ranking.
    pub score: f64,
    /// Unweighted retrieval score.
    pub raw_score: f64,
    /// Matched terms and keyword boosts.
    pub keyword_boosts: Vec<String>,
    /// Distance metric in vector space.
    pub semantic_distance: f64,
    /// Human-readable explanation of why it was matched.
    pub reasoning: String,
}

/// Data Transfer Object representing a log entry for recent user or background daemon actions on the entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActivityLogEntry {
    /// Unix timestamp of the activity.
    pub timestamp: u64,
    /// Action description: e.g. "Updated", "Accessed", "Consolidated".
    pub action: String,
    /// Additional context details.
    pub details: String,
}

/// Data Transfer Object representing the complete read-only knowledge context for an entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InspectorModel {
    /// Base entity parameters.
    pub entity: NodeDTO,
    /// System metadata annotations.
    pub metadata: HashMap<String, String>,
    /// Directed adjacency connection records.
    pub relationships: Vec<RelationshipDTO>,
    /// Provenance metadata tracking where the knowledge came from.
    pub provenance: ProvenanceDTO,
    /// Dedicated section explaining the retrieval parameters.
    pub retrieval_explanation: Option<RetrievalExplanationDTO>,
    /// Chronological list of updates or usage logs.
    pub recent_activity: Vec<ActivityLogEntry>,
}
