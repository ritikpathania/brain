//! Execution Context primitives for the Knowledge Compiler.

use brain_domain::SessionId;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Read-only execution context supplied to compiler passes and compilation requests.
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
    /// Configurable parameters for optimization passes and retention policies.
    pub config: crate::compiler::config::CompilerOptimizationConfig,
}

impl CompilerContext {
    /// Instantiates a default `CompilerContext` for a given session.
    pub fn for_session(session_id: SessionId) -> Self {
        Self {
            compilation_id: Uuid::new_v4(),
            session_id,
            graph_version: 1,
            dirty_set: None,
            min_confidence_threshold: 0.5,
            time_budget_ms: 5000,
            cancellation_token: CancellationToken::new(),
            config: crate::compiler::config::CompilerOptimizationConfig::default(),
        }
    }
}
