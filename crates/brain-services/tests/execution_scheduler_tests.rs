//! Integration test suite for ExecutionScheduler, WorkerLease, and DynamicProjectionRegistry (Phase 9 Milestone 9.1).

use brain_services::planning::{
    CheckpointCapabilitySet, DynamicProjectionRegistry, ExecutionScheduler, ExecutionWorker,
    LeaseState, LeastBusyPolicy, ProjectionId, RoundRobinPolicy, SchedulerError,
    SchedulingEventKind, SupervisionMetricsProjection, SupervisionStateProjection, TaskId,
    WorkerId, WorkerRegistry, WorkerStatus,
};
use uuid::Uuid;

fn sample_registry() -> (WorkerRegistry, WorkerId, WorkerId) {
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
        status: WorkerStatus::Busy,
    };

    registry.register_worker(worker1).unwrap();
    registry.register_worker(worker2).unwrap();

    (registry, id1, id2)
}

#[test]
fn test_scheduler_round_robin_placement_and_leasing() {
    let (registry, _id1, _id2) = sample_registry();

    let policy = Box::new(RoundRobinPolicy::new());
    let mut scheduler = ExecutionScheduler::new(policy);

    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    // Schedule task -> grants lease
    let lease = scheduler
        .schedule_task(task_id, &req, &registry, 5000, 10000)
        .unwrap();

    assert_eq!(lease.state, LeaseState::Active);
    assert_eq!(lease.task_id, task_id);
    assert!(!lease.is_expired(10000));

    // Renew lease
    scheduler.renew_lease(lease.lease_id, 2000, 12000).unwrap();

    // Release lease (idempotent)
    scheduler.release_lease(lease.lease_id, 13000).unwrap();
    scheduler.release_lease(lease.lease_id, 13000).unwrap();

    let events = scheduler.events();
    let kinds: Vec<SchedulingEventKind> = events.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&SchedulingEventKind::TaskScheduled));
    assert!(kinds.contains(&SchedulingEventKind::WorkerSelected));
    assert!(kinds.contains(&SchedulingEventKind::LeaseGranted));
    assert!(kinds.contains(&SchedulingEventKind::LeaseRenewed));
    assert!(kinds.contains(&SchedulingEventKind::LeaseReleased));
}

#[test]
fn test_scheduler_least_busy_policy_selection() {
    let (registry, id1, _id2) = sample_registry();

    let policy = Box::new(LeastBusyPolicy);
    let mut scheduler = ExecutionScheduler::new(policy);

    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    // Least busy policy must select Active worker (id1) over Busy worker (id2)
    let lease = scheduler
        .schedule_task(task_id, &req, &registry, 5000, 10000)
        .unwrap();

    assert_eq!(lease.worker_id, id1);
}

#[test]
fn test_scheduler_lease_ttl_expiration() {
    let (registry, _id1, _id2) = sample_registry();

    let policy = Box::new(RoundRobinPolicy::new());
    let mut scheduler = ExecutionScheduler::new(policy);

    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let lease = scheduler
        .schedule_task(task_id, &req, &registry, 1000, 10000)
        .unwrap();

    // Attempt renew after TTL expiration -> fails with LeaseExpired
    let res = scheduler.renew_lease(lease.lease_id, 1000, 12000);
    assert_eq!(res, Err(SchedulerError::LeaseExpired(lease.lease_id)));
}

#[test]
fn test_dynamic_projection_registry_dispatch() {
    let mut dyn_registry = DynamicProjectionRegistry::new();

    let proj1 = Box::new(SupervisionStateProjection::default());
    let proj2 = Box::new(SupervisionMetricsProjection::default());

    dyn_registry.register(ProjectionId("state_proj".to_string()), proj1);
    dyn_registry.register(ProjectionId("metrics_proj".to_string()), proj2);

    let event = brain_services::planning::SupervisionEvent {
        event_id: brain_services::planning::SupervisionEventId(Uuid::new_v4()),
        kind: brain_services::planning::SupervisionEventKind::CheckpointCreated,
        message: "Test event".to_string(),
        timestamp_ms: 10000,
    };

    // Dispatching event updates all registered projections
    dyn_registry.dispatch_event(&event);
}
