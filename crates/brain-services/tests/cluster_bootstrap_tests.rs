//! Integration & Operational Tooling Test Suite for `ClusterConfigValidator`, `ClusterBootstrapEngine`, and `CliClusterController` (Phase 15 Milestone 15.4).

use brain_services::planning::{
    CliClusterController, ClusterBootstrapEngine, ClusterConfigError, ClusterConfigValidator,
    ClusterNodeConfig, ConsensusRole, NodeId, SequenceNumber, TermId,
};
use uuid::Uuid;

#[test]
fn test_cluster_config_validator_invariants() {
    let node_id = NodeId(Uuid::new_v4());

    // 1. Valid config -> Success
    let valid_config = ClusterNodeConfig {
        node_id,
        listen_address: "127.0.0.1:8080".to_string(),
        peer_addresses: vec!["127.0.0.1:8081".to_string(), "127.0.0.1:8082".to_string()],
        snapshot_chunk_size: 64 * 1024,
        heartbeat_interval_ms: 50,
        election_timeout_ms: 150,
    };
    let validated = ClusterConfigValidator::validate(valid_config.clone()).unwrap();
    assert_eq!(validated.config, valid_config);

    // 2. Empty listen address -> InvalidAddress
    let mut invalid_addr = valid_config.clone();
    invalid_addr.listen_address = "".to_string();
    assert!(matches!(
        ClusterConfigValidator::validate(invalid_addr),
        Err(ClusterConfigError::InvalidAddress(_))
    ));

    // 3. Duplicate peer address -> DuplicatePeer
    let mut dup_peers = valid_config.clone();
    dup_peers.peer_addresses = vec!["127.0.0.1:8081".to_string(), "127.0.0.1:8081".to_string()];
    assert!(matches!(
        ClusterConfigValidator::validate(dup_peers),
        Err(ClusterConfigError::DuplicatePeer(_))
    ));

    // 4. Heartbeat >= Election timeout -> InvalidTimeoutRelationship
    let mut invalid_timeouts = valid_config.clone();
    invalid_timeouts.heartbeat_interval_ms = 200;
    invalid_timeouts.election_timeout_ms = 150;
    assert!(matches!(
        ClusterConfigValidator::validate(invalid_timeouts),
        Err(ClusterConfigError::InvalidTimeoutRelationship { .. })
    ));

    // 5. Chunk size < 1KB -> InvalidChunkSize
    let mut small_chunk = valid_config;
    small_chunk.snapshot_chunk_size = 512;
    assert!(matches!(
        ClusterConfigValidator::validate(small_chunk),
        Err(ClusterConfigError::InvalidChunkSize(_))
    ));
}

#[test]
fn test_cluster_bootstrap_engine_and_cli_controller_plans() {
    let node_id = NodeId(Uuid::new_v4());
    let peer_id1 = NodeId(Uuid::new_v4());
    let peer_id2 = NodeId(Uuid::new_v4());

    let config = ClusterNodeConfig {
        node_id,
        listen_address: "127.0.0.1:8080".to_string(),
        peer_addresses: vec!["127.0.0.1:8081".to_string(), "127.0.0.1:8082".to_string()],
        snapshot_chunk_size: 64 * 1024,
        heartbeat_interval_ms: 50,
        election_timeout_ms: 150,
    };

    let validated = ClusterConfigValidator::validate(config).unwrap();

    // 1. Bootstrap cluster -> Returns engine and report
    let (engine, report) = ClusterBootstrapEngine::bootstrap(&validated, 1000);
    assert_eq!(report.node_id, node_id);
    assert_eq!(report.peer_count, 2);
    assert!(report.is_success);

    // 2. Query cluster status via CliClusterController
    engine.transition_to(ConsensusRole::Leader, TermId(1), Some(node_id));
    let status = CliClusterController::get_cluster_status(node_id, &engine, 2);
    assert_eq!(status.node_id, node_id);
    assert_eq!(status.role, ConsensusRole::Leader);
    assert_eq!(status.term, TermId(1));
    assert_eq!(status.active_peers, 2);

    // 3. Plan add_node via CliClusterController
    let new_peer = NodeId(Uuid::new_v4());
    let add_plan = CliClusterController::plan_add_node(&[node_id, peer_id1, peer_id2], new_peer);
    assert_eq!(add_plan.action, "add_node");
    assert_eq!(add_plan.target_node, new_peer);

    // 4. Plan snapshot trigger via CliClusterController
    let snap_plan = CliClusterController::plan_snapshot_trigger(new_peer, SequenceNumber(100));
    assert_eq!(snap_plan.target_node, new_peer);
    assert_eq!(snap_plan.snapshot_sequence, SequenceNumber(100));
}
