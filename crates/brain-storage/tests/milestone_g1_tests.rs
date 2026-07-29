//! Unit and integration tests for Milestone G1 Persistence Contracts & Reference Implementations.

use brain_storage::{
    CheckpointStore, InMemoryCheckpointStore, InMemorySnapshotStore, SnapshotStore, StorageError,
};

#[test]
fn test_milestone_g1_in_memory_checkpoint_store() {
    let store = InMemoryCheckpointStore::new();

    // Verify initially empty
    let loaded = store.load_checkpoint("plan_g1_01").unwrap();
    assert_eq!(loaded, None);

    // Save checkpoint
    let cp_data = r#"{"plan_id":"plan_g1_01","stage_index":2}"#;
    store.save_checkpoint("plan_g1_01", cp_data).unwrap();

    // Load checkpoint
    let loaded = store.load_checkpoint("plan_g1_01").unwrap();
    assert_eq!(loaded, Some(cp_data.to_string()));
}

#[test]
fn test_milestone_g1_in_memory_snapshot_store() {
    let store = InMemorySnapshotStore::new();

    // Verify initially empty
    let loaded = store.load_snapshot("snap_g1_01").unwrap();
    assert_eq!(loaded, None);

    // Save binary snapshot
    let binary_data = vec![0xDE, 0xAD, 0xBE, 0xEF];
    store.save_snapshot("snap_g1_01", &binary_data).unwrap();

    // Load binary snapshot
    let loaded = store.load_snapshot("snap_g1_01").unwrap();
    assert_eq!(loaded, Some(binary_data));
}

#[test]
fn test_milestone_g1_storage_error_variants() {
    let err = StorageError::NotFound("Key missing".to_string());
    assert_eq!(err.to_string(), "Storage record not found: Key missing");
}
