use brain_domain::retrieval::models::{
    WeightSnapshot, SnapshotMetadata, SnapshotVersion, CalibrationMetadata,
    RankingWeights, RankingWeight
};
use brain_domain::temporal::TimePoint;
use brain_services::retrieval::active_weights::{ActiveWeightProvider, DefaultActiveWeightProvider};
use brain_services::retrieval::calibration::WeightCalibrationService;
use brain_services::retrieval::temporal::LearnedTemporalScorer;
use brain_core::retrieval::{RankingStrategy, RetrievalRequest};
use brain_core::repositories::NodeRepository;
use brain_domain::{Node, NodeType, NodeId};
use std::sync::Arc;

fn make_snapshot(version: u64, sem: f64, graph: f64, rec: f64, temp: f64) -> WeightSnapshot {
    let metadata = SnapshotMetadata {
        version: SnapshotVersion::new(version),
        created_at: TimePoint::from_unix_seconds(1620000000),
        calibration_metadata: CalibrationMetadata::new("LinearAdjustment".to_string(), None),
    };
    let weights = RankingWeights::new(
        RankingWeight::new(sem).unwrap(),
        RankingWeight::new(graph).unwrap(),
        RankingWeight::new(rec).unwrap(),
        RankingWeight::new(temp).unwrap(),
    );
    WeightSnapshot {
        metadata,
        weights,
    }
}

#[test]
fn test_invariant_publication_equivalence() {
    let test_store = brain_storage::TestStorage::new();
    let sqlite = Arc::new(test_store.storage().clone());

    // Setup active provider with initial version 1
    let v1 = make_snapshot(1, 1.0, 1.0, 1.0, 1.0);
    let provider = Arc::new(DefaultActiveWeightProvider::new(v1));
    let service = WeightCalibrationService::new(sqlite.clone(), provider.clone());

    // Record baseline behavior
    let base_active = provider.active_snapshot().unwrap();
    assert_eq!(base_active.metadata.version.value(), 1);

    // Publish version 2
    let v2 = make_snapshot(2, 2.0, 1.5, 0.5, 3.0);
    service.publish_snapshot(v2).unwrap();
    assert_eq!(provider.active_snapshot().unwrap().metadata.version.value(), 2);

    // Immediately rollback to version 1
    service.rollback_to(SnapshotVersion::new(1)).unwrap();

    // Verify active snapshot is exactly restored to version 1
    let active_restored = provider.active_snapshot().unwrap();
    assert_eq!(active_restored.metadata.version.value(), 1);
    assert_eq!(active_restored.weights.semantic().value(), 1.0);
    assert_eq!(active_restored.weights.graph().value(), 1.0);
    assert_eq!(active_restored.weights.recency().value(), 1.0);
    assert_eq!(active_restored.weights.temporal().value(), 1.0);

    test_store.assert_clean();
}

#[test]
fn test_invariant_model_transparency() {
    let test_store = brain_storage::TestStorage::new();
    let sqlite = test_store.storage();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    NodeRepository::save(sqlite, &Node::new(node_a, "Alpha".to_string(), NodeType::Concept)).unwrap();
    NodeRepository::save(sqlite, &Node::new(node_b, "Beta".to_string(), NodeType::Concept)).unwrap();

    // Setup active provider
    let initial = make_snapshot(1, 1.5, 2.0, 0.5, 1.0);
    let provider = Arc::new(DefaultActiveWeightProvider::new(initial));
    let sqlite_arc = Arc::new((*sqlite).clone());

    let scorer = LearnedTemporalScorer::new(
        provider.clone(),
        sqlite_arc.clone(),
        TimePoint::from_unix_seconds(1620000000),
        brain_domain::temporal::RecencyPolicy::None,
    );

    let req = RetrievalRequest {
        session_id: brain_domain::SessionId::new(),
        query: "Alpha".to_string(),
        limit: 2,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let input_nodes = vec![
        NodeRepository::find_by_id(sqlite, &node_a).unwrap().unwrap(),
        NodeRepository::find_by_id(sqlite, &node_b).unwrap().unwrap(),
    ];

    let ranked = scorer.rank(&req, input_nodes).unwrap();
    assert_eq!(ranked.len(), 2);
    // Node A matches "Alpha" query exactly, so it must rank first
    assert_eq!(ranked[0].id, node_a);

    test_store.assert_clean();
}
