
use brain_domain::retrieval::models::{
    WeightSnapshot, SnapshotMetadata, SnapshotVersion, CalibrationMetadata,
    RankingWeights, RankingWeight
};
use brain_domain::temporal::TimePoint;
use brain_services::retrieval::active_weights::{ActiveWeightProvider, DefaultActiveWeightProvider};

fn make_dummy_snapshot(version: u64) -> WeightSnapshot {
    let metadata = SnapshotMetadata {
        version: SnapshotVersion::new(version),
        created_at: TimePoint::from_unix_seconds(1620000000),
        calibration_metadata: CalibrationMetadata::new("LinearAdjustment".to_string(), None),
    };
    let weights = RankingWeights::new(
        RankingWeight::new(1.0).unwrap(),
        RankingWeight::new(1.0).unwrap(),
        RankingWeight::new(1.0).unwrap(),
        RankingWeight::new(1.0).unwrap(),
    );
    WeightSnapshot {
        metadata,
        weights,
    }
}

#[test]
fn test_default_active_weight_provider() {
    let initial = make_dummy_snapshot(1);
    let provider = DefaultActiveWeightProvider::new(initial);

    let active = provider.active_snapshot().unwrap();
    assert_eq!(active.metadata.version.value(), 1);

    let new_snap = make_dummy_snapshot(2);
    provider.swap_active(new_snap).unwrap();

    let active2 = provider.active_snapshot().unwrap();
    assert_eq!(active2.metadata.version.value(), 2);
}

