//! Semantic Compiler Passes for the Knowledge Compiler (KPP v1.1).

use crate::compiler::diagnostics::{Diagnostic, DiagnosticKind, DiagnosticLevel};
use crate::compiler::ir::{EntityIR, EntityId, FactId, KnowledgeIR};
use crate::compiler::pass::{CompilerContext, CompilerPass};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Pass 1: Scans entity aliases, resolves alias collisions, and redirects entity references to canonical entity IDs.
pub struct AliasResolutionPass;

impl CompilerPass for AliasResolutionPass {
    fn name(&self) -> &'static str {
        "alias_resolution"
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let diagnostics = Vec::new();
        let mut alias_map: HashMap<String, EntityId> = HashMap::new();

        // Register canonical names and aliases into alias_map
        for (id, entity) in &ir.entities {
            alias_map.insert(entity.canonical_name.to_lowercase(), id.clone());
            for alias in &entity.aliases {
                alias_map.insert(alias.to_lowercase(), id.clone());
            }
        }

        // Redirect relation source/target references if they match an alias string
        for rel in &mut ir.relations {
            if let Some(canonical_source) = alias_map.get(&rel.source_id.0.to_lowercase()) {
                if canonical_source != &rel.source_id {
                    rel.source_id = canonical_source.clone();
                }
            }
            if let Some(canonical_target) = alias_map.get(&rel.target_id.0.to_lowercase()) {
                if canonical_target != &rel.target_id {
                    rel.target_id = canonical_target.clone();
                }
            }
        }

        diagnostics
    }
}

/// Pass 2: Merges duplicate entity nodes matching identical canonical names into unified canonical entities.
///
/// **Deterministic Tie-Breaking Rule**:
/// Primary winner entity is chosen by:
/// 1. Highest confidence score.
/// 2. Most recent provenance timestamp (`timestamp_ms`).
/// 3. Lexicographically smallest `EntityId`.
pub struct EntityMergePass;

impl CompilerPass for EntityMergePass {
    fn name(&self) -> &'static str {
        "entity_merge"
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Group entities by lowercase canonical name
        let mut groups: BTreeMap<String, Vec<EntityIR>> = BTreeMap::new();
        for (_, entity) in std::mem::take(&mut ir.entities) {
            let key = entity.canonical_name.to_lowercase();
            groups.entry(key).or_default().push(entity);
        }

        for (name_key, mut candidates) in groups {
            if candidates.len() == 1 {
                let entity = candidates.remove(0);
                ir.insert_entity(entity);
                continue;
            }

            // Sort candidates by explicit deterministic tie-breaking policy:
            // Confidence (desc) -> Timestamp (desc) -> EntityId (asc)
            candidates.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.provenance.timestamp_ms.cmp(&a.provenance.timestamp_ms))
                    .then_with(|| a.id.cmp(&b.id))
            });

            let mut winner = candidates.remove(0);
            let winner_id = winner.id.clone();

            for secondary in candidates {
                let sec_id = secondary.id.clone();
                let sec_name = secondary.canonical_name.clone();
                winner.merge_from(secondary);

                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticKind::AmbiguousIdentity,
                    winner_id.0.clone(),
                    format!(
                        "Merged duplicate entity '{}' ({}) into canonical entity '{}' ({})",
                        sec_name, sec_id, name_key, winner_id
                    ),
                ));
            }

            ir.insert_entity(winner);
        }

        diagnostics
    }
}

/// Pass 3: Recomputes Bayesian aggregated confidence scores across additive provenance chains.
pub struct ConfidenceAggregationPass;

