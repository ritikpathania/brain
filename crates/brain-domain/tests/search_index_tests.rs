use brain_domain::bkf::*;
use brain_domain::projection::search_index::*;
use uuid::Uuid;

#[test]
fn test_search_index_tokenization_and_symmetric_query() {
    let mut state = SearchIndexState::default();
    let fact_id1 = FactVersionId(Uuid::new_v4());
    let fact_id2 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());
    let assertion_id2 = AssertionId(Uuid::new_v4());
    let entity_id1 = KnowledgeEntityId(Uuid::new_v4());
    let entity_id2 = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact1 = FactVersion {
        id: fact_id1.clone(),
        assertion_id: assertion_id1,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };
    let assertion1 = SemanticAssertion {
        id: assertion_id1,
        kind: AssertionKind::Relationship,
        subject: entity_id1.clone(),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Rust Knowledge Graph".to_string())),
    };

    let fact2 = FactVersion {
        id: fact_id2.clone(),
        assertion_id: assertion_id2,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };
    let assertion2 = SemanticAssertion {
        id: assertion_id2,
        kind: AssertionKind::Relationship,
        subject: entity_id2.clone(),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Compiler Optimization".to_string())),
    };

    // Test record_fact & duplicate record idempotency
    state.record_fact(&fact1, &assertion1);
    state.record_fact(&fact1, &assertion1);
    state.record_fact(&fact2, &assertion2);

    assert_eq!(state.len(), 5); // "rust", "knowledge", "graph", "compiler", "optimization"

    // Symmetric query
    let matched_entities = state.search_entities("rust-knowledge");
    assert!(matched_entities.contains(&entity_id1));

    // Multi-token OR query semantics ("rust" matches fact1/entity1, "compiler" matches fact2/entity2)
    let or_matched_entities = state.search_entities("rust compiler");
    assert_eq!(or_matched_entities.len(), 2);
    assert!(or_matched_entities.contains(&entity_id1));
    assert!(or_matched_entities.contains(&entity_id2));

    let matched_facts = state.search_facts("graph");
    assert!(matched_facts.contains(&fact_id1));

    // Test duplicate remove_active_fact idempotency & internal cleanup
    state.remove_active_fact(&fact_id1);
    state.remove_active_fact(&fact_id1);
    state.remove_active_fact(&fact_id2);
    state.remove_active_fact(&fact_id2);

    assert_eq!(state.len(), 0);
    assert!(state.is_empty());
    assert!(state.search_entities("rust").is_empty());
}
