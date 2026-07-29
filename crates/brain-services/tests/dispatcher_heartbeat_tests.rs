//! Integration test suite for ExecutionDispatcher, WorkerHeartbeatService, LeaseRecoveryEngine, and SchedulingMetricsProjection (Phase 9 Milestone 9.2).

use brain_services::planning::{
    CheckpointCapabilitySet, ExecutionDispatcher, ExecutionScheduler, ExecutionWorker,
    HeartbeatPolicy, LeaseRecoveryEngine, LocalExecutionDispatcher, RoundRobinPolicy,
    SchedulingMetricsProjection, TaskId, TaskStep, WorkerHeartbeatService, WorkerId,
    WorkerRegistry, WorkerStatus,
};
use uuid::Uuid;

#[test]
fn test_local_execution_dispatcher_task_delivery() {
    let mut registry = WorkerRegistry::new();
    let worker_id = WorkerId(Uuid::new_v4());
    let worker = ExecutionWorker {
        worker_id,
        name: "local-worker".to_string(),
        capabilities: CheckpointCapabilitySet::default_set(),
        status: WorkerStatus::Active,
    };
    registry.register_worker(worker).unwrap();

    let mut scheduler = ExecutionScheduler::new(Box::new(RoundRobinPolicy::new()));
    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let lease = scheduler
        .schedule_task(task_id, &req, &registry, 5000, 10000)
        .unwrap();

    let dispatcher = LocalExecutionDispatcher::default();
    let step = TaskStep {
        task_id,
        description: "Test dispatch step".to_string(),
        required_capabilities: vec![],
        confidence: 1.0,
    };

    let ack = dispatcher.dispatch_task(&lease, &step).unwrap();
    assert_eq!(ack.lease_id, lease.lease_id);
}

#[test]
fn test_worker_heartbeat_service_stale_eviction() {
    let mut registry = WorkerRegistry::new();
    let worker_id = WorkerId(Uuid::new_v4());
    let worker = ExecutionWorker {
        worker_id,
        name: "stale-worker".to_string(),
        capabilities: CheckpointCapabilitySet::default_set(),
        status: WorkerStatus::Active,
    };
    registry.register_worker(worker).unwrap();

    let mut hb_service = WorkerHeartbeatService::new();
    hb_service.record_heartbeat(worker_id, 10000);

    let policy = HeartbeatPolicy {
        stale_timeout_ms: 5000,
    };

    // At t=12000 (2000ms later), worker is NOT stale
    let evicted_1 = hb_service.evict_stale_workers(&mut registry, &policy, 12000);
    assert!(evicted_1.is_empty());
    assert_eq!(
        registry.get_worker(worker_id).unwrap().status,
        WorkerStatus::Active
    );

    // At t=16000 (6000ms later), worker IS stale -> evicted to Offline
    let evicted_2 = hb_service.evict_stale_workers(&mut registry, &policy, 16000);
    assert_eq!(evicted_2.len(), 1);
    assert_eq!(evicted_2[0], worker_id);
    assert_eq!(
        registry.get_worker(worker_id).unwrap().status,
        WorkerStatus::Offline
    );
}

#[test]
fn test_lease_recovery_engine_assignment_oriented_recovery() {
    let mut registry = WorkerRegistry::new();
    let worker1_id = WorkerId(Uuid::new_v4());
    let worker2_id = WorkerId(Uuid::new_v4());

    registry
        .register_worker(ExecutionWorker {
            worker_id: worker1_id,
            name: "worker-1".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: WorkerStatus::Active,
        })
        .unwrap();

    registry
        .register_worker(ExecutionWorker {
            worker_id: worker2_id,
            name: "worker-2".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: WorkerStatus::Active,
        })
        .unwrap();

    let mut scheduler = ExecutionScheduler::new(Box::new(RoundRobinPolicy::new()));
    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let lease_1 = scheduler
        .schedule_task(task_id, &req, &registry, 5000, 10000)
        .unwrap();

    // Invalidate worker 1 by marking offline
    registry
        .update_status(worker1_id, WorkerStatus::Offline)
        .unwrap();

    // Assignment-oriented recovery
    let assignment = brain_services::planning::TaskAssignment {
        assignment_id: lease_1.assignment_id,
        task_id,
        worker_id: worker1_id,
        assigned_at_ms: 10000,
    };

    let mut recovery_engine = LeaseRecoveryEngine::new();
    let lease_2 = recovery_engine
        .recover_assignment(
            &assignment,
            Some(&lease_1),
            &mut scheduler,
            &registry,
            &req,
            5000,
            12000,
        )
        .unwrap();

    // Verify lease_1 released and lease_2 granted to active worker2
    assert_ne!(lease_1.lease_id, lease_2.lease_id);
    assert_eq!(lease_2.worker_id, worker2_id);
}

#[test]
fn test_scheduling_metrics_projection_replay() {
    let mut registry = WorkerRegistry::new();
    let worker_id = WorkerId(Uuid::new_v4());
    registry
        .register_worker(ExecutionWorker {
            worker_id,
            name: "worker-1".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: WorkerStatus::Active,
        })
        .unwrap();

    let mut scheduler = ExecutionScheduler::new(Box::new(RoundRobinPolicy::new()));
    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let lease = scheduler
        .schedule_task(task_id, &req, &registry, 5000, 10000)
        .unwrap();

    scheduler.renew_lease(lease.lease_id, 2000, 12000).unwrap();
    scheduler.release_lease(lease.lease_id, 14000).unwrap();

    let events = scheduler.events();
    let mut proj = SchedulingMetricsProjection::new();
    proj.project_events(events);

    assert_eq!(proj.total_events, events.len());
    assert_eq!(proj.placements_requested_count, 1);
    assert_eq!(proj.leases_granted_count, 1);
    assert_eq!(proj.leases_renewed_count, 1);
    assert_eq!(proj.leases_released_count, 1);
}
