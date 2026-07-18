use crate::entities::{ExplanationChain, KnowledgeGraph};
use crate::identifiers::EdgeId;
use std::collections::HashSet;

/// Service expanding recursive reasoning chains explaining why edges exist.
pub struct ExplanationQueryService;

impl ExplanationQueryService {
    /// Explains the derivation reasoning chain for an edge.
    pub fn explain(graph: &KnowledgeGraph, edge_id: &EdgeId) -> Option<ExplanationChain> {
        Self::explain_edge_recursive(graph, edge_id, &mut HashSet::new())
    }

    fn explain_edge_recursive(
        graph: &KnowledgeGraph,
        edge_id: &EdgeId,
        visited: &mut HashSet<EdgeId>,
    ) -> Option<ExplanationChain> {
        if visited.contains(edge_id) {
            return None; // Prevent cycles
        }
        visited.insert(edge_id.clone());

        let edge = graph.edges.get(edge_id)?.clone();
        let mut supporting_chains = Vec::new();

        let rule = if let Some(ref derivation) = edge.derivation {
            for sub_id in &derivation.supporting_edges {
                if let Some(sub_chain) = Self::explain_edge_recursive(graph, sub_id, visited) {
                    supporting_chains.push(sub_chain);
                }
            }
            Some(derivation.rule)
        } else {
            None
        };

        visited.remove(edge_id);

        Some(ExplanationChain {
            edge,
            rule,
            supporting_chains,
        })
    }
}
