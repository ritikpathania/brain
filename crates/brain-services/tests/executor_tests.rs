use brain_domain::bkf::*;
use brain_services::reflection::executor::*;
use uuid::Uuid;

struct MockEmptySnapshot;

impl KnowledgeSnapshotView for MockEmptySnapshot {
    fn entities(&self) -> &[KnowledgeEntity] {
        &[]
    }
    fn assertions(&self) -> &[SemanticAssertion] {
        &[]
    }
    fn predicates(&self) -> &[Predicate] {
        &[]
    }
    fn active_facts(&self) -> &[FactVersion] {
        &[]
    }
}

#[test]
fn test_validator_rejects_self_supersession() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());

    let plan = RewritePlan {
        pass_id: PassId::new("test_pass"),
        reason: RewriteReason::Contradiction,
        rationale: "Self supersession test".to_string(),
        execution_cost: 1,
        operations: vec![RewriteOperation::SupersedeFact {
            old_fact_id: id1,
            new_fact_id: id1, // Self supersession error!
            closed_at: t1,
        }],
    };

    let snapshot = MockEmptySnapshot;
    assert!(RewriteValidator::validate(&plan, &snapshot).is_err());
}

#[test]
fn test_validator_accepts_valid_plan() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());
    let id2 = FactVersionId(Uuid::new_v4());

    let plan = RewritePlan {
        pass_id: PassId::new("test_pass"),
        reason: RewriteReason::Contradiction,
        rationale: "Valid plan".to_string(),
        execution_cost: 1,
        operations: vec![RewriteOperation::SupersedeFact {
            old_fact_id: id1,
            new_fact_id: id2,
            closed_at: t1,
        }],
    };

    let snapshot = MockEmptySnapshot;
    assert!(RewriteValidator::validate(&plan, &snapshot).is_ok());
}

#[test]
fn test_lowering_operations_to_fact_events() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());
    let id2 = FactVersionId(Uuid::new_v4());

    let plan = RewritePlan {
        pass_id: PassId::new("test_pass"),
        reason: RewriteReason::Contradiction,
        rationale: "Lowering plan".to_string(),
        execution_cost: 1,
        operations: vec![
            RewriteOperation::SupersedeFact {
                old_fact_id: id1,
                new_fact_id: id2,
                closed_at: t1,
            },
            RewriteOperation::ArchiveFact {
                fact_id: id1,
                archived_at: t1,
            },
        ],
    };

    let events = V2RewriteExecutor::lower_plan_to_events(&plan).unwrap();
    assert_eq!(events.len(), 2);
    match &events[0] {
        FactEvent::FactSuperseded {
            old_fact_id,
            new_fact_id,
            ..
        } => {
            assert_eq!(*old_fact_id, id1);
            assert_eq!(*new_fact_id, id2);
        }
        _ => panic!("Expected FactSuperseded event"),
    }
}
