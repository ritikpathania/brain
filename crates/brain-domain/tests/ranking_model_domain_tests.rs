use brain_domain::retrieval::models::{
    FeatureId, SplitThreshold, LeafScore, DecisionTreeNode,
    DecisionTreeDefinition, DecisionTreeRankingModel,
    RankingSignals, NormalizedSignal
};

fn make_signals(sem: f64, graph: f64, rec: f64, temp: f64) -> RankingSignals {
    RankingSignals::new(
        NormalizedSignal::new(sem).unwrap(),
        NormalizedSignal::new(graph).unwrap(),
        NormalizedSignal::new(rec).unwrap(),
        NormalizedSignal::new(temp).unwrap(),
    )
}

#[test]
fn test_split_threshold_and_leaf_score_validations() {
    assert!(SplitThreshold::new(0.5).is_ok());
    assert!(SplitThreshold::new(0.0).is_ok());
    assert!(SplitThreshold::new(f64::NAN).is_err());
    assert!(SplitThreshold::new(f64::INFINITY).is_err());

    assert!(LeafScore::new(10.0).is_ok());
    assert!(LeafScore::new(-1.0).is_ok());
    assert!(LeafScore::new(f64::NAN).is_err());
}

#[test]
fn test_decision_tree_serialization_round_trip() {
    let tree_node = DecisionTreeNode::Split {
        feature: FeatureId::Semantic,
        threshold: SplitThreshold::new(0.5).unwrap(),
        left: Box::new(DecisionTreeNode::Leaf { score: LeafScore::new(0.2).unwrap() }),
        right: Box::new(DecisionTreeNode::Leaf { score: LeafScore::new(0.8).unwrap() }),
    };
    let definition = DecisionTreeDefinition { root: tree_node };

    // Invariant 1: Model Serialization Round Trip
    let json = serde_json::to_string(&definition).unwrap();
    let deserialized: DecisionTreeDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(definition, deserialized);
}

#[test]
fn test_decision_path_determinism_and_immutability() {
    let left_leaf = Box::new(DecisionTreeNode::Leaf { score: LeafScore::new(0.1).unwrap() });
    let right_leaf = Box::new(DecisionTreeNode::Leaf { score: LeafScore::new(0.9).unwrap() });
    
    let root = DecisionTreeNode::Split {
        feature: FeatureId::Graph,
        threshold: SplitThreshold::new(0.7).unwrap(),
        left: left_leaf,
        right: right_leaf,
    };
    let definition = DecisionTreeDefinition { root };

    // Invariant 2: Definition Immutability
    // Constructing model does not modify definition
    let model = DecisionTreeRankingModel::new(definition.clone());
    assert_eq!(model.definition(), &definition);

    let signals_left = make_signals(0.5, 0.6, 0.5, 0.5); // graph value = 0.6 < 0.7 -> left branch
    let signals_right = make_signals(0.5, 0.8, 0.5, 0.5); // graph value = 0.8 >= 0.7 -> right branch

    // Invariant 3: Decision Path Determinism
    // Traversals must trace identical path and scores deterministically
    let (score_a1, path_a1) = model.compiled().evaluate_with_path(&signals_left);
    let (score_a2, path_a2) = model.compiled().evaluate_with_path(&signals_left);
    assert_eq!(score_a1, 0.1);
    assert_eq!(score_a1, score_a2);
    assert_eq!(path_a1, vec![FeatureId::Graph]);
    assert_eq!(path_a1, path_a2);

    let (score_b1, path_b1) = model.compiled().evaluate_with_path(&signals_right);
    assert_eq!(score_b1, 0.9);
    assert_eq!(path_b1, vec![FeatureId::Graph]);
}
