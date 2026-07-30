use brain_domain::bkf::*;
use brain_services::reflection::pass_context::*;
use brain_services::reflection::passes::confidence_recalculation::*;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct MockSnapshotForConfidence {
    active_facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for MockSnapshotForConfidence {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &self.active_facts }
}

#[test]
fn test_confidence_recalculation_id_and_deps() {
    let pass = V2ConfidenceRecalculationPass;
    assert_eq!(pass.id().as_str(), "confidence_recalculation");
    assert_eq!(pass.dependencies().len(), 3);
}

#[test]
fn test_confidence_recalculation_boosts_corroborated_facts() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());
    let parent_id = FactVersionId(Uuid::new_v4());

    let fact = FactVersion {
        id: id1,
        assertion_id: AssertionId(Uuid::new_v4()),
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.5).unwrap(),
        temporal: TemporalWindow::new(t1, t1, t1, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Inference {
                pass_id: "transitive".to_string(),
                rationale: "Derived rule".to_string(),
            },
            derived_from: vec![parent_id, FactVersionId(Uuid::new_v4())],
        },
    };

    let snapshot = MockSnapshotForConfidence {
        active_facts: vec![fact],
    };

    let context = V2ReflectionContext {
        now: t1,
        cancellation_token: CancellationToken::new(),
        max_operations_budget: 100,
    };

    let pass = V2ConfidenceRecalculationPass;
    let outcome = pass.analyze(&snapshot, &context).unwrap().unwrap();
    assert_eq!(outcome.plan.reason, RewriteReason::ConfidenceIncrease);
    assert_eq!(outcome.plan.operations.len(), 1);

    match &outcome.plan.operations[0] {
        RewriteOperation::RecordFact(new_fact) => {
            assert!(new_fact.confidence.value() > 0.5);
        }
        _ => panic!("Expected RecordFact operation"),
    }
}
