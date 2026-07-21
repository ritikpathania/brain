use crate::{NodeId, RelationKind};

/// Structured evidence supporting a reflection finding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct FindingEvidence {
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f64,
    /// Cosine similarity from semantic embedding comparisons, if applicable.
    pub semantic_similarity: Option<f64>,
    /// Edit distance (e.g. Levenshtein) from name/label matching, if applicable.
    pub edit_distance: Option<usize>,
    /// Quantified ratio of shared references or overlaps, if applicable.
    pub overlap_ratio: Option<f64>,
    /// Narrative description of why the finding was raised.
    pub details: String,
}

/// Potential consolidation or correction opportunity identified during reflection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ReflectionFinding {
    /// Two concept nodes representing duplicate entities.
    DuplicateFound {
        /// First duplicate node.
        node_a: NodeId,
        /// Second duplicate node.
        node_b: NodeId,
        /// Diagnostic evidence.
        evidence: FindingEvidence,
    },
    /// A contradiction or schema violation within a node's properties.
    ContradictionFound {
        /// Target node.
        node_id: NodeId,
        /// Property key containing the contradiction.
        property_key: String,
        /// Conflicting values asserted on the property.
        values: Vec<serde_json::Value>,
        /// Diagnostic evidence.
        evidence: FindingEvidence,
    },
    /// A new relationship inferred through structural transitivity or semantic similarity.
    LinkSuggested {
        /// Source concept node ID.
        source_id: NodeId,
        /// Target concept node ID.
        target_id: NodeId,
        /// Predicted relationship type.
        relation_kind: RelationKind,
        /// Diagnostic evidence.
        evidence: FindingEvidence,
    },
}

/// Commands describing intent to mutate the graph after reflection planning.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ReflectionDomainCommand {
    /// Merge duplicate nodes into a canonical model.
    MergeConcepts {
        /// The surviving canonical concept node ID.
        canonical_id: NodeId,
        /// The redundant node ID merged into the canonical node.
        duplicate_id: NodeId,
    },
    /// Create an inferred transitive relationship.
    CreateInferredRelation {
        /// Source concept node ID.
        source_id: NodeId,
        /// Target concept node ID.
        target_id: NodeId,
        /// Predicted relationship type.
        relation_kind: RelationKind,
        /// Confidence level of the relation.
        confidence: f64,
    },
}

/// Events documenting facts of completed reflection modifications.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ReflectionDomainEvent {
    /// Concept nodes merged due to duplicate detection.
    ConceptMerged {
        /// The surviving canonical concept node ID.
        canonical_id: NodeId,
        /// The redundant node ID merged into the canonical node.
        merged_id: NodeId,
        /// Metadata detailing the matching details and source conversations.
        provenance: String,
    },
    /// A new relationship inferred through structural transitivity.
    RelationInferred {
        /// Source concept node ID.
        source_id: NodeId,
        /// Target concept node ID.
        target_id: NodeId,
        /// Relationship type.
        relation_kind: RelationKind,
    },
}

/// An aggregated decision plan produced by the planner.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ReflectionPlan {
    /// Commands resolved and queued for transaction execution.
    pub commands: Vec<ReflectionDomainCommand>,
    /// Count of total findings evaluated.
    pub findings_processed: usize,
    /// Logs of findings that were skipped (e.g. low confidence).
    pub skipped_findings: Vec<(ReflectionFinding, String)>,
}
