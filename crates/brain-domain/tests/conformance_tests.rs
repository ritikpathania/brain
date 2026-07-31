use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::conformance::*;
use brain_domain::projection::entity_statistics::*;
use brain_domain::projection::graph_adjacency::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use uuid::Uuid;

#[test]
fn test_graph_adjacency_conformance() {
    let reducer = GraphAdjacencyReducer::new(ProjectionId::new("adj"), ProjectionVersion(1));
    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id,
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual {
                user_id: "test".to_string(),
            },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: KnowledgeEntityId(Uuid::new_v4()),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(
        reducer.clone(),
        std::slice::from_ref(&event),
    );
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer.clone(), &event);
    ProjectionConformanceSuite::assert_replay_equivalence(reducer.clone(), reducer, &[event]);
}

#[test]
fn test_temporal_state_conformance() {
    let reducer = TemporalStateReducer::new(ProjectionId::new("temporal"), ProjectionVersion(1));
    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id,
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual {
                user_id: "test".to_string(),
            },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: KnowledgeEntityId(Uuid::new_v4()),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(
        reducer.clone(),
        std::slice::from_ref(&event),
    );
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer.clone(), &event);
    ProjectionConformanceSuite::assert_replay_equivalence(reducer.clone(), reducer, &[event]);
}

#[test]
fn test_entity_statistics_conformance() {
    let reducer =
        EntityStatisticsReducer::new(ProjectionId::new("statistics"), ProjectionVersion(1));
    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id,
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual {
                user_id: "test".to_string(),
            },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: KnowledgeEntityId(Uuid::new_v4()),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(
        reducer.clone(),
        std::slice::from_ref(&event),
    );
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer.clone(), &event);
    ProjectionConformanceSuite::assert_replay_equivalence(reducer.clone(), reducer, &[event]);
}

#[test]
fn test_search_index_conformance() {
    let reducer = SearchIndexReducer::new(ProjectionId::new("search_index"), ProjectionVersion(1));
    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id,
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual {
                user_id: "test".to_string(),
            },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: KnowledgeEntityId(Uuid::new_v4()),
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Search Index Text".to_string())),
    };

    let event = FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    };

    ProjectionConformanceSuite::assert_reset_clears_state(
        reducer.clone(),
        std::slice::from_ref(&event),
    );
    ProjectionConformanceSuite::assert_duplicate_event_idempotency(reducer.clone(), &event);
    ProjectionConformanceSuite::assert_replay_equivalence(reducer.clone(), reducer, &[event]);
}
