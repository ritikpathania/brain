//! Integration test suite for WorkerRegistry, SupervisionProjectionEngine, and CheckpointMigrator (Phase 8 Milestone 8.4).

use brain_services::planning::{
    CheckpointCapabilitySet, CheckpointMigrator, DefaultCheckpointMigrator, ExecutionId,
    ExecutionPlanId, ExecutionSupervisor, ExecutionWorker, SupervisionAuditProjection,
    SupervisionMetricsProjection, SupervisionProjectionEngine, SupervisionState,
    SupervisionStateProjection, WorkerId, WorkerRegistry, WorkerRegistryError, WorkerStatus,
};
use uuid::Uuid;

#[test]
fn test_worker_registry_registration_and_offline_exclusion() {
    let mut registry = WorkerRegistry::new();

    let id1 = WorkerId(Uuid::new_v4());
    let id2 = WorkerId(Uuid::new_v4());

    let worker1 = ExecutionWorker {
        worker_id: id1,
        name: "worker-node-1".to_string(),
        capabilities: CheckpointCapabilitySet::default_set(),
        status: WorkerStatus::Active,
    };

    let worker2 = ExecutionWorker {
        worker_id: id2,
        name: "worker-node-2".to_string(),
        capabilities: CheckpointCapabilitySet::default_set(),
        status: WorkerStatus::Offline,
    };

    registry.register_worker(worker1.clone()).unwrap();
    registry.register_worker(worker2.clone()).unwrap();

    // Duplicate registration returns DuplicateWorker error
    let dup_res = registry.register_worker(worker1);
    assert_eq!(dup_res, Err(WorkerRegistryError::DuplicateWorker(id1)));

    // find_capable_workers returns worker1 but excludes offline worker2
    let req = CheckpointCapabilitySet::default_set();
    let capable = registry.find_capable_workers(&req);
    assert_eq!(capable.len(), 1);
    assert_eq!(capable[0].worker_id, id1);
}

#[test]
fn test_supervision_projection_engine_multi_projection_replay() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    supervisor.pause().unwrap();
    let chkpt = supervisor.create_checkpoint(0).unwrap();
    let _report = supervisor.restore_checkpoint(&chkpt).unwrap();

    let events = supervisor.events();

    let mut state_proj = SupervisionStateProjection::default();
    let mut metrics_proj = SupervisionMetricsProjection::default();
    let mut audit_proj = SupervisionAuditProjection::default();

    let engine = SupervisionProjectionEngine::new();
    engine.drive_projections(
        events,
        &mut [&mut state_proj, &mut metrics_proj, &mut audit_proj],
    );

    // Verify SupervisionStateProjection
    assert_eq!(state_proj.state, SupervisionState::Active);
    assert_eq!(state_proj.events_processed_count, events.len());

    // Verify SupervisionMetricsProjection
    assert_eq!(metrics_proj.total_events, events.len());
    assert_eq!(metrics_proj.checkpoints_created_count, 1);
    assert_eq!(metrics_proj.pauses_count, 1);

    // Verify SupervisionAuditProjection
    assert_eq!(audit_proj.entries.len(), events.len());
}

#[test]
fn test_checkpoint_migrator_boundary() {
    let exec_id = ExecutionId(Uuid::new_v4());
    let plan_id = ExecutionPlanId(Uuid::new_v4());

    let mut supervisor = ExecutionSupervisor::new(exec_id, plan_id);
    supervisor.pause().unwrap();
    let chkpt = supervisor.create_checkpoint(0).unwrap();

    let migrator = DefaultCheckpointMigrator;
    let migrated = migrator.migrate(&chkpt).unwrap();

    assert_eq!(migrated.schema_version, chkpt.schema_version);
    assert_eq!(migrated.checkpoint_id, chkpt.checkpoint_id);
}
