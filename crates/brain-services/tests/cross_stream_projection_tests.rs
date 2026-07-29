//! Integration & Cross-Stream Fitness Test Suite (Phase 10 Milestone 10.4).

use brain_services::planning::{
    CheckpointCapabilitySet, ClusterControlPlaneProjection, ClusterManager, ClusterNode,
    ClusterNodeRole, ClusterNodeStatus, CoordinatorElectionEngine, EventPublisher,
    ExecutionScheduler, ExecutionWorker, InMemoryEventPublisher, LeadershipEvent, NodeAddress,
    NodeId, RoundRobinPolicy, SingleCoordinatorStrategy, TaskId, WorkerId, WorkerRegistry,
    WorkerStatus, LEADERSHIP_EVENT_SCHEMA_VERSION,
};
use uuid::Uuid;

#[test]
fn test_generic_fallible_event_publisher_and_schema_version() {
    let mut publisher = InMemoryEventPublisher::<LeadershipEvent>::new();

    let mut cluster = ClusterManager::new();
    let coord_id = NodeId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id: coord_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    cluster.activate_node(coord_id, 10500).unwrap();

    let mut engine = CoordinatorElectionEngine::new(Box::new(SingleCoordinatorStrategy));
    let leader = engine.elect_leader(&mut cluster, 11000).unwrap();

    for ev in engine.events() {
        // Verify schema version is set to LEADERSHIP_EVENT_SCHEMA_VERSION
        assert_eq!(ev.schema_version, LEADERSHIP_EVENT_SCHEMA_VERSION);
        publisher.publish(ev.clone()).unwrap();
    }

    assert_eq!(publisher.events().len(), 2);
    assert_eq!(
        publisher.events()[1].kind,
        brain_services::planning::LeadershipEventKind::LeaderElected {
            leader_id: leader.node_id,
            epoch: leader.epoch,
        }
    );
}

#[test]
fn test_cluster_control_plane_cross_stream_projection() {
    // 1. Cluster events
    let mut cluster = ClusterManager::new();
    let node_id = NodeId(Uuid::new_v4());
    cluster
        .join_cluster(
            ClusterNode {
                node_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();
    cluster.activate_node(node_id, 10500).unwrap();

    // 2. Leadership events
    let mut election_engine = CoordinatorElectionEngine::new(Box::new(SingleCoordinatorStrategy));
    election_engine.elect_leader(&mut cluster, 11000).unwrap();

    // 3. Scheduling events
    let mut registry = WorkerRegistry::new();
    let worker_id = WorkerId(Uuid::new_v4());
    registry
        .register_worker(ExecutionWorker {
            worker_id,
            name: "worker-node".to_string(),
            capabilities: CheckpointCapabilitySet::default_set(),
            status: WorkerStatus::Active,
        })
        .unwrap();

    let mut local_sched = ExecutionScheduler::new(Box::new(RoundRobinPolicy::new()));
    let task_id = TaskId(Uuid::new_v4());
    let req = CheckpointCapabilitySet::default_set();
    local_sched
        .schedule_task(task_id, &req, &registry, 5000, 11500)
        .unwrap();

    // 4. Composite projection replay
    let mut composite_proj1 = ClusterControlPlaneProjection::new();
    composite_proj1.project_all(
        cluster.events(),
        election_engine.events(),
        local_sched.events(),
    );

    assert_eq!(
        composite_proj1.topology.total_events,
        cluster.events().len()
    );
    assert_eq!(
        composite_proj1.leadership.total_events,
        election_engine.events().len()
    );
    assert_eq!(
        composite_proj1.scheduling.total_events,
        local_sched.events().len()
    );

    // 5. Cross-Stream Fitness Invariant: Replay Idempotency across streams
    let mut composite_proj2 = ClusterControlPlaneProjection::new();
    composite_proj2.project_all(
        cluster.events(),
        election_engine.events(),
        local_sched.events(),
    );
    assert_eq!(composite_proj1, composite_proj2);
}

#[test]
fn test_cross_stream_isolation_invariant() {
    let mut cluster = ClusterManager::new();
    let node_id = NodeId(Uuid::new_v4());
    cluster
        .join_cluster(
            ClusterNode {
                node_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Joining,
            },
            10000,
        )
        .unwrap();

    let mut composite_proj = ClusterControlPlaneProjection::new();

    // Replaying only cluster events updates topology projection ONLY; leadership and scheduling remain at default zero
    composite_proj.project_all(cluster.events(), &[], &[]);

    assert_eq!(composite_proj.topology.nodes_joined_count, 1);
    assert_eq!(composite_proj.leadership.total_events, 0);
    assert_eq!(composite_proj.scheduling.total_events, 0);
}
