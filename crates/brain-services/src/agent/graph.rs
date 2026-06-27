use std::collections::HashMap;
use brain_core::errors::BrainError;
use crate::agent::{ExecutionContext, StageIdentifier, StageOutcome};
use crate::agent::engine::ExecutionStage;

/// Context passed to node execution policies for evaluation.
pub struct PolicyContext<'a> {
    /// Context containing service facade handles and parameters.
    pub execution: &'a ExecutionContext,
    /// The symbolic identifier of the stage being evaluated.
    pub stage: StageIdentifier,
    /// Number of attempts executed so far for this stage.
    pub attempts: usize,
    /// The control-flow outcome returned by the stage.
    pub outcome: &'a StageOutcome,
}

/// Extensible policy interface for execution constraints.
pub trait NodeExecutionPolicy: Send + Sync {
    /// Returns the unique name/identifier of this policy.
    fn name(&self) -> &'static str;

    /// Evaluates the policy, returning a continue or fail decision.
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Result<PolicyDecision, BrainError>;
}

/// Decision returned by execution policy evaluations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Proceed with execution.
    Continue,
    /// Abort execution with a validation error.
    Fail {
        /// Reason description for the execution failure.
        message: String,
    },
}

/// Retry policy enforcing maximum self-correction iterations.
pub struct RetryPolicy {
    /// Maximum allowed self-correction loop attempts.
    pub max_attempts: usize,
}

impl NodeExecutionPolicy for RetryPolicy {
    fn name(&self) -> &'static str {
        "RetryPolicy"
    }

    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Result<PolicyDecision, BrainError> {
        if let StageOutcome::Retry { .. } = ctx.outcome {
            if ctx.attempts >= self.max_attempts {
                return Ok(PolicyDecision::Fail {
                    message: format!("Stage self-correction failed after {} attempts.", self.max_attempts),
                });
            }
        }
        Ok(PolicyDecision::Continue)
    }
}

/// An individual node in the workflow graph.
pub struct WorkflowNode {
    /// The stage logic to run.
    pub stage: Box<dyn ExecutionStage>,
    /// Next stage to run on Success/Continue. None indicates terminal node.
    pub next_stage: Option<StageIdentifier>,
    /// Extensible list of execution policies.
    pub policies: Vec<Box<dyn NodeExecutionPolicy>>,
}

/// Declarative, immutable graph representing the workflow structure.
pub struct WorkflowGraph {
    /// Maps each stage identifier to its node configuration.
    pub nodes: HashMap<StageIdentifier, WorkflowNode>,
    /// The entry point of the workflow graph.
    pub start_node: StageIdentifier,
}

/// Mutable state tracking the runtime execution flow.
pub struct WorkflowExecutionState {
    /// The currently active stage.
    pub current_stage: StageIdentifier,
    /// Map tracking execution attempt counts per stage identifier.
    pub attempts: HashMap<StageIdentifier, usize>,
}
