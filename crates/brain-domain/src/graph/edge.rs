//! Domain representation for directed knowledge graph edges.

use super::relation::RelationKind;
use crate::identifiers::{EdgeId, NodeId, RelationId};
use serde::{Deserialize, Serialize};

/// Directed edge aggregate connecting two nodes in the knowledge graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeAggregate {
    /// Unique edge identifier key.
    pub id: EdgeId,
    /// Source node identifier.
    pub source: NodeId,
    /// Target node identifier.
    pub target: NodeId,
    /// Strongly-typed relation string identifier.
    pub relation: RelationId,
    /// Semantic classification of the relationship.
    pub kind: RelationKind,
    /// Relationship weight / confidence score in range [0.0, 1.0].
    pub weight: f32,
}

impl EdgeAggregate {
    /// Creates a new EdgeAggregate.
    pub fn new(source: NodeId, target: NodeId, relation: RelationId, kind: RelationKind) -> Self {
        let id = EdgeId::new(source, target, relation.clone());
        Self {
            id,
            source,
            target,
            relation,
            kind,
            weight: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_aggregate_construction() {
        let src = NodeId::new();
        let tgt = NodeId::new();
        let rel = RelationId::new("DEPENDS_ON");
        let edge = EdgeAggregate::new(src, tgt, rel, RelationKind::DependsOn);

        assert_eq!(edge.source, src);
        assert_eq!(edge.target, tgt);
        assert_eq!(edge.kind, RelationKind::DependsOn);
    }
}
