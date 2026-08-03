//! Integration Test Suite for Pluggable `ReadPolicy` Strategies (`LeasePriorityPolicy`, `QuorumOnlyPolicy`, `ReadPolicyEvaluator`) (Phase 14 Milestone 14.4).

use brain_services::planning::{
    ConsensusEngine, ConsensusRole, InMemoryEventLog, LeaderLease, LeasePriorityPolicy,
    LeadershipEvent, LinearizableReadEngine, NodeId, QuorumOnlyPolicy, ReadConsistencyStrategy,
    ReadIndexRequest, ReadPlanner, ReadPolicyEvaluator, TermId,
};
use uuid::Uuid;

#[test]
fn test_lease_priority_policy_selects_valid_lease_and_falls_back_to_quorum() {
    let policy = LeasePriorityPolicy::new();
    let leader_id = NodeId(Uuid::new_v4());
    let req = ReadIndexRequest {
        read_id: Uuid::new_v4(),
        leader_id,
        term: TermId(2),
    };

    let valid_lease = LeaderLease {
        leader_id,
        term: TermId(2),
        granted_at_ms: 1000,
        lease_ttl_ms: 500,
    };

    // 1. Valid lease at now_ms = 1200 -> Selects LeaderLease strategy
    let strat1 = ReadPolicyEvaluator::evaluate_policy(&policy, &req, Some(&valid_lease), 1200);
    assert_eq!(strat1, ReadConsistencyStrategy::LeaderLease(valid_lease));

    // 2. Expired lease at now_ms = 1600 -> Falls back to ReadIndexQuorum
    let strat2 = ReadPolicyEvaluator::evaluate_policy(&policy, &req, Some(&valid_lease), 1600);
    assert_eq!(strat2, ReadConsistencyStrategy::ReadIndexQuorum);

    // 3. Missing lease -> Falls back to ReadIndexQuorum
    let strat3 = ReadPolicyEvaluator::evaluate_policy(&policy, &req, None, 1200);
    assert_eq!(strat3, ReadConsistencyStrategy::ReadIndexQuorum);
}

#[test]
fn test_quorum_only_policy_strictly_enforces_quorum_confirmation() {
    let policy = QuorumOnlyPolicy::new();
    let leader_id = NodeId(Uuid::new_v4());
    let req = ReadIndexRequest {
        read_id: Uuid::new_v4(),
        leader_id,
        term: TermId(2),
    };

    let valid_lease = LeaderLease {
        leader_id,
        term: TermId(2),
        granted_at_ms: 1000,
        lease_ttl_ms: 500,
    };

    // Always returns ReadIndexQuorum even when lease is valid!
    let strat = ReadPolicyEvaluator::evaluate_policy(&policy, &req, Some(&valid_lease), 1200);
    assert_eq!(strat, ReadConsistencyStrategy::ReadIndexQuorum);
}

#[test]
fn test_end_to_end_policy_driven_read_plan_compilation_and_execution() {
    let leader_id = NodeId(Uuid::new_v4());
    let engine = ConsensusEngine::new();
    engine.transition_to(ConsensusRole::Leader, TermId(3), Some(leader_id));

    let log = InMemoryEventLog::<LeadershipEvent>::new();
    let req = ReadIndexRequest {
        read_id: Uuid::new_v4(),
        leader_id,
        term: TermId(3),
    };

    let valid_lease = LeaderLease {
        leader_id,
        term: TermId(3),
        granted_at_ms: 5000,
        lease_ttl_ms: 1000,
    };

    let policy = LeasePriorityPolicy::new();
    let strategy = ReadPolicyEvaluator::evaluate_policy(&policy, &req, Some(&valid_lease), 5200);

    let plan = ReadPlanner::plan_read(&req, &log, strategy);
    let result = LinearizableReadEngine::execute_read_plan(&engine, &plan, 5200);

    assert!(result.validation_result.is_success());
}
