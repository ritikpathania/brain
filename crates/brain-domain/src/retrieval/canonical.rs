//! Canonical transport-independent CanonicalRetrievalResult domain aggregate.

use super::confidence::ConfidenceAssessment;
use super::evidence::EvidenceItem;
use super::ids::QueryId;
use super::timing::RetrievalTiming;
use serde::{Deserialize, Serialize};

/// Strongly-typed metadata payload accompanying a retrieval result.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RetrievalMetadata {
    /// LLM model name if applicable.
    pub model: Option<String>,
    /// SQLite FTS5 / Vector index snapshot version.
    pub index_version: Option<String>,
    /// Detected query natural language.
    pub query_language: Option<String>,
}

/// Domain reference to a graph relationship edge.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationshipReference {
    /// Source node identifier.
    pub source_node: String,
    /// Target node identifier.
    pub target_node: String,
    /// Edge relationship classification.
    pub relation: String,
}

/// Canonical, transport-independent retrieval result aggregate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalRetrievalResult {
    /// Opaque query identifier.
    pub query_id: QueryId,
    /// Generated natural language answer string.
    pub answer: String,
    /// Canonical list of evidence items supporting the answer.
    pub evidence: Vec<EvidenceItem>,
    /// Related knowledge graph relationship references.
    pub relationships: Vec<RelationshipReference>,
    /// Structured confidence assessment.
    pub confidence: ConfidenceAssessment,
    /// Detailed timing breakdown across retrieval stages.
    pub timing: RetrievalTiming,
    /// Structured metadata.
    pub metadata: RetrievalMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_retrieval_result_serialization_roundtrip() {
        let result = CanonicalRetrievalResult {
            query_id: QueryId::new(),
            answer: "Brain uses hybrid search.".to_string(),
            evidence: vec![],
            relationships: vec![],
            confidence: ConfidenceAssessment::new(0.95),
            timing: RetrievalTiming::default(),
            metadata: RetrievalMetadata::default(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: CanonicalRetrievalResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
