pub mod config;
pub mod dependency_graph;
pub mod diagnostics;
pub mod dirty_set;
pub mod ir;
pub mod optimization_passes;
pub mod pass;
pub mod retention_passes;
pub mod runtime_state;
pub mod scheduler;
pub mod semantic_passes;
pub mod telemetry;

pub use config::CompilerOptimizationConfig;
pub use dependency_graph::CompilerDependencyGraph;
pub use diagnostics::{Diagnostic, DiagnosticKind, DiagnosticLevel};
pub use dirty_set::DirtySet;
pub use ir::{EntityIR, EntityId, FactIR, FactId, KnowledgeIR, ProvenanceIR, RelationIR};
pub use optimization_passes::{
    ProvenanceCompressionPass, RelationDeduplicationPass, TransitiveReductionPass,
};
pub use pass::{
    CanonicalEntityResolutionPass, CompilerContext, CompilerPass, FactDeduplicationPass,
    ObservationNormalizationPass, ValidationPass,
};
pub use retention_passes::{
    ConfidencePruningPass, DeadFactEliminationPass, UnreachableEntityPruningPass,
};
pub use runtime_state::{CompilationHistory, CompilerRuntimeState, CompilerSnapshot};
pub use scheduler::{
    CoalescingDirtyBuffer, CompilationResult, CompileDecision, CompilerScheduler,
    CompilerSchedulerConfig, CompilerSchedulingPolicy, SchedulerState,
};
pub use semantic_passes::{
    AliasResolutionPass, CanonicalFactSelectionPass, CompilerContradictionPass,
    ConfidenceAggregationPass, EntityMergePass, ProvenanceMergePass, RelationNormalizationPass,
    TemporalFactResolutionPass,
};
pub use telemetry::{CompilationMode, CompilerTelemetry, PassExecutionRecord, PassId, PassMetrics};

use brain_integrations::dto::v1::{DiagnosticDto, KnowledgeCompilationReport};
use std::sync::Arc;
use std::time::Instant;

/// Registry and executor for ordered compiler passes.
pub struct PassManager {
    passes: Vec<Box<dyn CompilerPass>>,
}

impl PassManager {
    /// Creates a new `PassManager` with the standard default compiler pass suite.
    pub fn default_pipeline() -> Self {
        let mut manager = Self { passes: Vec::new() };
        // 1. Structural Passes
        manager.register(Box::new(ObservationNormalizationPass));
        // 2. Semantic Passes
        manager.register(Box::new(AliasResolutionPass));
        manager.register(Box::new(EntityMergePass));
        manager.register(Box::new(CanonicalEntityResolutionPass));
        manager.register(Box::new(ConfidenceAggregationPass));
        manager.register(Box::new(ProvenanceMergePass));
        manager.register(Box::new(FactDeduplicationPass));
        manager.register(Box::new(TemporalFactResolutionPass));
        manager.register(Box::new(CanonicalFactSelectionPass));
        manager.register(Box::new(RelationNormalizationPass));
        manager.register(Box::new(CompilerContradictionPass));
        // 3. Graph Optimization Passes
        manager.register(Box::new(RelationDeduplicationPass));
        manager.register(Box::new(TransitiveReductionPass));
        // 4. Evidence Optimization Passes
        manager.register(Box::new(ProvenanceCompressionPass));
        // 5. Retention & Lifecycle Policy Passes
        manager.register(Box::new(DeadFactEliminationPass));
        manager.register(Box::new(UnreachableEntityPruningPass));
        manager.register(Box::new(ConfidencePruningPass));
        // 6. General Structural Invariant Validation
        manager.register(Box::new(ValidationPass));
        manager
    }

    /// Registers a new compiler pass into the execution pipeline.
    pub fn register(&mut self, pass: Box<dyn CompilerPass>) {
        self.passes.push(pass);
    }

