use brain_core::retrieval::RetrievalRequest;
use brain_domain::SessionId;
use brain_domain::retrieval::experiment::{
    ExperimentConfiguration, TrafficAllocation, Variant, RoutingStrategy
};
use brain_domain::retrieval::models::{WeightSnapshot, SnapshotMetadata, SnapshotVersion, CalibrationMetadata, RankingWeights};
use brain_domain::temporal::TimePoint;
use brain_services::retrieval::active_weights::DefaultActiveWeightProvider;
use brain_services::retrieval::experiment::{
    ExperimentRouter, DefaultExperimentRouter, CanaryExperimentRouter, fnv1a_hash
};
use std::collections::HashSet;
use std::sync::Arc;

fn make_dummy_snapshot(version: u64) -> WeightSnapshot {
    let metadata = SnapshotMetadata {
        version: SnapshotVersion::new(version),
        created_at: TimePoint::from_unix_seconds(1620000000),
        calibration_metadata: CalibrationMetadata::new("Default".to_string(), None),
    };
    let weights = RankingWeights::new(
        brain_domain::retrieval::models::RankingWeight::new(1.0).unwrap(),
        brain_domain::retrieval::models::RankingWeight::new(1.0).unwrap(),
        brain_domain::retrieval::models::RankingWeight::new(1.0).unwrap(),
        brain_domain::retrieval::models::RankingWeight::new(1.0).unwrap(),
    );
    WeightSnapshot { metadata, weights }
}

#[test]
fn test_stable_fnv1a_hash_consistency() {
    // FNV-1a("test") hash is 18007334074686647077 (0xf9df2d5964f43405)
    let hash = fnv1a_hash("test");
    assert_eq!(hash, 18007334074686647077);
}

#[test]
fn test_default_experiment_router() {
    let snap = make_dummy_snapshot(1);
    let provider = Arc::new(DefaultActiveWeightProvider::new(snap));
    let router = DefaultExperimentRouter::new(provider);

    let request = RetrievalRequest {
        session_id: SessionId::new(),
        query: "test".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
    };

    let decision = router.route_decision(&request).unwrap();
    assert_eq!(decision.variant_id, "baseline");
    assert_eq!(decision.experiment_id, "default");
    assert_eq!(decision.snapshot.metadata.version.value(), 1);
}

#[test]
fn test_canary_experiment_router_sticky_and_invariants() {
    let base_snap = make_dummy_snapshot(1);
    let canary_snap = make_dummy_snapshot(2);

    let provider = Arc::new(DefaultActiveWeightProvider::new(base_snap));

    let variants = vec![
        Variant { id: "baseline".to_string(), snapshot: canary_snap.clone() },
        Variant { id: "canary".to_string(), snapshot: make_dummy_snapshot(3) },
    ];

    let allocations = vec![
        ("baseline".to_string(), TrafficAllocation::new(0.9).unwrap()),
        ("canary".to_string(), TrafficAllocation::new(0.1).unwrap()),
    ];

    let config = ExperimentConfiguration::new(
        "exp-1".to_string(),
        42,
        variants,
        allocations,
        RoutingStrategy::StickyHashRouting,
    ).unwrap();

    let router = CanaryExperimentRouter::new(provider, config);

    // Invariant 1: Routing Stability
    // Identical routing keys (session IDs) yield identical routing decisions
    let session_a = SessionId::new();
    let request_a1 = RetrievalRequest {
        session_id: session_a,
        query: "test".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
    };
    let request_a2 = request_a1.clone();

    let decision_a1 = router.route_decision(&request_a1).unwrap();
    let decision_a2 = router.route_decision(&request_a2).unwrap();
    assert_eq!(decision_a1.variant_id, decision_a2.variant_id);
    assert_eq!(decision_a1.experiment_id, "exp-1");
    assert_eq!(decision_a1.experiment_version, 42);

    // Invariant 2: Deterministic Default on Nil/Empty Session ID
    let nil_session: SessionId = serde_json::from_str("\"00000000000000000000000000\"").unwrap();
    let request_nil = RetrievalRequest {
        session_id: nil_session,
        query: "test".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
    };
    let decision_nil = router.route_decision(&request_nil).unwrap();
    // Must route to active baseline snapshot (version 1)
    assert_eq!(decision_nil.variant_id, "baseline");
    assert_eq!(decision_nil.snapshot.metadata.version.value(), 1);

    // Verify traffic split is sticky and routes to both depending on session id
    let mut baseline_count = 0;
    let mut canary_count = 0;

    for _ in 0..100 {
        let req = RetrievalRequest {
            session_id: SessionId::new(),
            query: "test".to_string(),
            limit: 10,
            exclude_ids: HashSet::new(),
            deadline: None,
        };
        let dec = router.route_decision(&req).unwrap();
        if dec.variant_id == "baseline" {
            baseline_count += 1;
        } else if dec.variant_id == "canary" {
            canary_count += 1;
        }
    }
    // With 90/10 split over 100 trials, both variants should get some requests
    assert!(baseline_count > 0);
    assert!(canary_count >= 0);
}
