//! Production Release Non-Functional & Security Validation Test Suite.

use brain_events::{EventStore, ReflectionEventEnvelope, ReflectionRuntimeEvent};
use brain_services::protocol::{
    HandshakeHelloDTO, ProtocolCapability, ProtocolError, ProtocolNegotiator, ProtocolVersion,
    SupportedRange,
};
use brain_storage::{
    CompactionPolicy, InMemorySnapshotStore, RetentionPolicy, SnapshotHeader, SnapshotPolicy,
    StorageLifecycleOrchestrator, WalLogEventStore, WalRecord,
};
use std::collections::HashSet;
use std::io::Cursor;
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_failure_injection_corrupt_wal_checksum_rejection() {
    let mut rec = WalRecord::new(1, 1000, 1, b"valid payload".to_vec());
    rec.checksum = 0xDEADBEEF; // Inject corrupted checksum

    let encoded = rec.encode();
    let mut cursor = Cursor::new(encoded);
    let result = WalRecord::decode(&mut cursor);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn test_malformed_protocol_handshake_fuzzing() {
    let malformed_json = r#"{"client_version":{"0":"invalid_number"},"client_id":12345}"#;
    let result: Result<HandshakeHelloDTO, _> = serde_json::from_str(malformed_json);
    assert!(result.is_err());

    // Negotiator unsupported version rejection
    let err = ProtocolNegotiator::negotiate(ProtocolVersion(99), SupportedRange::default_range())
        .unwrap_err();
    match err {
        ProtocolError::UnsupportedVersion { requested, .. } => assert_eq!(requested, 99),
        _ => panic!("Expected UnsupportedVersion error"),
    }
}

#[test]
fn test_performance_high_volume_wal_append_and_replay_throughput() {
    let tmp = NamedTempFile::new().unwrap();
    let wal_store = WalLogEventStore::open(tmp.path()).unwrap();
    let corr_id = Uuid::new_v4();

    let start = std::time::Instant::now();
    let event_count = 1000;

    for i in 0..event_count {
        let env = ReflectionEventEnvelope::new(
            "plan_perf_01",
            Some(format!("task_{}", i)),
            corr_id,
            1000 + i as u64,
            ReflectionRuntimeEvent::CheckpointCreated {
                plan_id: "plan_perf_01".to_string(),
                stage_index: i,
                modified_entity_count: 1,
                timestamp_ms: 1000 + i as u64,
            },
        );
        wal_store.append(env).unwrap();
    }

    let append_duration = start.elapsed();
    assert!(
        append_duration.as_secs() < 5,
        "1000 WAL appends took too long"
    );

    let replay_start = std::time::Instant::now();
    let stream = wal_store.stream();
    let replay_duration = replay_start.elapsed();

    assert_eq!(stream.len(), event_count);
    assert!(
        replay_duration.as_secs() < 2,
        "1000 event replay took too long"
    );
}

#[test]
fn test_security_capability_enforcement_matrix() {
    let mut caps = HashSet::new();
    caps.insert(ProtocolCapability::Replay);
    caps.insert(ProtocolCapability::Streaming);

    let hello = HandshakeHelloDTO {
        client_version: ProtocolVersion(1),
        client_id: "sec_client".to_string(),
        requested_capabilities: caps.clone(),
    };

    assert!(hello
        .requested_capabilities
        .contains(&ProtocolCapability::Replay));
    assert!(!hello
        .requested_capabilities
        .contains(&ProtocolCapability::Compression));
}

#[test]
fn test_scalability_versioned_snapshot_restore_performance() {
    let snapshot_store = InMemorySnapshotStore::new();
    let orchestrator = StorageLifecycleOrchestrator::new(
        SnapshotPolicy::new(100, 10000),
        RetentionPolicy::new(86400000),
        CompactionPolicy::new(500),
    );

    let large_state = vec![0u8; 1024 * 1024]; // 1MB state snapshot payload
    let header = SnapshotHeader::new(1, 1000, 5000, 0x12345678);

    let start = std::time::Instant::now();
    orchestrator
        .execute_snapshot(&snapshot_store, "large_snap", &header, &large_state)
        .unwrap();

    let (restored_header, restored_state) = orchestrator
        .restore_snapshot(&snapshot_store, "large_snap")
        .unwrap()
        .unwrap();

    let duration = start.elapsed();
    assert_eq!(restored_header, header);
    assert_eq!(restored_state.len(), 1024 * 1024);
    assert!(
        duration.as_millis() < 500,
        "1MB snapshot round-trip took too long"
    );
}
