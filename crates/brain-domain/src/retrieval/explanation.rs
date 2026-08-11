//! Extensible explanation reasons detailing why evidence was retrieved.

use serde::{Deserialize, Serialize};

/// Structured reason describing why a piece of evidence matched a query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvidenceReason {
    /// Exact or fuzzy FTS5 keyword match.
    KeywordMatch {
        /// Matched query term string.
        term: String,
    },
    /// Cosine/dot-product semantic embedding vector similarity.
    SemanticSimilarity {
        /// Cosine similarity score in range [0.0, 1.0].
        score: f32,
    },
    /// Evidence discovered via relational graph traversal.
    GraphTraversal {
        /// Hop distance depth from seed node.
        depth: u32,
        /// Edge relation classification type.
        edge_type: String,
    },
    /// Evidence boosted due to temporal recency.
    RecentMemory,
    /// Evidence explicitly pinned into context by the user.
    ManualPin,
    /// Extensible plugin provider custom match explanation.
    Plugin {
        /// Plugin provider identifier string.
        provider: String,
        /// Explanation details.
        reason: String,
    },
}

/// Structured explanation bundle associated with an evidence item.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct StructuredRetrievalExplanation {
    /// List of contributing match reasons.
    pub reasons: Vec<EvidenceReason>,
    /// Final overall candidate rank index (1-indexed).
    pub final_rank: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_reason_serialization() {
        let reason = EvidenceReason::KeywordMatch {
            term: "SQLite".to_string(),
        };
        let json = serde_json::to_string(&reason).unwrap();
        let deserialized: EvidenceReason = serde_json::from_str(&json).unwrap();
        assert_eq!(reason, deserialized);
    }
}
