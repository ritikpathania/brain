use brain_core::repositories::NodeRepository;
use brain_domain::retrieval::evaluation::{
    EvaluationDataset, EvaluationTestCase, NoRegressionPolicy, PublicationRecommendation,
    RelevanceJudgment,
};
use brain_domain::retrieval::features::MinMaxNormalizer;
use brain_domain::retrieval::models::{
    CalibrationMetadata, RankingWeights, SnapshotMetadata, SnapshotVersion, WeightSnapshot,
};
use brain_domain::{
    temporal::{Clock, RecencyPolicy, TimePoint},
    Node, NodeId, NodeType,
};
use brain_services::retrieval::evaluator::{EvaluationContext, OfflineEvaluator};
use brain_services::retrieval::feature_extractor::DefaultFeatureExtractor;
use brain_storage::TestStorage;
use std::sync::Arc;

struct FixedClock(TimePoint);
impl Clock for FixedClock {
    fn now(&self) -> TimePoint {
        self.0
    }
}

fn make_snapshot(version: u64, sem: f64, graph: f64, rec: f64, temp: f64) -> WeightSnapshot {
    let metadata = SnapshotMetadata {
        version: SnapshotVersion::new(version),
        created_at: TimePoint::from_unix_seconds(1620000000),
        calibration_metadata: CalibrationMetadata::new("LinearAdjustment".to_string(), None),
    };
    let weights = RankingWeights::new(
        brain_domain::retrieval::models::RankingWeight::new(sem).unwrap(),
        brain_domain::retrieval::models::RankingWeight::new(graph).unwrap(),
        brain_domain::retrieval::models::RankingWeight::new(rec).unwrap(),
        brain_domain::retrieval::models::RankingWeight::new(temp).unwrap(),
    );
    WeightSnapshot { metadata, weights }
}

#[test]
fn test_offline_evaluator_determinism_and_invariants() {
    let test_store = TestStorage::new();
    let sqlite = test_store.storage();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    NodeRepository::save(
        sqlite,
        &Node::new(node_a, "UniqueQueryA".to_string(), NodeType::Concept),
    )
    .unwrap();
    NodeRepository::save(
        sqlite,
        &Node::new(node_b, "AlphaBetaNode".to_string(), NodeType::Concept),
    )
    .unwrap();

    let candidate_nodes = vec![
        NodeRepository::find_by_id(sqlite, &node_a)
            .unwrap()
            .unwrap(),
        NodeRepository::find_by_id(sqlite, &node_b)
            .unwrap()
            .unwrap(),
    ];

    let judgments = vec![
        RelevanceJudgment {
            node_id: node_a,
            score: 3.0,
        },
        RelevanceJudgment {
            node_id: node_b,
            score: 0.0,
        },
    ];

    let case1 = EvaluationTestCase {
        query: "UniqueQueryA".to_string(),
        candidates: candidate_nodes.clone(),
        temporal_edges: vec![],
        judgments: judgments.clone(),
    };

    // Task 4: EvaluationDataset packaging
    let dataset = EvaluationDataset {
        version: "v1_test_dataset".to_string(),
        cases: vec![case1],
    };

    let ref_time = TimePoint::from_unix_seconds(1620000000);
    let policy = RecencyPolicy::None;
    let extractor = Arc::new(DefaultFeatureExtractor::new(ref_time, policy));
    let normalizer = Arc::new(MinMaxNormalizer);

    let evaluator = OfflineEvaluator::new(extractor, normalizer);

    let baseline = make_snapshot(1, 1.0, 1.0, 1.0, 1.0);
    let candidate = make_snapshot(2, 1.2, 1.0, 1.0, 1.0);

    let pub_policy = NoRegressionPolicy;
    let clock = FixedClock(TimePoint::from_unix_seconds(1625000000));

    let context = EvaluationContext {
        dataset: &dataset,
        k: 2,
        normalizer_strategy: brain_domain::retrieval::features::NormalizationContext::BatchMinMax,
        publication_policy: &pub_policy,
        repos: sqlite,
        clock: &clock,
    };

    // Invariant 1: Ranking Determinism
    // Running evaluate() twice on identical dataset, snapshots, repos, and clock produces byte-for-byte identical reports.
    let report1 = evaluator.evaluate(&candidate, &baseline, &context).unwrap();
    let report2 = evaluator.evaluate(&candidate, &baseline, &context).unwrap();

    let json1 = serde_json::to_string(&report1).unwrap();
    let json2 = serde_json::to_string(&report2).unwrap();
    assert_eq!(json1, json2);

    // Verify aggregate score calculations and Approve recommendation
    assert_eq!(report1.recommendation, PublicationRecommendation::Approve);
    assert_eq!(report1.comparison.baseline.ndcg_k.value(), 1.0);
    assert_eq!(report1.comparison.candidate.ndcg_k.value(), 1.0);

    test_store.assert_clean();
}

