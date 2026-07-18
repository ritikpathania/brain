use brain_domain::retrieval::features::{
    FeatureNormalizer, FeaturePipelineReporter, MinMaxNormalizer, NormalizationContext,
    RawFeatureVector,
};
use brain_domain::{Node, NodeId, NodeType};

#[test]
fn test_min_max_normalizer_batch_min_max() {
    let normalizer = MinMaxNormalizer;
    let raw = vec![
        RawFeatureVector {
            semantic: 10.0,
            graph: 1.0,
            recency: 0.1,
            temporal: 5.0,
        },
        RawFeatureVector {
            semantic: 20.0,
            graph: 3.0,
            recency: 0.9,
            temporal: 5.0,
        },
    ];
    let context = NormalizationContext::BatchMinMax;
    let signals = normalizer.normalize(&raw, &context).unwrap();
    assert_eq!(signals.len(), 2);

    // Dynamic min-max checks
    assert_eq!(signals[0].semantic.value(), 0.0);
    assert_eq!(signals[1].semantic.value(), 1.0);

    assert_eq!(signals[0].graph.value(), 0.0);
    assert_eq!(signals[1].graph.value(), 1.0);

    assert_eq!(signals[0].recency.value(), 0.0);
    assert_eq!(signals[1].recency.value(), 1.0);

    // Constant range maps to 1.0
    assert_eq!(signals[0].temporal.value(), 1.0);
    assert_eq!(signals[1].temporal.value(), 1.0);
}

#[test]
fn test_min_max_normalizer_fixed_ranges() {
    let normalizer = MinMaxNormalizer;
    let raw = vec![RawFeatureVector {
        semantic: 15.0,
        graph: 2.0,
        recency: 0.5,
        temporal: 4.0,
    }];
    let context = NormalizationContext::FixedRanges {
        semantic_range: (10.0, 20.0),
        graph_range: (0.0, 4.0),
        recency_range: (0.0, 1.0),
        temporal_range: (2.0, 6.0),
    };
    let signals = normalizer.normalize(&raw, &context).unwrap();
    assert_eq!(signals.len(), 1);

    // Expected value checks: (val - min) / (max - min)
    assert_eq!(signals[0].semantic.value(), 0.5);
    assert_eq!(signals[0].graph.value(), 0.5);
    assert_eq!(signals[0].recency.value(), 0.5);
    assert_eq!(signals[0].temporal.value(), 0.5);
}

#[test]
fn test_invariant_normalization_stability_and_ordering() {
    let normalizer = MinMaxNormalizer;
    let raw1 = vec![
        RawFeatureVector {
            semantic: 1.0,
            graph: 10.0,
            recency: 0.2,
            temporal: 3.0,
        },
        RawFeatureVector {
            semantic: 2.0,
            graph: 20.0,
            recency: 0.8,
            temporal: 6.0,
        },
    ];
    let raw2 = raw1.clone();
    let context = NormalizationContext::BatchMinMax;

    let signals1 = normalizer.normalize(&raw1, &context).unwrap();
    let signals2 = normalizer.normalize(&raw2, &context).unwrap();

    // Normalization Stability: byte-for-byte identical output for identical input
    assert_eq!(signals1, signals2);

    // Feature Ordering: index mapping matches exactly
    assert_eq!(signals1[0].semantic.value(), 0.0);
    assert_eq!(signals1[1].semantic.value(), 1.0);
}

#[test]
fn test_feature_pipeline_reporter() {
    let raw = vec![RawFeatureVector {
        semantic: 10.0,
        graph: 1.0,
        recency: 0.1,
        temporal: 5.0,
    }];
    let context = NormalizationContext::BatchMinMax;
    let signals = MinMaxNormalizer.normalize(&raw, &context).unwrap();

    let node_id = NodeId::new();
    let nodes = vec![Node::new(
        node_id,
        "TestNode".to_string(),
        NodeType::Concept,
    )];

    let reports = FeaturePipelineReporter::build_reports(&nodes, &raw, &signals, &context);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].node_id, node_id);
    assert_eq!(reports[0].raw_features, raw[0]);
    assert_eq!(reports[0].normalization_context, context);
    assert_eq!(reports[0].normalized_signals, signals[0]);
}
