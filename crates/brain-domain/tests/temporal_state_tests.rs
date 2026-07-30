use brain_domain::bkf::*;
use brain_domain::projection::temporal_state::*;
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
