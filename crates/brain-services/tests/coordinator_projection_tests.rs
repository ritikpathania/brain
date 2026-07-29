//! Integration test suite for CoordinatorLeader election, ClusterTopologyProjection, and GlobalScheduler (Phase 10 Milestone 10.2).

use brain_services::planning::{
    CheckpointCapabilitySet, ClusterManager, ClusterNode, ClusterNodeRole, ClusterNodeStatus,
    ClusterTopologyProjection, CoordinatorElectionEngine, EpochId, ExecutionWorker,
    GlobalScheduler, LeadershipState, NodeAddress, NodeId, SingleCoordinatorStrategy,
    StaticLeaderStrategy, TaskId, WorkerId, WorkerRegistry, WorkerStatus,
};
use uuid::Uuid;

#[test]
fn test_coordinator_election_and_leadership_handoff() {
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

    // 1. SingleCoordinatorStrategy picks the first coordinator sorted deterministically by NodeId
    let expected_leader1_id = std::cmp::min(coord1_id, coord2_id);
    let mut engine = CoordinatorElectionEngine::new(Box::new(SingleCoordinatorStrategy));
    let leader1 = engine.elect_leader(&mut cluster, 11000).unwrap();

    assert_eq!(leader1.node_id, expected_leader1_id);
    assert_eq!(leader1.state, LeadershipState::Leader);
    assert_eq!(leader1.epoch, EpochId(1));

    // 2. Leadership handoff to coord2 advances cluster epoch to EpochId(2)
    let leader2 = engine
        .handoff_leadership(&mut cluster, coord2_id, 12000)
        .unwrap();

    assert_eq!(leader2.node_id, coord2_id);
    assert_eq!(leader2.state, LeadershipState::Leader);
    assert_eq!(leader2.epoch, EpochId(2));
}

#[test]
fn test_static_leader_election_strategy() {
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

    // StaticLeaderStrategy targeting coord2
    let strategy = Box::new(StaticLeaderStrategy {
        target_leader: coord2_id,
    });
    let mut engine = CoordinatorElectionEngine::new(strategy);

    let leader = engine.elect_leader(&mut cluster, 11000).unwrap();
    assert_eq!(leader.node_id, coord2_id);
}

#[test]
fn test_cluster_topology_projection_replay() {
    let mut cluster = ClusterManager::new();
    let node_id = NodeId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::WorkerNode,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    cluster.activate_node(node_id, 10500).unwrap();
    cluster.suspect_node(node_id, 11000).unwrap();
    cluster.activate_node(node_id, 11500).unwrap(); // Recovered
    cluster.advance_epoch(12000);
    cluster.leave_cluster(node_id, 12500).unwrap();

    let events = cluster.events();
    let mut proj = ClusterTopologyProjection::new();
    proj.project_events(events);

    assert_eq!(proj.total_events, events.len());
    assert_eq!(proj.nodes_joined_count, 1);
    assert_eq!(proj.nodes_activated_count, 1);
    assert_eq!(proj.nodes_suspected_count, 1);
    assert_eq!(proj.nodes_recovered_count, 1);
    assert_eq!(proj.epoch_advancements_count, 1);
    assert_eq!(proj.nodes_left_count, 1);
}

#[test]
fn test_global_scheduler_facade_and_monotonic_fencing() {
    let cluster = ClusterManager::new();
    let mut registry = WorkerRegistry::new();
    let worker_id = WorkerId(Uuid::new_v4());

    registry
        .register_worker(ExecutionWorker {
            worker_id,
            name: "global-worker".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: WorkerStatus::Active,
        })
        .unwrap();

    let mut global_sched = GlobalScheduler::default();
    let task1_id = TaskId(Uuid::new_v4());
    let task2_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();

    let fenced_1 = global_sched
        .schedule_global_task(task1_id, &req, &registry, &cluster, 5000, 10000)
        .unwrap();

    let fenced_2 = global_sched
        .schedule_global_task(task2_id, &req, &registry, &cluster, 5000, 10500)
        .unwrap();

    // Verify fence token monotonicity across global task placements
    assert_eq!(fenced_1.fence_token, 1);
    assert_eq!(fenced_2.fence_token, 2);
    assert_eq!(fenced_1.epoch, EpochId(1));
    assert_eq!(fenced_2.epoch, EpochId(1));
}
