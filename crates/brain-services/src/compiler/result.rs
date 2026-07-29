//! Decoupled CompilerResult output structure for the Knowledge Compiler.

use crate::compiler::delta::GraphDelta;
use crate::compiler::diagnostics::Diagnostic;
use brain_domain::DomainEvent;
use serde::{Deserialize, Serialize};

/// The decoupled result of knowledge compilation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerResult {
    /// Calculated graph mutations.
    pub graph_delta: GraphDelta,
    /// Emitted runtime domain events.
    pub events: Vec<DomainEvent>,
    /// Diagnostics emitted during compilation passes.
    pub diagnostics: Vec<Diagnostic>,
}

impl CompilerResult {
    /// Creates an empty `CompilerResult`.
    pub fn empty() -> Self {
        Self {
            graph_delta: GraphDelta::empty(),
            events: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Creates a new `CompilerResult` with the given delta, events, and diagnostics.
    pub fn new(
        graph_delta: GraphDelta,
        events: Vec<DomainEvent>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            graph_delta,
            events,
            diagnostics,
        }
    }

    /// Returns `true` if the compiler result contains no graph mutations.
    pub fn is_empty(&self) -> bool {
        self.graph_delta.is_empty() && self.events.is_empty() && self.diagnostics.is_empty()
    }
}
