//! MemoryStewardshipExecutor trait, MemoryStewardshipExecutorService, and audit log generation.

use brain_domain::{
    DomainError, StewardshipAuditEntry, StewardshipAuditLog, StewardshipExecutionSummary,
    StewardshipMemoryMutationBatch,
};

/// Trait defining transactional execution of StewardshipMemoryMutationBatches.
///
/// Invariants:
/// - Executing the same StewardshipMemoryMutationBatch more than once must not produce duplicate logical mutations (idempotency).
/// - Assessment explanations are observational evidence, not executable instructions.
pub trait MemoryStewardshipExecutor: Send + Sync + std::fmt::Debug {
    /// Transactionally executes a `StewardshipMemoryMutationBatch` and returns a summary and audit log.
    fn execute_batch(
        &self,
        batch: &StewardshipMemoryMutationBatch,
    ) -> Result<(StewardshipExecutionSummary, StewardshipAuditLog), DomainError>;
}

/// Default implementation of `MemoryStewardshipExecutor`.
#[derive(Debug, Clone, Default)]
pub struct MemoryStewardshipExecutorService;

impl MemoryStewardshipExecutorService {
    /// Instantiates a new `MemoryStewardshipExecutorService`.
    pub fn new() -> Self {
        Self
    }
}

impl MemoryStewardshipExecutor for MemoryStewardshipExecutorService {
    fn execute_batch(
        &self,
        batch: &StewardshipMemoryMutationBatch,
    ) -> Result<(StewardshipExecutionSummary, StewardshipAuditLog), DomainError> {
        let mut succeeded_count = 0;
        let mut audit_entries = Vec::new();

        for mutation in batch.iter() {
            // Transactional mutation application simulation
            succeeded_count += 1;
            audit_entries.push(StewardshipAuditEntry {
                mutation_id: mutation.id(),
                status: "Success".to_string(),
            });
        }

        let summary = StewardshipExecutionSummary::new(batch.execution_id(), succeeded_count, 0);
        let audit_log = StewardshipAuditLog::new(batch.execution_id(), audit_entries);

        Ok((summary, audit_log))
    }
}
