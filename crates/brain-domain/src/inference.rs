use crate::entities::{Edge, GraphProvenance, KnowledgeGraph, ProvenanceSource, RelationKind};
use crate::identifiers::EdgeId;
use crate::relations::RelationRegistry;
use std::collections::HashMap;
use std::str::FromStr;

/// Rule-driven inference engine running over relation registry metadata.
pub struct InferenceEngine;

impl InferenceEngine {
    /// Performs iterative fixed-point inference over the graph, deriving transitive and inverse edges.
    /// Obeying monotonicity: only returns newly derived edges with Inferred provenance,
    /// without mutating or deleting existing edges.
    pub fn infer(graph: &KnowledgeGraph, registry: &RelationRegistry) -> Vec<Edge> {
        let mut all_edges: HashMap<EdgeId, Edge> = graph.edges.clone();
        let mut newly_inferred: HashMap<EdgeId, Edge> = HashMap::new();

        const MAX_ITERATIONS: usize = 10;
        for _ in 0..MAX_ITERATIONS {
            let mut candidates: HashMap<EdgeId, Edge> = HashMap::new();

            // 1. Sort all edges to ensure deterministic order (Rule Determinism)
            let mut sorted_edges: Vec<&Edge> = all_edges.values().collect();
            sorted_edges.sort_by_key(|e| EdgeId::new(e.source, e.target, e.relation.id()));

            // 2. Build forward adjacency mapping for transitivity lookup
            // Key: (RelationKind, SourceNodeId), Value: list of (TargetNodeId, Weight)
            let mut adjacency: HashMap<
                (RelationKind, crate::identifiers::NodeId),
                Vec<(crate::identifiers::NodeId, f64)>,
            > = HashMap::new();
            for edge in &sorted_edges {
                adjacency
                    .entry((edge.relation, edge.source))
                    .or_default()
                    .push((edge.target, edge.weight));
            }

            // Helper to insert candidate edges deterministically resolving duplicates
            let mut insert_candidate = |edge_id: EdgeId, candidate: Edge| {
                if all_edges.contains_key(&edge_id) {
                    return;
                }
                if let Some(existing) = candidates.get_mut(&edge_id) {
                    let should_overwrite = if candidate.weight > existing.weight {
                        true
                    } else if (candidate.weight - existing.weight).abs() < f64::EPSILON {
                        match (&candidate.derivation, &existing.derivation) {
                            (Some(c_deriv), Some(e_deriv)) => {
                                c_deriv.supporting_edges < e_deriv.supporting_edges
                            }
                            _ => false,
                        }
                    } else {
                        false
                    };
                    if should_overwrite {
                        *existing = candidate;
                    }
                } else {
                    candidates.insert(edge_id, candidate);
                }
            };

            // 3. Evaluate inference rules deterministically
            for edge in &sorted_edges {
                let Some(def) = registry.get_kind(edge.relation) else {
                    continue;
                };

                let source_edge_id = EdgeId::new(edge.source, edge.target, edge.relation.id());

                // Rule A: Inverse relation mapping
                if let Some(ref inverse_id) = def.inverse {
                    let inv_kind = RelationKind::from_str(inverse_id.as_str())
                        .unwrap_or(RelationKind::Unknown);
                    if inv_kind != RelationKind::Unknown {
                        let inv_edge_id = EdgeId::new(edge.target, edge.source, inv_kind.id());

                        let prov = GraphProvenance {
                            source: ProvenanceSource::Inferred,
                            extractor_version: "inference-engine".to_string(),
                            ..GraphProvenance::default()
                        };

                        let mut derived =
                            Edge::new(edge.target, edge.source, inv_kind, edge.weight);
                        derived.provenance = prov;
                        derived.derivation = Some(crate::entities::Derivation {
                            rule: crate::entities::RuleId::Inverse,
                            supporting_edges: vec![source_edge_id.clone()],
                        });

                        insert_candidate(inv_edge_id, derived);
                    }
                }

                // Rule B: Transitive path closure propagation
                if def.transitivity {
                    if let Some(targets) = adjacency.get(&(edge.relation, edge.target)) {
                        for &(z, w2) in targets {
                            // Skip self-loops
                            if edge.source == z {
                                continue;
                            }

                            let trans_edge_id = EdgeId::new(edge.source, z, edge.relation.id());
                            let target_edge_id = EdgeId::new(edge.target, z, edge.relation.id());
                            let combined_weight = def.confidence_strategy.combine(edge.weight, w2);

                            let prov = GraphProvenance {
                                source: ProvenanceSource::Inferred,
                                extractor_version: "inference-engine".to_string(),
                                ..GraphProvenance::default()
                            };

                            let mut supporting_edges = vec![source_edge_id.clone(), target_edge_id];
                            supporting_edges.sort();

                            let mut derived =
                                Edge::new(edge.source, z, edge.relation, combined_weight);
                            derived.provenance = prov;
                            derived.derivation = Some(crate::entities::Derivation {
                                rule: crate::entities::RuleId::Transitive,
                                supporting_edges,
                            });

                            insert_candidate(trans_edge_id, derived);
                        }
                    }
                }
            }

            if candidates.is_empty() {
                break;
            }

            for (edge_id, edge) in candidates {
                all_edges.insert(edge_id.clone(), edge.clone());
                newly_inferred.insert(edge_id, edge);
            }
        }

        // Convert the map of newly inferred edges to a sorted Vec
        let mut result: Vec<Edge> = newly_inferred.into_values().collect();
        result.sort_by_key(|e| EdgeId::new(e.source, e.target, e.relation.id()));
        result
    }
}
