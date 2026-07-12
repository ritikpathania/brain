use crate::storage::ExtractedGraph;
use brain_domain::bkf::CompiledKnowledge;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Representation of a single difference between KPP compiled knowledge and legacy extraction graphs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiffItem {
    /// Entity exists in legacy graph but is missing from compiled KPP knowledge.
    MissingEntity {
        /// Entity ID.
        id: String,
        /// Entity label.
        label: String,
    },
    /// Entity exists in compiled KPP knowledge but is missing from legacy graph.
    ExtraEntity {
        /// Entity ID.
        id: String,
        /// Entity label.
        label: String,
    },
    /// Relationship exists in legacy graph but is missing from KPP compiled knowledge.
    MissingRelationship {
        /// Source node ID.
        source: String,
        /// Target node ID.
        target: String,
        /// Edge relation.
        relation: String,
    },
}

/// Report containing all identified schema and semantic mismatches between legacy and KPP pipelines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiffReport {
    /// List of mismatches.
    pub mismatches: Vec<DiffItem>,
}

/// Comparator for checking output alignment between Legacy extraction and KPP pipelines.
pub struct ShadowComparator;

impl ShadowComparator {
    /// Performs an element-by-element semantic comparison between legacy ExtractedGraph and compiled KPP knowledge.
    pub fn compare(legacy: &ExtractedGraph, compiled: &CompiledKnowledge) -> DiffReport {
        let mut mismatches = Vec::new();

        let legacy_node_ids: HashSet<String> = legacy.nodes.iter().map(|n| n.id.clone()).collect();
        let compiled_node_ids: HashSet<String> = compiled.nodes.iter().map(|n| n.id.clone()).collect();

        // 1. Missing Entities
        for legacy_node in &legacy.nodes {
            if !compiled_node_ids.contains(&legacy_node.id) {
                mismatches.push(DiffItem::MissingEntity {
                    id: legacy_node.id.clone(),
                    label: legacy_node.label.clone(),
                });
            }
        }

        // 2. Extra Entities
        for compiled_node in &compiled.nodes {
            if !legacy_node_ids.contains(&compiled_node.id) {
                mismatches.push(DiffItem::ExtraEntity {
                    id: compiled_node.id.clone(),
                    label: compiled_node.label.clone(),
                });
            }
        }

        // 3. Relationships check
        let legacy_edges: HashSet<(String, String, String)> = legacy
            .edges
            .iter()
            .map(|e| (e.source.clone(), e.target.clone(), e.relation.to_lowercase()))
            .collect();

        let compiled_edges: HashSet<(String, String, String)> = compiled
            .edges
            .iter()
            .map(|e| (e.source.clone(), e.target.clone(), e.relation.to_lowercase()))
            .collect();

        for (src, dst, rel) in &legacy_edges {
            if !compiled_edges.contains(&(src.clone(), dst.clone(), rel.clone())) {
                mismatches.push(DiffItem::MissingRelationship {
                    source: src.clone(),
                    target: dst.clone(),
                    relation: rel.clone(),
                });
            }
        }

        DiffReport { mismatches }
    }
}
