//! Service coordinating memory consolidation lifecycle sweeps.

use brain_core::errors::BrainError;
use brain_domain::ConsolidationPolicy;
use brain_storage::SqliteStorage;
use std::sync::Arc;

/// Orchestrator service managing the memory consolidation lifecycle sweeps.
pub struct MemoryConsolidationService {
    storage: Arc<SqliteStorage>,
    policy: ConsolidationPolicy,
}

impl MemoryConsolidationService {
    /// Creates a new `MemoryConsolidationService`.
    pub fn new(storage: Arc<SqliteStorage>, policy: ConsolidationPolicy) -> Self {
        Self { storage, policy }
    }

    /// Evaluates and applies a memory consolidation lifecycle sweep transaction.
    pub fn run_consolidation_sweep(
        &self,
    ) -> Result<Vec<brain_domain::ConsolidationAction>, BrainError> {
        self.storage.consolidate_memories(self.policy)
    }
}
