//! Transient CompilerState internal container for Knowledge Compiler pass transformations.

use crate::compiler::context::CompilerContext;
use crate::compiler::diagnostics::Diagnostic;
use crate::compiler::graph::CanonicalGraph;
use crate::compiler::ir::KnowledgeIR;
use crate::compiler::trace::CompilerTrace;

/// Transient working state container passed across compiler pass transformations.
#[derive(Debug, Clone)]
pub struct CompilerState {
    /// Execution context.
    pub ctx: CompilerContext,
    /// Initial Knowledge IR payload.
    pub ir_before: KnowledgeIR,
    /// Transformed target Knowledge IR payload.
    pub ir_after: KnowledgeIR,
    /// Canonical Graph state before compilation.
    pub graph_before: CanonicalGraph,
    /// Canonical Graph state after compilation.
    pub graph_after: CanonicalGraph,
    /// Emitted diagnostics across passes.
    pub diagnostics: Vec<Diagnostic>,
    /// Accumulated execution trace telemetry.
    pub trace: CompilerTrace,
}

impl CompilerState {
    /// Instantiates a new `CompilerState` for a given context and incoming target Knowledge IR payload.
    pub fn new(ctx: CompilerContext, ir: KnowledgeIR) -> Self {
        Self {
            ctx,
            ir_before: KnowledgeIR::new(),
            ir_after: ir,
            graph_before: CanonicalGraph::new(),
            graph_after: CanonicalGraph::new(),
            diagnostics: Vec::new(),
            trace: CompilerTrace::new(),
        }
    }

    /// Instantiates a `CompilerState` with an explicit initial base Knowledge IR state.
    pub fn with_base_ir(
        ctx: CompilerContext,
        base_ir: KnowledgeIR,
        target_ir: KnowledgeIR,
    ) -> Self {
        let graph_before = CanonicalGraph::from_ir(&base_ir);
        Self {
            ctx,
            ir_before: base_ir,
            ir_after: target_ir,
            graph_before,
            graph_after: CanonicalGraph::new(),
            diagnostics: Vec::new(),
            trace: CompilerTrace::new(),
        }
    }

    /// Finalizes the canonical graph after pass execution.
    pub fn sync_graph_after(&mut self) {
        self.graph_after = CanonicalGraph::from_ir(&self.ir_after);
    }
}