impl CompilerPass for ConfidenceAggregationPass {
    fn name(&self) -> &'static str {
        "confidence_aggregation"
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Aggregate entity confidence scores
        for (id, entity) in &mut ir.entities {
            if let Some(ref ds) = ctx.dirty_set {
                if !ds.is_entity_dirty(id) {
                    continue;
                }
            }

            if !entity.provenance_chain.is_empty() {
                let mut combined = 1.0;
                for prov in &entity.provenance_chain {
                    let c = prov.confidence.clamp(0.0, 1.0);
                    combined *= 1.0 - c;
                }
                entity.confidence = (1.0 - combined).clamp(0.0, 1.0);
            }

            if entity.confidence < ctx.min_confidence_threshold {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticLevel::Warning,
                        DiagnosticKind::LowConfidence,
                        id.0.clone(),
                        format!(
                            "Entity '{}' confidence {:.2} is below minimum threshold {:.2}",
                            entity.canonical_name, entity.confidence, ctx.min_confidence_threshold
                        ),
                    )
                    .with_suggestion(
                        "Provide additional observation evidence to increase confidence.",
                    ),
                );
            }
        }

        // Aggregate fact confidence scores
        for (id, fact) in &mut ir.facts {
            if let Some(ref ds) = ctx.dirty_set {
                if !ds.is_fact_dirty(id) {
                    continue;
                }
            }

            if !fact.provenance_chain.is_empty() {
                let mut combined = 1.0;
                for prov in &fact.provenance_chain {
                    let c = prov.confidence.clamp(0.0, 1.0);
                    combined *= 1.0 - c;
                }
                fact.confidence = (1.0 - combined).clamp(0.0, 1.0);
            }

            if fact.confidence < ctx.min_confidence_threshold {
                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticKind::LowConfidence,
                    id.0.clone(),
                    format!(
                        "Fact '{}' confidence {:.2} is below minimum threshold {:.2}",
                        id.0, fact.confidence, ctx.min_confidence_threshold
                    ),
                ));
            }
        }

        diagnostics
    }
}

/// Pass 4: Merges and deduplicates additive provenance history entries without losing explainability.
pub struct ProvenanceMergePass;

impl CompilerPass for ProvenanceMergePass {
    fn name(&self) -> &'static str {
        "provenance_merge"
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (id, entity) in &mut ir.entities {
            let mut seen = BTreeSet::new();
            entity.provenance_chain.retain(|p| {
                let key = (p.source_origin.clone(), p.timestamp_ms);
                seen.insert(key)
            });

            if entity.provenance_chain.is_empty() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticKind::MissingEvidence,
                    id.0.clone(),
                    format!(
                        "Entity '{}' has an empty provenance chain",
                        entity.canonical_name
                    ),
                ));
            }
        }

        for (id, fact) in &mut ir.facts {
            let mut seen = BTreeSet::new();
            fact.provenance_chain.retain(|p| {
                let key = (p.source_origin.clone(), p.timestamp_ms);
                seen.insert(key)
            });

            if fact.provenance_chain.is_empty() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticKind::MissingEvidence,
                    id.0.clone(),
                    format!("Fact '{}' has an empty provenance chain", id.0),
                ));
            }
        }

        diagnostics
    }
}

/// Pass 5: Evaluates temporal validity windows (`valid_from_ms`, `valid_until_ms`) and resolves active vs expired facts.
pub struct TemporalFactResolutionPass;

impl CompilerPass for TemporalFactResolutionPass {
    fn name(&self) -> &'static str {
        "temporal_fact_resolution"
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (id, fact) in &mut ir.facts {
            if let (Some(from), Some(until)) = (fact.valid_from_ms, fact.valid_until_ms) {
                if from > until {
                    fact.is_canonical = false;
                    diagnostics.push(Diagnostic::new(
                        DiagnosticLevel::Error,
                        DiagnosticKind::ConflictingFacts,
                        id.0.clone(),
                        format!(
                            "Fact '{}' has invalid temporal validity window: valid_from ({}) > valid_until ({})",
                            id.0, from, until
                        ),
                    ));
                }
            }
        }

        diagnostics
    }
}

/// Pass 6: Selects a single active canonical fact for non-repeatable predicates using explicit tie-breaking rules.
///
/// **Deterministic Tie-Breaking Rule**:
/// Primary canonical fact is chosen by:
/// 1. Highest confidence score.
/// 2. Most recent provenance timestamp (`timestamp_ms`).
/// 3. Lexicographically smallest `FactId`.
pub struct CanonicalFactSelectionPass;

