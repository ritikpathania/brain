use brain_domain::bkf::*;
use brain_services::reflection::conflict_resolver::*;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[test]
fn property_test_confidence_bounds_invariant() {
    // Confidence must strictly reject values < 0.0 or > 1.0
    assert!(Confidence::new(-0.01).is_err());
    assert!(Confidence::new(1.01).is_err());
    assert!(Confidence::new(0.0).is_ok());
    assert!(Confidence::new(1.0).is_ok());
    assert!(Confidence::new(0.5).is_ok());
}

#[test]
fn property_test_temporal_window_invariants() {
    let base = SystemTime::UNIX_EPOCH;
    let t1 = Timestamp(base + Duration::from_secs(100));
    let t2 = Timestamp(base + Duration::from_secs(200));

    // asserted_at > observed_at is invalid
    assert!(TemporalWindow::new(t2, t1, t1, None).is_err());
    // valid_from > valid_to is invalid
    assert!(TemporalWindow::new(t1, t1, t2, Some(t1)).is_err());
    // valid window
    assert!(TemporalWindow::new(t1, t1, t1, Some(t2)).is_ok());
}

#[test]
fn property_test_conflict_resolver_idempotence_and_shuffle_invariance() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());
    let id2 = FactVersionId(Uuid::new_v4());

    let plan1 = RewritePlan {
        pass_id: PassId::new("pass_1"),
        reason: RewriteReason::Contradiction,
        rationale: "Rationale 1".to_string(),
        execution_cost: 1,
        operations: vec![RewriteOperation::SupersedeFact {
            old_fact_id: id1,
            new_fact_id: id2,
            closed_at: t1,
        }],
    };

    let plan2 = RewritePlan {
        pass_id: PassId::new("pass_2"),
        reason: RewriteReason::TemporalExpiration,
        rationale: "Rationale 2".to_string(),
        execution_cost: 2,
        operations: vec![RewriteOperation::ArchiveFact {
            fact_id: id1,
            archived_at: t1,
        }],
    };

    let res_a = ConflictResolver::resolve(vec![plan1.clone(), plan2.clone()]).unwrap();
    let res_b = ConflictResolver::resolve(vec![plan2.clone(), plan1.clone()]).unwrap();

    // Deterministic equality regardless of input order
    assert_eq!(res_a.operations, res_b.operations);
    assert_eq!(res_a.execution_cost, res_b.execution_cost);
}
