use brain_domain::retrieval::models::{
    RankingWeight, NormalizedSignal, SnapshotVersion, CalibrationMetadata,
    SnapshotMetadata, RankingWeights, WeightSnapshot,
    RankingSignals, RankingModelVersion,
    RankingModel, LinearRankingModel
};
use brain_domain::consolidation::MetricConstructionError;
use brain_domain::temporal::TimePoint;

#[test]
fn test_ranking_weight_validation() {
    assert!(RankingWeight::new(1.5).is_ok());
    assert!(matches!(RankingWeight::new(-0.1), Err(MetricConstructionError::OutOfRange { .. })));
    assert!(matches!(RankingWeight::new(f64::NAN), Err(MetricConstructionError::NotFinite { .. })));
}

#[test]
fn test_normalized_signal_validation() {
    assert!(NormalizedSignal::new(0.5).is_ok());
    assert!(matches!(NormalizedSignal::new(-0.01), Err(MetricConstructionError::OutOfRange { .. })));
    assert!(matches!(NormalizedSignal::new(1.01), Err(MetricConstructionError::OutOfRange { .. })));
}

#[test]
fn test_linear_ranking_model() {
    let w_sem = RankingWeight::new(1.0).unwrap();
    let w_graph = RankingWeight::new(2.0).unwrap();
    let w_rec = RankingWeight::new(0.5).unwrap();
    let w_temp = RankingWeight::new(1.5).unwrap();

    let weights = RankingWeights::new(w_sem, w_graph, w_rec, w_temp);
    assert_eq!(weights.semantic().value(), 1.0);
    assert_eq!(weights.graph().value(), 2.0);
    assert_eq!(weights.recency().value(), 0.5);
    assert_eq!(weights.temporal().value(), 1.5);

    let model = LinearRankingModel::new(weights);
    assert_eq!(model.version(), RankingModelVersion::V1Linear);

    let sig_sem = NormalizedSignal::new(0.8).unwrap();
    let sig_graph = NormalizedSignal::new(0.5).unwrap();
    let sig_rec = NormalizedSignal::new(1.0).unwrap();
    let sig_temp = NormalizedSignal::new(0.2).unwrap();

    let signals = RankingSignals::new(sig_sem, sig_graph, sig_rec, sig_temp);

    // Score = 1.0*0.8 + 2.0*0.5 + 0.5*1.0 + 1.5*0.2
    //       = 0.8 + 1.0 + 0.5 + 0.3 = 2.6
    let score = model.score(&signals);
    assert!((score - 2.6).abs() < 1e-9);
}

#[test]
fn test_snapshot_metadata_and_snapshot() {
    let version = SnapshotVersion::new(42);
    let created_at = TimePoint::from_unix_seconds(1600000000);
    let cal_meta = CalibrationMetadata::new("LinearAdjustment".to_string(), Some(0.01));
    assert_eq!(cal_meta.algorithm_used(), "LinearAdjustment");
    assert_eq!(cal_meta.validation_loss(), Some(0.01));

    let metadata = SnapshotMetadata {
        version,
        created_at,
        calibration_metadata: cal_meta,
    };

    let weights = RankingWeights::new(
        RankingWeight::new(1.0).unwrap(),
        RankingWeight::new(1.0).unwrap(),
        RankingWeight::new(1.0).unwrap(),
        RankingWeight::new(1.0).unwrap(),
    );

    let snapshot = WeightSnapshot {
        metadata,
        weights,
    };

    assert_eq!(snapshot.metadata.version.value(), 42);
}
