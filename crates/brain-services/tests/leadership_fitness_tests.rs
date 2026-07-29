//! Architecture Fitness Invariant & Leadership Event Projection Test Suite (Phase 10 Milestone 10.3).

use brain_services::planning::{
    CheckpointCapabilitySet, ClusterManager, ClusterNode, ClusterNodeRole, ClusterNodeStatus,
    CoordinatorElectionEngine, EpochId, ExecutionWorker, GlobalScheduler, LeadershipEventKind,
    LeadershipProjection, LeadershipState, NodeAddress, NodeId, SingleCoordinatorStrategy, TaskId,
    WorkerId, WorkerRegistry, WorkerStatus,
};
use uuid::Uuid;

#[test]
fn test_leadership_event_stream_and_projection_replay() {
    let mut cluster = ClusterManager::new();
    let coord1_id = NodeId(Uuid::new_v4());
    let coord2_id = NodeId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id: coord1_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    cluster
        .join_cluster(
            ClusterNode {
                node_id: coord2_id,
                address: NodeAddress("10.0.0.2:8002".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    cluster.activate_node(coord1_id, 10500).unwrap();
    cluster.activate_node(coord2_id, 10500).unwrap();

    let mut engine = CoordinatorElectionEngine::new(Box::new(SingleCoordinatorStrategy));

    // 1. Elect leader
    let leader1 = engine.elect_leader(&mut cluster, 11000).unwrap();

    // 2. Handoff leadership
    let target_id = if leader1.node_id == coord1_id {
        coord2_id
    } else {
        coord1_id
    };
    engine
        .handoff_leadership(&mut cluster, target_id, 12000)
        .unwrap();

    let events = engine.events();
    assert_eq!(events.len(), 3);

    // 3. Fitness Invariant 1: Causal event ordering (LeaderElectionStarted -> LeaderElected -> LeadershipTransferred)
    assert!(matches!(
        events[0].kind,
        LeadershipEventKind::LeaderElectionStarted { .. }
    ));
    assert!(matches!(
        events[1].kind,
        LeadershipEventKind::LeaderElected { .. }
    ));
    assert!(matches!(
        events[2].kind,
        LeadershipEventKind::LeadershipTransferred { .. }
    ));

    // 4. Fitness Invariant 2: Pure LeadershipProjection replay
    let mut proj1 = LeadershipProjection::new();
    proj1.project_events(events);

    assert_eq!(proj1.total_events, 3);
    assert_eq!(proj1.election_started_count, 1);
    assert_eq!(proj1.leaders_elected_count, 1);
    assert_eq!(proj1.transfers_count, 1);

    // 5. Fitness Invariant 3: Idempotent Replay (Replay(events) == Replay(events))
    let mut proj2 = LeadershipProjection::new();
    proj2.project_events(events);
    assert_eq!(proj1, proj2);
}

#[test]
fn test_architecture_fitness_single_leader_per_epoch_and_handoff_epoch_advancement() {
    let mut cluster = ClusterManager::new();
    let coord1_id = NodeId(Uuid::new_v4());
    let coord2_id = NodeId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id: coord1_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    cluster
        .join_cluster(
            ClusterNode {
                node_id: coord2_id,
                address: NodeAddress("10.0.0.2:8002".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    cluster.activate_node(coord1_id, 10500).unwrap();
    cluster.activate_node(coord2_id, 10500).unwrap();

    let mut engine = CoordinatorElectionEngine::new(Box::new(SingleCoordinatorStrategy));
    let leader1 = engine.elect_leader(&mut cluster, 11000).unwrap();

    // Fitness Invariant: Single Leader per Epoch
    let current = engine.current_leader().unwrap();
    assert_eq!(current.node_id, leader1.node_id);
    assert_eq!(current.state, LeadershipState::Leader);
    assert_eq!(current.epoch, EpochId(1));

    // Handoff advances epoch monotonically
    let target_id = if leader1.node_id == coord1_id {
        coord2_id
    } else {
        coord1_id
    };
    let leader2 = engine
        .handoff_leadership(&mut cluster, target_id, 12000)
        .unwrap();

    assert_eq!(leader2.epoch, EpochId(2));
    assert_ne!(leader1.node_id, leader2.node_id);
}

#[test]
fn test_architecture_fitness_global_scheduler_facade_delegation() {
    let cluster = ClusterManager::new();
    let mut registry = WorkerRegistry::new();
    let worker_id = WorkerId(Uuid::new_v4());

    registry
        .register_worker(ExecutionWorker {
            worker_id,
            name: "fitness-worker".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: WorkerStatus::Active,
        })
        .unwrap();

    let mut global_sched = GlobalScheduler::default();
    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let fenced_lease = global_sched
        .schedule_global_task(task_id, &req, &registry, &cluster, 5000, 10000)
        .unwrap();

    // Fitness Invariant: GlobalScheduler delegates placement to inner ExecutionScheduler
    assert_eq!(global_sched.local_scheduler().events().len(), 3); // TaskScheduled, WorkerSelected, LeaseGranted
    assert_eq!(fenced_lease.lease.task_id, task_id);
}
