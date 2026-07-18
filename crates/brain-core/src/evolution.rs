use crate::events::CorrelationId;
use brain_domain::{EpochId, NodeId};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Metadata tracking where the input originated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// Name or ID of the originating adapter.
    pub source_adapter: String,
    /// Time when the observation was captured.
    pub timestamp: SystemTime,
    /// Correlation tracing ID.
    pub correlation_id: CorrelationId,
}

/// Raw observations entering the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Unstructured payload content.
    pub payload: Vec<u8>,
    /// Format identifier (e.g. text/markdown, text/plain).
    pub media_type: String,
    /// Metadata provenance.
    pub provenance: Provenance,
}

/// Structural validation error (formatting/deserialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralValidationError {
    /// Invalid field name.
    pub field: String,
    /// Unique code identifying the format violation.
    pub error_code: String,
}

/// Semantic validation error (graph constraints/auth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticValidationError {
    /// Optional identifier of the affected entity.
    pub entity_id: Option<NodeId>,
    /// Constraint violation description.
    pub violation: String,
}

/// Ingestion boundary interface.
pub trait IngestionValidator: Send + Sync + 'static {
    /// Associated error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Validates raw inputs structurally.
    fn validate_structure(&self, obs: &Observation) -> Result<(), Vec<StructuralValidationError>>;

    /// Validates inputs against active graph invariants and security policies.
    fn validate_semantics(&self, obs: &Observation) -> Result<(), Vec<SemanticValidationError>>;
}

/// Opaque descriptor for emitted domain events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainEventDescriptor {
    /// Simple type identifier of the domain event.
    pub event_type: String,
    /// Creation timestamp.
    pub timestamp: SystemTime,
}

/// Unique reference to a target projection instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProjectionInstanceId(pub String);

/// Detailed stage execution timings.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct StageTimings {
    /// Duration of structural/semantic validation and graph canonicalization database transaction.
    pub canonicalization: std::time::Duration,
    /// Duration of post-canonicalization relationship reflection.
    pub reflection: std::time::Duration,
    /// Duration of projection invalidation event dispatching.
    pub dispatch: std::time::Duration,
}

/// Rich feedback context returned when canonicalization completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalizationResult {
    /// Monotonic epoch representing the new state of the graph.
    pub epoch: EpochId,
    /// List of durable domain events committed to the store.
    pub domain_events: Vec<DomainEventDescriptor>,
    /// Entities created or mutated during this transaction.
    pub affected_entities: Vec<NodeId>,
    /// Target projection instances invalidated.
    pub invalidated_projections: Vec<ProjectionInstanceId>,
    /// Detailed stage execution timings.
    #[serde(default)]
    pub stage_timings: StageTimings,
}

/// Core interface for mutating the canonical graph.
pub trait Canonicalizer: Send + Sync + 'static {
    /// Associated error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Normalizes, deduplicates, and commits the validated observation.
    /// Returns a comprehensive result payload.
    fn canonicalize(&self, obs: Observation) -> Result<CanonicalizationResult, Self::Error>;
}
