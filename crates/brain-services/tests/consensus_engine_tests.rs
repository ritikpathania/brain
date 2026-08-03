//! Integration & Consensus Invariant Test Suite for `ConsensusEngine` & `RaftConsensusStrategy` (Phase 11 Milestone 11.2).

use brain_services::planning::{
    ClusterManager, ClusterNode, ClusterNodeRole, ClusterNodeStatus, ConsensusEngine,
    ConsensusError, ConsensusProtocol, ConsensusRole, ConsensusState, CoordinatorElectionEngine,
    NodeAddress, NodeId, QuorumEvaluator, RaftConsensusStrategy, TermId, VoteResult,
};
use uuid::Uuid;

#[test]
fn test_consensus_term_and_role_state_transitions() {
    let engine = ConsensusEngine::new();
    assert_eq!(
        engine.current_state(),
        ConsensusState {
            role: ConsensusRole::Follower,
            current_term: TermId(0),
            voted_for: None,
        }
    );

    let candidate_id = NodeId(Uuid::new_v4());
    engine.transition_to(ConsensusRole::Candidate, TermId(1), Some(candidate_id));
    assert_eq!(
        engine.current_state(),
        ConsensusState {
            role: ConsensusRole::Candidate,
            current_term: TermId(1),
            voted_for: Some(candidate_id),
        }
    );

    engine.transition_to(ConsensusRole::Leader, TermId(1), Some(candidate_id));
    assert_eq!(engine.current_state().role, ConsensusRole::Leader);
}

#[test]
fn test_consensus_vote_result_classification() {
    let engine = ConsensusEngine::new();
    let cand1 = NodeId(Uuid::new_v4());
    let cand2 = NodeId(Uuid::new_v4());

    // 1. Initial vote granted for higher term
    let res1 = engine.request_vote(TermId(1), cand1).unwrap();
    assert_eq!(res1, VoteResult::Granted);

    // 2. Stale term vote rejected
    let res_stale = engine.request_vote(TermId(0), cand2).unwrap();
    assert_eq!(res_stale, VoteResult::StaleTerm);

    // 3. Same term vote for another candidate rejected as AlreadyVoted
    let res_already = engine.request_vote(TermId(1), cand2).unwrap();
    assert_eq!(res_already, VoteResult::AlreadyVoted);

    // 4. Same candidate in same term succeeds
    let res_same = engine.request_vote(TermId(1), cand1).unwrap();
    assert_eq!(res_same, VoteResult::Granted);
}

#[test]
fn test_quorum_evaluator_majority_mathematics() {
    // 0 nodes -> false
    assert!(!QuorumEvaluator::evaluate_quorum(1, 0));

    // 1 node -> 1 vote required
    assert!(QuorumEvaluator::evaluate_quorum(1, 1));

    // 3 nodes -> 2 votes required (3/2 + 1 = 2)
    assert!(!QuorumEvaluator::evaluate_quorum(1, 3));
    assert!(QuorumEvaluator::evaluate_quorum(2, 3));
    assert!(QuorumEvaluator::evaluate_quorum(3, 3));

    // 5 nodes -> 3 votes required (5/2 + 1 = 3)
    assert!(!QuorumEvaluator::evaluate_quorum(2, 5));
    assert!(QuorumEvaluator::evaluate_quorum(3, 5));
}

#[test]
fn test_consensus_protocol_append_entries_unsupported_default() {
    let engine = ConsensusEngine::new();
    let leader_id = NodeId(Uuid::new_v4());
    let res = engine.append_entries_raw(TermId(1), leader_id);
    assert_eq!(res, Err(ConsensusError::Unsupported));
}

#[test]
fn test_raft_consensus_strategy_election_and_coordinator_integration() {
    let mut cluster = ClusterManager::new();
    let node_id = NodeId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    cluster.activate_node(node_id, 10500).unwrap();

    let mut election_engine =
        CoordinatorElectionEngine::new(Box::new(RaftConsensusStrategy::new()));
    let leader = election_engine.elect_leader(&mut cluster, 11000).unwrap();

    assert_eq!(leader.node_id, node_id);
    assert_eq!(leader.epoch.0, 1);
    assert_eq!(election_engine.events().len(), 2);
}
