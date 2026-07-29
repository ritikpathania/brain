//! Integration test suite for Milestone G3 Append-Only WAL Log Backend, Tail Crash Recovery & Triple Replay Parity.

use brain_events::{
    EventStore, InMemoryEventStore, ReflectionEventEnvelope, ReflectionRuntimeEvent,
};
use brain_storage::{SqliteEventStore, WalLogEventStore, WalRecord};
use std::fs::OpenOptions;
use std::io::Write;
use tempfile::NamedTempFile;
use uuid::Uuid;

#[test]
fn test_wal_record_crc32_checksum_validation() {
    let rec = WalRecord::new(1, 1000, 1, b"test payload".to_vec());

    let encoded = rec.encode();
    let mut cursor = std::io::Cursor::new(encoded);
    let decoded = WalRecord::decode(&mut cursor).unwrap().unwrap();
    assert_eq!(decoded.sequence_number, 1);
    assert_eq!(decoded.payload, b"test payload");
}

#[test]
fn test_wal_log_append_query_stream_and_compaction() {
    let tmp = NamedTempFile::new().unwrap();
    let store = WalLogEventStore::open(tmp.path()).unwrap();

    let corr_id = Uuid::new_v4();
    let env = ReflectionEventEnvelope::new(
        "plan_wal_01",
        Some("task_01".to_string()),
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_wal_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    );

    store.append(env.clone()).unwrap();

    let queried = store.query("plan_wal_01");
    assert_eq!(queried.len(), 1);
    assert_eq!(queried[0], env);

    let stream = store.stream();
    assert_eq!(stream.len(), 1);

    // Compact before timestamp 2000
    let removed = store.compact(2000);
    assert_eq!(removed, 1);
    assert_eq!(store.stream().len(), 0);
}

#[test]
fn test_wal_log_truncated_tail_crash_recovery() {
    let tmp = NamedTempFile::new().unwrap();
    let path = tmp.path().to_path_buf();

    let corr_id = Uuid::new_v4();
    let env = ReflectionEventEnvelope::new(
        "plan_crash_01",
        None,
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_crash_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    );

    {
        let store = WalLogEventStore::open(&path).unwrap();
        store.append(env.clone()).unwrap();
    }

    // Simulate power-loss partial record append by writing 5 garbage bytes to tail
    {
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"12345").unwrap();
        f.flush().unwrap();
    }

    // Re-open WalLogEventStore — should safely recover & truncate corrupted 5 tail bytes
    let recovered_store = WalLogEventStore::open(&path).unwrap();
    let stream = recovered_store.stream();
    assert_eq!(stream.len(), 1);
    assert_eq!(stream[0], env);
}

#[test]
fn test_triple_backend_replay_parity_inmemory_vs_sqlite_vs_wal() {
    let tmp = NamedTempFile::new().unwrap();

    let mem_store = InMemoryEventStore::new();
    let sql_store = SqliteEventStore::in_memory();
    let wal_store = WalLogEventStore::open(tmp.path()).unwrap();

    let corr_id = Uuid::new_v4();
    let env1 = ReflectionEventEnvelope::new(
        "plan_triple_01",
        Some("task_a".to_string()),
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_triple_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    );

    let env2 = ReflectionEventEnvelope::new(
        "plan_triple_01",
        Some("task_b".to_string()),
        corr_id,
        2000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_triple_01".to_string(),
            stage_index: 1,
            modified_entity_count: 3,
            timestamp_ms: 2000,
        },
    );

    mem_store.append(env1.clone()).unwrap();
    mem_store.append(env2.clone()).unwrap();

    sql_store.append(env1.clone()).unwrap();
    sql_store.append(env2.clone()).unwrap();

    wal_store.append(env1).unwrap();
    wal_store.append(env2).unwrap();

    let mem_stream = mem_store.stream();
    let sql_stream = sql_store.stream();
    let wal_stream = wal_store.stream();

    assert_eq!(mem_stream, sql_stream);
    assert_eq!(sql_stream, wal_stream);
}
