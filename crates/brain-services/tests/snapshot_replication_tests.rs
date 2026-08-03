//! Integration & Protocol Test Suite for `InstallSnapshot` RPC, `SnapshotReplicationPlanner`, and `SnapshotRestoreEngine` (Phase 13 Milestone 13.2).

use brain_services::planning::{
    ConsensusEngine, JsonSnapshotCodec, LeadershipProjection, NodeId, SequenceNumber,
    SnapshotBuilder, SnapshotReplicationPlanner, SnapshotReplicator, SnapshotRestoreEngine,
    SnapshotTransferState, TermId,
};
use uuid::Uuid;

#[test]
fn test_snapshot_replication_planner_chunk_generation_and_offset_invariants() {
    let codec = JsonSnapshotCodec::<LeadershipProjection>::new();
    let mut proj = LeadershipProjection::new();
    proj.total_events = 100;
    proj.election_started_count = 50;

    let snapshot =
        SnapshotBuilder::build_snapshot(&proj, SequenceNumber(50), TermId(2), &codec, 1, 10000)
            .unwrap();

    let target_node = NodeId(Uuid::new_v4());
    let chunk_size = 16; // 16 bytes per chunk
    let plan = SnapshotReplicationPlanner::plan_transfer(&snapshot, target_node, chunk_size);

    assert_eq!(plan.target_node, target_node);
    assert!(plan.total_chunks > 1);

    // Verify Resumable Offset Invariant: offset(N+1) == offset(N) + bytes_sent(N)
    for i in 0..(plan.chunks.len() - 1) {
        let curr = &plan.chunks[i];
        let next = &plan.chunks[i + 1];
        assert_eq!(next.offset, curr.offset + curr.data.len() as u64);
        assert!(!curr.is_last);
    }

    assert!(plan.chunks.last().unwrap().is_last);
}

#[test]
fn test_consensus_engine_install_snapshot_term_validation() {
    let engine = ConsensusEngine::new();
    let leader_id = NodeId(Uuid::new_v4());

    let req = brain_services::planning::InstallSnapshotRequest {
        term: TermId(1),
        leader_id,
        last_included_sequence: SequenceNumber(10),
        last_included_term: TermId(1),
        offset: 0,
        data: vec![1, 2, 3, 4],
        done: true,
    };

    let resp = engine.install_snapshot(&req).unwrap();
    assert!(resp.success);
    assert_eq!(resp.bytes_written, 4);

    // Stale term request -> Rejected
    let stale_req = brain_services::planning::InstallSnapshotRequest {
        term: TermId(0),
        leader_id,
        last_included_sequence: SequenceNumber(10),
        last_included_term: TermId(1),
        offset: 0,
        data: vec![1, 2, 3, 4],
        done: true,
    };
    let stale_resp = engine.install_snapshot(&stale_req).unwrap();
    assert!(!stale_resp.success);
    assert_eq!(stale_resp.bytes_written, 0);
}

#[test]
fn test_snapshot_replicator_streaming_lifecycle_and_acks() {
    let codec = JsonSnapshotCodec::<LeadershipProjection>::new();
    let proj = LeadershipProjection::new();
    let snapshot =
        SnapshotBuilder::build_snapshot(&proj, SequenceNumber(20), TermId(1), &codec, 1, 2000)
            .unwrap();

    let target_node = NodeId(Uuid::new_v4());
    let leader_id = NodeId(Uuid::new_v4());
    let plan = SnapshotReplicationPlanner::plan_transfer(&snapshot, target_node, 10);
    let mut replicator = SnapshotReplicator::new(plan);

    assert_eq!(replicator.state(), SnapshotTransferState::Preparing);

    let req = replicator
        .next_request(TermId(1), leader_id, SequenceNumber(20), TermId(1))
        .unwrap();

    assert_eq!(replicator.state(), SnapshotTransferState::Streaming);
    assert_eq!(req.offset, 0);

    let expected_bytes = req.data.len() as u64;
    replicator.process_ack(true, expected_bytes);

    if replicator.plan().chunks.len() == 1 {
        assert_eq!(replicator.state(), SnapshotTransferState::Completed);
    } else {
        assert_eq!(replicator.state(), SnapshotTransferState::Streaming);
    }
}

#[test]
fn test_snapshot_restore_engine_follower_state_restoration() {
    let codec = JsonSnapshotCodec::<LeadershipProjection>::new();
    let mut proj = LeadershipProjection::new();
    proj.total_events = 75;
    proj.leaders_elected_count = 3;

    let snapshot =
        SnapshotBuilder::build_snapshot(&proj, SequenceNumber(75), TermId(4), &codec, 1, 8000)
            .unwrap();

    let mut follower_proj = LeadershipProjection::new();
    let restored_seq =
        SnapshotRestoreEngine::restore_snapshot(&snapshot, &mut follower_proj, &codec).unwrap();

    assert_eq!(restored_seq, SequenceNumber(75));
    assert_eq!(follower_proj, proj);
}
