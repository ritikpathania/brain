//! Integration test suite for Milestone G4 Snapshot, Retention & Compaction Policies & Replay Parity.

use brain_events::{
    EventStore, InMemoryEventStore, ReflectionEventEnvelope, ReflectionRuntimeEvent,
};
use brain_storage::{
    CompactionPolicy, InMemorySnapshotStore, RetentionPolicy, SnapshotHeader, SnapshotPolicy,
    StorageLifecycleOrchestrator, WalLogEventStore, CURRENT_SNAPSHOT_SCHEMA_VERSION,
};
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_milestone_g4_pure_policy_evaluators() {
    let snapshot_policy = SnapshotPolicy::new(10, 5000);
    assert!(!snapshot_policy.should_snapshot(5, 2000));
    assert!(snapshot_policy.should_snapshot(10, 2000));
    assert!(snapshot_policy.should_snapshot(5, 5000));

    let retention_policy = RetentionPolicy::new(1000);
    assert_eq!(retention_policy.calculate_cutoff_timestamp(5000), 4000);

    let compaction_policy = CompactionPolicy::new(20);
    assert!(!compaction_policy.should_compact(15));
    assert!(compaction_policy.should_compact(25));
}

#[test]
fn test_milestone_g4_snapshot_header_and_restore_roundtrip() {
    let snapshot_store = InMemorySnapshotStore::new();
    let orchestrator = StorageLifecycleOrchestrator::new(
        SnapshotPolicy::new(5, 1000),
        RetentionPolicy::new(2000),
        CompactionPolicy::new(10),
    );

    let header = SnapshotHeader::new(1, 1000, 42, 0x12345678);
    assert_eq!(header.schema_version, CURRENT_SNAPSHOT_SCHEMA_VERSION);

    let state_bytes = b"canonical graph state buffer".to_vec();
    orchestrator
        .execute_snapshot(&snapshot_store, "snap_g4_01", &header, &state_bytes)
        .expect("Snapshot execution failed");

    let (restored_header, restored_state) = orchestrator
        .restore_snapshot(&snapshot_store, "snap_g4_01")
        .expect("Snapshot restore failed")
        .expect("Snapshot record not found");

    assert_eq!(restored_header, header);
    assert_eq!(restored_state, state_bytes);
}

#[test]
fn test_milestone_g4_replay_parity_full_log_vs_snapshot_plus_delta() {
    let tmp = NamedTempFile::new().unwrap();
    let wal_store = WalLogEventStore::open(tmp.path()).unwrap();
    let snapshot_store = InMemorySnapshotStore::new();

    let orchestrator = StorageLifecycleOrchestrator::new(
        SnapshotPolicy::new(2, 5000),
        RetentionPolicy::new(2000),
        CompactionPolicy::new(5),
    );

    let corr_id = Uuid::new_v4();

    // Event 1 & Event 2
    let env1 = ReflectionEventEnvelope::new(
        "plan_g4_01",
        Some("task_1".to_string()),
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_g4_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    );

    let env2 = ReflectionEventEnvelope::new(
        "plan_g4_01",
        Some("task_2".to_string()),
        corr_id,
        2000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_g4_01".to_string(),
            stage_index: 1,
            modified_entity_count: 2,
            timestamp_ms: 2000,
        },
    );

    wal_store.append(env1.clone()).unwrap();
    wal_store.append(env2.clone()).unwrap();

    let full_stream = wal_store.stream();
    assert_eq!(full_stream.len(), 2);

    // Save snapshot after Event 2 (sequence 2)
    let snapshot_state = serde_json::to_vec(&full_stream).unwrap();
    let header = SnapshotHeader::new(1, 2000, 2, 0);

    orchestrator
        .execute_snapshot(
            &snapshot_store,
            "snap_g4_checkpoint",
            &header,
            &snapshot_state,
        )
        .unwrap();

    // Event 3 (Delta event after snapshot)
    let env3 = ReflectionEventEnvelope::new(
        "plan_g4_01",
        Some("task_3".to_string()),
        corr_id,
        3000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_g4_01".to_string(),
            stage_index: 2,
            modified_entity_count: 3,
            timestamp_ms: 3000,
        },
    );
    wal_store.append(env3.clone()).unwrap();

    // Reconstruct state from Snapshot + Delta events
    let (_restored_header, restored_snapshot_bytes) = orchestrator
        .restore_snapshot(&snapshot_store, "snap_g4_checkpoint")
        .unwrap()
        .unwrap();

    let mut restored_events: Vec<ReflectionEventEnvelope> =
        serde_json::from_slice(&restored_snapshot_bytes).unwrap();

    let delta_events: Vec<_> = wal_store
        .stream()
        .into_iter()
        .filter(|e| e.timestamp_ms > 2000)
        .collect();

    restored_events.extend(delta_events);

    // Replay parity check: FullLog (3 events) == Snapshot + Delta (3 events)
    let memory_store = InMemoryEventStore::new();
    for env in &restored_events {
        memory_store.append(env.clone()).unwrap();
    }

    assert_eq!(wal_store.stream().len(), 3);
    assert_eq!(memory_store.stream(), wal_store.stream());
}
