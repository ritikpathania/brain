//! Integration test suite for ClusterManager, FencedLease, and RemoteExecutionDispatcher (Phase 10 Milestone 10.1).

use brain_services::planning::{
    CheckpointCapabilitySet, ClusterError, ClusterEventKind, ClusterManager, ClusterNode,
    ClusterNodeRole, ClusterNodeStatus, EpochId, ExecutionDispatcher, ExecutionScheduler,
    FencedLease, NodeAddress, NodeId, RemoteExecutionDispatcher, RoundRobinPolicy, TaskId,
    TaskStep, WorkerRegistry,
};
use uuid::Uuid;

#[test]
fn test_cluster_manager_membership_roles_and_state_machine() {
    let mut cluster = ClusterManager::new();
    assert_eq!(cluster.current_epoch(), EpochId(1));

    let coord_id = NodeId(Uuid::new_v4());
    let worker_id = NodeId(Uuid::new_v4());

    let coord_node = ClusterNode {
        node_id: coord_id,
        address: NodeAddress("10.0.0.1:8001".to_string()),
        role: ClusterNodeRole::Coordinator,
        status: ClusterNodeStatus::Joining,
    };

    let worker_node = ClusterNode {
        node_id: worker_id,
        address: NodeAddress("10.0.0.2:8002".to_string()),
        role: ClusterNodeRole::WorkerNode,
        status: ClusterNodeStatus::Joining,
    };

    // 1. Join cluster
    cluster.join_cluster(coord_node, 10000).unwrap();
    cluster.join_cluster(worker_node, 10000).unwrap();

    // 2. Activate nodes
    cluster.activate_node(coord_id, 10500).unwrap();
    cluster.activate_node(worker_id, 10500).unwrap();

    assert_eq!(cluster.get_coordinators().len(), 1);
    assert_eq!(cluster.get_workers().len(), 1);

    // 3. Epoch advancement
    let next_epoch = cluster.advance_epoch(11000);
    assert_eq!(next_epoch, EpochId(2));

    // 4. Suspect -> Recover -> Leave state machine transitions
    cluster.suspect_node(worker_id, 11500).unwrap();
    assert!(cluster.get_workers().is_empty()); // Suspect worker excluded from active workers

    cluster.activate_node(worker_id, 12000).unwrap(); // Recovered to Active
    assert_eq!(cluster.get_workers().len(), 1);

    cluster.leave_cluster(worker_id, 12500).unwrap();
    assert!(cluster.get_workers().is_empty());

    let events = cluster.events();
    let kinds: Vec<ClusterEventKind> = events.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&ClusterEventKind::NodeJoined));
    assert!(kinds.contains(&ClusterEventKind::NodeActivated));
    assert!(kinds.contains(&ClusterEventKind::EpochAdvanced));
    assert!(kinds.contains(&ClusterEventKind::NodeSuspected));
    assert!(kinds.contains(&ClusterEventKind::NodeRecovered));
    assert!(kinds.contains(&ClusterEventKind::NodeLeft));
}

#[test]
fn test_fenced_lease_verification_and_zombie_rejection() {
    let mut registry = WorkerRegistry::new();
    let worker_id = brain_services::planning::WorkerId(Uuid::new_v4());
    registry
        .register_worker(brain_services::planning::ExecutionWorker {
            worker_id,
            name: "fenced-worker".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: brain_services::planning::WorkerStatus::Active,
        })
        .unwrap();

    let mut scheduler = ExecutionScheduler::new(Box::new(RoundRobinPolicy::new()));
    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let lease = scheduler
        .schedule_task(task_id, &req, &registry, 5000, 10000)
        .unwrap();

    let fenced_lease = FencedLease::new(lease, EpochId(2), 100);

    // Fence verification with expected minimum 100 passes
    assert!(fenced_lease.verify_fence_token(100).is_ok());
    assert!(fenced_lease.verify_fence_token(50).is_ok());

    // Fence verification with expected minimum 101 fails (stale token)
    let err = fenced_lease.verify_fence_token(101).unwrap_err();
    assert_eq!(
        err,
        ClusterError::InvalidFenceToken {
            expected: 101,
            found: 100,
        }
    );
}

#[test]
fn test_remote_execution_dispatcher_delivery() {
    let mut registry = WorkerRegistry::new();
    let worker_id = brain_services::planning::WorkerId(Uuid::new_v4());
    registry
        .register_worker(brain_services::planning::ExecutionWorker {
            worker_id,
            name: "remote-worker".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: brain_services::planning::WorkerStatus::Active,
        })
        .unwrap();

    let mut scheduler = ExecutionScheduler::new(Box::new(RoundRobinPolicy::new()));
    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let lease = scheduler
        .schedule_task(task_id, &req, &registry, 5000, 10000)
        .unwrap();

    let dispatcher = RemoteExecutionDispatcher;
    let step = TaskStep {
        task_id,
        description: "Remote task step".to_string(),
        required_capabilities: vec![],
        confidence: 1.0,
    };

    let ack = dispatcher.dispatch_task(&lease, &step).unwrap();
    assert_eq!(ack.lease_id, lease.lease_id);
}
