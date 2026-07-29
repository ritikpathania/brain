//! Unit and property tests for Milestone A1 domain models.

use brain_domain::{
    CanonicalEntity, ContradictionKind, EntityId, KnowledgeEvidence, KnowledgeState,
    ObservationRecord, ObservationSummary, RetentionTier, SourceId,
};

#[test]
fn test_knowledge_state_ordering_and_defaults() {
    assert_eq!(KnowledgeState::default(), KnowledgeState::Observed);
    assert!(KnowledgeState::Observed < KnowledgeState::Verified);
    assert!(KnowledgeState::Verified < KnowledgeState::Reinforced);
    assert!(KnowledgeState::Reinforced < KnowledgeState::Weak);
    assert!(KnowledgeState::Weak < KnowledgeState::Deprecated);
    assert!(KnowledgeState::Deprecated < KnowledgeState::Archived);
}

#[test]
fn test_observation_record_and_retention_tier() {
    let source = SourceId("test_source".to_string());
    let obs = ObservationRecord {
        source_id: source.clone(),
        timestamp: 1700000000,
        confidence: 0.95,
        extractor_info: "unit_test_extractor".to_string(),
    };

    let retention = RetentionTier::Recent(vec![obs]);
    match &retention {
        RetentionTier::Recent(vec) => {
            assert_eq!(vec.len(), 1);
            assert_eq!(vec[0].source_id.to_string(), "test_source");
            assert_eq!(vec[0].confidence, 0.95);
        }
        _ => panic!("Expected Recent tier"),
    }

    let summary = ObservationSummary {
        total_observations: 10,
        first_observed_at: 1700000000,
        last_observed_at: 1700000100,
        last_reinforced_at: 1700000050,
        average_confidence: 0.88,
    };
    let aggregated = RetentionTier::Aggregated(summary);
    assert!(matches!(aggregated, RetentionTier::Aggregated(_)));
}

#[test]
fn test_canonical_entity_immutability_and_evidence() {
    let id = EntityId::new();
    let entity = CanonicalEntity {
        id,
        preferred_name: "John Doe".to_string(),
        aliases: vec!["J. Doe".to_string(), "Johnny".to_string()],
        merge_history: vec![],
        evidence: KnowledgeEvidence::default(),
        state: KnowledgeState::Observed,
    };

    assert_eq!(entity.id, id);
    assert_eq!(entity.preferred_name, "John Doe");
    assert_eq!(entity.aliases.len(), 2);
    assert_eq!(entity.state, KnowledgeState::Observed);
    assert_eq!(entity.evidence.source_reliability, 1.0);
}

#[test]
fn test_contradiction_kind_variants() {
    assert_ne!(ContradictionKind::Logical, ContradictionKind::Temporal);
}
