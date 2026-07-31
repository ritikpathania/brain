# Adaptive Memory Policy Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement an adaptive, extensible, context-driven promotion policy engine determining when volatile STM memories are consolidated to long-term database storage.

**Architecture:** Refine the memory promotion pipeline by decoupling evaluation from execution via a dedicated `PromotionEngine`. Evaluate short-term memories (STM) through a capability-based context (`PromotionContext`) and abstract semantic scoring/goal matching into focused collaborator traits.

**Tech Stack:** Rust (`brain-services`, `brain-domain`, `brain-session`)

## Global Constraints
- **Pure Evaluator Invariant**: `PromotionEngine` is a pure evaluator. It performs no repository access, cache access, or other side effects. Context acquisition remains the responsibility of the caller (`ConversationManager`).
- **Preserve Behavior**: All existing session, agent, and storage database integration tests must pass cleanly.
- **Zero Warnings**: Compile under `cargo clippy --all-targets -- -D warnings`.
- **Confidence normalisation**:
  - `0.0` = definitely do not promote
  - `1.0` = definitely promote
  - Composite policies must normalize confidences into `[0.0, 1.0]`.
  - Confidence remains advisory; `promote: bool` is authoritative.
- **Reason Order & Merge**:
  - Combined reasons must be deduplicated.
  - Reason order is deterministic and matches the sequence of evaluation.
- **Linker Flags on MacOS**: Compilations/tests must pass in the workspace shell with standard python frameworks paths:
  ```bash
  RUSTFLAGS="-L /Library/Developer/CommandLineTools/Library/Frameworks/Python3.framework/Versions/3.9/lib" \
  DYLD_FRAMEWORK_PATH="/Library/Developer/CommandLineTools/Library/Frameworks" \
  PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo test
  ```

---

### Task 1: Core Traits & Decoupled Collaborators

**Files:**
- Modify: `crates/brain-services/src/conversation.rs`

**Interfaces:**
- Produces: `StmView` trait, `SessionMetadata` struct, `PromotionReason` enum, `PromotionDecision` struct, `PromotionContext` struct, `PromotionPolicy` trait.
- Produces: `GoalMatcher`, `ExactStringGoalMatcher`, `ImportanceScorer`, `PropertyImportanceScorer` structs.
- Produces: `PromotionEngine` trait and `PromotionEngineImpl` struct.

- [ ] **Step 1: Write the failing type compilation test**
  Write tests in a new module at the bottom of `conversation.rs` checking type compatibility.
- [ ] **Step 2: Declare Core Types & Traits**
  - Implement `StmView` on `SessionContext`:
    ```rust
    pub trait StmView {
        fn nodes(&self) -> &[StmNode];
        fn len(&self) -> usize;
        fn is_empty(&self) -> bool;
    }
    ```
  - Implement `PromotionReason` enum:
    ```rust
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum PromotionReason {
        RecencyThreshold,
        TimeThreshold,
        HighImportance,
        GoalMatch,
        UserPinned,
        CompositeSatisfied,
        WeightedThresholdExceeded,
    }
    ```
  - Implement `PromotionDecision` and `PromotionContext`:
    ```rust
    pub struct PromotionDecision {
        pub promote: bool,
        pub confidence: f32,
        pub reasons: Vec<PromotionReason>,
    }
    ```
  - Refine `PromotionPolicy` trait:
    ```rust
    pub trait PromotionPolicy: Send + Sync {
        fn should_promote(&self, ctx: &PromotionContext) -> Result<PromotionDecision, BrainError>;
    }
    ```
  - Implement `PromotionEngine` trait and `PromotionEngineImpl` delegating to root policy:
    ```rust
    pub trait PromotionEngine: Send + Sync {
        fn evaluate(&self, ctx: &PromotionContext) -> Result<PromotionDecision, BrainError>;
    }
    ```
  - Define `GoalMatcher` & `ImportanceScorer` traits and default struct implementations.
- [ ] **Step 3: Update `CountThresholdPromotionPolicy`**
  Update the existing count threshold policy to conform to the new trait signature.
