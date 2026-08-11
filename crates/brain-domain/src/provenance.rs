//! Canonical three-dimensional execution provenance: RuntimeProvenance.

use crate::execution::ExecutionId;
use crate::replay::RuntimeReplaySnapshot;
use crate::telemetry::RuntimePolicySet;
use crate::version::RuntimeSchemaVersion;

/// Immutable canonical three-dimensional execution provenance value object.
///
/// Invariants:
/// - Every observable runtime output must be attributable to a specific execution (`ExecutionId`), policy configuration (`RuntimePolicySet`), and replay context (`RuntimeReplaySnapshot`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RuntimeProvenance {
    /// Execution run ID (Execution Provenance).
    pub execution_id: ExecutionId,
    /// Policy configuration recorded at execution start (Configuration Provenance).
    pub policy_set: RuntimePolicySet,
    /// Optional replay snapshot (Replay Provenance).
    pub replay_snapshot: Option<RuntimeReplaySnapshot>,
    /// Schema version contract identifier.
    pub schema_version: RuntimeSchemaVersion,
}

impl RuntimeProvenance {
    /// Instantiates a new `RuntimeProvenance`.
    pub fn new(
        execution_id: ExecutionId,
        policy_set: RuntimePolicySet,
        replay_snapshot: Option<RuntimeReplaySnapshot>,
    ) -> Self {
        Self {
            execution_id,
            policy_set,
            replay_snapshot,
            schema_version: RuntimeSchemaVersion::CURRENT,
        }
    }
}
