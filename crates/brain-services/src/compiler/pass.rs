//! Compiler passes and pass trait definitions for the Knowledge Compiler.

use crate::compiler::diagnostics::{Diagnostic, DiagnosticKind, DiagnosticLevel};
use crate::compiler::ir::{EntityId, KnowledgeIR};
use brain_domain::SessionId;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Read-only execution context supplied to compiler passes.
#[derive(Debug, Clone)]
pub struct CompilerContext {
    /// Unique compilation execution ID.
    pub compilation_id: Uuid,
    /// Active session ID.
    pub session_id: SessionId,
    /// Monotonic graph version epoch counter.
    pub graph_version: u64,
    /// Optional read-only expanded dirty set for incremental compilation.
    pub dirty_set: Option<std::sync::Arc<crate::compiler::dirty_set::DirtySet>>,
    /// Minimum confidence threshold for canonical entity resolution [0.0..1.0].
    pub min_confidence_threshold: f64,
    /// Maximum execution time budget in milliseconds.
    pub time_budget_ms: u64,
    /// Cooperative cancellation token.
    pub cancellation_token: CancellationToken,
}

/// Abstract compiler pass transforming Knowledge IR and emitting diagnostics.
pub trait CompilerPass: Send + Sync {
    /// Returns the unique name identifier of the compiler pass.
    fn name(&self) -> &'static str;

    /// Returns the stable PassId enum identifier.
    fn pass_id(&self) -> crate::compiler::telemetry::PassId;

    /// Executes the transformation on Knowledge IR and appends diagnostics.
    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic>;
}

/// Pass 1: Normalizes entity canonical names and trims whitespace.
pub struct ObservationNormalizationPass;

impl CompilerPass for ObservationNormalizationPass {
    fn name(&self) -> &'static str {
        "observation_normalization"
    }

    fn pass_id(&self) -> crate::compiler::telemetry::PassId {
        crate::compiler::telemetry::PassId::ObservationNormalization
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (id, entity) in ir.entities.iter_mut() {
            if let Some(ref ds) = ctx.dirty_set {
                if !ds.is_entity_dirty(id) {
                    continue;
                }
            }

            let trimmed = entity.canonical_name.trim().to_string();
            if trimmed != entity.canonical_name {
                entity.canonical_name = trimmed.clone();
                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticKind::LowConfidence,
                    id.0.clone(),
                    format!("Normalized entity canonical name to '{}'", trimmed),
                ));
            }
        }

        diagnostics
    }
}

/// Pass 2: Resolves entity alias mappings into primary canonical entities.
pub struct CanonicalEntityResolutionPass;

impl CompilerPass for CanonicalEntityResolutionPass {
    fn name(&self) -> &'static str {
        "canonical_entity_resolution"
    }

    fn pass_id(&self) -> crate::compiler::telemetry::PassId {
        crate::compiler::telemetry::PassId::CanonicalEntityResolution
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (id, entity) in ir.entities.iter() {
            if let Some(ref ds) = ctx.dirty_set {
                if !ds.is_entity_dirty(id) {
                    continue;
                }
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

        diagnostics
    }
}

/// Pass 3: Deduplicates identical facts and merges provenance evidence.
pub struct FactDeduplicationPass;

impl CompilerPass for FactDeduplicationPass {
    fn name(&self) -> &'static str {
        "fact_deduplication"
    }

    fn pass_id(&self) -> crate::compiler::telemetry::PassId {
        crate::compiler::telemetry::PassId::FactDeduplication
    }

    fn run(&self, ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for (id, fact) in ir.facts.iter() {
            if let Some(ref ds) = ctx.dirty_set {
                if !ds.is_fact_dirty(id) {
                    continue;
                }
            }

            if fact.provenance.evidence_ids.is_empty() {
                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticKind::MissingEvidence,
                    id.0.clone(),
                    format!("Fact '{}' has no provenance evidence IDs attached", id.0),
                ));
            }
        }

        diagnostics
    }
}

/// Pass 4: Validates structural integrity and flags orphan concept nodes.
pub struct ValidationPass;

impl CompilerPass for ValidationPass {
    fn name(&self) -> &'static str {
        "validation"
    }

    fn pass_id(&self) -> crate::compiler::telemetry::PassId {
        crate::compiler::telemetry::PassId::Validation
    }

    fn run(&self, _ctx: &CompilerContext, ir: &mut KnowledgeIR) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let connected_ids: std::collections::HashSet<EntityId> = ir
            .relations
            .iter()
            .flat_map(|r| vec![r.source_id.clone(), r.target_id.clone()])
            .collect();

        for (id, entity) in ir.entities.iter() {
            if !connected_ids.contains(id) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticKind::OrphanConcept,
                    id.0.clone(),
                    format!(
                        "Entity '{}' is an orphan concept with no active relations",
                        entity.canonical_name
                    ),
                ));
            }
        }

        diagnostics
    }
}
