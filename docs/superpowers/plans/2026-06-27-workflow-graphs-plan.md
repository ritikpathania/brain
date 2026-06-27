# Workflow Graphs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transition the agent execution pipeline from a static linear sequence of stages to a Directed Acyclic Graph (DAG) executor driven by stage outcomes and stateless policies, verified by graph structure tests.

**Architecture:** We introduce `WorkflowGraph`, `WorkflowNode`, and a private `WorkflowGraphValidator` to validate topological and policy configurations. The `ExecutionRunner` is refactored to traverse nodes using `WorkflowExecutionState`.

**Tech Stack:** Rust (Tokio, standard library collections).

## Global Constraints
* Code must compile under `cargo clippy --all-targets -- -D warnings`.
* Every task must run unit and integration tests successfully.
* Follow the exact symbol signatures and file placements specified.

---

### Task 1: Core Graph, Policy, and Stage Trait Extensibility

**Files:**
- Create: `crates/brain-services/src/agent/graph.rs`
- Modify: `crates/brain-services/src/agent.rs`
- Modify: `crates/brain-services/src/agent/engine.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct PolicyContext<'a> {
      pub execution: &'a ExecutionContext,
      pub stage: StageIdentifier,
      pub attempts: usize,
      pub outcome: &'a StageOutcome,
  }

  pub trait NodeExecutionPolicy: Send + Sync {
      fn name(&self) -> &'static str;
      fn evaluate(&self, ctx: &PolicyContext<'_>) -> Result<PolicyDecision, BrainError>;
  }

  pub enum PolicyDecision {
      Continue,
      Fail { message: String },
  }

  pub struct RetryPolicy {
      pub max_attempts: usize,
  }

  pub struct WorkflowNode {
      pub stage: Box<dyn ExecutionStage>,
      pub next_stage: Option<StageIdentifier>,
      pub policies: Vec<Box<dyn NodeExecutionPolicy>>,
  }

  pub struct WorkflowGraph {
      pub nodes: HashMap<StageIdentifier, WorkflowNode>,
      pub start_node: StageIdentifier,
  }

  pub struct WorkflowExecutionState {
      pub current_stage: StageIdentifier,
      pub attempts: HashMap<StageIdentifier, usize>,
  }
  ```

- [ ] **Step 1: Extend ExecutionStage with supports_retry**
  Update `ExecutionStage` trait inside `crates/brain-services/src/agent/engine.rs` (around line 130):
  ```rust
  pub trait ExecutionStage: Send + Sync {
      fn name(&self) -> &'static str;
      fn id(&self) -> StageIdentifier;
      fn supports_retry(&self) -> bool {
          false
      }
      fn execute(
          &self,
          ctx: &ExecutionContext,
          state: &mut ExecutionState,
      ) -> Result<StageOutcome, BrainError>;
  }
  ```
  Override it in `ReflectionStage`:
  ```rust
  impl ExecutionStage for ReflectionStage {
      fn name(&self) -> &'static str { "Reflection" }
      fn id(&self) -> StageIdentifier { StageIdentifier::Reflection }
      fn supports_retry(&self) -> bool { true }
      // ...
  }
  ```

- [ ] **Step 2: Write the skeleton structures in graph.rs**
  Create `crates/brain-services/src/agent/graph.rs` and implement the basic types, traits, and `RetryPolicy`:
  ```rust
  use std::collections::HashMap;
  use brain_core::extensibility::BrainError;
  use crate::agent::{ExecutionContext, StageIdentifier, StageOutcome, ExecutionStage};

  pub struct PolicyContext<'a> {
      pub execution: &'a ExecutionContext,
      pub stage: StageIdentifier,
      pub attempts: usize,
      pub outcome: &'a StageOutcome,
  }

  pub trait NodeExecutionPolicy: Send + Sync {
      fn name(&self) -> &'static str;
      fn evaluate(&self, ctx: &PolicyContext<'_>) -> Result<PolicyDecision, BrainError>;
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PolicyDecision {
      Continue,
      Fail { message: String },
  }

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

  pub struct WorkflowNode {
      pub stage: Box<dyn ExecutionStage>,
      pub next_stage: Option<StageIdentifier>,
      pub policies: Vec<Box<dyn NodeExecutionPolicy>>,
  }

  pub struct WorkflowGraph {
      pub nodes: HashMap<StageIdentifier, WorkflowNode>,
      pub start_node: StageIdentifier,
  }

  pub struct WorkflowExecutionState {
      pub current_stage: StageIdentifier,
      pub attempts: HashMap<StageIdentifier, usize>,
  }
  ```

