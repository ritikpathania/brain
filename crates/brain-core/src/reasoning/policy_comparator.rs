//! Experimental PolicyComparisonFramework comparing execution reports across policy configurations over datasets.

use crate::reasoning::runtime_facade::ReasoningRuntime;
use brain_domain::{DomainError, EvaluationDataset, RuntimeExecutionReport, RuntimePolicySet};
use std::sync::Arc;

/// Comparative summary outcome for a single scenario evaluated under two policies.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PolicyComparisonResult {
    /// Scenario name.
    pub scenario_name: String,
    /// Report produced under Policy A.
    pub report_a: RuntimeExecutionReport,
    /// Report produced under Policy B.
    pub report_b: RuntimeExecutionReport,
    /// Boolean indicating whether decisions diverged between policies.
    pub decisions_diverged: bool,
}

/// Evaluation tool for policy comparison and regression analysis.
///
/// Invariants:
/// - PolicyComparisonFramework is an evaluation tool layered on top of `ReasoningRuntime`.
#[derive(Debug, Clone)]
pub struct PolicyComparisonFramework {
    runtime: Arc<ReasoningRuntime>,
}

impl PolicyComparisonFramework {
    /// Instantiates a new `PolicyComparisonFramework` using a `ReasoningRuntime`.
    pub fn new(runtime: Arc<ReasoningRuntime>) -> Self {
        Self { runtime }
    }

    /// Evaluates an `EvaluationDataset` across two policy configurations.
    pub async fn compare_dataset(
        &self,
        dataset: &EvaluationDataset,
        policy_a: &RuntimePolicySet,
        policy_b: &RuntimePolicySet,
    ) -> Result<Vec<PolicyComparisonResult>, DomainError> {
        let mut results = Vec::new();

        for scenario in &dataset.scenarios {
            let report_a = self
                .runtime
                .run_cycle_with_policy(&scenario.context, &scenario.query, policy_a.clone())
                .await?;

            let report_b = self
                .runtime
                .run_cycle_with_policy(&scenario.context, &scenario.query, policy_b.clone())
                .await?;

            let decisions_diverged =
                report_a.policy_set.policy_version != report_b.policy_set.policy_version;

            results.push(PolicyComparisonResult {
                scenario_name: scenario.name.clone(),
                report_a,
                report_b,
                decisions_diverged,
            });
        }

        Ok(results)
    }
}
