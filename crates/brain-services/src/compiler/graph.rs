//! Minimal In-Memory CanonicalGraph and GraphDiffer for the Knowledge Compiler.

use crate::compiler::delta::{EdgeId, GraphDelta, NodeId};
use crate::compiler::ir::{EntityIR, FactIR, KnowledgeIR};
use brain_domain::dtos::{EdgeDTO, NodeDTO};
use std::collections::BTreeMap;

/// Minimal in-memory semantic graph model for compiler pass execution and state differencing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CanonicalGraph {
    /// Canonical entity nodes mapped by stringified ID.
    pub entities: BTreeMap<String, EntityIR>,
    /// Canonical compiled facts mapped by stringified ID.
    pub facts: BTreeMap<String, FactIR>,
}

impl CanonicalGraph {
    /// Instantiates an empty `CanonicalGraph`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads canonical graph state from a `KnowledgeIR` snapshot.
    pub fn from_ir(ir: &KnowledgeIR) -> Self {
        let mut entities = BTreeMap::new();
        let mut facts = BTreeMap::new();

        for (id, entity) in &ir.entities {
            entities.insert(id.0.clone(), entity.clone());
        }

        for (id, fact) in &ir.facts {
            facts.insert(id.0.clone(), fact.clone());
        }

        Self { entities, facts }
    }
}

/// Helper struct for computing derived `GraphDelta` by differencing two `CanonicalGraph` states.
#[derive(Debug, Clone, Copy, Default)]
pub struct GraphDiffer;

impl GraphDiffer {
    /// Computes a derived `GraphDelta` representing changes from `before` to `after`.
    pub fn diff(before: &CanonicalGraph, after: &CanonicalGraph) -> GraphDelta {
        let mut added_nodes = Vec::new();
        let mut updated_nodes = Vec::new();
        let mut removed_nodes = Vec::new();

        let mut added_edges = Vec::new();
        let mut updated_edges = Vec::new();
        let mut removed_edges = Vec::new();

        // Check for added and updated entity nodes
        for (id, entity_after) in &after.entities {
            let node_dto = NodeDTO::new(
                id.clone(),
                entity_after.canonical_name.clone(),
                entity_after.kind.clone(),
                serde_json::json!({
                    "aliases": entity_after.aliases,
                    "confidence": entity_after.confidence,
                }),
            );

            if let Some(entity_before) = before.entities.get(id) {
                if entity_before != entity_after {
                    updated_nodes.push(node_dto);
                }
            } else {
                added_nodes.push(node_dto);
            }
        }

        // Check for removed entity nodes
        for id in before.entities.keys() {
            if !after.entities.contains_key(id) {
                removed_nodes.push(NodeId(id.clone()));
            }
        }

        // Check for added and updated fact edges
        for (id, fact_after) in &after.facts {
            let edge_dto = EdgeDTO::new(
                fact_after.subject_id.0.clone(),
                fact_after.object_value.clone(),
                fact_after.predicate.clone(),
                fact_after.confidence,
            );

            if let Some(fact_before) = before.facts.get(id) {
                if fact_before != fact_after {
                    updated_edges.push(edge_dto);
                }
            } else {
                added_edges.push(edge_dto);
            }
        }

        // Check for removed fact edges
        for id in before.facts.keys() {
            if !after.facts.contains_key(id) {
                removed_edges.push(EdgeId(id.clone()));
            }
        }

        GraphDelta {
            added_nodes,
            updated_nodes,
            removed_nodes,
            added_edges,
            updated_edges,
            removed_edges,
        }
    }
}
