use crate::entities::KnowledgeGraph;
use crate::relations::RelationRegistry;
use std::collections::HashSet;

/// Conflict resolution engine that filters suppressed fallback relations.
pub struct SuppressionEngine;

impl SuppressionEngine {
    /// Filters out edges whose relation has fallback_suppression == false
    /// if the same node pair is connected by a relation with fallback_suppression == true.
    /// Obeying provenance monotonicity: does not alter or rewrite provenance of remaining edges.
    pub fn apply_suppression(
        mut graph: KnowledgeGraph,
        registry: &RelationRegistry,
    ) -> KnowledgeGraph {
        let mut has_suppressor = HashSet::new();

        // 1. Identify all node pairs containing a specific/suppression-triggering relation
        for edge in graph.edges.values() {
            if let Some(def) = registry.get_kind(edge.relation) {
                if def.fallback_suppression {
                    let u = std::cmp::min(edge.source, edge.target);
                    let v = std::cmp::max(edge.source, edge.target);
                    has_suppressor.insert((u, v));
                }
            }
        }

        // 2. Suppress generic fallback relations connecting those same node pairs
        graph.edges.retain(|_, edge| {
            if let Some(def) = registry.get_kind(edge.relation) {
                if !def.fallback_suppression {
                    let u = std::cmp::min(edge.source, edge.target);
                    let v = std::cmp::max(edge.source, edge.target);
                    if has_suppressor.contains(&(u, v)) {
                        // Drop this generic relationship edge
                        return false;
                    }
                }
            }
            true
        });

        graph
    }
}
