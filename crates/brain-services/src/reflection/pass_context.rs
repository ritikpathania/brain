//! Reflection v2 context, diagnostic severity, outcome, and pass execution trait contracts.

use brain_domain::bkf::*;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

/// Execution context for a Reflection Engine v2 critique run.
#[derive(Debug, Clone)]
pub struct V2ReflectionContext {
    /// Wall-clock evaluation timestamp snapshot.
    pub now: Timestamp,
    /// Cancellation token to abort pass execution early.
    pub cancellation_token: CancellationToken,
    /// Operation budget limit to prevent unbounded rewrites.
    pub max_operations_budget: usize,
}

/// Diagnostic severity levels for pass findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    /// Informational telemetry message.
    Info,
    /// Non-fatal warning diagnostic.
    Warning,
    /// Critical diagnostic error.
    Error,
}

/// Diagnostic message emitted by a reflection pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassDiagnostic {
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Diagnostic code identifier.
    pub code: String,
    /// Human-readable diagnostic detail.
    pub message: String,
}

/// Outcome emitted by a successful reflection pass analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectionOutcome {
    /// The proposed declarative rewrite plan.
    pub plan: RewritePlan,
    /// Diagnostics produced during analysis.
    pub diagnostics: Vec<PassDiagnostic>,
}

/// Pure observational Reflection Pass interface for Reflection Engine v2.
pub trait V2ReflectionPass: Send + Sync {
    /// Returns the stable pass identifier.
    fn id(&self) -> PassId;

    /// Returns the pass identifiers this pass depends on (for topological execution DAG).
    fn dependencies(&self) -> &[PassId];

    /// Performs pure observational analysis on a read-only snapshot.
    fn analyze(
        &self,
        snapshot: &dyn KnowledgeSnapshotView,
        context: &V2ReflectionContext,
    ) -> Result<Option<ReflectionOutcome>, String>;
}
