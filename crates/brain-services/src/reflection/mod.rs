use brain_core::errors::BrainError;
use brain_core::repositories::Storage;
use brain_domain::{ReflectionFinding, ReflectionPassId, SessionId};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Read-only snapshot context for reflection passes.
pub mod snapshot;
pub use snapshot::ReflectionSnapshot;

/// Execution context and constraints for a reflection run.
#[derive(Debug, Clone)]
pub struct ReflectionContext {
    /// Unique execution task ID.
    pub execution_id: Uuid,
    /// Active Session ID target for consolidation.
    pub session_id: SessionId,
    /// Cutoff epoch representing the historical window snapshot.
    pub cutoff_epoch: u64,
    /// Maximum number of nodes to load into memory for analysis.
    pub max_nodes: usize,
    /// Time budget in milliseconds.
    pub time_budget_ms: u64,
    /// Cancellation token to abort the task.
    pub cancellation_token: CancellationToken,
}

/// Trait defining a single self-reflection analysis pass.
pub trait ReflectionPass: Send + Sync {
    /// Returns the stable pass identifier.
    fn id(&self) -> ReflectionPassId;

    /// Returns the logical version of the pass.
    fn version(&self) -> u32;

    /// Runs the pass on an immutable graph snapshot.
    fn run(
        &self,
        snapshot: &ReflectionSnapshot,
        context: &ReflectionContext,
    ) -> Result<Vec<ReflectionFinding>, BrainError>;
}

/// Governs registered passes and their execution sequence.
#[derive(Default)]
pub struct ReflectionRegistry {
    passes: Vec<Box<dyn ReflectionPass>>,
}

impl ReflectionRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a reflection pass.
    pub fn register(&mut self, pass: Box<dyn ReflectionPass>) {
        self.passes.push(pass);
    }

    /// Returns a slice of all registered passes.
    pub fn passes(&self) -> &[Box<dyn ReflectionPass>] {
        &self.passes
    }
}

/// Coordinates read-only self-reflection cycles.
pub struct ReflectionEngine {
    registry: Arc<ReflectionRegistry>,
    storage: Arc<dyn Storage>,
}

impl ReflectionEngine {
    /// Creates a new `ReflectionEngine` using the specified registry and storage engine.
    pub fn new(registry: Arc<ReflectionRegistry>, storage: Arc<dyn Storage>) -> Self {
        Self { registry, storage }
    }

    /// Executes all registered passes over a read-only transaction snapshot.
    ///
    /// ### Pass Isolation & Atomicity
    /// If any single pass fails, the entire cycle fails atomically, ensuring no partial results are produced.
    pub fn reflect(
        &self,
        context: &ReflectionContext,
    ) -> Result<Vec<ReflectionFinding>, BrainError> {
        let mut findings = Vec::new();

        self.storage.run_transaction(&mut |tx| {
            let repositories = tx.repositories();
            let snapshot = ReflectionSnapshot::new(repositories);
            for pass in self.registry.passes() {
                if context.cancellation_token.is_cancelled() {
                    return Err(BrainError::Validation {
                        message: "Reflection aborted by cancellation token".to_string(),
                    });
                }
                let pass_findings = pass.run(&snapshot, context)?;
                findings.extend(pass_findings);
            }
            Ok(())
        })?;

        Ok(findings)
    }
}

/// Decision planner translating findings to commands.
pub mod planner;
pub use planner::ReflectionPlanner;

/// Command execution handler.
pub mod handler;
pub use handler::ReflectionCommandHandler;

/// Analysis passes for graph inspection.
pub mod passes;

/// Background scheduler and execution task.
pub mod scheduler;
pub use scheduler::BackgroundReflectionScheduler;