#[test]
fn test_order_independence_and_zero_bias_invariants() {
    let test_store = TestStorage::new();
    let sqlite = test_store.storage();

    let node_a = NodeId::new();
    let node_b = NodeId::new();

    NodeRepository::save(
        sqlite,
        &Node::new(node_a, "QueryA".to_string(), NodeType::Concept),
    )
    .unwrap();
    NodeRepository::save(
        sqlite,
        &Node::new(node_b, "QueryB".to_string(), NodeType::Concept),
    )
    .unwrap();

    let candidate_a = NodeRepository::find_by_id(sqlite, &node_a)
        .unwrap()
        .unwrap();
    let candidate_b = NodeRepository::find_by_id(sqlite, &node_b)
        .unwrap()
        .unwrap();

    let case_a = EvaluationTestCase {
        query: "QueryA".to_string(),
        candidates: vec![candidate_a],
        temporal_edges: vec![],
        judgments: vec![RelevanceJudgment {
            node_id: node_a,
            score: 2.0,
        }],
    };

    let case_b = EvaluationTestCase {
        query: "QueryB".to_string(),
        candidates: vec![candidate_b],
        temporal_edges: vec![],
        judgments: vec![RelevanceJudgment {
            node_id: node_b,
            score: 2.0,
        }],
    };

    // Case c is empty candidates — must be skipped for zero evaluation bias (Invariant 3)
    let case_c_empty = EvaluationTestCase {
        query: "EmptyCandidates".to_string(),
        candidates: vec![],
        temporal_edges: vec![],
        judgments: vec![],
    };

    // Dataset 1 order: [a, b, c]
    let dataset1 = EvaluationDataset {
        version: "ds1".to_string(),
        cases: vec![case_a.clone(), case_b.clone(), case_c_empty.clone()],
    };

    // Dataset 2 order: [b, a] (different ordering of cases)
    let dataset2 = EvaluationDataset {
        version: "ds1".to_string(),
        cases: vec![case_b, case_a],
    };

    let extractor = Arc::new(DefaultFeatureExtractor::new(
        TimePoint::from_unix_seconds(1620000000),
        RecencyPolicy::None,
    ));
    let normalizer = Arc::new(MinMaxNormalizer);
    let evaluator = OfflineEvaluator::new(extractor, normalizer);

    let baseline = make_snapshot(1, 1.0, 1.0, 1.0, 1.0);
    let candidate = make_snapshot(2, 1.2, 1.0, 1.0, 1.0);
    let pub_policy = NoRegressionPolicy;
    let clock = FixedClock(TimePoint::from_unix_seconds(1625000000));

    let context1 = EvaluationContext {
        dataset: &dataset1,
        k: 1,
        normalizer_strategy: brain_domain::retrieval::features::NormalizationContext::BatchMinMax,
        publication_policy: &pub_policy,
        repos: sqlite,
        clock: &clock,
    };

    let context2 = EvaluationContext {
        dataset: &dataset2,
        k: 1,
        normalizer_strategy: brain_domain::retrieval::features::NormalizationContext::BatchMinMax,
        publication_policy: &pub_policy,
        repos: sqlite,
        clock: &clock,
    };

    let report1 = evaluator
        .evaluate(&candidate, &baseline, &context1)
        .unwrap();
    let report2 = evaluator
        .evaluate(&candidate, &baseline, &context2)
        .unwrap();

    // Invariant 2: Evaluation Dataset Order Independence
    // The aggregates must be mathematically identical (metrics values are identical)
    assert_eq!(
        report1.comparison.baseline.ndcg_k.value(),
        report2.comparison.baseline.ndcg_k.value()
    );
    assert_eq!(
        report1.comparison.candidate.ndcg_k.value(),
        report2.comparison.candidate.ndcg_k.value()
    );

    // Invariant 3: Zero Evaluation Bias
    // Assert evaluation count skips case_c_empty. Evaluated count is 2, not 3.
    // If it was 3, average would be biased (e.g. sum / 3 instead of sum / 2).
    // Let's verify by ensuring NDCG is exactly 1.0 (since individual case NDCGs are both 1.0)
    assert_eq!(report1.comparison.candidate.ndcg_k.value(), 1.0);

    test_store.assert_clean();
}
