//! Integration & Protocol Test Suite for `ReadPlanner`, `LeaderLeaseValidator`, and `LinearizableReadEngine` (Phase 13 Milestone 13.3).

use brain_services::planning::{
    ConsensusEngine, ConsensusRole, EventLog, InMemoryEventLog, LeaderLease, LeaderLeaseValidator,
    LeadershipEvent, LeadershipEventId, LeadershipEventKind, LinearizableReadEngine, NodeId,
    ReadConsistencyStrategy, ReadIndexRequest, ReadPlanKind, ReadPlanner, ReadValidationResult,
    SequenceNumber, TermId, LEADERSHIP_EVENT_SCHEMA_VERSION,
};
use uuid::Uuid;

#[test]
fn test_leader_lease_validator_time_and_term_bounds() {
    let leader_id = NodeId(Uuid::new_v4());
    let lease = LeaderLease {
        leader_id,
        term: TermId(2),
        granted_at_ms: 1000,
        lease_ttl_ms: 500, // Valid from 1000ms to 1500ms
    };

    // 1. Valid lease
    assert!(LeaderLeaseValidator::validate_lease(&lease, &leader_id, TermId(2), 1200).is_ok());

    // 2. Expired lease (now_ms >= 1500)
    let err =
        LeaderLeaseValidator::validate_lease(&lease, &leader_id, TermId(2), 1500).unwrap_err();
    assert_eq!(err, ReadValidationResult::LeaseExpired);

    // 3. Stale leader or term mismatch
    let other_node = NodeId(Uuid::new_v4());
    let err_node =
        LeaderLeaseValidator::validate_lease(&lease, &other_node, TermId(2), 1200).unwrap_err();
    assert_eq!(err_node, ReadValidationResult::StaleLeader);

    let err_term =
        LeaderLeaseValidator::validate_lease(&lease, &leader_id, TermId(3), 1200).unwrap_err();
    assert_eq!(err_term, ReadValidationResult::StaleLeader);
}

#[test]
fn test_linearizable_read_planner_and_execution_success() {
    let log = InMemoryEventLog::<LeadershipEvent>::new();
    let leader_id = NodeId(Uuid::new_v4());
    let engine = ConsensusEngine::new();
    engine.transition_to(ConsensusRole::Leader, TermId(2), Some(leader_id));

    // Append 3 entries to log
    for i in 1..=3 {
        let event = LeadershipEvent {
            schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
            event_id: LeadershipEventId(Uuid::new_v4()),
            kind: LeadershipEventKind::LeaderElectionStarted {
                candidates_count: 2,
            },
            timestamp_ms: 1000 + i,
        };
        log.append(event, 1000 + i, 1).unwrap();
    }

    let lease = LeaderLease {
        leader_id,
        term: TermId(2),
        granted_at_ms: 1000,
        lease_ttl_ms: 1000,
    };

    let req = ReadIndexRequest {
        read_id: Uuid::new_v4(),
        leader_id,
        term: TermId(2),
    };

    let plan = ReadPlanner::plan_read(&req, &log, ReadConsistencyStrategy::LeaderLease(lease));
    assert_eq!(plan.kind, ReadPlanKind::LeaseValidated);
    assert_eq!(plan.target_read_index, SequenceNumber(3));

    let resp = LinearizableReadEngine::execute_read_plan(&engine, &plan, 1500);
    assert_eq!(resp.validation_result, ReadValidationResult::LeaseValid);
    assert_eq!(resp.read_index, SequenceNumber(3));
    assert_eq!(resp.term, TermId(2));
}

#[test]
fn test_linearizable_read_stale_leader_and_expired_lease_rejection() {
    let log = InMemoryEventLog::<LeadershipEvent>::new();
    let leader_id = NodeId(Uuid::new_v4());
    let engine = ConsensusEngine::new();
    engine.transition_to(ConsensusRole::Leader, TermId(3), Some(leader_id)); // Current term = 3

    let event = LeadershipEvent {
        schema_version: LEADERSHIP_EVENT_SCHEMA_VERSION,
        event_id: LeadershipEventId(Uuid::new_v4()),
        kind: LeadershipEventKind::LeaderElectionStarted {
            candidates_count: 2,
        },
        timestamp_ms: 1000,
    };
    log.append(event, 1000, 1).unwrap();

    let lease = LeaderLease {
        leader_id,
        term: TermId(2), // Stale term lease = 2
        granted_at_ms: 1000,
        lease_ttl_ms: 500,
    };

    // Stale request term (term 2 vs engine term 3)
    let stale_req = ReadIndexRequest {
        read_id: Uuid::new_v4(),
        leader_id,
        term: TermId(2),
    };

    let stale_plan = ReadPlanner::plan_read(
        &stale_req,
        &log,
        ReadConsistencyStrategy::LeaderLease(lease),
    );
    let stale_resp = LinearizableReadEngine::execute_read_plan(&engine, &stale_plan, 1200);

    assert_eq!(
        stale_resp.validation_result,
        ReadValidationResult::StaleLeader
    );
    assert_eq!(stale_resp.read_index, SequenceNumber(0));
}
