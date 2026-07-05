use brain_domain::retrieval::models::{
    WeightSnapshot, SnapshotMetadata, SnapshotVersion, CalibrationMetadata,
    RankingWeights, RankingModelVersion, FeatureId, SplitThreshold,
    LeafScore, DecisionTreeNode, DecisionTreeDefinition
};
use brain_domain::temporal::TimePoint;
use brain_services::retrieval::model_resolver::ModelDeserializer;

fn make_snapshot(version: u64, cal: CalibrationMetadata) -> WeightSnapshot {
    let metadata = SnapshotMetadata {
        version: SnapshotVersion::new(version),
        created_at: TimePoint::from_unix_seconds(1620000000),
        calibration_metadata: cal,
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
fn test_model_deserializer_linear_fallback_and_tree() {
    // Case 1: V1Linear / Default version
    let cal_linear = CalibrationMetadata::new("Linear".to_string(), None);
    let snap_linear = make_snapshot(1, cal_linear);

    let model = ModelDeserializer::resolve(&snap_linear).unwrap();
    assert_eq!(model.version(), RankingModelVersion::V1Linear);

    // Case 2: Valid V2DecisionTree
    let left_leaf = Box::new(DecisionTreeNode::Leaf { score: LeafScore::new(0.5).unwrap() });
    let right_leaf = Box::new(DecisionTreeNode::Leaf { score: LeafScore::new(1.5).unwrap() });
    let root = DecisionTreeNode::Split {
        feature: FeatureId::Semantic,
        threshold: SplitThreshold::new(0.4).unwrap(),
        left: left_leaf,
        right: right_leaf,
    };
    let definition = DecisionTreeDefinition { root };
    let json_params = serde_json::to_string(&definition).unwrap();

    let cal_tree = CalibrationMetadata::new("DecisionTree".to_string(), None)
        .with_model_details(Some(RankingModelVersion::V2DecisionTree), Some(json_params));
    let snap_tree = make_snapshot(2, cal_tree);

    let model_tree = ModelDeserializer::resolve(&snap_tree).unwrap();
    assert_eq!(model_tree.version(), RankingModelVersion::V2DecisionTree);

    // Case 3: Explicit failure on corrupted JSON
    let cal_corrupt = CalibrationMetadata::new("DecisionTree".to_string(), None)
        .with_model_details(Some(RankingModelVersion::V2DecisionTree), Some("corrupted_json_string".to_string()));
    let snap_corrupt = make_snapshot(3, cal_corrupt);

    let res_corrupt = ModelDeserializer::resolve(&snap_corrupt);
    assert!(res_corrupt.is_err());
    let err_msg = format!("{:?}", res_corrupt.err().unwrap());
    assert!(err_msg.contains("DecisionTree parsing failed"));

    // Case 4: Explicit failure on missing parameters
    let cal_missing = CalibrationMetadata::new("DecisionTree".to_string(), None)
        .with_model_details(Some(RankingModelVersion::V2DecisionTree), None);
    let snap_missing = make_snapshot(4, cal_missing);

    let res_missing = ModelDeserializer::resolve(&snap_missing);
    assert!(res_missing.is_err());
    let err_msg_missing = format!("{:?}", res_missing.err().unwrap());
    assert!(err_msg_missing.contains("Missing parameters"));
}
