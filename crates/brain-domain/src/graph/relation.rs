//! Semantic classification of graph relationships.

use serde::{Deserialize, Serialize};

/// High-level semantic relationship classification between graph nodes.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum RelationKind {
    /// Generic citation or document reference link.
    #[default]
    References,
    /// Explicit architectural or code dependency.
    DependsOn,
    /// Code implementation of an abstract contract or trait.
    Implements,
    /// Structural container or parent-child hierarchy.
    Contains,
    /// Textual mention of an entity or concept.
    Mentions,
    /// Semantic vector similarity alignment.
    SimilarTo,
    /// Derived or transformed knowledge projection.
    DerivedFrom,
}

impl RelationKind {
    /// Returns abbreviated display label for rendering on edges without layout overflow.
    pub fn abbreviation(&self) -> &'static str {
        match self {
            RelationKind::References => "ref",
            RelationKind::DependsOn => "dep",
            RelationKind::Implements => "impl",
            RelationKind::Contains => "has",
            RelationKind::Mentions => "mention",
            RelationKind::SimilarTo => "similar",
            RelationKind::DerivedFrom => "derived",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_kind_abbreviations() {
        assert_eq!(RelationKind::References.abbreviation(), "ref");
        assert_eq!(RelationKind::DependsOn.abbreviation(), "dep");
    }
}
