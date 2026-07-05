use brain_domain::retrieval::evaluation::{
    NdcgScore, MrrScore, RecallScore, PrecisionScore,
    RelevanceJudgment, MetricCalculator, NoRegressionPolicy, PublicationPolicy,
    EvaluationComparison, PublicationRecommendation, EvaluationMetrics
};
use brain_domain::NodeId;

#[test]
fn test_metric_value_objects_invariants() {
    assert!(NdcgScore::new(0.5).is_ok());
    assert!(NdcgScore::new(0.0).is_ok());
    assert!(NdcgScore::new(1.0).is_ok());
    assert!(NdcgScore::new(-0.1).is_err());
    assert!(NdcgScore::new(1.1).is_err());
    assert!(NdcgScore::new(f64::NAN).is_err());
    assert!(NdcgScore::new(f64::INFINITY).is_err());

    let m = MrrScore::new(0.75).unwrap();
    assert_eq!(m.value(), 0.75);
}

#[test]
fn test_metric_calculator_precision_recall_mrr_ndcg() {
    let node1 = NodeId::new();
    let node2 = NodeId::new();
    let node3 = NodeId::new();

    let ranked = vec![node1, node2, node3];
    let judgments = vec![
        RelevanceJudgment { node_id: node1, score: 3.0 },
        RelevanceJudgment { node_id: node2, score: 0.0 },
        RelevanceJudgment { node_id: node3, score: 1.0 },
    ];

    // Total relevant (score > 0.0) is 2 (node1, node3)
    // At K = 2: top 2 contains node1 (relevant) and node2 (not relevant)
    let p_2 = MetricCalculator::precision(&ranked, &judgments, 2);
    assert_eq!(p_2, 0.5); // 1 relevant retrieved / 2 = 0.5

    let r_2 = MetricCalculator::recall(&ranked, &judgments, 2);
    assert_eq!(r_2, 0.5); // 1 relevant retrieved / 2 total relevant = 0.5

    let rr = MetricCalculator::reciprocal_rank(&ranked, &judgments);
    assert_eq!(rr, 1.0); // node1 is first and relevant -> 1/1 = 1.0

    // DCG@3 = (2^3 - 1)/log2(2) + (2^0 - 1)/log2(3) + (2^1 - 1)/log2(4)
    //       = 7 / 1.0 + 0 / 1.58 + 1 / 2.0 = 7.5
    // IDCG@3 (ideal order node1, node3, node2) = 7 / log2(2) + 1 / log2(3) + 0 = 7.63092975
    // NDCG@3 = 7.5 / 7.63092975 = 0.9828422
    let ndcg_3 = MetricCalculator::ndcg(&ranked, &judgments, 3);
    assert!((ndcg_3 - 0.9828422279067397).abs() < 1e-9);

    // Ideal order ranking gets exactly 1.0
    let ideal_ranked = vec![node1, node3, node2];
    let ndcg_ideal = MetricCalculator::ndcg(&ideal_ranked, &judgments, 3);
    assert_eq!(ndcg_ideal, 1.0);
}

#[test]
fn test_metric_monotonicity_invariant() {
    let node1 = NodeId::new();
    let node2 = NodeId::new();
    let node3 = NodeId::new();

    let ranked = vec![node1, node2, node3];
    
    // Baseline judgments
    let judgments_baseline = vec![
        RelevanceJudgment { node_id: node1, score: 1.0 },
        RelevanceJudgment { node_id: node2, score: 1.0 },
        RelevanceJudgment { node_id: node3, score: 1.0 },
    ];

    let dcg_baseline = MetricCalculator::dcg(&ranked, &judgments_baseline, 3);

    // Increased relevance score for node1 (from 1.0 to 3.0)
    let judgments_increased = vec![
        RelevanceJudgment { node_id: node1, score: 3.0 },
        RelevanceJudgment { node_id: node2, score: 1.0 },
        RelevanceJudgment { node_id: node3, score: 1.0 },
    ];

    let dcg_increased = MetricCalculator::dcg(&ranked, &judgments_increased, 3);

    // Metric Monotonicity: Increasing relevance score must not decrease DCG/NDCG
    assert!(dcg_increased >= dcg_baseline);
}

#[test]
fn test_publication_policy_no_regression() {
    let policy = NoRegressionPolicy;
    assert_eq!(policy.name(), "NoRegressionPolicy");

    let met_baseline = EvaluationMetrics {
        ndcg_k: NdcgScore::new(0.8).unwrap(),
        mrr: MrrScore::new(0.7).unwrap(),
        recall_k: RecallScore::new(0.9).unwrap(),
        precision_k: PrecisionScore::new(0.6).unwrap(),
    };

    let met_candidate_better = EvaluationMetrics {
        ndcg_k: NdcgScore::new(0.85).unwrap(),
        mrr: MrrScore::new(0.75).unwrap(),
        recall_k: RecallScore::new(0.9).unwrap(),
        precision_k: PrecisionScore::new(0.65).unwrap(),
    };

    let met_candidate_worse = EvaluationMetrics {
        ndcg_k: NdcgScore::new(0.75).unwrap(),
        mrr: MrrScore::new(0.65).unwrap(),
        recall_k: RecallScore::new(0.85).unwrap(),
        precision_k: PrecisionScore::new(0.55).unwrap(),
    };

    // Case 1: Candidate is better (positive NDCG improvement)
    let comp_better = EvaluationComparison {
        baseline: met_baseline,
        candidate: met_candidate_better,
        ndcg_improvement: 0.05,
        mrr_improvement: 0.05,
    };
    assert_eq!(policy.evaluate_recommendation(&comp_better), PublicationRecommendation::Approve);

    // Case 2: Candidate is worse (negative NDCG improvement)
    let comp_worse = EvaluationComparison {
        baseline: met_baseline,
        candidate: met_candidate_worse,
        ndcg_improvement: -0.05,
        mrr_improvement: -0.05,
    };
    match policy.evaluate_recommendation(&comp_worse) {
        PublicationRecommendation::Reject { reason } => {
            assert!(reason.contains("Candidate NDCG degraded"));
        }
        _ => panic!("Expected rejection"),
    }
}
