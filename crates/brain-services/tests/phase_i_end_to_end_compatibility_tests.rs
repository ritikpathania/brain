//! End-to-End Compatibility & Lifecycle Verification Suite for Phase I (WAL -> Snapshot -> Restore -> Replay & Config V1/V2).

use brain_config::{ConfigVersion, ConfigurationManager};
use brain_events::{
    EventStore, InMemoryEventStore, ReflectionEventEnvelope, ReflectionRuntimeEvent,
};
use brain_services::protocol::{
    ExecuteGoalCommandDTO, HandshakeAckDTO, HandshakeHelloDTO, ProtocolCapability,
    ProtocolNegotiator, ProtocolVersion, SupportedRange,
};
use brain_storage::{
    CompactionPolicy, InMemorySnapshotStore, RetentionPolicy, SnapshotHeader, SnapshotPolicy,
    StorageLifecycleOrchestrator, WalLogEventStore,
};
use std::collections::HashSet;
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_e2e_full_lifecycle_wal_snapshot_restore_replay() {
    let tmp = NamedTempFile::new().unwrap();
    let wal_store = WalLogEventStore::open(tmp.path()).unwrap();
    let snapshot_store = InMemorySnapshotStore::new();

    let orchestrator = StorageLifecycleOrchestrator::new(
        SnapshotPolicy::new(2, 5000),
        RetentionPolicy::new(10000),
        CompactionPolicy::new(10),
    );

    let corr_id = Uuid::new_v4();

    // Stage 1: Append Events 1 & 2 to WAL
    let env1 = ReflectionEventEnvelope::new(
        "plan_e2e_01",
        Some("task_1".to_string()),
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_e2e_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    );

    let env2 = ReflectionEventEnvelope::new(
        "plan_e2e_01",
        Some("task_2".to_string()),
        corr_id,
        2000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_e2e_01".to_string(),
            stage_index: 1,
            modified_entity_count: 2,
            timestamp_ms: 2000,
        },
    );

    wal_store.append(env1.clone()).unwrap();
    wal_store.append(env2.clone()).unwrap();

    let wal_stream = wal_store.stream();
    assert_eq!(wal_stream.len(), 2);

    // Stage 2: Create Versioned Snapshot after Event 2
    let snapshot_payload = serde_json::to_vec(&wal_stream).unwrap();
    let header = SnapshotHeader::new(1, 2000, 2, 0);

    orchestrator
        .execute_snapshot(&snapshot_store, "e2e_snap_v1", &header, &snapshot_payload)
        .unwrap();

    // Stage 3: Append Event 3 (Delta Event)
    let env3 = ReflectionEventEnvelope::new(
        "plan_e2e_01",
        Some("task_3".to_string()),
        corr_id,
        3000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_e2e_01".to_string(),
            stage_index: 2,
            modified_entity_count: 3,
            timestamp_ms: 3000,
        },
    );
    wal_store.append(env3.clone()).unwrap();

    // Stage 4: Restore Snapshot + Replay Delta Events
    let (restored_header, restored_bytes) = orchestrator
        .restore_snapshot(&snapshot_store, "e2e_snap_v1")
        .unwrap()
        .unwrap();

    assert_eq!(restored_header.event_sequence, 2);

    let mut reconstructed_events: Vec<ReflectionEventEnvelope> =
        serde_json::from_slice(&restored_bytes).unwrap();

    let delta_events: Vec<_> = wal_store
        .stream()
        .into_iter()
        .filter(|e| e.timestamp_ms > restored_header.created_at_ms)
        .collect();

    reconstructed_events.extend(delta_events);

    // Stage 5: Replay Parity Assertion
    let mem_store = InMemoryEventStore::new();
    for env in &reconstructed_events {
        mem_store.append(env.clone()).unwrap();
    }

    assert_eq!(mem_store.stream(), wal_store.stream());
    assert_eq!(mem_store.stream().len(), 3);
}

#[test]
fn test_e2e_config_v1_v2_replay_determinism() {
    let cfg_manager = ConfigurationManager::new();

    // Create & activate V2
    let mut draft_v2 = cfg_manager.create_draft(ConfigVersion::v1()).unwrap();
    draft_v2.max_task_retries = 10;
    let v2 = cfg_manager.activate(draft_v2).unwrap();

    let corr_id = Uuid::new_v4();
    let env_v1 = ReflectionEventEnvelope::new(
        "plan_e2e_cfg",
        None,
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_e2e_cfg".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    )
    .with_config_version(ConfigVersion::v1().0);

    let env_v2 = env_v1.clone().with_config_version(v2.0);

    let store = InMemoryEventStore::new();
    store.append(env_v1).unwrap();
    store.append(env_v2).unwrap();

    let events = store.stream();
    let cfg1 = cfg_manager
        .get_version(ConfigVersion(events[0].config_version))
        .unwrap();
    let cfg2 = cfg_manager
        .get_version(ConfigVersion(events[1].config_version))
        .unwrap();

    assert_eq!(cfg1.max_task_retries, 3);
    assert_eq!(cfg2.max_task_retries, 10);
    assert_ne!(cfg1.max_task_retries, cfg2.max_task_retries);
}

#[test]
fn test_e2e_client_server_handshake_and_command_execution() {
    let client_version = ProtocolVersion(1);
    let server_range = SupportedRange::default_range();

    let negotiated_version = ProtocolNegotiator::negotiate(client_version, server_range).unwrap();
    assert_eq!(negotiated_version, ProtocolVersion(1));

    let mut caps = HashSet::new();
    let _ = caps.insert(ProtocolCapability::Replay);
    let _ = caps.insert(ProtocolCapability::Streaming);

    let hello = HandshakeHelloDTO {
        client_version,
        client_id: "e2e_client".to_string(),
        requested_capabilities: caps.clone(),
    };

    let ack = HandshakeAckDTO {
        negotiated_version,
        server_range,
        accepted_capabilities: caps,
    };

    assert_eq!(hello.client_version, ack.negotiated_version);

    let cmd = ExecuteGoalCommandDTO {
        goal_prompt: "Verify E2E Protocol Integration".to_string(),
        workspace_id: "/tmp/e2e_workspace".to_string(),
        timeout_seconds: 30,
    };

    let serialized = serde_json::to_string(&cmd).unwrap();
    let deserialized: ExecuteGoalCommandDTO = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized, cmd);
}