- [ ] **Step 3: Register the graph module in agent.rs**
  Expose the module in `crates/brain-services/src/agent.rs`:
  ```rust
  pub mod graph;
  ```

- [ ] **Step 4: Run compiler checks to verify it builds**
  Run: `cargo check --lib -p brain-services`
  Expected: Success.

- [ ] **Step 5: Commit changes**
  ```bash
  git add crates/brain-services/src/agent.rs crates/brain-services/src/agent/graph.rs crates/brain-services/src/agent/engine.rs
  git commit -m "feat: add core workflow graph, policy types, and stage capability checks"
  ```

---

### Task 2: Graph Builder & Private Validator

**Files:**
- Modify: `crates/brain-services/src/agent/graph.rs`
- Create: `crates/brain-services/tests/graph_tests.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct WorkflowGraphBuilder {
      start_node: Option<StageIdentifier>,
      nodes: HashMap<StageIdentifier, WorkflowNode>,
  }

  impl WorkflowGraphBuilder {
      pub fn new() -> Self;
      pub fn start_node(mut self, id: StageIdentifier) -> Self;
      pub fn node(mut self, id: StageIdentifier, node: WorkflowNode) -> Self;
      pub fn build(self) -> Result<WorkflowGraph, BrainError>;
  }
  ```

- [ ] **Step 1: Implement the Graph Builder and Graph Validator**
  Append `WorkflowGraphBuilder` and `WorkflowGraphValidator` to `crates/brain-services/src/agent/graph.rs`:
  ```rust
  pub struct WorkflowGraphBuilder {
      start_node: Option<StageIdentifier>,
      nodes: HashMap<StageIdentifier, WorkflowNode>,
  }

  impl WorkflowGraphBuilder {
      pub fn new() -> Self {
          Self {
              start_node: None,
              nodes: HashMap::new(),
          }
      }

      pub fn start_node(mut self, id: StageIdentifier) -> Self {
          self.start_node = Some(id);
          self
      }

      pub fn node(mut self, id: StageIdentifier, node: WorkflowNode) -> Self {
          self.nodes.insert(id, node);
          self
      }

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
  ```

- [ ] **Step 2: Write Builder & Validator unit tests**
  Create `crates/brain-services/tests/graph_tests.rs` verifying all invariants:
  ```rust
  use std::collections::HashMap;
  use brain_core::extensibility::BrainError;
  use brain_services::agent::{StageIdentifier, StageOutcome, ExecutionStage, ExecutionContext, ExecutionState};
  use brain_services::agent::graph::{WorkflowGraphBuilder, WorkflowNode, RetryPolicy};

  struct StubStage(StageIdentifier, bool);
  impl ExecutionStage for StubStage {
      fn name(&self) -> &'static str { "stub" }
      fn id(&self) -> StageIdentifier { self.0 }
      fn supports_retry(&self) -> bool { self.1 }
      fn execute(&self, _ctx: &ExecutionContext, _state: &mut ExecutionState) -> Result<StageOutcome, BrainError> {
          Ok(StageOutcome::Continue)
      }
  }

  #[test]
  fn test_valid_linear_graph() {
      let g = WorkflowGraphBuilder::new()
          .start_node(StageIdentifier::Planning)
          .node(StageIdentifier::Planning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Planning, false)),
              next_stage: Some(StageIdentifier::Reasoning),
              policies: vec![],
          })
          .node(StageIdentifier::Reasoning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Reasoning, false)),
              next_stage: None,
              policies: vec![],
          })
          .build();
      assert!(g.is_ok());
  }

  #[test]
  fn test_missing_start_node() {
      let g = WorkflowGraphBuilder::new()
          .node(StageIdentifier::Planning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Planning, false)),
              next_stage: None,
              policies: vec![],
          })
          .build();
      assert!(g.is_err());
  }

  #[test]
  fn test_dangling_edge() {
      let g = WorkflowGraphBuilder::new()
          .start_node(StageIdentifier::Planning)
          .node(StageIdentifier::Planning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Planning, false)),
              next_stage: Some(StageIdentifier::Reasoning),
              policies: vec![],
          })
          .build();
      assert!(g.is_err());
  }

  #[test]
  fn test_unreachable_node() {
      let g = WorkflowGraphBuilder::new()
          .start_node(StageIdentifier::Planning)
          .node(StageIdentifier::Planning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Planning, false)),
              next_stage: None,
              policies: vec![],
          })
          .node(StageIdentifier::Reasoning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Reasoning, false)),
              next_stage: None,
              policies: vec![],
          })
          .build();
      assert!(g.is_err());
  }

  #[test]
  fn test_sequential_cycle() {
      let g = WorkflowGraphBuilder::new()
          .start_node(StageIdentifier::Planning)
          .node(StageIdentifier::Planning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Planning, false)),
              next_stage: Some(StageIdentifier::Reasoning),
              policies: vec![],
          })
          .node(StageIdentifier::Reasoning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Reasoning, false)),
              next_stage: Some(StageIdentifier::Planning),
              policies: vec![],
          })
          .build();
      assert!(g.is_err());
  }

  #[test]
  fn test_reflection_lacks_retry_policy() {
      let g = WorkflowGraphBuilder::new()
          .start_node(StageIdentifier::Reflection)
          .node(StageIdentifier::Reflection, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Reflection, true)),
              next_stage: None,
              policies: vec![],
          })
          .build();
      assert!(g.is_err());
  }

  #[test]
  fn test_duplicate_policies() {
      let g = WorkflowGraphBuilder::new()
          .start_node(StageIdentifier::Reflection)
          .node(StageIdentifier::Reflection, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Reflection, true)),
              next_stage: None,
              policies: vec![
                  Box::new(RetryPolicy { max_attempts: 3 }),
                  Box::new(RetryPolicy { max_attempts: 2 }),
              ],
          })
          .build();
      assert!(g.is_err());
  }

  #[test]
  fn test_no_terminal_path() {
      let g = WorkflowGraphBuilder::new()
          .start_node(StageIdentifier::Planning)
          .node(StageIdentifier::Planning, WorkflowNode {
              stage: Box::new(StubStage(StageIdentifier::Planning, false)),
              next_stage: Some(StageIdentifier::Planning),
              policies: vec![],
          })
          .build();
      assert!(g.is_err());
  }
  ```

