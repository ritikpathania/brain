//! Integration & Protocol Test Suite for Replicated Log Entries & Dynamic Cluster Configuration (Phase 12 Milestone 12.3).

use brain_services::planning::{
    AppendEntriesRejectReason, AppendEntriesRequest, AppendEntriesResponse, ConfigurationApplier,
    ConfigurationPlanner, ConfigurationTransition, ConfigurationVersion, LeadershipEvent,
    LogReplicationState, MembershipView, NodeId, SequenceNumber, TermId,
};
use uuid::Uuid;

#[test]
fn test_append_entries_request_heartbeat_and_payload_invariants() {
    let leader_id = NodeId(Uuid::new_v4());
    let req: AppendEntriesRequest<LeadershipEvent> = AppendEntriesRequest {
        term: TermId(2),
        leader_id,
        prev_log_index: SequenceNumber(10),
        prev_log_term: TermId(1),
        entries: vec![],
        leader_commit: SequenceNumber(10),
    };

    assert_eq!(req.term, TermId(2));
    assert_eq!(req.leader_id, leader_id);
    assert!(req.entries.is_empty()); // Heartbeat request

    let resp = AppendEntriesResponse {
        term: TermId(2),
        success: true,
        match_index: SequenceNumber(10),
        reject_reason: None,
    };
    assert!(resp.success);
    assert_eq!(resp.match_index, SequenceNumber(10));
}

#[test]
fn test_append_entries_stale_term_rejection_reason() {
    let resp = AppendEntriesResponse {
        term: TermId(3),
        success: false,
        match_index: SequenceNumber(0),
        reject_reason: Some(AppendEntriesRejectReason::StaleTerm),
    };

    assert!(!resp.success);
    assert_eq!(
        resp.reject_reason,
        Some(AppendEntriesRejectReason::StaleTerm)
    );
}

#[test]
fn test_log_replication_state_per_follower_index_tracking() {
    let last_index = SequenceNumber(15);
    let mut state = LogReplicationState::new(last_index);

    assert_eq!(state.next_index, SequenceNumber(16));
    assert_eq!(state.match_index, SequenceNumber(0));

    // Update replication index post-ack
    state.match_index = SequenceNumber(16);
    state.next_index = SequenceNumber(17);
    assert_eq!(state.match_index, SequenceNumber(16));
    assert_eq!(state.next_index, SequenceNumber(17));
}

#[test]
fn test_membership_view_and_configuration_planner_joint_transition() {
    let node1 = NodeId(Uuid::new_v4());
    let node2 = NodeId(Uuid::new_v4());
    let node3 = NodeId(Uuid::new_v4());

    let initial_view =
        MembershipView::new(ConfigurationVersion(1), vec![node1, node2], vec![node3]);

    assert_eq!(initial_view.version, ConfigurationVersion(1));
    assert!(initial_view.is_voter(&node1));
    assert!(initial_view.is_voter(&node2));
    assert!(!initial_view.is_voter(&node3));

    // Plan transition introducing node3 as a voter
    let transition =
        ConfigurationPlanner::plan_transition(&initial_view, vec![node1, node2, node3], vec![]);

    match &transition {
        ConfigurationTransition::Joint { old, new } => {
            assert_eq!(old.version, ConfigurationVersion(1));
            assert_eq!(new.version, ConfigurationVersion(2));
            assert_eq!(new.voters.len(), 3);
        }
        _ => panic!("Expected joint consensus configuration transition"),
    }

    // Finalize transition
    let final_view = ConfigurationApplier::finalize_transition(&transition);
    assert_eq!(final_view.version, ConfigurationVersion(3));
    assert_eq!(final_view.voters.len(), 3);
    assert!(final_view.is_voter(&node3));
}

#[test]
fn test_joint_consensus_dual_quorum_evaluation() {
    // C_old has 3 nodes (majority = 2), C_new has 5 nodes (majority = 3)

    // 1. Quorum satisfied in both C_old and C_new
    assert!(ConfigurationApplier::evaluate_joint_quorum(2, 3, 3, 5));

    // 2. Quorum satisfied in C_old but NOT C_new -> FALSE
    assert!(!ConfigurationApplier::evaluate_joint_quorum(2, 3, 2, 5));

    // 3. Quorum NOT satisfied in C_old -> FALSE
    assert!(!ConfigurationApplier::evaluate_joint_quorum(1, 3, 3, 5));
}
