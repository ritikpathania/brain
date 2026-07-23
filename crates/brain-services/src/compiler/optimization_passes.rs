//! Optimization passes for Knowledge IR graph and evidence representations (KPP v1.4).

use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::ir::{EntityId, KnowledgeIR, ProvenanceIR, RelationIR};
use crate::compiler::pass::{CompilerContext, CompilerPass};
use crate::compiler::telemetry::PassId;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Pass 11: Merges parallel relation edges between identical (source, target, relation_kind) tuples.
///
/// **Idempotence Guarantee**: `RelationDeduplication(RelationDeduplication(IR)) == RelationDeduplication(IR)`.
pub struct RelationDeduplicationPass;

impl CompilerPass for RelationDeduplicationPass {
    fn name(&self) -> &'static str {
        "relation_deduplication"
    }

    fn pass_id(&self) -> PassId {
        PassId::RelationDeduplication
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let diagnostics = Vec::new();
        if ir.relations.is_empty() {
            return diagnostics;
        }

        // Group relations by (source_id, target_id, normalized_kind)
        let mut grouped: BTreeMap<(EntityId, EntityId, String), Vec<RelationIR>> = BTreeMap::new();
        for rel in std::mem::take(&mut ir.relations) {
            let key = (
                rel.source_id.clone(),
                rel.target_id.clone(),
                rel.relation_kind.to_lowercase(),
            );
            grouped.entry(key).or_default().push(rel);
        }

        let mut deduplicated = Vec::new();
        for ((source_id, target_id, relation_kind), rels) in grouped {
            if rels.len() == 1 {
                deduplicated.push(rels.into_iter().next().unwrap());
                continue;
            }

            // Combine parallel edges: max weight & merged provenance
            let mut max_weight: f64 = 0.0;
            let mut primary_provenance: Option<ProvenanceIR> = None;
            let mut combined_chain: Vec<ProvenanceIR> = Vec::new();

            for r in rels {
                if r.weight > max_weight {
                    max_weight = r.weight;
                    primary_provenance = Some(r.provenance.clone());
                }
                combined_chain.extend(r.provenance_chain);
            }

            // Deduplicate combined provenance chain
            let mut seen_prov = BTreeSet::new();
            combined_chain.retain(|p| {
                let key = (p.source_origin.clone(), p.timestamp_ms);
                seen_prov.insert(key)
            });

            let provenance = primary_provenance.unwrap_or_else(|| ProvenanceIR {
                source_origin: "merged_dedup".to_string(),
                evidence_ids: vec![],
                confidence: max_weight,
                timestamp_ms: 0,
            });

            deduplicated.push(RelationIR {
                source_id,
                target_id,
                relation_kind,
                weight: max_weight,
                provenance,
                provenance_chain: combined_chain,
            });
        }

        ir.relations = deduplicated;
        diagnostics
    }
}

/// Pass 12: Performs transitive reduction on explicit transitive relation categories (DAG).
///
/// **Idempotence Guarantee**: `TransitiveReduction(TransitiveReduction(IR)) == TransitiveReduction(IR)`.
pub struct TransitiveReductionPass;