- [ ] **Step 3: Run the newly added validation tests**
  Run: `cargo test --test graph_tests`
  Expected: PASS.

- [ ] **Step 4: Commit changes**
  ```bash
  git add crates/brain-services/src/agent/graph.rs crates/brain-services/tests/graph_tests.rs
  git commit -m "test: implement WorkflowGraphBuilder validation test matrix"
  ```

---

### Task 3: Refactor ExecutionRunner to Graph Traversal

**Files:**
- Modify: `crates/brain-services/src/agent/engine.rs`

**Interfaces:**
- Consumes:
  - `WorkflowGraph`
  - `WorkflowExecutionState`
  - `PolicyContext`
- Modify:
  ```rust
  pub struct ExecutionRunner {
      graph: WorkflowGraph,
  }

  impl ExecutionRunner {
      pub fn new(graph: WorkflowGraph) -> Self;
  }
  ```

- [ ] **Step 1: Refactor ExecutionRunner structure & constructor**
  Update `ExecutionRunner` to wrap `WorkflowGraph` instead of `Vec<Box<dyn ExecutionStage>>`:
  ```rust
  pub struct ExecutionRunner {
      graph: WorkflowGraph,
  }

  impl ExecutionRunner {
      /// Creates a new `ExecutionRunner` from a validated workflow graph.
      pub fn new(graph: WorkflowGraph) -> Self {
          Self { graph }
      }
  }
  ```

- [ ] **Step 2: Update run execution loop with graph traversal**
  Refactor `ExecutionRunner::run` to navigate stage nodes dynamically based on `StageOutcome`, matching all refinements:
  ```rust
      pub async fn run(&self, ctx: &ExecutionContext, state: &mut ExecutionState) -> Result<(), BrainError> {
          let mut exec_state = WorkflowExecutionState {
              current_stage: self.graph.start_node,
              attempts: HashMap::new(),
          };

          loop {
              let node = match self.graph.nodes.get(&exec_state.current_stage) {
                  Some(n) => n,
                  None => {
                      return Err(BrainError::Validation {
                          message: format!("Stage {:?} not found in graph during execution", exec_state.current_stage),
                      });
                  }
              };

              // Execute the stage
              let outcome = node.stage.execute(ctx, state).await?;

              // Increment retry counter ONLY on StageOutcome::Retry outcomes
              if let StageOutcome::Retry { .. } = &outcome {
                  let attempt_count = exec_state.attempts.entry(exec_state.current_stage).or_insert(0);
                  *attempt_count += 1;
              }

              // Retrieve attempt count (0 if no retries yet)
              let attempt_count = exec_state.attempts.get(&exec_state.current_stage).cloned().unwrap_or(0);

              // Evaluate policies in insertion order (short-circuiting on failure)
              let policy_ctx = PolicyContext {
                  execution: ctx,
                  stage: exec_state.current_stage,
                  attempts: attempt_count,
                  outcome: &outcome,
              };

              for policy in &node.policies {
                  if let PolicyDecision::Fail { message } = policy.evaluate(&policy_ctx)? {
                      return Err(BrainError::Validation { message });
                  }
              }

              // Process routing transitions driven directly by outcome and terminal semantics
              match outcome {
                  StageOutcome::Continue => {
                      // Reset retry counter on successful progress leaving the stage
                      exec_state.attempts.remove(&exec_state.current_stage);
                      if let Some(next) = node.next_stage {
                          exec_state.current_stage = next;
                      } else {
                          // Structural Termination (next_stage is None)
                          break;
                      }
                  }
                  StageOutcome::Retry { target, feedback } => {
                      // Prepend feedback for self-correction in state
                      state.feedback_prompt = Some(feedback);
                      exec_state.current_stage = target;
                  }
                  StageOutcome::Finish => {
                      // Behavioral Termination (halting early)
                      break;
                  }
                  StageOutcome::Cancelled => {
                      return Err(BrainError::Cancelled);
                  }
              }
          }

          Ok(())
      }
  ```