impl CompilerPass for CanonicalFactSelectionPass {
    fn name(&self) -> &'static str {
        "canonical_fact_selection"
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Group active facts by (subject_id, predicate)
        let mut groups: BTreeMap<(EntityId, String), Vec<FactId>> = BTreeMap::new();
        for (id, fact) in &ir.facts {
            if fact.is_canonical {
                groups
                    .entry((fact.subject_id.clone(), fact.predicate.to_lowercase()))
                    .or_default()
                    .push(id.clone());
            }
        }

        for ((subject_id, predicate), mut fact_ids) in groups {
            if fact_ids.len() <= 1 {
                continue;
            }

            // Sort fact IDs by explicit deterministic tie-breaking policy:
            // Confidence (desc) -> Timestamp (desc) -> FactId (asc)
            fact_ids.sort_by(|a, b| {
                let f_a = ir.facts.get(a).unwrap();
                let f_b = ir.facts.get(b).unwrap();
                f_b.confidence
                    .partial_cmp(&f_a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        f_b.provenance
                            .timestamp_ms
                            .cmp(&f_a.provenance.timestamp_ms)
                    })
                    .then_with(|| a.cmp(b))
            });

            let winner_id = fact_ids.remove(0);

            for secondary_id in fact_ids {
                if let Some(sec_fact) = ir.facts.get_mut(&secondary_id) {
                    sec_fact.is_canonical = false;
                    sec_fact.superseded_by = Some(winner_id.clone());

                    diagnostics.push(Diagnostic::new(
                        DiagnosticLevel::Info,
                        DiagnosticKind::ConflictingFacts,
                        secondary_id.0.clone(),
                        format!(
                            "Fact '{}' (subject: {}, predicate: '{}') superseded by canonical fact '{}'",
                            secondary_id, subject_id, predicate, winner_id
                        ),
                    ));
                }
            }
        }

        diagnostics
    }
}

/// Pass 7: Normalizes relation categories, clamps edge weights to [0.0..1.0], and prunes self-referential relation edges.
pub struct RelationNormalizationPass;

impl CompilerPass for RelationNormalizationPass {
    fn name(&self) -> &'static str {
        "relation_normalization"
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let mut normalized_relations = Vec::new();
        for mut rel in std::mem::take(&mut ir.relations) {
            // Prune self-referential edges
            if rel.source_id == rel.target_id {
                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticKind::OrphanConcept,
                    rel.source_id.0.clone(),
                    format!(
                        "Pruned self-referential relation edge on entity '{}'",
                        rel.source_id
                    ),
                ));
                continue;
            }

            rel.relation_kind = rel.relation_kind.trim().to_lowercase();
            rel.weight = rel.weight.clamp(0.0, 1.0);
            normalized_relations.push(rel);
        }

        ir.relations = normalized_relations;
        diagnostics
    }
}

/// Pass 8: Inspects active canonical facts and entities for explicit compiler contradictions.
pub struct CompilerContradictionPass;

impl CompilerPass for CompilerContradictionPass {
    fn name(&self) -> &'static str {
        "compiler_contradiction"
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check for contradicting active facts on same subject with identical predicate but conflicting values
        let mut seen_values: BTreeMap<(EntityId, String), (String, FactId)> = BTreeMap::new();
        for (id, fact) in &ir.facts {
            if !fact.is_canonical {
                continue;
            }
            let key = (fact.subject_id.clone(), fact.predicate.to_lowercase());
            if let Some((existing_val, existing_id)) = seen_values.get(&key) {
                if existing_val != &fact.object_value {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticLevel::Error,
                            DiagnosticKind::ConflictingFacts,
                            fact.subject_id.0.clone(),
                            format!(
                                "Contradictory facts detected on subject '{}' for predicate '{}': '{}' (fact {}) vs '{}' (fact {})",
                                fact.subject_id, fact.predicate, existing_val, existing_id, fact.object_value, id
                            ),
                        )
                        .with_suggestion("Review source observations or resolve contradiction via reflection engine."),
                    );
                }
            } else {
                seen_values.insert(key, (fact.object_value.clone(), id.clone()));
            }
        }

        diagnostics
    }
}
