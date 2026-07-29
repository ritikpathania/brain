//! Reserved Checkpoint Migration Boundary (`CheckpointMigrator`) (Phase 8 Milestone 8.4).
//!
//! ### Architectural Invariants:
//! 1. Decoupled Migration: `CheckpointMigrator` is completely decoupled from replay engines and supervisors.
//! 2. Schema Evolution: Upgrades `ExecutionCheckpoint` artifacts to `CURRENT_SCHEMA_VERSION` before restoration.

use crate::planning::supervision::{ExecutionCheckpoint, SupervisionError};

/// Trait defining checkpoint schema migration capabilities.
pub trait CheckpointMigrator: Send + Sync {
    /// Migrates an `ExecutionCheckpoint` artifact to `ExecutionCheckpoint::CURRENT_SCHEMA_VERSION`.
    fn migrate(
        &self,
        checkpoint: &ExecutionCheckpoint,
    ) -> Result<ExecutionCheckpoint, SupervisionError>;
}

/// Default reference implementation of `CheckpointMigrator`.
#[derive(Debug, Clone, Default)]
pub struct DefaultCheckpointMigrator;

impl CheckpointMigrator for DefaultCheckpointMigrator {
    fn migrate(
        &self,
        checkpoint: &ExecutionCheckpoint,
    ) -> Result<ExecutionCheckpoint, SupervisionError> {
        if checkpoint.schema_version > ExecutionCheckpoint::CURRENT_SCHEMA_VERSION {
            return Err(SupervisionError::CorruptedCheckpoint(format!(
                "Future schema_version {} cannot be migrated to current version {}",
                checkpoint.schema_version,
                ExecutionCheckpoint::CURRENT_SCHEMA_VERSION
            )));
        }

        // Current version requires no transformation
        Ok(checkpoint.clone())
    }
}
