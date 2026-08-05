//! Domain models for Stewardship Memory Mutations: MemoryMutationId, StewardshipMemoryMutation, StewardshipMemoryMutationPlan, StewardshipMemoryMutationBatch, StewardshipExecutionSummary, and StewardshipAuditLog.

use crate::candidate::KnowledgeCandidateId;
use crate::errors::DomainError;
use crate::evolution::DomainEntityId;
use crate::execution::ExecutionId;
use crate::value::StructuredValue;
use std::collections::BTreeMap;
use std::fmt;
use uuid::Uuid;

/// Strongly-typed identifier for a memory mutation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct MemoryMutationId(pub Uuid);

impl MemoryMutationId {
    /// Instantiates a new unique `MemoryMutationId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing Uuid.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for MemoryMutationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MemoryMutationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mut-{}", self.0.simple())
    }
}

/// Capability-oriented high-level domain operation for evolving memory state.
/// Invariants:
/// - Represents semantic capability intents (`CreateEntity`, `MergeEntity`, etc.).
/// - Contains zero low-level database or table concerns (`InsertRow`, `UpdateColumn`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StewardshipMemoryMutation {
    /// Create a new long-term entity from a candidate.
    CreateEntity {
        /// Unique mutation identifier.
        id: MemoryMutationId,
        /// Target new domain entity ID.
        target_id: DomainEntityId,
        /// Source knowledge candidate ID.
        candidate_id: KnowledgeCandidateId,
        /// Structured payload value.
        payload: StructuredValue,
    },
    /// Merge candidate into an existing memory entity.
    MergeEntity {
        /// Unique mutation identifier.
        id: MemoryMutationId,
        /// Target existing entity ID.
        target_id: DomainEntityId,
        /// Source knowledge candidate ID.
        candidate_id: KnowledgeCandidateId,
        /// Structured payload value.
        payload: StructuredValue,
    },
    /// Archive an outdated memory entity.
    ArchiveEntity {
        /// Unique mutation identifier.
        id: MemoryMutationId,
        /// Target entity ID to archive.
        target_id: DomainEntityId,
    },
    /// Reinforce relationship between source and target entities.
    ReinforceRelationship {
        /// Unique mutation identifier.
        id: MemoryMutationId,
        /// Source entity ID.
        source: DomainEntityId,
        /// Target entity ID.
        target: DomainEntityId,
    },
}

impl StewardshipMemoryMutation {
    /// Returns the unique `MemoryMutationId` of this mutation.
    pub fn id(&self) -> MemoryMutationId {
        match self {
            Self::CreateEntity { id, .. } => *id,
            Self::MergeEntity { id, .. } => *id,
            Self::ArchiveEntity { id, .. } => *id,
            Self::ReinforceRelationship { id, .. } => *id,
        }
    }
}

/// Declarative plan representing high-level intent prior to batch compilation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StewardshipMemoryMutationPlan {
    /// Target execution run ID.
    pub execution_id: ExecutionId,
    /// List of proposed mutations.
    pub proposed_mutations: Vec<StewardshipMemoryMutation>,
}

impl StewardshipMemoryMutationPlan {
    /// Instantiates a new `StewardshipMemoryMutationPlan`.
    pub fn new(
        execution_id: ExecutionId,
        proposed_mutations: Vec<StewardshipMemoryMutation>,
    ) -> Self {
        Self {
            execution_id,
            proposed_mutations,
        }
    }
}

/// Opaque, deterministically ordered batch of memory mutations ready for execution.
/// Invariants:
/// - Opaque storage via `BTreeMap<MemoryMutationId, StewardshipMemoryMutation>`.
/// - Guarantees uniqueness and deterministic execution order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StewardshipMemoryMutationBatch {
    execution_id: ExecutionId,
    mutations: BTreeMap<MemoryMutationId, StewardshipMemoryMutation>,
}

impl StewardshipMemoryMutationBatch {
    /// Instantiates a new empty `StewardshipMemoryMutationBatch`.
    pub fn new(execution_id: ExecutionId) -> Self {
        Self {
            execution_id,
            mutations: BTreeMap::new(),
        }
    }

    /// Compiles a `StewardshipMemoryMutationPlan` into an executable `StewardshipMemoryMutationBatch`.
    pub fn compile(plan: StewardshipMemoryMutationPlan) -> Result<Self, DomainError> {
        let mut batch = Self::new(plan.execution_id);
        for mutation in plan.proposed_mutations {
            batch.insert(mutation);
        }
        Ok(batch)
    }

    /// Inserts a mutation into the batch.
    pub fn insert(&mut self, mutation: StewardshipMemoryMutation) {
        self.mutations.insert(mutation.id(), mutation);
    }

    /// Returns target execution ID.
    pub fn execution_id(&self) -> ExecutionId {
        self.execution_id
    }

    /// Returns iterator over mutations in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = &StewardshipMemoryMutation> {
        self.mutations.values()
    }

    /// Returns number of mutations in batch.
    pub fn len(&self) -> usize {
        self.mutations.len()
    }

    /// Returns true if batch is empty.
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }
}

/// Public summary object returned to callers detailing batch execution results.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StewardshipExecutionSummary {
    /// Target execution ID.
    pub execution_id: ExecutionId,
    /// Total succeeded mutations count.
    pub succeeded_count: usize,
    /// Total failed mutations count.
    pub failed_count: usize,
}

impl StewardshipExecutionSummary {
    /// Instantiates a new `StewardshipExecutionSummary`.
    pub fn new(execution_id: ExecutionId, succeeded_count: usize, failed_count: usize) -> Self {
        Self {
            execution_id,
            succeeded_count,
            failed_count,
        }
    }
}

/// Individual item in audit log recording execution of a single mutation.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StewardshipAuditEntry {
    /// Target mutation ID.
    pub mutation_id: MemoryMutationId,
    /// Execution status string.
    pub status: String,
}

/// Persistent audit log recording detailed execution history for compliance and auditing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StewardshipAuditLog {
    /// Target execution run ID.
    pub execution_id: ExecutionId,
    /// Entries recorded during batch execution.
    pub entries: Vec<StewardshipAuditEntry>,
}

impl StewardshipAuditLog {
    /// Instantiates a new `StewardshipAuditLog`.
    pub fn new(execution_id: ExecutionId, entries: Vec<StewardshipAuditEntry>) -> Self {
        Self {
            execution_id,
            entries,
        }
    }
}
