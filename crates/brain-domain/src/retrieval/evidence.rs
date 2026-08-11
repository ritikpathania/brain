use super::explanation::StructuredRetrievalExplanation;
use super::ids::{DocumentId, EvidenceId, SourceId};
pub use crate::bkf::retrieval::RetrievalWeight;
use serde::{Deserialize, Serialize};

/// Transport-independent EvidenceItem containing opaque IDs and structured explanation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Opaque evidence identifier.
    pub id: EvidenceId,
    /// Referenced document identifier.
    pub document: DocumentId,
    /// Referenced source asset identifier.
    pub source: SourceId,
    /// Text excerpt snippet representing the matched evidence.
    pub excerpt: String,
    /// Optional line range boundaries (start_line, end_line).
    pub line_range: Option<(usize, usize)>,
    /// Overall candidate score.
    pub score: f32,
    /// Strategic importance classification.
    pub weight: RetrievalWeight,
    /// Structured explanation of match reasons.
    pub explanation: StructuredRetrievalExplanation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_item_serialization_roundtrip() {
        let item = EvidenceItem {
            id: EvidenceId::new(),
            document: DocumentId::new(),
            source: SourceId("src/lib.rs".to_string()),
            excerpt: "Hybrid search combines vectors and keyword search.".to_string(),
            line_range: Some((10, 25)),
            score: 0.94,
            weight: RetrievalWeight::Critical,
            explanation: StructuredRetrievalExplanation::default(),
        };

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: EvidenceItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }
}
