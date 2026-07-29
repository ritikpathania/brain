//! Descriptive `WorkerRegistry` managing worker discovery, health status, and capability matching (Phase 8 Milestone 8.4).
//!
//! ### Architectural Invariants:
//! 1. Descriptive Scope: `WorkerRegistry` tracks worker discovery, health status, and capability flags ONLY; it is NOT a scheduler.
//! 2. Unique Worker IDs: `WorkerId` identifiers are globally unique; duplicate registrations return `WorkerRegistryError::DuplicateWorker`.
//! 3. Offline Worker Exclusion: Offline workers (`WorkerStatus::Offline`) are strictly excluded from capability matching.
//! 4. Deterministic Filtering: Capability matching is deterministic and independent of registration order.

use crate::planning::supervision::CheckpointCapabilitySet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Strongly-typed worker node identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkerId(pub Uuid);

impl std::fmt::Display for WorkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "worker_{}", self.0)
    }
}

/// Operational health and availability status of an execution worker node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum WorkerStatus {
    /// Worker active and available for execution.
    #[default]
    Active,
    /// Worker currently executing an active workload.
    Busy,
    /// Worker offline or unreachable.
    Offline,
}

/// Strongly-typed error classification for worker registry operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerRegistryError {
    /// Attempted to register a worker with a duplicate WorkerId.
    DuplicateWorker(WorkerId),
    /// Target WorkerId not found in registry.
    UnknownWorker(WorkerId),
    /// Illegal worker status transition.
    InvalidStatusTransition {
        /// Current status.
        from: String,
        /// Attempted status.
        to: String,
    },
}

impl std::fmt::Display for WorkerRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateWorker(id) => write!(f, "Worker '{}' already registered", id),
            Self::UnknownWorker(id) => write!(f, "Worker '{}' not found", id),
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "Invalid status transition from '{}' to '{}'", from, to)
            }
        }
    }
}

impl std::error::Error for WorkerRegistryError {}

/// Descriptive model of an execution worker node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionWorker {
    /// Unique worker node ID.
    pub worker_id: WorkerId,
    /// Descriptive name.
    pub name: String,
    /// Supported capability set.
    pub capabilities: CheckpointCapabilitySet,
    /// Current operational status.
    pub status: WorkerStatus,
}

/// Descriptive registry managing execution worker discovery, health status, and capability matching.
#[derive(Debug, Clone, Default)]
pub struct WorkerRegistry {
    workers: HashMap<WorkerId, ExecutionWorker>,
}

impl WorkerRegistry {
    /// Instantiates a new empty `WorkerRegistry`.
    pub fn new() -> Self {
        Self {
            workers: HashMap::new(),
        }
    }

    /// Registers a new `ExecutionWorker`. Returns `WorkerRegistryError::DuplicateWorker` if already registered.
    pub fn register_worker(&mut self, worker: ExecutionWorker) -> Result<(), WorkerRegistryError> {
        if self.workers.contains_key(&worker.worker_id) {
            return Err(WorkerRegistryError::DuplicateWorker(worker.worker_id));
        }

        self.workers.insert(worker.worker_id, worker);
        Ok(())
    }

    /// Updates the `WorkerStatus` of an existing registered worker.
    pub fn update_status(
        &mut self,
        worker_id: WorkerId,
        new_status: WorkerStatus,
    ) -> Result<(), WorkerRegistryError> {
        let worker = self
            .workers
            .get_mut(&worker_id)
            .ok_or(WorkerRegistryError::UnknownWorker(worker_id))?;

        worker.status = new_status;
        Ok(())
    }

    /// Retrieves an immutable reference to a registered worker by ID.
    pub fn get_worker(&self, worker_id: WorkerId) -> Option<&ExecutionWorker> {
        self.workers.get(&worker_id)
    }

    /// Returns all active/busy workers satisfying required capability flags (strictly excludes offline workers).
    pub fn find_capable_workers(
        &self,
        required: &CheckpointCapabilitySet,
    ) -> Vec<&ExecutionWorker> {
        let mut capable: Vec<&ExecutionWorker> = self
            .workers
            .values()
            .filter(|w| {
                if w.status == WorkerStatus::Offline {
                    return false;
                }

                let missing =
                    crate::planning::supervision_replay::CapabilityNegotiator::check_compatibility(
                        &w.capabilities,
                        required,
                    );

                matches!(
                    missing,
                    crate::planning::supervision_replay::CapabilityCompatibility::Compatible
                )
            })
            .collect();

        // Deterministic sorting by WorkerId
        capable.sort_by_key(|w| w.worker_id);
        capable
    }
}
