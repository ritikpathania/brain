use brain_domain::bkf::*;
use brain_services::reflection::conflict_resolver::*;
use uuid::Uuid;

#[test]
fn test_conflict_resolver_merge_and_shuffle_invariance() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());
    let id2 = FactVersionId(Uuid::new_v4());
    let id3 = FactVersionId(Uuid::new_v4());

    let plan_a = RewritePlan {
        pass_id: PassId::new("pass_a"),
        reason: RewriteReason::Contradiction,
        rationale: "Contradiction pass".to_string(),
        execution_cost: 10,
        operations: vec![RewriteOperation::SupersedeFact {
            old_fact_id: id1,
            new_fact_id: id2,
            closed_at: t1,
        }],
    };

    let plan_b = RewritePlan {
        pass_id: PassId::new("pass_b"),
        reason: RewriteReason::Duplicate,
        rationale: "Duplicate pass".to_string(),
        execution_cost: 5,
        operations: vec![RewriteOperation::ArchiveFact {
            fact_id: id3,
            archived_at: t1,
        }],
    };

    // Order 1: [plan_a, plan_b]
    let merged_1 = ConflictResolver::resolve(vec![plan_a.clone(), plan_b.clone()]).unwrap();

    // Order 2: [plan_b, plan_a]
    let merged_2 = ConflictResolver::resolve(vec![plan_b.clone(), plan_a.clone()]).unwrap();

    // Verification: Both merged plans must contain identical operations in identical order
    assert_eq!(merged_1.operations, merged_2.operations);
    assert_eq!(merged_1.operations.len(), 2);
}

#[test]
fn test_conflict_resolver_deduplicates_identical_operations() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());

    let plan_a = RewritePlan {
        pass_id: PassId::new("pass_a"),
        reason: RewriteReason::TemporalExpiration,
        rationale: "Archive old fact".to_string(),
        execution_cost: 1,
        operations: vec![RewriteOperation::ArchiveFact {
            fact_id: id1,
            archived_at: t1,
        }],
    };

    let plan_b = RewritePlan {
        pass_id: PassId::new("pass_b"),
        reason: RewriteReason::TemporalExpiration,
        rationale: "Archive old fact".to_string(),
        execution_cost: 1,
        operations: vec![RewriteOperation::ArchiveFact {
            fact_id: id1,
            archived_at: t1,
        }],
    };

    let merged = ConflictResolver::resolve(vec![plan_a, plan_b]).unwrap();
    assert_eq!(merged.operations.len(), 1);
}
