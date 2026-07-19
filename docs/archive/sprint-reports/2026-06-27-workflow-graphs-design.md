# Workflow Graphs Design Specification

This specification details the architecture for transitioning the linear stage runner inside the `brain` execution engine into a Directed Acyclic Graph (DAG) executor supporting conditional loop-backs and extensible node execution policies.

## 1. Unified Node Execution Policies

We decouple execution constraints (retries, timeouts, limits) from the structural routing of the graph. Execution policies are stateless, thread-safe, and immutable configuration objects.

```rust
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

pub enum PolicyDecision {
    /// Proceed with execution.
    Continue,
    /// Abort execution with a validation error.
    Fail { message: String },
}
```

### Invariants:
- **Deterministic Policy Ordering**: Node execution policies are evaluated sequentially in their insertion order within the `WorkflowNode`.
- **Short-Circuiting**: Evaluation terminates immediately on the first `PolicyDecision::Fail` returned by a policy.

### Concrete Policy: `RetryPolicy`
Tracks execution attempts. If a stage returns `StageOutcome::Retry` and the node-level retry count exceeds the configured limit, it overrides the outcome and returns a validation error:
```rust
pub struct RetryPolicy {
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
```

---

## 2. Declarative Graph vs. Mutable Runtime State

All mutable execution details (attempt counters, current node pointer) are fully isolated from the immutable graph structure.

```rust
/// Declarative, immutable graph representing the workflow structure.
pub struct WorkflowGraph {
    /// Maps each stage identifier to its node configuration.
    pub nodes: HashMap<StageIdentifier, WorkflowNode>,
    /// The entry point of the workflow graph.
    pub start_node: StageIdentifier,
}

/// An individual node in the graph.
pub struct WorkflowNode {
    /// The stage logic to run.
    pub stage: Box<dyn ExecutionStage>,
    /// Next stage to run on Success/Continue. None indicates terminal node.
    pub next_stage: Option<StageIdentifier>,
    /// Extensible list of execution policies.
    pub policies: Vec<Box<dyn NodeExecutionPolicy>>,
}

/// Mutable state tracking the runtime execution flow.
pub struct WorkflowExecutionState {
    /// The currently active stage.
    pub current_stage: StageIdentifier,
    /// Map tracking execution attempt counts per stage identifier.
    pub attempts: HashMap<StageIdentifier, usize>,
}
```

---

## 3. Direct Stage Outcome & Terminal Semantics

Outcomes directly drive the runner's transition logic:
- **`StageOutcome::Continue`**: Transitions to `node.next_stage`.
- **`StageOutcome::Retry { target, feedback }`**: Transitions to the symbolic `target` (e.g. `StageIdentifier::Reasoning`), incrementing the retry attempt counter for the current node.
- **`StageOutcome::Cancelled`**: Aborts execution and returns a cancelled status.

### Terminal Paths:
We distinguish terminal paths behaviorally and structurally:
- **Structural Termination**: `next_stage == None`. The workflow has naturally reached its final node in the graph topology and terminates successfully.
- **Behavioral Termination**: `StageOutcome::Finish`. A stage intentionally elects to stop execution early regardless of graph topology (e.g., verifying a safe, final state without needing further stages).

---

## 4. Graph Construction & Private Validator

To enforce clean separation of concerns, the validation logic is isolated inside a private `WorkflowGraphValidator` collaborator.

```
WorkflowGraphBuilder
        │ (build)
        ▼
WorkflowGraphValidator::validate(...)
        │ (success)
        ▼
Immutable WorkflowGraph
```

### Validator Invariants:
1. **Entry Point Integrity**: Exactly one `start_node` is defined and exists in the node map.
2. **Referential Integrity**: Every `next_stage` target or retry `target` referenced in any node actually exists in the graph.
3. **No Unreachable Nodes**: Verifies that every registered node is reachable from the `start_node`.
4. **Terminal Completeness**: At least one node can terminate (either by having `next_stage: None` or returning `StageOutcome::Finish`).
5. **No Infinite Implicit Cycles**: Validates that the graph is a Directed Acyclic Graph (DAG) for all sequential `next_stage` transitions. Backwards loopback transitions are only permitted via explicit `StageOutcome::Retry` signals controlled by `RetryPolicy`.
6. **Policy Configuration Integrity**:
   - If a node is capable of returning `StageOutcome::Retry`, it **must** have a `RetryPolicy` configured.
   - Stage configurations must contain no duplicate policy types.