    /// Executes all registered passes in strict deterministic order.
    pub fn execute(
        &self,
        ctx: &CompilerContext,
        ir: &mut KnowledgeIR,
    ) -> (
        usize,
        Vec<Diagnostic>,
        Vec<String>,
        Vec<PassExecutionRecord>,
    ) {
        let mut all_diagnostics = Vec::new();
        let mut details = Vec::new();
        let mut pass_records = Vec::new();

        for pass in &self.passes {
            let start = Instant::now();
            let pass_diags = pass.run(ctx, ir);
            let elapsed_ns = start.elapsed().as_nanos() as u64;
            let elapsed_ms = elapsed_ns / 1_000_000;

            details.push(format!(
                "Pass '{}' executed in {} ms (emitted {} diagnostics)",
                pass.name(),
                elapsed_ms,
                pass_diags.len()
            ));

            pass_records.push(PassExecutionRecord {
                pass_id: pass.pass_id(),
                duration_ns: elapsed_ns,
                diagnostics_emitted: pass_diags.len(),
            });

            all_diagnostics.extend(pass_diags);
        }

        // Deterministic sorting on diagnostics: level -> kind -> target -> message
        all_diagnostics.sort_by(|a, b| {
            a.level
                .cmp(&b.level)
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.target.cmp(&b.target))
                .then_with(|| a.message.cmp(&b.message))
        });

        (self.passes.len(), all_diagnostics, details, pass_records)
    }
}

/// Central composition root for the Knowledge Compiler.
pub struct KnowledgeCompiler {
    pass_manager: Arc<PassManager>,
    runtime_state: Arc<CompilerRuntimeState>,
}

impl Default for KnowledgeCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeCompiler {
    /// Instantiates a new `KnowledgeCompiler` with standard pass pipeline.
    pub fn new() -> Self {
        Self {
            pass_manager: Arc::new(PassManager::default_pipeline()),
            runtime_state: Arc::new(CompilerRuntimeState::new()),
        }
    }

    /// Creates a custom compiler with specific pass manager pipeline.
    pub fn with_pipeline(pass_manager: PassManager) -> Self {
        Self {
            pass_manager: Arc::new(pass_manager),
            runtime_state: Arc::new(CompilerRuntimeState::new()),
        }
    }

    /// Returns a reference to the compiler runtime state owner.
    pub fn runtime_state(&self) -> Arc<CompilerRuntimeState> {
        Arc::clone(&self.runtime_state)
    }

    /// Compiles raw Knowledge IR into canonical knowledge, emitting diagnostics and an immutable compilation report.
    pub fn compile(
        &self,
        ctx: &CompilerContext,
        ir: &mut KnowledgeIR,
    ) -> (KnowledgeIR, KnowledgeCompilationReport) {
        let mode = if ctx.dirty_set.is_some() {
            CompilationMode::Incremental
        } else {
            CompilationMode::Full
        };

        let start = Instant::now();
        let (passes_executed, diagnostics, mut details, pass_records) =
            self.pass_manager.execute(ctx, ir);
        let duration_ms = start.elapsed().as_millis() as u64;

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let diagnostic_dtos: Vec<DiagnosticDto> = diagnostics
            .iter()
            .map(|d| DiagnosticDto {
                level: d.level.to_string(),
                kind: d.kind.to_string(),
                target: d.target.clone(),
                message: d.message.clone(),
                suggestion: d.suggestion.clone(),
            })
            .collect();

        details.push(format!(
            "Knowledge compilation completed in {} ms (Entities: {}, Facts: {}, Diagnostics: {})",
            duration_ms,
            ir.entities.len(),
            ir.facts.len(),
            diagnostics.len()
        ));

        let report = KnowledgeCompilationReport {
            compilation_id: ctx.compilation_id.to_string(),
            timestamp_ms,
            duration_ms,
            passes_executed,
            entities_compiled: ir.entities.len(),
            facts_compiled: ir.facts.len(),
            diagnostics: diagnostic_dtos,
            details,
        };

        self.runtime_state
            .record_compilation(mode, &report, &pass_records);

        (ir.clone(), report)
    }

    /// Performs incremental compilation over a dirty subset of Knowledge IR.
    pub fn compile_incremental(
        &self,
        ctx: &CompilerContext,
        ir: &mut KnowledgeIR,
        input_dirty_set: DirtySet,
    ) -> (KnowledgeIR, KnowledgeCompilationReport) {
        let mut dirty_set = input_dirty_set;

        // Force full re-compilation if graph version epoch mismatches
        if dirty_set.graph_version != ctx.graph_version {
            dirty_set.is_full_recompile = true;
        }

        // Discover dependencies prior to pass execution
        let dep_graph = CompilerDependencyGraph::build_from_ir(ir);
        let expanded_dirty_set = Arc::new(dep_graph.expand_dirty_set(&dirty_set));

        let mut inc_ctx = ctx.clone();
        inc_ctx.dirty_set = Some(expanded_dirty_set);

        self.compile(&inc_ctx, ir)
    }
}
