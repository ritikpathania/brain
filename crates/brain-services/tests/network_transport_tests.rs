//! Integration Test Suite for `MessageFramingCodec`, `TransportConnectionPool`, `GrpcSnapshotTransport`, and `QuicSnapshotTransport` (Phase 15 Milestone 15.1).

use brain_services::planning::{
    ChunkedStreamAdapter, ConnectionStatus, ConsensusEngine, FramingError, GrpcSnapshotTransport,
    IntegrityPolicy, JsonSnapshotCodec, LeadershipProjection, MessageFramingCodec, NodeId,
    QuicSnapshotTransport, SequenceNumber, SnapshotBuilder, SnapshotTransferState, TermId,
    TransportConnectionPool, TRANSPORT_FRAME_MAGIC,
};
use uuid::Uuid;

#[test]
fn test_message_framing_codec_crc32_round_trip_and_corruption_rejection() {
    let payload = b"Hello, Brain Cluster Distributed Framing!";

    // 1. Encode with CRC32 integrity policy
    let encoded = MessageFramingCodec::encode_frame(1, payload, IntegrityPolicy::Crc32).unwrap();
    assert_eq!(encoded[0], TRANSPORT_FRAME_MAGIC);

    // 2. Decode valid frame -> Success
    let (header, decoded_payload) =
        MessageFramingCodec::decode_frame(&encoded, IntegrityPolicy::Crc32).unwrap();
    assert_eq!(header.msg_type, 1);
    assert_eq!(header.payload_len, payload.len() as u32);
    assert_eq!(decoded_payload, payload);

    // 3. Corrupt a byte in payload -> ChecksumMismatch error
    let mut corrupted = encoded.clone();
    corrupted[15] ^= 0xFF;
    let err = MessageFramingCodec::decode_frame(&corrupted, IntegrityPolicy::Crc32);
    assert!(matches!(err, Err(FramingError::ChecksumMismatch { .. })));
}

#[test]
fn test_message_framing_codec_none_policy_and_oversized_protection() {
    let payload = vec![0xAB; 100];

    // Encode with IntegrityPolicy::None
    let encoded = MessageFramingCodec::encode_frame(2, &payload, IntegrityPolicy::None).unwrap();
    let (header, decoded) =
        MessageFramingCodec::decode_frame(&encoded, IntegrityPolicy::None).unwrap();
    assert_eq!(header.checksum, 0);
    assert_eq!(decoded, payload);

    // Truncated buffer -> TruncatedFrame error
    let err = MessageFramingCodec::decode_frame(&encoded[..5], IntegrityPolicy::None);
    assert_eq!(err, Err(FramingError::TruncatedFrame));
}

#[test]
fn test_transport_connection_pool_acquisition_and_degradation() {
    let pool = TransportConnectionPool::new();
    let node1 = NodeId(Uuid::new_v4());

    assert_eq!(pool.active_connection_count(), 0);

    // 1. Get or create connection -> Active
    let status1 = pool.get_or_create_connection(node1);
    assert_eq!(status1, ConnectionStatus::Active);
    assert_eq!(pool.active_connection_count(), 1);

    // 2. Mark degraded -> Degraded
    pool.mark_degraded(&node1);
    assert_eq!(pool.active_connection_count(), 0);
}

#[test]
fn test_grpc_and_quic_snapshot_transports_streaming() {
    let follower_id = NodeId(Uuid::new_v4());
    let leader_id = NodeId(Uuid::new_v4());

    // 1. gRPC Transport
    let grpc_transport = GrpcSnapshotTransport::new();
    let engine1 = ConsensusEngine::new();
    grpc_transport.register_node(follower_id, engine1);

    let codec = JsonSnapshotCodec::<LeadershipProjection>::new();
    let mut proj = LeadershipProjection::new();
    proj.total_events = 20;
    let snapshot =
        SnapshotBuilder::build_snapshot(&proj, SequenceNumber(20), TermId(1), &codec, 1, 1000)
            .unwrap();

    let adapter1 = ChunkedStreamAdapter::new(&grpc_transport);
    let state1 = adapter1
        .stream_snapshot(
            &snapshot,
            follower_id,
            leader_id,
            TermId(1),
            SequenceNumber(20),
            TermId(1),
            32,
        )
        .unwrap();
    assert_eq!(state1, SnapshotTransferState::Completed);

    // 2. QUIC Transport
    let quic_transport = QuicSnapshotTransport::new();
    let engine2 = ConsensusEngine::new();
    quic_transport.register_node(follower_id, engine2);

    let adapter2 = ChunkedStreamAdapter::new(&quic_transport);
    let state2 = adapter2
        .stream_snapshot(
            &snapshot,
            follower_id,
            leader_id,
            TermId(1),
            SequenceNumber(20),
            TermId(1),
            32,
        )
        .unwrap();
    assert_eq!(state2, SnapshotTransferState::Completed);
}
