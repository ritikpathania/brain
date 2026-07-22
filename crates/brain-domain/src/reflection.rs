use crate::{NodeId, RelationKind};

/// Unique identifier for reflection passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ReflectionPassId {
    /// Duplicate entity detection pass.
    DuplicateDetection,
    /// Property contradiction detection pass.
    Contradiction,
    /// Relationship link suggestion pass.
    LinkSuggestion,
    /// Triad closure synthesis pass.
    Synthesis,
}

impl std::fmt::Display for ReflectionPassId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateDetection => write!(f, "duplicate_detection"),
            Self::Contradiction => write!(f, "contradiction"),
            Self::LinkSuggestion => write!(f, "link_suggestion"),
            Self::Synthesis => write!(f, "synthesis"),
        }
    }
}

/// Category classification for reflection findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FindingKind {
    /// Duplicate entity finding.
    Duplicate,
    /// Property contradiction finding.
    Contradiction,
    /// Transitive link suggestion finding.
    LinkSuggestion,
}

impl std::fmt::Display for FindingKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate => write!(f, "duplicate"),
            Self::Contradiction => write!(f, "contradiction"),
            Self::LinkSuggestion => write!(f, "link_suggestion"),
        }
    }
}

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

impl ReflectionFinding {
    /// Returns the kind of finding.
    pub fn kind(&self) -> FindingKind {
        match self {
            Self::DuplicateFound { .. } => FindingKind::Duplicate,
            Self::ContradictionFound { .. } => FindingKind::Contradiction,
            Self::LinkSuggested { .. } => FindingKind::LinkSuggestion,
        }
    }

    /// Returns the confidence score associated with the finding evidence.
    pub fn confidence(&self) -> f64 {
        match self {
            Self::DuplicateFound { evidence, .. } => evidence.confidence,
            Self::ContradictionFound { evidence, .. } => evidence.confidence,
            Self::LinkSuggested { evidence, .. } => evidence.confidence,
        }
    }
}

/// Candidate action produced by the planner prior to policy evaluation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ReflectionRecommendation {
    /// Originating pass ID.
    pub pass_id: ReflectionPassId,
    /// Kind of finding that generated this recommendation.
    pub finding_kind: FindingKind,
    /// Confidence score between 0.0 and 1.0.
    pub confidence: f64,
    /// Primary target node IDs involved in the recommendation.
    pub target_ids: Vec<NodeId>,
    /// Narrative rationale explaining why the action is recommended.
    pub rationale: String,
    /// Proposed command.
    pub command: ReflectionDomainCommand,
}

/// Policy governing confidence thresholds for automated command generation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ReflectionPolicy {
    /// Minimum confidence threshold to auto-merge concept nodes.
    pub auto_merge_confidence_threshold: f64,
    /// Minimum confidence threshold to auto-create inferred relations.
    pub auto_link_confidence_threshold: f64,
}

impl Default for ReflectionPolicy {
    fn default() -> Self {
        Self {
            auto_merge_confidence_threshold: 0.90,
            auto_link_confidence_threshold: 0.80,
        }
    }
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
    /// Candidate recommendations generated by the planner.
    pub recommendations: Vec<ReflectionRecommendation>,
    /// Commands resolved and queued for transaction execution following policy evaluation.
    pub commands: Vec<ReflectionDomainCommand>,
    /// Count of total findings evaluated.
    pub findings_processed: usize,
    /// Logs of findings that were skipped (e.g. low confidence).
    pub skipped_findings: Vec<(ReflectionFinding, String)>,
}
