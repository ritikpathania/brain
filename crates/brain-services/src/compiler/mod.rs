//! Knowledge Processing Pipeline (KPP) Knowledge Compiler.

pub mod diagnostics;
pub mod ir;
pub mod pass;

pub use diagnostics::{Diagnostic, DiagnosticKind, DiagnosticLevel};
pub use ir::{EntityIR, EntityId, FactIR, FactId, KnowledgeIR, ProvenanceIR, RelationIR};
pub use pass::{
    CanonicalEntityResolutionPass, CompilerContext, CompilerPass, FactDeduplicationPass,
    ObservationNormalizationPass, ValidationPass,
};

use brain_integrations::dto::v1::{DiagnosticDto, KnowledgeCompilationReport};
use std::sync::Arc;
use std::time::Instant;

/// Registry and executor for ordered compiler passes.
pub struct PassManager {
    passes: Vec<Box<dyn CompilerPass>>,
}

impl PassManager {
    /// Creates a new `PassManager` with standard default compiler passes.
    pub fn default_pipeline() -> Self {
        let mut manager = Self { passes: Vec::new() };
        manager.register(Box::new(ObservationNormalizationPass));
        manager.register(Box::new(CanonicalEntityResolutionPass));
        manager.register(Box::new(FactDeduplicationPass));
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
    ) -> (usize, Vec<Diagnostic>, Vec<String>) {
        let mut all_diagnostics = Vec::new();
        let mut details = Vec::new();

        for pass in &self.passes {
            let start = Instant::now();
            let pass_diags = pass.run(ctx, ir);
            let elapsed = start.elapsed().as_millis();
            details.push(format!(
                "Pass '{}' executed in {} ms (emitted {} diagnostics)",
                pass.name(),
                elapsed,
                pass_diags.len()
            ));
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

        (self.passes.len(), all_diagnostics, details)
    }
}

/// Central composition root for the Knowledge Compiler.
pub struct KnowledgeCompiler {
    pass_manager: Arc<PassManager>,
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
        }
    }

    /// Creates a custom compiler with specific pass manager pipeline.
    pub fn with_pipeline(pass_manager: PassManager) -> Self {
        Self {
            pass_manager: Arc::new(pass_manager),
        }
    }

    /// Compiles raw Knowledge IR into canonical knowledge, emitting diagnostics and an immutable compilation report.
    pub fn compile(
        &self,
        ctx: &CompilerContext,
        ir: &mut KnowledgeIR,
    ) -> (KnowledgeIR, KnowledgeCompilationReport) {
        let start = Instant::now();
        let (passes_executed, diagnostics, mut details) = self.pass_manager.execute(ctx, ir);
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

        (ir.clone(), report)
    }
}
