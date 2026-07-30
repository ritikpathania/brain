use brain_domain::bkf::events::*;
use brain_domain::bkf::*;
use brain_domain::projection::temporal_state::*;
use brain_domain::projection::*;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

fn ts(secs: u64) -> Timestamp {
    Timestamp(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
}

#[test]
fn test_temporal_state_record_insert_close_and_point_in_time_lookup() {
    let mut state = TemporalState::default();
    let fact_id = TemporalFactId(FactVersionId(Uuid::new_v4()));
    let entity_id = KnowledgeEntityId(Uuid::new_v4());
    let predicate_id = PredicateId(Uuid::new_v4());
    let t10 = ts(10);
    let t20 = ts(20);

    let record = TemporalRecord {
        id: fact_id.clone(),
        entity_id: entity_id.clone(),
        predicate_id: predicate_id.clone(),
        valid_from: t10,
        valid_until: None,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(1.0).unwrap(),
        previous_version: None,
    };

    // Test insert and active status
    state.insert_record(record.clone());
    assert_eq!(state.active_facts(&entity_id), &[fact_id.clone()]);
    assert_eq!(state.timeline(&entity_id), &[fact_id.clone()]);
    assert!(state.is_active(&fact_id));

    // Test duplicate insertion idempotency
    state.insert_record(record);
    assert_eq!(state.timeline(&entity_id).len(), 1);

    // Test close interval & duplicate closure idempotency
    state.close_interval(&fact_id, t20, FactLifecycle::Archived);
    state.close_interval(&fact_id, t20, FactLifecycle::Archived);
    assert!(state.active_facts(&entity_id).is_empty());
    assert!(!state.is_active(&fact_id));
    assert_eq!(state.record(&fact_id).unwrap().valid_until, Some(t20));

    // Test half-open interval boundaries [10, 20)
    assert!(state.facts_at(&entity_id, ts(9)).is_empty());
    assert_eq!(state.facts_at(&entity_id, ts(10)).len(), 1);
    assert_eq!(state.facts_at(&entity_id, ts(19)).len(), 1);
    assert!(state.facts_at(&entity_id, ts(20)).is_empty());
}

#[test]
fn test_temporal_state_reducer_event_application_and_reset() {
    let mut reducer = TemporalStateReducer::new(ProjectionId::new("temporal_state"), ProjectionVersion(1));
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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let record_event1 = FactEvent::FactRecorded {
        fact: fact1,
        assertion: Some(assertion1),
    };
    reducer.apply_event(&record_event1).unwrap();

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
        object: AssertionTarget::Entity(KnowledgeEntityId(Uuid::new_v4())),
    };

    let record_event2 = FactEvent::FactRecorded {
        fact: fact2,
        assertion: Some(assertion2),
    };
    reducer.apply_event(&record_event2).unwrap();

    // Verify previous_version lineage preservation
    let rec2 = reducer.state().record(&TemporalFactId(fact_id2)).unwrap();
    assert_eq!(rec2.previous_version, Some(fact_id1.clone()));

    // Test reset() empties state
    reducer.reset().unwrap();
    assert!(reducer.state().timeline(&entity_id).is_empty());
}
