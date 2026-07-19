# Design Specification: Adaptive Memory Policy Engine (PR-017)

This design details the implementation of an extensible, behavioral memory policy engine for Standalone Relational Memory Engine (`brain`). It defines how short-term memories (STM) are evaluated and promoted to long-term storage (LTM) based on contextual scoring, user pinning, goal alignment, and logical combination rules.

---

## 1. Architectural Changes

We will introduce capability abstractions for evaluating session context and returning structured decisions.

### Trait, Context, and Decoupled View Definitions

```rust
use std::time::Instant;
use brain_domain::{Node, SessionId};

/// Read-only abstraction over volatile short-term memory (STM) cached state.
pub trait StmView {
    /// Returns a slice reference of the current short-term memory nodes.
    fn nodes(&self) -> &[StmNode];
    /// Returns the count of nodes currently residing in STM cache.
    fn len(&self) -> usize {
        self.nodes().len()
    }
    /// Returns true if STM cache is empty.
    fn is_empty(&self) -> bool {
        self.nodes().is_empty()
    }
}

/// Dynamic and contextual metadata for the session.
pub struct SessionMetadata {
    /// Active goals and objectives configured for the current session.
    pub active_goals: Vec<String>,
}

/// Structured decision containing detailed explanation telemetry for explainability.
#[derive(Debug, Clone, PartialEq)]
pub struct PromotionDecision {
    /// Whether the promotion action should trigger.
    pub promote: bool,
    /// Confidence score of the decision (0.0 to 1.0).
    pub confidence: f32,
    /// Detailed reasons and telemetry tag logs explaining the decision.
    pub reasons: Vec<String>,
}

/// Contextual capability wrapper passed to policies.
pub struct PromotionContext<'a> {
    /// The unique session identifier.
    pub session_id: &'a SessionId,
    /// Read-only slice view of STM.
    pub stm: &'a dyn StmView,
    /// Dynamic session metadata.
    pub metadata: &'a SessionMetadata,
    /// Monotonic timestamp for time-based evaluation.
    pub now: Instant,
}

/// Decoupled interface deciding when to promote STM memory to LTM.
pub trait PromotionPolicy: Send + Sync {
    /// Evaluates context capability signals and returns a promotion decision.
    fn should_promote(&self, ctx: &PromotionContext) -> Result<PromotionDecision, BrainError>;
}
```

---

## 2. Policy Concrete Implementations & Collaborators

We decouple semantic scoring and goal matching logic from the policies themselves by introducing focused collaborator traits:

### 1. Extensible Collaborators

#### `GoalMatcher`
```rust
pub trait GoalMatcher: Send + Sync {
    /// Returns true if the node is relevant to the target goal string.
    fn matches(&self, goal: &str, node: &StmNode) -> bool;
}

/// Default exact-string matches against label or properties["goal_tags"].
pub struct ExactStringGoalMatcher;
```

#### `ImportanceScorer`
```rust
pub trait ImportanceScorer: Send + Sync {
    /// Returns the semantic importance weight of the target node.
    fn score(&self, node: &Node) -> f32;
}

/// Default scorer reading properties["importance"] float values.
pub struct PropertyImportanceScorer;
```

### 2. Policies

#### A. `RecencyPolicy`
- **Behavior**: Promotes based on node counts or time elapsed since the oldest node's insertion relative to `ctx.now`.
- **Confidence**: Returns `1.0` if triggered, `0.0` otherwise.

#### B. `SemanticImportancePolicy`
- **Behavior**: Delegates score evaluation to `ImportanceScorer`. Triggers if individual or average importance scores exceed `min_importance`.
- **Confidence**: Maps to average/maximum matched score scaled to $[0, 1]$.

#### C. `GoalAwarePolicy`
- **Behavior**: Delegates goal evaluation to `GoalMatcher` comparing against `ctx.metadata.active_goals`.
- **Confidence**: Evaluates match presence.

#### D. `UserPinnedPolicy`
- **Behavior**: Checks if any node contains `"pinned": true` in properties. Triggers immediately on presence.

#### E. `CompositePolicy` (Boolean Orchestration)
- **Behavior**: Evaluates child policies and joins outcomes using logical operations (`And`, `Or`, `Not`).
- **Short-circuiting**:
  - `And`: Evaluates sequentially; stops and returns false on the first failure.
  - `Or`: Evaluates sequentially; stops and returns true on the first success.

#### F. `WeightedCompositePolicy` (Heuristic Scoring)
- **Behavior**: Assigns weights to sub-policies. Accumulates scores from passing policies. Triggers promotion if sum of weights exceeds `threshold`.
- **Confidence**: Scales accumulated weight relative to the sum of all sub-policy weights.

---

## 3. Metadata Mapping & Session State

- **Active Session Goals**: Loaded from the conversation log's metadata map (`Conversation.metadata`) under key `"active_goals"`, parsed as a comma-separated list, and passed to the evaluation context.
- **Node Metadata**: Custom parameters (e.g. `pinned`, `importance`, `goal_tags`) reside within the `properties` map on each ingested `Node`.

---

## 4. Verification Plan

### Automated Tests
- **Unit Tests (`conversation_tests.rs`)**:
  - `test_recency_policy`: Verify count/time triggers.
  - `test_importance_policy`: Test using customized scorer implementations.
  - `test_goal_aware_policy`: Validate exact string goal matcher checks.
  - `test_user_pinned_policy`: Test immediate trigger on pinned status.
  - `test_composite_boolean_policy`: Check combinations of OR/AND gates.
  - `test_weighted_composite_policy`: Validate weight thresholds.
- **Regression Tests**:
  - **Determinism**: Assert that identical `PromotionContext` feeds produce identical `PromotionDecision` structures.
  - **Composite Short-Circuit**: Verify that child evaluations cease as soon as the result is boolean-determined.
  - **Weighted Boundary**: Explicitly check threshold boundary cases (sum of weights exactly equal to trigger threshold).
