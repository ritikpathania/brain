use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use brain_core::errors::BrainError;
use brain_domain::{Node, NodeId, SessionId, NodeType};
use brain_session::StmNode;
use brain_services::conversation::{
    PromotionContext, PromotionDecision, PromotionPolicy, StmView, SessionMetadata,
    PromotionReason, RecencyPolicy, SemanticImportancePolicy, GoalAwarePolicy, UserPinnedPolicy,
    CompositePolicy, WeightedCompositePolicy, LogicalOp, ExactStringGoalMatcher, PropertyImportanceScorer,
};

// --- Mock STM View ---

struct MockStmView {
    nodes: Vec<StmNode>,
}

impl StmView for MockStmView {
    fn get_nodes(&self) -> Vec<StmNode> {
        self.nodes.clone()
    }
    fn len(&self) -> usize {
        self.nodes.len()
    }
}

fn make_stm_node(label: &str, properties: std::collections::HashMap<String, serde_json::Value>) -> StmNode {
    let node = Node::new(NodeId::new(), label.to_string(), NodeType::Concept);
    StmNode {
        node: node.with_properties(properties),
        epoch: brain_session::EpochId(0),
    }
}

// --- Eval Tracking Policy for Short-Circuit & Determinism Tests ---

struct EvalTrackingPolicy {
    eval_count: Arc<AtomicUsize>,
    result: bool,
    reason: PromotionReason,
}

impl EvalTrackingPolicy {
    fn new(eval_count: Arc<AtomicUsize>, result: bool, reason: PromotionReason) -> Self {
        Self { eval_count, result, reason }
    }
}

impl PromotionPolicy for EvalTrackingPolicy {
    fn should_promote(&self, _ctx: &PromotionContext<'_>) -> Result<PromotionDecision, BrainError> {
        self.eval_count.fetch_add(1, Ordering::SeqCst);
        Ok(PromotionDecision {
            promote: self.result,
            confidence: if self.result { 1.0 } else { 0.0 },
            reasons: if self.result { vec![self.reason] } else { vec![] },
        })
    }
}

// --- Test Cases ---

