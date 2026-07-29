//! Integration test suite for Phase G.5 Immutable Configuration Persistence & Replay Binding.

use brain_config::{ConfigVersion, ConfigurationManager};
use brain_events::{
    EventStore, InMemoryEventStore, ReflectionEventEnvelope, ReflectionRuntimeEvent,
};
use uuid::Uuid;

#[test]
fn test_config_version_monotonic_allocation() {
    let v1 = ConfigVersion::v1();
    assert_eq!(v1.0, 1);
    let v2 = v1.next();
    assert_eq!(v2.0, 2);
}

#[test]
fn test_copy_on_write_immutability_and_activation() {
    let manager = ConfigurationManager::new();

    let v1_config = manager.active_configuration();
    assert_eq!(v1_config.version, ConfigVersion::v1());
    assert_eq!(v1_config.max_task_retries, 3);

    // Create Copy-on-Write Draft for V2
    let mut draft_v2 = manager.create_draft(ConfigVersion::v1()).unwrap();
    draft_v2.max_task_retries = 5;

    let v2 = manager.activate(draft_v2).unwrap();
    assert_eq!(v2, ConfigVersion(2));

    // Verify V1 remains unchanged (Immutability Invariant)
    let v1_retrieved = manager.get_version(ConfigVersion::v1()).unwrap();
    assert_eq!(v1_retrieved.max_task_retries, 3);

    // Verify active configuration is now V2
    let v2_active = manager.active_configuration();
    assert_eq!(v2_active.version, ConfigVersion(2));
    assert_eq!(v2_active.max_task_retries, 5);
}

#[test]
fn test_configuration_diff_and_rollback() {
    let manager = ConfigurationManager::new();

    let mut draft_v2 = manager.create_draft(ConfigVersion::v1()).unwrap();
    draft_v2.max_task_retries = 10;
    manager.activate(draft_v2).unwrap();

    let diff = manager
        .compute_diff(ConfigVersion::v1(), ConfigVersion(2))
        .unwrap();
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].0, "max_task_retries");

    // Perform Rollback to V1 — should allocate V3 with V1 settings
    let v3 = manager.rollback(ConfigVersion::v1()).unwrap();
    assert_eq!(v3, ConfigVersion(3));

    let v3_config = manager.active_configuration();
    assert_eq!(v3_config.version, ConfigVersion(3));
    assert_eq!(v3_config.max_task_retries, 3);
}

#[test]
fn test_replay_determinism_config_version_binding() {
    let manager = ConfigurationManager::new();

    let mut draft_v2 = manager.create_draft(ConfigVersion::v1()).unwrap();
    draft_v2.max_task_retries = 99;
    let v2 = manager.activate(draft_v2).unwrap();

    let corr_id = Uuid::new_v4();
    let env_v1 = ReflectionEventEnvelope::new(
        "plan_cfg_01",
        Some("task_1".to_string()),
        corr_id,
        1000,
        ReflectionRuntimeEvent::CheckpointCreated {
            plan_id: "plan_cfg_01".to_string(),
            stage_index: 0,
            modified_entity_count: 1,
            timestamp_ms: 1000,
        },
    )
    .with_config_version(ConfigVersion::v1().0);

    let env_v2 = env_v1.clone().with_config_version(v2.0);

    let store = InMemoryEventStore::new();
    store.append(env_v1.clone()).unwrap();
    store.append(env_v2.clone()).unwrap();

    let stream = store.stream();
    assert_eq!(stream.len(), 2);
    assert_eq!(stream[0].config_version, 1);
    assert_eq!(stream[1].config_version, 2);

    // Replay evaluation under bound configuration version
    let cfg1 = manager
        .get_version(ConfigVersion(stream[0].config_version))
        .unwrap();
    let cfg2 = manager
        .get_version(ConfigVersion(stream[1].config_version))
        .unwrap();

    assert_ne!(cfg1.max_task_retries, cfg2.max_task_retries);
    assert_eq!(cfg1.max_task_retries, 3);
    assert_eq!(cfg2.max_task_retries, 99);
}
