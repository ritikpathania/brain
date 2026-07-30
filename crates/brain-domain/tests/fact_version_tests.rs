use brain_domain::bkf::fact_version::*;
use brain_domain::bkf::value_objects::*;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[test]
fn test_temporal_window_invariants() {
    let base = SystemTime::UNIX_EPOCH;
    let t1 = Timestamp(base + Duration::from_secs(100));
    let t2 = Timestamp(base + Duration::from_secs(200));

    assert!(TemporalWindow::new(t1, t2, t1, Some(t2)).is_ok());
    
    // Invalid asserted_at > observed_at (t2 > t1)
    assert!(TemporalWindow::new(t2, t1, t1, None).is_err());
    
    // Invalid valid_from > valid_to (t2 > t1)
    assert!(TemporalWindow::new(t1, t1, t2, Some(t1)).is_err());
}

#[test]
fn test_fact_version_construction() {
    let t1 = Timestamp::now();
    let window = TemporalWindow::new(t1, t1, t1, None).unwrap();
    let confidence = Confidence::new(0.95).unwrap();
    
    let fact = FactVersion {
        id: FactVersionId(Uuid::new_v4()),
        assertion_id: AssertionId(Uuid::new_v4()),
        lifecycle: FactLifecycle::Candidate,
        confidence,
        temporal: window,
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "user1".to_string() },
            derived_from: vec![],
        },
    };

    assert_eq!(fact.lifecycle, FactLifecycle::Candidate);
    assert!(!fact.temporal.is_historical());
}

#[test]
fn test_derived_historical_status() {
    let base = SystemTime::UNIX_EPOCH;
    let t1 = Timestamp(base + Duration::from_secs(100));
    let t2 = Timestamp(base + Duration::from_secs(200));

    let active_window = TemporalWindow::new(t1, t1, t1, None).unwrap();
    let closed_window = TemporalWindow::new(t1, t1, t1, Some(t2)).unwrap();
    
    assert!(!active_window.is_historical());
    assert!(closed_window.is_historical());
}
