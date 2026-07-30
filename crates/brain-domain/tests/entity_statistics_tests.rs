use brain_domain::bkf::*;
use brain_domain::projection::entity_statistics::*;
use uuid::Uuid;

#[test]
fn test_entity_statistics_record_supersede_archive_lifecycle() {
    let mut state = EntityStatisticsState::default();
    let fact_id1 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let pred_id1 = PredicateId(Uuid::new_v4());
    let t10 = Timestamp::now();

    let fact1 = FactVersion {
        id: fact_id1.clone(),
        assertion_id: assertion_id1,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.8).unwrap(),
        temporal: TemporalWindow::new(t10, t10, t10, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };

    let assertion1 = SemanticAssertion {
        id: assertion_id1,
        kind: AssertionKind::Relationship,
        subject: entity_id.clone(),
        predicate: pred_id1.clone(),
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    // Test record_fact & duplicate record idempotency
    state.record_fact(&fact1, &assertion1);
    state.record_fact(&fact1, &assertion1);

    let stats = state.get(&entity_id).unwrap();
    assert_eq!(stats.total_fact_versions, 1);
    assert_eq!(stats.active_facts_count, 1);
    assert_eq!(stats.unique_predicates_count, 1);
    assert!((stats.average_confidence() - 0.8).abs() < 1e-4);

    // Test duplicate supersede_fact idempotency
    let t15 = Timestamp::now();
    state.supersede_fact(&fact_id1, t15);
    state.supersede_fact(&fact_id1, t15);

    let stats_after_supersede = state.get(&entity_id).unwrap();
    assert_eq!(stats_after_supersede.active_facts_count, 0);
    assert_eq!(stats_after_supersede.superseded_facts_count, 1);
    assert_eq!(stats_after_supersede.unique_predicates_count, 0);

    // Test duplicate archive_fact idempotency
    let t20 = Timestamp::now();
    state.archive_fact(&fact_id1, t20);
    state.archive_fact(&fact_id1, t20);

    let stats_after_archive = state.get(&entity_id).unwrap();
    assert_eq!(stats_after_archive.active_facts_count, 0);
    assert_eq!(stats_after_archive.archived_facts_count, 0); // Already removed by supersede
    assert_eq!(stats_after_archive.unique_predicates_count, 0);
    assert_eq!(stats_after_archive.average_confidence(), 0.0);
    assert_eq!(stats_after_archive.active_confidence_sum, 0.0);
}
