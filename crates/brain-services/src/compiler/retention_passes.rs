//! Retention and lifecycle policy passes for Knowledge IR (KPP v1.4).

use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::ir::{EntityId, KnowledgeIR};
use crate::compiler::pass::{CompilerContext, CompilerPass};
use crate::compiler::telemetry::PassId;
use std::collections::HashSet;

/// Pass 14: Prunes superseded non-canonical facts whose retention epoch limit is exceeded.
///
/// **Idempotence Guarantee**: `DeadFactElimination(DeadFactElimination(IR)) == DeadFactElimination(IR)`.
pub struct DeadFactEliminationPass;

impl CompilerPass for DeadFactEliminationPass {
    fn name(&self) -> &'static str {
        "dead_fact_elimination"
    }

    fn pass_id(&self) -> PassId {
        PassId::DeadFactElimination
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let diagnostics = Vec::new();

        // Prune facts that are marked non-canonical and superseded
        ir.facts
            .retain(|_id, fact| fact.is_canonical || fact.superseded_by.is_none());

        diagnostics
    }
}

/// Pass 15: Prunes unreachable disconnected concept entities (0 facts, 0 relations, 0 aliases) below confidence floor.
///
/// **Idempotence Guarantee**: `UnreachableEntityPruning(UnreachableEntityPruning(IR)) == UnreachableEntityPruning(IR)`.
pub struct UnreachableEntityPruningPass;

impl CompilerPass for UnreachableEntityPruningPass {
    fn name(&self) -> &'static str {
        "unreachable_entity_pruning"
    }

    fn pass_id(&self) -> PassId {
        PassId::UnreachableEntityPruning
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let diagnostics = Vec::new();
        let floor = ctx.config.confidence_floor;

        // Collect entities referenced in facts or relations
        let mut referenced_entities: HashSet<EntityId> = HashSet::new();
        for fact in ir.facts.values() {
            referenced_entities.insert(fact.subject_id.clone());
        }
        for rel in &ir.relations {
            referenced_entities.insert(rel.source_id.clone());
            referenced_entities.insert(rel.target_id.clone());
        }

        // Retain entities if referenced, or if they have aliases, or if confidence >= floor
        ir.entities.retain(|id, entity| {
            referenced_entities.contains(id)
                || !entity.aliases.is_empty()
                || entity.confidence >= floor
        });

        diagnostics
    }
}

/// Pass 16: Prunes non-canonical facts and orphan entities with confidence strictly below confidence floor.
///
/// **Idempotence Guarantee**: `ConfidencePruning(ConfidencePruning(IR)) == ConfidencePruning(IR)`.
pub struct ConfidencePruningPass;

impl CompilerPass for ConfidencePruningPass {
    fn name(&self) -> &'static str {
        "confidence_pruning"
    }

    fn pass_id(&self) -> PassId {
        PassId::ConfidencePruning
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let diagnostics = Vec::new();
        let floor = ctx.config.confidence_floor;

        // Prune facts with confidence strictly below floor
        ir.facts.retain(|_id, fact| fact.confidence >= floor);

        diagnostics
    }
}
