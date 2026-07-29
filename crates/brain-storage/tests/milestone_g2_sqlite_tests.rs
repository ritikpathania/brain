//! Integration test suite for Milestone G2 SQLite Persistence (SqliteCheckpointStore, SqliteSnapshotStore, SqliteEventStore) & Replay Parity.

use brain_events::{
    EventStore, InMemoryEventStore, ReflectionEventEnvelope, ReflectionRuntimeEvent,
};
use brain_storage::{
    CheckpointStore, SnapshotStore, SqliteCheckpointStore, SqliteEventStore, SqliteSnapshotStore,
};
use uuid::Uuid;

#[test]
fn test_sqlite_checkpoint_store_save_load_and_update() {
    let store = SqliteCheckpointStore::in_memory();

    assert_eq!(store.load_checkpoint("plan_sqlite_01").unwrap(), None);

    let cp1 = r#"{"plan_id":"plan_sqlite_01","stage":0}"#;
    store.save_checkpoint("plan_sqlite_01", cp1).unwrap();
    assert_eq!(
        store.load_checkpoint("plan_sqlite_01").unwrap(),
        Some(cp1.to_string())
    );

    // Update on conflict
    let cp2 = r#"{"plan_id":"plan_sqlite_01","stage":1}"#;
    store.save_checkpoint("plan_sqlite_01", cp2).unwrap();
    assert_eq!(
        store.load_checkpoint("plan_sqlite_01").unwrap(),
        Some(cp2.to_string())
    );
}

#[test]
fn test_sqlite_snapshot_store_binary_roundtrip() {
    let store = SqliteSnapshotStore::in_memory();

    assert_eq!(store.load_snapshot("snap_sqlite_01").unwrap(), None);

    let blob = vec![0x11, 0x22, 0x33, 0x44, 0x55];
    store.save_snapshot("snap_sqlite_01", &blob).unwrap();
    assert_eq!(store.load_snapshot("snap_sqlite_01").unwrap(), Some(blob));
}

#[test]
fn test_sqlite_event_store_append_query_compact_and_replay_parity() {
    let memory_store = InMemoryEventStore::new();
    let sqlite_store = SqliteEventStore::in_memory();

    let corr_id = Uuid::new_v4();

    let env1 = ReflectionEventEnvelope::new(
        "plan_parity_01",
        Some("task_01".to_string()),
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_parity_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    );

    let env2 = ReflectionEventEnvelope::new(
        "plan_parity_01",
        Some("task_02".to_string()),
        corr_id,
        2000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_parity_01".to_string(),
            stage_index: 1,
            modified_entity_count: 2,
            timestamp_ms: 2000,
        },
    );

    // Append to both backends
    memory_store.append(env1.clone()).unwrap();
    memory_store.append(env2.clone()).unwrap();

    sqlite_store.append(env1).unwrap();
    sqlite_store.append(env2).unwrap();

    // Verify Replay Parity (Query & Stream equivalence)
    let mem_queried = memory_store.query("plan_parity_01");
    let sql_queried = sqlite_store.query("plan_parity_01");

    assert_eq!(mem_queried.len(), sql_queried.len());
    assert_eq!(mem_queried, sql_queried);

    let mem_stream = memory_store.stream();
    let sql_stream = sqlite_store.stream();

    assert_eq!(mem_stream, sql_stream);

    // Test compaction parity
    let mem_removed = memory_store.compact(1500);
    let sql_removed = sqlite_store.compact(1500);

    assert_eq!(mem_removed, sql_removed);
    assert_eq!(memory_store.stream(), sqlite_store.stream());
}