- [ ] **Step 4: Verify Compilation**
  Run `cargo check` inside `crates/brain-services` to verify compilation.
- [ ] **Step 5: Commit**
  `git commit -am "PR-017: Core traits and context wrappers implemented"`

---

### Task 2: Policy Implementations

**Files:**
- Modify: `crates/brain-services/src/conversation.rs`

**Interfaces:**
- Consumes: Task 1 traits and context structures.
- Produces: `RecencyPolicy`, `SemanticImportancePolicy`, `GoalAwarePolicy`, `UserPinnedPolicy` structs.

- [ ] **Step 1: Implement RecencyPolicy**
  - Support both `count_threshold` and `time_threshold_seconds`.
  - Calculate oldest node age relative to `ctx.now`.
- [ ] **Step 2: Implement SemanticImportancePolicy**
  - Injects `scorer: Arc<dyn ImportanceScorer>`.
  - Supports individual threshold matching or average score calculations.
- [ ] **Step 3: Implement GoalAwarePolicy**
  - Injects `matcher: Arc<dyn GoalMatcher>`.
  - Cross-references `ctx.metadata.active_goals`.
- [ ] **Step 4: Implement UserPinnedPolicy**
  - Looks for `"pinned": true` in the underlying node properties.
- [ ] **Step 5: Write unit tests**
  Write tests confirming each policy operates correctly on mock contexts.
- [ ] **Step 6: Run tests and commit**
  Run `cargo test` and commit.

---

### Task 3: Composite Policies & Microbenchmarks

**Files:**
- Modify: `crates/brain-services/src/conversation.rs`
- Create: `crates/brain-services/benches/policy_benchmarks.rs`

**Interfaces:**
- Consumes: Task 1 and 2 policies.
- Produces: `CompositePolicy`, `WeightedCompositePolicy` structs.

- [ ] **Step 1: Implement CompositePolicy**
  - Evaluate child policies sequentially with boolean operator (`And`, `Or`, `Not`).
  - Implement short-circuit logic: stop AND evaluation on first false, OR evaluation on first true.
  - Merge, deduplicate, and order reason lists deterministically based on evaluation sequence.
- [ ] **Step 2: Implement WeightedCompositePolicy**
  - Evaluate sub-policies, accumulate weights.
  - Trigger `promote = true` if total weight >= `threshold`.
  - Normalize confidence and preserve deterministic reason ordering.
- [ ] **Step 3: Write Regression Tests**
  - Test **Determinism**: Same context guarantees same output decision.
  - Test **Short-circuiting**: Assert child evaluations short-circuit.
  - Test **Weighted Boundary**: Assert behavior when accumulated score is exactly equal to the weight threshold.
- [ ] **Step 4: Write Criterion Microbenchmarks**
  - Target `RecencyPolicy`, `SemanticImportancePolicy`, `CompositePolicy`, and `WeightedCompositePolicy`.
  - Benchmark workload contexts with STM sizes of 16, 64, 256, and 1024 nodes.
- [ ] **Step 5: Run tests and benchmarks**
  Run tests and `cargo bench -p brain-services` to verify performance baselines, then commit.

---

### Task 4: Runtime Integration & Verification

**Files:**
- Modify: `crates/brain-services/src/conversation.rs`
- Modify: `crates/brain-services/src/runtime.rs`

- [ ] **Step 1: Update ConversationManager Ingestion**
  - Replace `promotion_policy` with `promotion_engine: Arc<dyn PromotionEngine>` in `ConversationManagerImpl`.
  - Update `ingest_interaction` to:
    - Load conversation log, parse active goals from key `"active_goals"` in `Conversation.metadata`.
    - Construct `SessionMetadata` and `PromotionContext`.
    - Evaluate policy decision via `promotion_engine.evaluate(&ctx)`, trigger `promote_memories` if `promote == true`.
- [ ] **Step 2: Update runtime initialization**
  - Initialize the engine's conversation manager with `PromotionEngineImpl` holding the default policies.
- [ ] **Step 3: Verify entire test suite**
  - Verify all unit and integration tests across the workspace.
- [ ] **Step 4: Commit and finalize**
  Commit work.
