//! Evolution actions representing individual graph mutations.

use crate::identifiers::NodeId;
use serde::{Deserialize, Serialize};

/// Classification category of an evolution action.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum EvolutionActionKind {
    /// Consolidate duplicate node vertices into a canonical node.
    #[default]
    MergeNodes,
    /// Prune invalid, expired, or contradicted relationship edge.
    PruneEdge,
    /// Upgrade concept node into a canonical entity node.
    PromoteEntity,
    /// Disambiguate overloaded concept node into distinct entities.
    SplitConcept,
    /// Update temporal validity range for superseded facts.
    UpdateTemporalBoundary,
}

/// Opaque newtype identifier for an evolution action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ActionId(pub uuid::Uuid);

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionId {
    /// Generates a new random ActionId.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "act-{}", self.0)
    }
}

/// Individual graph mutation action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionAction {
    /// Unique action identifier.
    pub id: ActionId,
    /// Action classification category.
    pub kind: EvolutionActionKind,
    /// Target node identifier.
    pub target_id: NodeId,
    /// Optional secondary target node identifier.
    pub secondary_id: Option<NodeId>,
    /// Rationale explaining why this action is necessary.
    pub rationale: String,
}

impl EvolutionAction {
    /// Creates a new EvolutionAction.
    pub fn new(
        kind: EvolutionActionKind,
        target_id: NodeId,
        secondary_id: Option<NodeId>,
        rationale: impl Into<String>,
    ) -> Self {
        Self {
            id: ActionId::new(),
            kind,
            target_id,
            secondary_id,
            rationale: rationale.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evolution_action_construction() {
        let node_a = NodeId::new();
        let node_b = NodeId::new();
        let action = EvolutionAction::new(
            EvolutionActionKind::MergeNodes,
            node_a,
            Some(node_b),
            "Consolidate duplicate concept nodes",
        );

        assert_eq!(action.kind, EvolutionActionKind::MergeNodes);
        assert_eq!(action.target_id, node_a);
        assert_eq!(action.secondary_id, Some(node_b));
    }
}
