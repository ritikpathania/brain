//! ReplayEngine verifying deterministic execution of RuntimeReplaySnapshots via ReasoningRuntime.

use crate::reasoning::runtime_facade::ReasoningRuntime;
use brain_domain::{DomainError, EvaluationMetrics, RuntimeExecutionReport, RuntimeReplaySnapshot};
use std::sync::Arc;

/// Automated replay engine for regression benchmarking and determinism verification.
///
/// Invariants:
/// - ReplayEngine depends strictly on the `ReasoningRuntime` façade; it does not rebuild or manually wire internal pipeline services.
/// - Replay execution must use the exact recorded `RuntimePolicySet` and `RuntimeSchemaVersion`; no current defaults may be substituted.
#[derive(Debug, Clone)]
pub struct ReplayEngine {
    runtime: Arc<ReasoningRuntime>,
}

impl ReplayEngine {
    /// Instantiates a new `ReplayEngine` using a `ReasoningRuntime` façade.
    pub fn new(runtime: Arc<ReasoningRuntime>) -> Self {
        Self { runtime }
    }

    /// Verifies deterministic replay of a `RuntimeReplaySnapshot` against a target report.
    pub async fn verify_replay(
        &self,
        snapshot: &RuntimeReplaySnapshot,
        target_report: &RuntimeExecutionReport,
    ) -> Result<EvaluationMetrics, DomainError> {
        // Enforce schema version compatibility check
        if !snapshot
            .schema_version
            .is_compatible_with(&target_report.provenance.schema_version)
        {
            return Err(DomainError::ValidationError {
                message: format!(
                    "Incompatible schema version: snapshot {}, report {}",
                    snapshot.schema_version, target_report.provenance.schema_version
                ),
                rule_id: Some("VAL-REPLAY-001".to_string()),
            });
        }

        // Replay using the exact recorded policy set from target_report
        let replayed_report = self
            .runtime
            .run_cycle_with_policy(
                &snapshot.execution_context,
                &snapshot.query,
                target_report.policy_set.clone(),
            )
            .await?;

        let is_match = replayed_report.execution_id == target_report.execution_id
            && replayed_report.session.stage == target_report.session.stage
            && replayed_report.policy_set == target_report.policy_set;

        let determinism_score = if is_match { 1.0 } else { 0.0 };

        Ok(EvaluationMetrics::new(is_match, determinism_score, 1))
    }
}