#[test]
fn test_recency_policy_count() {
    let policy = RecencyPolicy::new(Some(3), None);

    let session_id = SessionId::new();
    let metadata = SessionMetadata::default();
    let now = Instant::now();

    // 1. Below count threshold
    let stm_below = MockStmView {
        nodes: vec![
            make_stm_node("A", Default::default()),
            make_stm_node("B", Default::default()),
        ],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_below,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(!dec.promote);
    assert_eq!(dec.confidence, 0.0);
    assert!(dec.reasons.is_empty());

    // 2. Reached threshold
    let stm_reached = MockStmView {
        nodes: vec![
            make_stm_node("A", Default::default()),
            make_stm_node("B", Default::default()),
            make_stm_node("C", Default::default()),
        ],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_reached,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(dec.promote);
    assert_eq!(dec.confidence, 1.0);
    assert_eq!(dec.reasons, vec![PromotionReason::RecencyThreshold]);
}

#[test]
fn test_recency_policy_age() {
    let policy = RecencyPolicy::new(None, Some(10)); // 10 seconds age threshold

    let session_id = SessionId::new();
    let metadata = SessionMetadata::default();
    let now = Instant::now();

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // 1. Not old enough (updated just now)
    let mut node1 = make_stm_node("A", Default::default());
    node1.node.updated_at = now_unix;
    let stm_young = MockStmView { nodes: vec![node1] };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_young,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(!dec.promote);

    // 2. Old enough (updated 15 seconds ago)
    let mut node2 = make_stm_node("B", Default::default());
    node2.node.updated_at = now_unix - 15;
    let stm_old = MockStmView { nodes: vec![node2] };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_old,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(dec.promote);
    assert_eq!(dec.reasons, vec![PromotionReason::TimeThreshold]);
}

#[test]
fn test_importance_policy() {
    let scorer = Arc::new(PropertyImportanceScorer);
    let policy = SemanticImportancePolicy::new(7.0, false, scorer.clone());

    let session_id = SessionId::new();
    let metadata = SessionMetadata::default();
    let now = Instant::now();

    // 1. Below importance threshold
    let mut props1 = std::collections::HashMap::new();
    props1.insert("importance".to_string(), serde_json::json!(5.5));
    let stm_low = MockStmView {
        nodes: vec![make_stm_node("A", props1)],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_low,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(!dec.promote);
    assert_eq!(dec.confidence, 5.5 / 7.0);

    // 2. Reached importance threshold
    let mut props2 = std::collections::HashMap::new();
    props2.insert("importance".to_string(), serde_json::json!(8.0));
    let stm_high = MockStmView {
        nodes: vec![make_stm_node("B", props2)],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_high,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(dec.promote);
    assert_eq!(dec.confidence, 1.0); // capped at 1.0
    assert_eq!(dec.reasons, vec![PromotionReason::HighImportance]);
}

#[test]
fn test_goal_aware_policy() {
    let matcher = Arc::new(ExactStringGoalMatcher);
    let policy = GoalAwarePolicy::new(matcher);

    let session_id = SessionId::new();
    let metadata = SessionMetadata {
        active_goals: vec!["refactor".to_string()],
    };
    let now = Instant::now();

    // 1. No match
    let stm_no_match = MockStmView {
        nodes: vec![make_stm_node("setup database", Default::default())],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_no_match,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(!dec.promote);

    // 2. Goal match in label
    let stm_match = MockStmView {
        nodes: vec![make_stm_node("refactor auth module", Default::default())],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_match,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(dec.promote);
    assert_eq!(dec.reasons, vec![PromotionReason::GoalMatch]);
}

#[test]
fn test_user_pinned_policy() {
    let policy = UserPinnedPolicy::new();

    let session_id = SessionId::new();
    let metadata = SessionMetadata::default();
    let now = Instant::now();

    // 1. Unpinned
    let stm_unpinned = MockStmView {
        nodes: vec![make_stm_node("A", Default::default())],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_unpinned,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(!dec.promote);

    // 2. Pinned via property
    let mut props = std::collections::HashMap::new();
    props.insert("pinned".to_string(), serde_json::json!(true));
    let stm_pinned = MockStmView {
        nodes: vec![make_stm_node("B", props)],
    };
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm_pinned,
        metadata: &metadata,
        now,
    };
    let dec = policy.should_promote(&ctx).unwrap();
    assert!(dec.promote);
    assert_eq!(dec.reasons, vec![PromotionReason::UserPinned]);
}

// --- Task 3: Regression & Composite Tests ---

#[test]
fn test_composite_policy_and_short_circuit() {
    let eval_count1 = Arc::new(AtomicUsize::new(0));
    let eval_count2 = Arc::new(AtomicUsize::new(0));

    let p1 = Arc::new(EvalTrackingPolicy::new(eval_count1.clone(), false, PromotionReason::RecencyThreshold));
    let p2 = Arc::new(EvalTrackingPolicy::new(eval_count2.clone(), true, PromotionReason::HighImportance));

    let composite = CompositePolicy::new(vec![p1, p2], LogicalOp::And);

    let session_id = SessionId::new();
    let stm = MockStmView { nodes: vec![] };
    let metadata = SessionMetadata::default();
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm,
        metadata: &metadata,
        now: Instant::now(),
    };

    let dec = composite.should_promote(&ctx).unwrap();
    assert!(!dec.promote);

    // Short-circuit check: AND stops on first false.
    // p1 returns false, so p2 should NOT have been evaluated.
    assert_eq!(eval_count1.load(Ordering::SeqCst), 1);
    assert_eq!(eval_count2.load(Ordering::SeqCst), 0);
}

#[test]
fn test_composite_policy_or_short_circuit() {
    let eval_count1 = Arc::new(AtomicUsize::new(0));
    let eval_count2 = Arc::new(AtomicUsize::new(0));

    let p1 = Arc::new(EvalTrackingPolicy::new(eval_count1.clone(), true, PromotionReason::RecencyThreshold));
    let p2 = Arc::new(EvalTrackingPolicy::new(eval_count2.clone(), false, PromotionReason::HighImportance));

    let composite = CompositePolicy::new(vec![p1, p2], LogicalOp::Or);

    let session_id = SessionId::new();
    let stm = MockStmView { nodes: vec![] };
    let metadata = SessionMetadata::default();
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm,
        metadata: &metadata,
        now: Instant::now(),
    };

    let dec = composite.should_promote(&ctx).unwrap();
    assert!(dec.promote);

    // Short-circuit check: OR stops on first true.
    // p1 returns true, so p2 should NOT have been evaluated.
    assert_eq!(eval_count1.load(Ordering::SeqCst), 1);
    assert_eq!(eval_count2.load(Ordering::SeqCst), 0);
    assert_eq!(dec.reasons, vec![PromotionReason::CompositeSatisfied, PromotionReason::RecencyThreshold]);
}

#[test]
fn test_weighted_composite_boundary() {
    let p1 = Arc::new(EvalTrackingPolicy::new(Arc::new(AtomicUsize::new(0)), true, PromotionReason::RecencyThreshold));
    let p2 = Arc::new(EvalTrackingPolicy::new(Arc::new(AtomicUsize::new(0)), false, PromotionReason::HighImportance));

    // Threshold is 5.0. p1 weight is 5.0. Accumulated matches = 5.0.
    // This tests the exact boundary (accumulated == threshold).
    let weighted = WeightedCompositePolicy::new(
        vec![
            (p1.clone(), 5.0),
            (p2.clone(), 3.0),
        ],
        5.0,
    );

    let session_id = SessionId::new();
    let stm = MockStmView { nodes: vec![] };
    let metadata = SessionMetadata::default();
    let ctx = PromotionContext {
        session_id: &session_id,
        stm: &stm,
        metadata: &metadata,
        now: Instant::now(),
    };

    let dec = weighted.should_promote(&ctx).unwrap();
    assert!(dec.promote);
    assert_eq!(dec.confidence, 5.0 / 8.0);
    assert_eq!(dec.reasons, vec![PromotionReason::WeightedThresholdExceeded, PromotionReason::RecencyThreshold]);
}

#[test]
fn test_determinism_invariant() {
    let p1 = Arc::new(RecencyPolicy::new(Some(2), None));
    let p2 = Arc::new(UserPinnedPolicy::new());
    let composite = CompositePolicy::new(vec![p1, p2], LogicalOp::Or);

    let session_id = SessionId::new();
    let mut props = std::collections::HashMap::new();
    props.insert("pinned".to_string(), serde_json::json!(true));
    let stm = MockStmView {
        nodes: vec![make_stm_node("A", props)],
    };
    let metadata = SessionMetadata::default();
    let now = Instant::now();

    let ctx1 = PromotionContext {
        session_id: &session_id,
        stm: &stm,
        metadata: &metadata,
        now,
    };
    let ctx2 = PromotionContext {
        session_id: &session_id,
        stm: &stm,
        metadata: &metadata,
        now,
    };

    // Assert that identical contexts produce identical promotion decisions
    let dec1 = composite.should_promote(&ctx1).unwrap();
    let dec2 = composite.should_promote(&ctx2).unwrap();
    assert_eq!(dec1, dec2);
}
