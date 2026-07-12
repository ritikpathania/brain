use std::time::Duration;
use brain_domain::consolidation::{
    SimilarityScore, PromotionScore, ConfidenceScore, StalenessAge,
    MetricConstructionError, Consolidator, ConsolidationPolicy,
    ConsolidationActionType
};
use brain_domain::entities::{KnowledgeGraph, Node, Edge, RelationKind};
use brain_domain::identifiers::{NodeId, EdgeId};

#[test]
fn test_metric_wrappers_range_validation() {
    // SimilarityScore [0.0, 1.0]
    assert!(SimilarityScore::new(0.5).is_ok());
    assert!(SimilarityScore::new(0.0).is_ok());
    assert!(SimilarityScore::new(1.0).is_ok());
    
    assert_eq!(
        SimilarityScore::new(-0.1).unwrap_err(),
        MetricConstructionError::OutOfRange { val: -0.1, min: 0.0, max: 1.0 }
    );
    assert_eq!(
        SimilarityScore::new(1.05).unwrap_err(),
        MetricConstructionError::OutOfRange { val: 1.05, min: 0.0, max: 1.0 }
    );
    assert_eq!(
        SimilarityScore::new(f64::NAN).unwrap_err(),
        MetricConstructionError::NotFinite { val: f64::NAN }
    );

    // PromotionScore [0.0, 1.0]
    assert!(PromotionScore::new(0.9).is_ok());
    assert_eq!(
        PromotionScore::new(-0.2).unwrap_err(),
        MetricConstructionError::OutOfRange { val: -0.2, min: 0.0, max: 1.0 }
    );

    // ConfidenceScore [0.0, 1.0]
    assert!(ConfidenceScore::new(0.95).is_ok());
    assert_eq!(
        ConfidenceScore::new(1.5).unwrap_err(),
        MetricConstructionError::OutOfRange { val: 1.5, min: 0.0, max: 1.0 }
    );

    // StalenessAge (always non-negative via Duration wrapper)
    let age = StalenessAge::new(Duration::from_secs(3600)).unwrap();
    assert_eq!(age.value(), 3600);
}

#[test]
fn test_metric_round_trip_invariant() {
    let inputs = [0.0, 0.1, 0.5, 0.9, 1.0];
    for &val in &inputs {
        let sim = SimilarityScore::new(val).unwrap();
        assert_eq!(sim.value(), val);

        let prom = PromotionScore::new(val).unwrap();
        assert_eq!(prom.value(), val);

        let conf = ConfidenceScore::new(val).unwrap();
        assert_eq!(conf.value(), val);
    }
}

#[test]
fn test_consolidator_analyze_and_plan_behavior() {
    let mut graph = KnowledgeGraph::new();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    
    // Create duplicate nodes
    let n1 = Node::new(node_a, "Duplicate Label".to_string(), brain_domain::entities::NodeKind::Concept);
    let n2 = Node::new(node_b, "duplicate label  ".to_string(), brain_domain::entities::NodeKind::Concept); // case and trim variance
    
    graph.nodes.insert(node_a, n1);
    graph.nodes.insert(node_b, n2);

    // Edge for promotion (high weight 0.95)
    let edge_promote_id = EdgeId::new(node_a, node_b, RelationKind::Uses.id());
    let edge_promote = Edge::new(node_a, node_b, RelationKind::Uses, 0.95);
    graph.edges.insert(edge_promote_id.clone(), edge_promote);

    // Edge for archival (low weight 0.05)
    let node_c = NodeId::new();
    let edge_archive_id = EdgeId::new(node_b, node_c, RelationKind::DependsOn.id());
    let edge_archive = Edge::new(node_b, node_c, RelationKind::DependsOn, 0.05);
    graph.edges.insert(edge_archive_id.clone(), edge_archive);

    let policy = ConsolidationPolicy {
        promotion_weight_threshold: 0.8,
        pruning_weight_threshold: 0.1,
        staleness_age_threshold_secs: 100,
    };
    let consolidator = Consolidator::new(policy);

    // 1. Analyze
    let analysis = consolidator.analyze(&graph);
    assert_eq!(analysis.duplicate_node_groups.len(), 1);
    assert_eq!(analysis.promotion_candidates.len(), 1);
    assert_eq!(analysis.archival_candidates.len(), 1);

    // 2. Plan
    let actions = consolidator.plan(analysis);
    
    // There should be:
    // - 1 MergeNodes action
    // - 1 PromoteToSemantic action
    // - 1 ArchiveEdge action
    assert_eq!(actions.len(), 3);

    let has_merge = actions.iter().any(|act| matches!(act.action, ConsolidationActionType::MergeNodes { .. }));
    let has_promote = actions.iter().any(|act| matches!(act.action, ConsolidationActionType::PromoteToSemantic { .. }));
    let has_archive = actions.iter().any(|act| matches!(act.action, ConsolidationActionType::ArchiveEdge { .. }));

    assert!(has_merge);
    assert!(has_promote);
    assert!(has_archive);
}

#[test]
fn test_idempotence_and_ordering_determinism_invariants() {
    let mut graph = KnowledgeGraph::new();
    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let n1 = Node::new(node_a, "Duplicate".to_string(), brain_domain::entities::NodeKind::Concept);
    let n2 = Node::new(node_b, "Duplicate".to_string(), brain_domain::entities::NodeKind::Concept);
    graph.nodes.insert(node_a, n1);
    graph.nodes.insert(node_b, n2);

    let policy = ConsolidationPolicy {
        promotion_weight_threshold: 0.8,
        pruning_weight_threshold: 0.1,
        staleness_age_threshold_secs: 100,
    };
    let consolidator = Consolidator::new(policy);

    // Invariant: Action Ordering Determinism
    // Repeated plans on the same input generate identical sorted list outputs
    let analysis1 = consolidator.analyze(&graph);
    let actions1 = consolidator.plan(analysis1);

    let analysis2 = consolidator.analyze(&graph);
    let actions2 = consolidator.plan(analysis2);

    assert_eq!(actions1.len(), actions2.len());
    for i in 0..actions1.len() {
        assert_eq!(actions1[i].action, actions2[i].action);
        assert_eq!(actions1[i].rationale, actions2[i].rationale);
        assert_eq!(actions1[i].confidence.value(), actions2[i].confidence.value());
    }

    // Invariant: Idempotence
    // If we apply the plan (here mocked by removing the duplicate cluster from analysis view),
    // running consolidator on the consolidated graph should yield no new actions.
    let mut graph_consolidated = graph.clone();
    // Simulate node merge: remove redundant node
    graph_consolidated.nodes.remove(&node_b);

    let analysis_idemp = consolidator.analyze(&graph_consolidated);
    let actions_idemp = consolidator.plan(analysis_idemp);
    assert_eq!(actions_idemp.len(), 0, "Idempotency violated: expected zero actions on a consolidated graph");
}
