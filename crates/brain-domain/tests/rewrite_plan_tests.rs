use brain_domain::bkf::fact_version::*;
use brain_domain::bkf::rewrite_plan::*;
use brain_domain::bkf::value_objects::*;
use uuid::Uuid;

#[test]
fn test_rewrite_plan_construction() {
    let t1 = Timestamp::now();
    let window = TemporalWindow::new(t1, t1, t1, None).unwrap();
    let confidence = Confidence::new(0.95).unwrap();
    let id1 = FactVersionId(Uuid::new_v4());
    let id2 = FactVersionId(Uuid::new_v4());

    let fact = FactVersion {
        id: id1,
        assertion_id: AssertionId(Uuid::new_v4()),
        lifecycle: FactLifecycle::Candidate,
        confidence,
        temporal: window,
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual {
                user_id: "user1".to_string(),
            },
            derived_from: vec![],
        },
    };

    let plan = RewritePlan {
        pass_id: PassId::new("contradiction_pass"),
        reason: RewriteReason::Contradiction,
        rationale: "Closed superseded temporal window for exclusive predicate".to_string(),
        execution_cost: 2,
        operations: vec![
            RewriteOperation::RecordFact(fact),
            RewriteOperation::SupersedeFact {
                old_fact_id: id1,
                new_fact_id: id2,
                closed_at: t1,
            },
            RewriteOperation::MergeFacts {
                source_fact_ids: vec![id1],
                target_fact_id: id2,
            },
            RewriteOperation::ArchiveFact {
                fact_id: id1,
                archived_at: t1,
            },
        ],
    };

    assert_eq!(plan.pass_id.as_str(), "contradiction_pass");
    assert_eq!(plan.reason, RewriteReason::Contradiction);
    assert_eq!(plan.operations.len(), 4);
}
