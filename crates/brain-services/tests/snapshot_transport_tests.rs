//! Integration & Transport Abstraction Test Suite for `SnapshotTransport`, `MockSnapshotTransport`, and `ChunkedStreamAdapter` (Phase 14 Milestone 14.3).

use brain_services::planning::{
    ChunkedStreamAdapter, ConsensusEngine, JsonSnapshotCodec, LeadershipProjection,
    MockSnapshotTransport, NodeId, SequenceNumber, SnapshotBuilder, SnapshotTransferState, TermId,
};
use uuid::Uuid;

#[test]
fn test_mock_snapshot_transport_streaming_completion() {
    let transport = MockSnapshotTransport::new();
    let engine = ConsensusEngine::new();
    let follower_id = NodeId(Uuid::new_v4());
    let leader_id = NodeId(Uuid::new_v4());

    transport.register_node(follower_id, engine);

    let codec = JsonSnapshotCodec::<LeadershipProjection>::new();
    let mut proj = LeadershipProjection::new();
    proj.total_events = 50;

    let snapshot =
        SnapshotBuilder::build_snapshot(&proj, SequenceNumber(50), TermId(2), &codec, 1, 10000)
            .unwrap();

    let adapter = ChunkedStreamAdapter::new(&transport);
    let state = adapter
        .stream_snapshot(
            &snapshot,
            follower_id,
            leader_id,
            TermId(2),
            SequenceNumber(50),
            TermId(2),
            16, // 16 bytes per chunk
        )
        .unwrap();

    assert_eq!(state, SnapshotTransferState::Completed);
}

#[test]
fn test_mock_snapshot_transport_packet_drop_error_propagation() {
    let transport = MockSnapshotTransport::new();
    let engine = ConsensusEngine::new();
    let follower_id = NodeId(Uuid::new_v4());
    let leader_id = NodeId(Uuid::new_v4());

    transport.register_node(follower_id, engine);
    transport.set_drop_rate(100); // 100% packet drop rate

    let codec = JsonSnapshotCodec::<LeadershipProjection>::new();
    let proj = LeadershipProjection::new();
    let snapshot =
        SnapshotBuilder::build_snapshot(&proj, SequenceNumber(10), TermId(1), &codec, 1, 1000)
            .unwrap();

    let adapter = ChunkedStreamAdapter::new(&transport);
    let res = adapter.stream_snapshot(
        &snapshot,
        follower_id,
        leader_id,
        TermId(1),
        SequenceNumber(10),
        TermId(1),
        16,
    );

    assert!(res.is_err());
}
