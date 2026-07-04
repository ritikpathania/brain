
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
