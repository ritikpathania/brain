use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::search_index::*;
use brain_domain::projection::*;
use brain_services::projection::instance::*;
use brain_services::projection::runtime::*;
use brain_services::projection::store::*;
use uuid::Uuid;

#[test]
fn test_search_index_runtime_replay_equivalence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(SearchIndexReducer::new(
        ProjectionId::new("search_idx"),
        ProjectionVersion(1),
    ));
    let instance = ProjectionInstance::new(reducer);
    runtime.register_projection(instance).unwrap();

    let fact_id = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let now = Timestamp::now();

    let fact = FactVersion {
        id: fact_id.clone(),
        assertion_id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };

    let assertion = SemanticAssertion {
        id: assertion_id,
        kind: AssertionKind::Relationship,
        subject: entity_id,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Inverted Lexical Search Engine".to_string())),
    };

    let events = vec![FactEvent::FactRecorded {
        fact,
        assertion: Some(assertion),
    }];
    runtime.catchup_all(events.iter(), Watermark(1)).unwrap();
}

#[test]
fn test_search_index_mixed_event_sequence() {
    let store = Box::new(InMemoryCheckpointStore::new());
    let mut runtime = ProjectionRuntime::new(store);

    let reducer = Box::new(SearchIndexReducer::new(
        ProjectionId::new("search_mixed"),
        ProjectionVersion(1),
    ));
    let instance = ProjectionInstance::new(reducer);
    runtime.register_projection(instance).unwrap();

    let fact_id1 = FactVersionId(Uuid::new_v4());
    let fact_id2 = FactVersionId(Uuid::new_v4());
    let assertion_id1 = AssertionId(Uuid::new_v4());
    let assertion_id2 = AssertionId(Uuid::new_v4());
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
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
        subject: entity_id,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("Old Version Content".to_string())),
    };

    let fact2 = FactVersion {
        id: fact_id2.clone(),
        assertion_id: assertion_id2,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        temporal: TemporalWindow::new(now, now, now, None).unwrap(),
        supersedes: Some(fact_id1.clone()),
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "test".to_string() },
            derived_from: vec![],
        },
    };
    let assertion2 = SemanticAssertion {
        id: assertion_id2,
        kind: AssertionKind::Relationship,
        subject: entity_id,
        predicate: PredicateId(Uuid::new_v4()),
        object: AssertionTarget::Value(LiteralValue::String("New Version Content".to_string())),
    };

    let events = vec![
        FactEvent::FactRecorded { fact: fact1, assertion: Some(assertion1) },
        FactEvent::FactRecorded { fact: fact2, assertion: Some(assertion2) },
        FactEvent::FactSuperseded {
            old_fact_id: fact_id1.clone(),
            new_fact_id: fact_id2.clone(),
            superseded_at: now,
        },
        FactEvent::FactArchived {
            fact_id: fact_id2.clone(),
            archived_at: now,
        },
    ];

    runtime.catchup_all(events.iter(), Watermark(4)).unwrap();
}
