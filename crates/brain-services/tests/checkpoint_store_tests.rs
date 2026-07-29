//! Integration test suite for CheckpointStore, SupervisionReplayEngine, and CapabilityNegotiator (Phase 8 Milestone 8.3).

use brain_services::planning::{
    CapabilityCompatibility, CapabilityNegotiator, CheckpointCapability, CheckpointCapabilitySet,
    CheckpointStore, ExecutionId, ExecutionPlanId, ExecutionSupervisor, InMemoryCheckpointStore,
    SupervisionState,
};
use uuid::Uuid;

#[test]
fn test_in_memory_checkpoint_store_save_load_list() {
    let store = InMemoryCheckpointStore::new();

    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    supervisor.pause().unwrap();
    let chkpt1 = supervisor.create_checkpoint(0).unwrap();

    // Save checkpoint
    store.save_checkpoint(&chkpt1).unwrap();

    // Load checkpoint
    let loaded = store.load_checkpoint(chkpt1.checkpoint_id).unwrap();
    assert_eq!(loaded.checkpoint_id, chkpt1.checkpoint_id);
    assert_eq!(loaded.execution_id, exec_id);

    // Duplicate save rejected
    let dup_res = store.save_checkpoint(&chkpt1);
    assert!(dup_res.is_err());

    // List checkpoints
    let list = store.list_checkpoints(exec_id).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].checkpoint_id, chkpt1.checkpoint_id);
}

#[test]
fn test_supervision_replay_engine_determinism() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    supervisor.pause().unwrap();
    let chkpt = supervisor.create_checkpoint(1).unwrap();
    let _report = supervisor.restore_checkpoint(&chkpt).unwrap();

    let events = supervisor.events();

    let mut proj1 = brain_services::planning::SupervisionStateProjection::default();
    let mut proj2 = brain_services::planning::SupervisionStateProjection::default();

    let engine = brain_services::planning::SupervisionProjectionEngine::new();

    engine.drive_projections(events, &mut [&mut proj1]);
    engine.drive_projections(events, &mut [&mut proj2]);

    // Replay determinism & idempotency invariant
    assert_eq!(proj1, proj2);
    assert_eq!(proj1.state, SupervisionState::Active);
    assert_eq!(proj1.events_processed_count, events.len());
}

#[test]
fn test_capability_negotiator_diagnostics() {
    let supported = CheckpointCapabilitySet::default_set();
    let mut required = CheckpointCapabilitySet::default_set();

    // Both equal -> Compatible
    let res1 = CapabilityNegotiator::check_compatibility(&supported, &required);
    assert_eq!(res1, CapabilityCompatibility::Compatible);

    // Required asks for extra capability missing in supported
    required.insert(CheckpointCapability::SupportsStateReplay);
    let mut incomplete_supported = CheckpointCapabilitySet::default();
    incomplete_supported.insert(CheckpointCapability::SupportsStageResume);

    let res2 = CapabilityNegotiator::check_compatibility(&incomplete_supported, &required);
    if let CapabilityCompatibility::MissingCapabilities(missing) = res2 {
        assert!(
            missing.contains(&CheckpointCapability::SupportsTaskRetry)
                || missing.contains(&CheckpointCapability::SupportsStateReplay)
        );
    } else {
        panic!("Expected CapabilityCompatibility::MissingCapabilities");
    }
}
