use brain_domain::retrieval::experiment::{
    ExperimentConfiguration, ExperimentValidationError, RoutingStrategy, TrafficAllocation, Variant,
};
use brain_domain::retrieval::models::{
    CalibrationMetadata, RankingWeights, SnapshotMetadata, SnapshotVersion, WeightSnapshot,
};
use brain_domain::temporal::TimePoint;

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
fn test_traffic_allocation_validation() {
    assert!(TrafficAllocation::new(0.5).is_ok());
    assert!(TrafficAllocation::new(0.0).is_ok());
    assert!(TrafficAllocation::new(1.0).is_ok());
    assert!(TrafficAllocation::new(-0.01).is_err());
    assert!(TrafficAllocation::new(1.01).is_err());
    assert!(TrafficAllocation::new(f64::NAN).is_err());
}

#[test]
fn test_allocation_conservation_invariant() {
    let snap1 = make_dummy_snapshot(1);
    let snap2 = make_dummy_snapshot(2);

    let variants = vec![
        Variant {
            id: "baseline".to_string(),
            snapshot: snap1,
        },
        Variant {
            id: "canary".to_string(),
            snapshot: snap2,
        },
    ];

    // Case 1: Sum is exactly 1.0 -> Valid
    let allocations_valid = vec![
        ("baseline".to_string(), TrafficAllocation::new(0.9).unwrap()),
        ("canary".to_string(), TrafficAllocation::new(0.1).unwrap()),
    ];
    let config = ExperimentConfiguration::new(
        "exp-1".to_string(),
        1,
        variants.clone(),
        allocations_valid,
        RoutingStrategy::StickyHashRouting,
    );
    assert!(config.is_ok());
    assert_eq!(config.unwrap().version, 1);

    // Case 2: Sum is not 1.0 (e.g. 0.8) -> Fails with InvalidAllocationSum
    let allocations_invalid = vec![
        ("baseline".to_string(), TrafficAllocation::new(0.7).unwrap()),
        ("canary".to_string(), TrafficAllocation::new(0.1).unwrap()),
    ];
    let config_err = ExperimentConfiguration::new(
        "exp-1".to_string(),
        1,
        variants,
        allocations_invalid,
        RoutingStrategy::StickyHashRouting,
    );
    match config_err {
        Err(ExperimentValidationError::InvalidAllocationSum { sum }) => {
            assert!((sum - 0.8).abs() < 1e-9);
        }
        _ => panic!("Expected InvalidAllocationSum error"),
    }
}
