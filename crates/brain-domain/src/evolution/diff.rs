//! Semantic diffs explaining graph transformations in human-readable terms.

use serde::{Deserialize, Serialize};

/// High-level semantic graph transformation change item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticChange {
    /// Merged duplicate concepts into canonical node.
    MergedConcepts {
        /// Primary canonical label.
        canonical: String,
        /// Merged concept label.
        merged: String,
        /// Explanation rationale.
        reason: String,
    },
    /// Promoted concept node to first-class canonical entity.
    PromotedEntity {
        /// Entity label.
        label: String,
        /// Explanation rationale.
        reason: String,
    },
    /// Pruned invalid or contradicted relationship edge.
    PrunedRelationship {
        /// Source node label.
        source: String,
        /// Target node label.
        target: String,
        /// Relation type label.
        relation: String,
        /// Explanation rationale.
        reason: String,
    },
}

/// Semantic diff collection attached to an evolution proposal.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SemanticDiff {
    /// High-level semantic change items.
    pub changes: Vec<SemanticChange>,
}

impl SemanticDiff {
    /// Creates a new empty SemanticDiff.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a semantic change item to the diff.
    pub fn add_change(&mut self, change: SemanticChange) {
        self.changes.push(change);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_diff_aggregation() {
        let mut diff = SemanticDiff::new();
        diff.add_change(SemanticChange::MergedConcepts {
            canonical: "SQLite Full Text Search".to_string(),
            merged: "SQLite FTS".to_string(),
            reason: "Duplicate concept overlap".to_string(),
        });

        assert_eq!(diff.changes.len(), 1);
    }
}
