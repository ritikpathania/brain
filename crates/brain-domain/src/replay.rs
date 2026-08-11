//! Minimal deterministic replay snapshot: RuntimeReplaySnapshot.

use crate::runtime_report::RuntimeContext;
use crate::version::RuntimeSchemaVersion;
use uuid::Uuid;

/// Minimal deterministic input required to replay a reasoning cycle.
/// Invariant: Contains minimum deterministic inputs; does not embed expected report payloads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeReplaySnapshot {
    /// Distributed trace ID.
    pub trace_id: Uuid,
    /// Runtime context.
    pub execution_context: RuntimeContext,
    /// Original user query string.
    pub query: String,
    /// Deterministic seed value.
    pub initial_seed: u64,
    /// Schema version contract identifier.
    pub schema_version: RuntimeSchemaVersion,
}

impl RuntimeReplaySnapshot {
    /// Instantiates a new `RuntimeReplaySnapshot`.
    pub fn new(
        trace_id: Uuid,
        execution_context: RuntimeContext,
        query: String,
        initial_seed: u64,
    ) -> Self {
        Self {
            trace_id,
            execution_context,
            query,
            initial_seed,
            schema_version: RuntimeSchemaVersion::CURRENT,
        }
    }
}