#[test]
fn test_learned_temporal_scorer() {
    let test_store = brain_storage::TestStorage::new();
    let sqlite = test_store.storage();

    // 1. Create and save some nodes
    use brain_core::repositories::NodeRepository;
    use brain_domain::{Node, Edge, NodeType, RelationKind, NodeId, temporal::RecencyPolicy};
    use brain_services::retrieval::temporal::LearnedTemporalScorer;
    use brain_core::retrieval::{RankingStrategy, RetrievalRequest};

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();

    NodeRepository::save(sqlite, &Node::new(node_a, "EntityA".to_string(), NodeType::Concept)).unwrap();
    NodeRepository::save(sqlite, &Node::new(node_b, "EntityB".to_string(), NodeType::Concept)).unwrap();
    NodeRepository::save(sqlite, &Node::new(node_c, "EntityC".to_string(), NodeType::Concept)).unwrap();

    // 2. Create temporal edges
    // Node A is linked twice, Node B is linked once, Node C is linked zero times
    let temp_edge_1 = brain_domain::temporal::TemporalEdge {
        edge: Edge::new(node_a, node_b, RelationKind::AssociatedWith, 1.0),
        validity: brain_domain::temporal::TemporalValidity::new(vec![]),
        observed_at: TimePoint::from_unix_seconds(1620000000),
    };
    let temp_edge_2 = brain_domain::temporal::TemporalEdge {
        edge: Edge::new(node_a, node_c, RelationKind::AssociatedWith, 1.0),
        validity: brain_domain::temporal::TemporalValidity::new(vec![]),
        observed_at: TimePoint::from_unix_seconds(1620000010),
    };

    sqlite.save_temporal_edge(&temp_edge_1).unwrap();
    sqlite.save_temporal_edge(&temp_edge_2).unwrap();

    // 3. Initialize Scorer with 1.0 weights
    let initial = make_dummy_snapshot(1);
    let provider = std::sync::Arc::new(DefaultActiveWeightProvider::new(initial));
    let sqlite_arc = std::sync::Arc::new((*sqlite).clone());

    let scorer = LearnedTemporalScorer::new(
        provider.clone(),
        sqlite_arc.clone(),
        TimePoint::from_unix_seconds(1620000020),
        RecencyPolicy::Linear { horizon_secs: 100.0 },
    );

    let req = RetrievalRequest {
        session_id: brain_domain::SessionId::new(),
        query: "EntityA".to_string(),
        limit: 3,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let input_nodes = vec![
        NodeRepository::find_by_id(sqlite, &node_a).unwrap().unwrap(),
        NodeRepository::find_by_id(sqlite, &node_b).unwrap().unwrap(),
        NodeRepository::find_by_id(sqlite, &node_c).unwrap().unwrap(),
    ];

    let ranked = scorer.rank(&req, input_nodes).unwrap();
    assert_eq!(ranked.len(), 3);
    // Node A should be first because it matches "EntityA" exactly in the query and has highest temporal centrality
    assert_eq!(ranked[0].id, node_a);

    test_store.assert_clean();
}

#[test]
fn test_calibration_engine_idempotency_and_linear_math() {
    use brain_domain::retrieval::models::{
        CalibrationPolicy, CalibrationPolicyVersion, CalibrationAlgorithmType,
        FeedbackEvent, NormalizedSignal, RankingSignals
    };
    use brain_domain::identifiers::NodeId;
    use brain_services::retrieval::calibration::CalibrationEngine;

    let engine = CalibrationEngine::new();
    let initial = make_dummy_snapshot(1);

    let policy = CalibrationPolicy {
        version: CalibrationPolicyVersion::new(1),
        algorithm: CalibrationAlgorithmType::LinearAdjustment,
        learning_rate: 0.1,
        regularization: 0.01,
        min_feedback_events: 1,
    };

    // 1. Idempotency test (no events)
    let (candidate, report) = engine.run_calibration(&initial, &[], &policy).unwrap();
    assert_eq!(candidate.metadata.version.value(), 1);
    assert!(!report.publication_decision);
    assert_eq!(report.validation_loss, 0.0);

    // 2. Linear math test
    let sig_val = NormalizedSignal::new(0.9).unwrap();
    let signals = RankingSignals::new(sig_val, sig_val, sig_val, sig_val);
    let event = FeedbackEvent {
        id: "evt-1".to_string(),
        schema_version: 1,
        query: "test".to_string(),
        node_id: NodeId::new(),
        selected: true,
        timestamp: 1620000000,
        ranking_position: 2,
        context: serde_json::to_string(&signals).unwrap(),
    };

    let (candidate2, report2) = engine.run_calibration(&initial, &[event], &policy).unwrap();
    assert_eq!(candidate2.metadata.version.value(), 2);
    assert!(report2.publication_decision);
    // Loss should be (rank - 1.0)^2 = (2.0 - 1.0)^2 = 1.0
    assert_eq!(report2.validation_loss, 1.0);

    // Initial weight was 1.0
    // Delta = lr * (signal - 0.5) = 0.1 * (0.9 - 0.5) = 0.04
    // Intermediate weight = 1.0 + 0.04 = 1.04
    // Regularized weight = 1.04 * (1.0 - reg) = 1.04 * 0.99 = 1.0296
    assert!((candidate2.weights.semantic().value() - 1.0296).abs() < 1e-6);
}

#[test]
fn test_weight_calibration_service_lifecycle_and_rollback() {
    use brain_domain::retrieval::models::{
        CalibrationPolicy, CalibrationPolicyVersion, CalibrationAlgorithmType,
        FeedbackEvent, NormalizedSignal, RankingSignals, SnapshotVersion
    };
    use brain_domain::identifiers::NodeId;
    use brain_services::retrieval::calibration::WeightCalibrationService;

    let test_store = brain_storage::TestStorage::new();
    let sqlite = std::sync::Arc::new(test_store.storage().clone());
    let initial = make_dummy_snapshot(1);
    let provider = std::sync::Arc::new(DefaultActiveWeightProvider::new(initial));

    let service = WeightCalibrationService::new(sqlite.clone(), provider.clone());

    // Ingest event
    let sig_val = NormalizedSignal::new(0.8).unwrap();
    let signals = RankingSignals::new(sig_val, sig_val, sig_val, sig_val);
    let event = FeedbackEvent {
        id: "evt-2".to_string(),
        schema_version: 1,
        query: "test".to_string(),
        node_id: NodeId::new(),
        selected: true,
        timestamp: 1620000000,
        ranking_position: 1,
        context: serde_json::to_string(&signals).unwrap(),
    };
    service.ingest_feedback(event).unwrap();

    // Calibrate
    let policy = CalibrationPolicy {
        version: CalibrationPolicyVersion::new(1),
        algorithm: CalibrationAlgorithmType::LinearAdjustment,
        learning_rate: 0.1,
        regularization: 0.01,
        min_feedback_events: 1,
    };
    let (candidate, _report) = service.calibrate_weights(&policy).unwrap();
    assert_eq!(candidate.metadata.version.value(), 2);

    // Publish
    service.publish_snapshot(candidate).unwrap();
    assert_eq!(provider.active_snapshot().unwrap().metadata.version.value(), 2);

    // Monotonicity check: publishing version 1 or 2 again must fail
    let bad_snapshot = make_dummy_snapshot(1);
    let res = service.publish_snapshot(bad_snapshot);
    assert!(res.is_err());

    // Rollback to version 1
    service.rollback_to(SnapshotVersion::new(1)).unwrap();
    assert_eq!(provider.active_snapshot().unwrap().metadata.version.value(), 1);

    test_store.assert_clean();
}


