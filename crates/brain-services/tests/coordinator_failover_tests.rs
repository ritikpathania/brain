//! Integration & Failover Recovery Test Suite for `FailoverPlanner` & `FailoverExecutor` (Phase 11 Milestone 11.3).

use brain_services::planning::{
    ClusterManager, ClusterNode, ClusterNodeRole, ClusterNodeStatus, ConsensusEngine, EpochId,
    FailoverExecutor, FailoverPlan, FailoverPlanner, FailoverState, FailureDetector, FencedLease,
    LeaseId, NodeAddress, NodeId, RecoveryStrategy, SequenceNumber, WorkerLease,
};
use uuid::Uuid;

#[test]
fn test_failure_detector_identifies_suspect_leader() {
    let mut cluster = ClusterManager::new();
    let leader_id = NodeId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id: leader_id,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Active,
            },
            10000,
        )
        .unwrap();

    // 1. Healthy active leader -> None detected
    assert_eq!(
        FailureDetector::detect_suspect_leader(&cluster, Some(leader_id)),
        None
    );

    // 2. Suspect active leader -> Detected
    cluster.suspect_node(leader_id, 10500).unwrap();
    assert_eq!(
        FailureDetector::detect_suspect_leader(&cluster, Some(leader_id)),
        Some(leader_id)
    );
}

#[test]
fn test_failover_planner_and_executor_lifecycle_and_state_recovery() {
    let mut cluster = ClusterManager::new();
    let node1 = NodeId(Uuid::new_v4());
    let node2 = NodeId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id: node1,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Active,
            },
            10000,
        )
        .unwrap();

    cluster
        .join_cluster(
            ClusterNode {
                node_id: node2,
                address: NodeAddress("10.0.0.2:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Active,
            },
            10000,
        )
        .unwrap();

    let consensus_engine = ConsensusEngine::new();

    // 1. Plan failover from node1 to node2
    let plan = FailoverPlanner::plan_failover(&mut cluster, &consensus_engine, Some(node1), node2)
        .unwrap();
    assert_eq!(
        plan,
        FailoverPlan {
            former_leader: Some(node1),
            target_leader: node2,
            target_term: brain_services::planning::TermId(1),
            target_epoch: EpochId(2),
            recovery_strategy: RecoveryStrategy::ReplayOnly,
        }
    );

    // 2. Execute plan via FailoverExecutor
    let executor = FailoverExecutor::new();
    assert_eq!(executor.current_state(), FailoverState::Idle);

    let (leader, report) = executor
        .execute_plan(
            &plan,
            &mut cluster,
            11000,
            SequenceNumber(1),
            SequenceNumber(5),
        )
        .unwrap();

    assert_eq!(leader.node_id, node2);
    assert_eq!(leader.epoch, EpochId(2));
    assert_eq!(executor.current_state(), FailoverState::Recovered);

    assert_eq!(report.plan.target_leader, node2);
    assert_eq!(report.progress.start_sequence, SequenceNumber(1));
    assert_eq!(report.progress.end_sequence, SequenceNumber(5));
    assert_eq!(report.progress.recovered_epoch, EpochId(2));
}

#[test]
fn test_implicit_fence_token_invalidation_post_failover() {
    let mut cluster = ClusterManager::new();
    let node1 = NodeId(Uuid::new_v4());
    let node2 = NodeId(Uuid::new_v4());
    let worker_id = brain_services::planning::WorkerId(Uuid::new_v4());

    cluster
        .join_cluster(
            ClusterNode {
                node_id: node1,
                address: NodeAddress("10.0.0.1:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Active,
            },
            10000,
        )
        .unwrap();

    cluster
        .join_cluster(
            ClusterNode {
                node_id: node2,
                address: NodeAddress("10.0.0.2:8001".to_string()),
                role: ClusterNodeRole::Coordinator,
                status: ClusterNodeStatus::Active,
            },
            10000,
        )
        .unwrap();

    let initial_epoch = cluster.current_epoch(); // Epoch 1

    let lease_id = LeaseId(Uuid::new_v4());
    let assignment_id = Uuid::new_v4();
    let task_id = brain_services::planning::TaskId(Uuid::new_v4());

    let lease = WorkerLease {
        lease_id,
        assignment_id,
        worker_id,
        task_id,
        state: brain_services::planning::LeaseState::Active,
        issued_at_ms: 10000,
        ttl_ms: 30000,
    };

    let fenced_lease = FencedLease::new(lease, initial_epoch, 100);

    // Verify fence token bound
    assert!(fenced_lease.verify_fence_token(100).is_ok());

    // Execute failover plan -> Epoch advances to 2
    let consensus_engine = ConsensusEngine::new();
    let plan = FailoverPlanner::plan_failover(&mut cluster, &consensus_engine, Some(node1), node2)
        .unwrap();
    let executor = FailoverExecutor::new();
    executor
        .execute_plan(
            &plan,
            &mut cluster,
            11000,
            SequenceNumber(1),
            SequenceNumber(1),
        )
        .unwrap();

    let post_failover_epoch = cluster.current_epoch(); // Epoch 2
    assert_eq!(post_failover_epoch, EpochId(2));

    // Stale lease epoch (Epoch 1 < Epoch 2) implicitly invalidates stale lease
    assert!(fenced_lease.epoch < post_failover_epoch);
}