- [ ] **Step 3: Run compiler checks to verify structural changes**
  Run: `cargo check --lib -p brain-services`
  Expected: Only compiles error in tests and runtimes where `ExecutionRunner::new` is invoked.

- [ ] **Step 4: Commit changes**
  ```bash
  git add crates/brain-services/src/agent/engine.rs
  git commit -m "feat: refactor ExecutionRunner to perform graph traversal"
  ```

---

### Task 4: Graph Construction & Integration in Runtime

**Files:**
- Modify: `crates/brain-services/src/runtime.rs`
- Modify: `crates/brain-services/tests/agent_tests.rs`

- [ ] **Step 1: Update Runtime initialization to build the default graph**
  Modify `crates/brain-services/src/runtime.rs` to build and validate the default workflow graph matching the linear pipeline (Planning -> Retrieval -> Tool -> Reasoning -> Reflection -> Verification -> Commit):
  ```rust
  // Inside AgentExecutionEngine instantiation:
  let graph = WorkflowGraphBuilder::new()
      .start_node(StageIdentifier::Planning)
      .node(StageIdentifier::Planning, WorkflowNode {
          stage: Box::new(PlanningStage::new(planning_engine)),
          next_stage: Some(StageIdentifier::Retrieval),
          policies: vec![],
      })
      .node(StageIdentifier::Retrieval, WorkflowNode {
          stage: Box::new(RetrievalStage::new(retrieval_service)),
          next_stage: Some(StageIdentifier::Tool),
          policies: vec![],
      })
      .node(StageIdentifier::Tool, WorkflowNode {
          stage: Box::new(ToolStage::new(tool_executor)),
          next_stage: Some(StageIdentifier::Reasoning),
          policies: vec![],
      })
      .node(StageIdentifier::Reasoning, WorkflowNode {
          stage: Box::new(ReasoningStage::new(chat_model)),
          next_stage: Some(StageIdentifier::Reflection),
          policies: vec![],
      })
      .node(StageIdentifier::Reflection, WorkflowNode {
          stage: Box::new(ReflectionStage::new(reflection_engine)),
          next_stage: Some(StageIdentifier::Verification),
          policies: vec![Box::new(RetryPolicy { max_attempts: 3 })],
      })
      .node(StageIdentifier::Verification, WorkflowNode {
          stage: Box::new(VerificationStage::new(verification_engine)),
          next_stage: Some(StageIdentifier::Commit),
          policies: vec![],
      })
      .node(StageIdentifier::Commit, WorkflowNode {
          stage: Box::new(CommitStage::new(conversation_manager)),
          next_stage: None,
          policies: vec![],
      })
      .build()?;

  let runner = ExecutionRunner::new(graph);
  ```

- [ ] **Step 2: Update existing agent tests to build mock graphs**
  Modify `crates/brain-services/tests/agent_tests.rs` to build custom validated mock graphs for testing:
  ```rust
  let graph = WorkflowGraphBuilder::new()
      .start_node(StageIdentifier::Planning)
      // register mock nodes
      .build()?;
  ```

- [ ] **Step 3: Run clippy and verify all workspace tests pass**
  Run: `cargo clippy --all-targets -- -D warnings`
  Run: `cargo test`
  Expected: PASS (106 tests passing, zero warnings).

- [ ] **Step 4: Commit and finalize branch**
  ```bash
  git add crates/brain-services/src/runtime.rs crates/brain-services/tests/agent_tests.rs
  git commit -m "feat: integrate workflow graph assembly into Agent runtime and verify tests"
  ```
