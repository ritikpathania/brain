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

/// Builder for constructing and validating a `WorkflowGraph`.
pub struct WorkflowGraphBuilder {
    start_node: Option<StageIdentifier>,
    nodes: HashMap<StageIdentifier, WorkflowNode>,
}

impl WorkflowGraphBuilder {
    /// Creates a new `WorkflowGraphBuilder`.
    pub fn new() -> Self {
        Self {
            start_node: None,
            nodes: HashMap::new(),
        }
    }

    /// Sets the starting entry stage of the workflow.
    pub fn start_node(mut self, id: StageIdentifier) -> Self {
        self.start_node = Some(id);
        self
    }

    /// Registers a stage node in the workflow graph.
    pub fn node(mut self, id: StageIdentifier, node: WorkflowNode) -> Self {
        self.nodes.insert(id, node);
        self
    }

    /// Builds and validates the `WorkflowGraph`.
    pub fn build(self) -> Result<WorkflowGraph, BrainError> {
        let graph = WorkflowGraph {
            nodes: self.nodes,
            start_node: self.start_node.ok_or_else(|| BrainError::Validation {
                message: "start_node not configured".to_string(),
            })?,
        };
        WorkflowGraphValidator::validate(&graph)?;
        Ok(graph)
      }
}

impl Default for WorkflowGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

struct WorkflowGraphValidator;

impl WorkflowGraphValidator {
    fn validate(graph: &WorkflowGraph) -> Result<(), BrainError> {
        // 1. Entry point integrity
        if !graph.nodes.contains_key(&graph.start_node) {
            return Err(BrainError::Validation {
                message: format!("start_node {:?} does not exist in graph", graph.start_node),
            });
        }

        // 2. Referential integrity & Cycle checks
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();
        Self::dfs(graph.start_node, graph, &mut visited, &mut rec_stack)?;

        // 3. Unreachable nodes check
        for node_id in graph.nodes.keys() {
            if !visited.contains(node_id) {
                return Err(BrainError::Validation {
                    message: format!("Unreachable node: {:?}", node_id),
                });
            }
        }

        // 4. Policies duplicate and retry safety checks
        for (id, node) in &graph.nodes {
            let mut seen_policies = std::collections::HashSet::new();
            for p in &node.policies {
                if !seen_policies.insert(p.name()) {
                    return Err(BrainError::Validation {
                        message: format!("Duplicate policy {:?} on node {:?}", p.name(), id),
                    });
                }
            }

            // Retry validation using dynamic ExecutionStage capability
            if node.stage.supports_retry() {
                if !seen_policies.contains("RetryPolicy") {
                    return Err(BrainError::Validation {
                        message: format!("Stage {:?} is capable of retrying but lacks a RetryPolicy", id),
                      });
                }
            }
        }

        // 5. Terminal completeness
        let mut has_terminal = false;
        for node in graph.nodes.values() {
            if node.next_stage.is_none() {
                has_terminal = true;
                break;
            }
        }
        if !has_terminal {
            return Err(BrainError::Validation {
                message: "Graph has no terminal path (no node with next_stage: None)".to_string(),
            });
        }

        Ok(())
    }

    fn dfs(
        curr: StageIdentifier,
        graph: &WorkflowGraph,
        visited: &mut std::collections::HashSet<StageIdentifier>,
        rec_stack: &mut std::collections::HashSet<StageIdentifier>,
    ) -> Result<(), BrainError> {
        visited.insert(curr);
        rec_stack.insert(curr);

        if let Some(node) = graph.nodes.get(&curr) {
            if let Some(next) = node.next_stage {
                if !graph.nodes.contains_key(&next) {
                    return Err(BrainError::Validation {
                        message: format!("Dangling edge: next_stage {:?} of {:?} does not exist", next, curr),
                    });
                }
                if rec_stack.contains(&next) {
                    return Err(BrainError::Validation {
                        message: format!("Sequential cycle detected involving {:?}", next),
                    });
                }
                Self::dfs(next, graph, visited, rec_stack)?;
            }
        }
        rec_stack.remove(&curr);
        Ok(())
    }
}