impl CompilerPass for TransitiveReductionPass {
    fn name(&self) -> &'static str {
        "transitive_reduction"
    }

    fn pass_id(&self) -> PassId {
        PassId::TransitiveReduction
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let diagnostics = Vec::new();

        let allowed_kinds = &ctx.config.transitive_reduction_relations;
        if allowed_kinds.is_empty() || ir.relations.is_empty() {
            return diagnostics;
        }

        // Group relations into transitive candidates vs non-transitive
        let mut transitive_rels: Vec<RelationIR> = Vec::new();
        let mut passthrough_rels: Vec<RelationIR> = Vec::new();

        for rel in std::mem::take(&mut ir.relations) {
            if allowed_kinds.contains(&rel.relation_kind.to_lowercase()) {
                transitive_rels.push(rel);
            } else {
                passthrough_rels.push(rel);
            }
        }

        if transitive_rels.is_empty() {
            ir.relations = passthrough_rels;
            return diagnostics;
        }

        // Group by relation kind
        let mut by_kind: HashMap<String, Vec<RelationIR>> = HashMap::new();
        for rel in transitive_rels {
            by_kind
                .entry(rel.relation_kind.to_lowercase())
                .or_default()
                .push(rel);
        }

        for (kind, rels) in by_kind {
            // Build adjacency list for this relation kind
            let mut adj: HashMap<EntityId, HashSet<EntityId>> = HashMap::new();
            for r in &rels {
                adj.entry(r.source_id.clone())
                    .or_default()
                    .insert(r.target_id.clone());
            }

            // Helper BFS to check if path of length >= 2 exists between u and v
            let has_indirect_path = |u: &EntityId, v: &EntityId| -> bool {
                let mut visited = HashSet::new();
                let mut queue = Vec::new();

                if let Some(neighbors) = adj.get(u) {
                    for neighbor in neighbors {
                        if neighbor != v {
                            queue.push(neighbor.clone());
                            visited.insert(neighbor.clone());
                        }
                    }
                }

                while let Some(curr) = queue.pop() {
                    if &curr == v {
                        return true;
                    }
                    if let Some(neighbors) = adj.get(&curr) {
                        for neighbor in neighbors {
                            if visited.insert(neighbor.clone()) {
                                queue.push(neighbor.clone());
                            }
                        }
                    }
                }
                false
            };

            // Retain direct edge u -> v only if NO indirect path u -> ... -> v exists
            for rel in rels {
                if !has_indirect_path(&rel.source_id, &rel.target_id) {
                    passthrough_rels.push(RelationIR {
                        relation_kind: kind.clone(),
                        ..rel
                    });
                }
            }
        }

        ir.relations = passthrough_rels;
        diagnostics
    }
}

/// Pass 13: Compresses provenance chains while preserving earliest, latest, highest-confidence, and diverse evidence items.
///
/// **Idempotence Guarantee**: `ProvenanceCompression(ProvenanceCompression(IR)) == ProvenanceCompression(IR)`.
pub struct ProvenanceCompressionPass;

impl ProvenanceCompressionPass {
    /// Compresses a provenance chain vector based on retention limits and fidelity rules.
    pub fn compress_chain(chain: &[ProvenanceIR], limit: usize) -> Vec<ProvenanceIR> {
        if chain.len() <= limit || limit == 0 {
            return chain.to_vec();
        }

        let mut preserved: BTreeSet<usize> = BTreeSet::new();

        // 1. Preserve earliest item (min timestamp)
        let min_idx = chain
            .iter()
            .enumerate()
            .min_by_key(|(_, p)| p.timestamp_ms)
            .map(|(i, _)| i)
            .unwrap_or(0);
        preserved.insert(min_idx);

        // 2. Preserve latest item (max timestamp)
        let max_idx = chain
            .iter()
            .enumerate()
            .max_by_key(|(_, p)| p.timestamp_ms)
            .map(|(i, _)| i)
            .unwrap_or(0);
        preserved.insert(max_idx);

        // 3. Preserve highest-confidence item
        let max_conf_idx = chain
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        preserved.insert(max_conf_idx);

        // 4. Preserve distinct source origin classes
        let mut seen_origins = HashSet::new();
        for (i, p) in chain.iter().enumerate() {
            if preserved.len() >= limit {
                break;
            }
            if seen_origins.insert(p.source_origin.clone()) {
                preserved.insert(i);
            }
        }

        // Fill remaining quota up to limit
        for i in 0..chain.len() {
            if preserved.len() >= limit {
                break;
            }
            preserved.insert(i);
        }

        preserved.into_iter().map(|i| chain[i].clone()).collect()
    }
}

impl CompilerPass for ProvenanceCompressionPass {
    fn name(&self) -> &'static str {
        "provenance_compression"
    }

    fn pass_id(&self) -> PassId {
        PassId::ProvenanceCompression
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let diagnostics = Vec::new();
        let limit = ctx.config.provenance_limit;

        for entity in ir.entities.values_mut() {
            entity.provenance_chain = Self::compress_chain(&entity.provenance_chain, limit);
        }

        for fact in ir.facts.values_mut() {
            fact.provenance_chain = Self::compress_chain(&fact.provenance_chain, limit);
        }

        for rel in &mut ir.relations {
            rel.provenance_chain = Self::compress_chain(&rel.provenance_chain, limit);
        }

        diagnostics
    }
}
