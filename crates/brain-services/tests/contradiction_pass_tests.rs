use brain_domain::bkf::*;
use brain_services::reflection::pass_context::*;
use brain_services::reflection::passes::contradiction::*;
use std::time::{Duration, SystemTime};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct MockSnapshotWithContradiction {
    predicates: Vec<Predicate>,
    assertions: Vec<SemanticAssertion>,
    active_facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for MockSnapshotWithContradiction {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &self.assertions }
    fn predicates(&self) -> &[Predicate] { &self.predicates }
    fn active_facts(&self) -> &[FactVersion] { &self.active_facts }
}

#[test]
fn test_contradiction_pass_id_and_deps() {
    let pass = V2ContradictionPass;
    assert_eq!(pass.id().as_str(), "contradiction");
    assert_eq!(pass.dependencies().len(), 1);
    assert_eq!(pass.dependencies()[0].as_str(), "canonicalization");
}

#[test]
fn test_contradiction_pass_detects_exclusive_predicate_conflict() {
    let base = SystemTime::UNIX_EPOCH;
    let t1 = Timestamp(base + Duration::from_secs(100));
    let t2 = Timestamp(base + Duration::from_secs(200));

    let subject_id = KnowledgeEntityId(Uuid::new_v4());
    let pred_id = PredicateId(Uuid::new_v4());

    let pred = Predicate {
        id: pred_id,
        name: PredicateName::new("LivesIn").unwrap(),
        cardinality: PredicateCardinality::Exclusive,
        is_temporal: true,
        inverse: None,
    };

    let assert1 = SemanticAssertion {
        id: AssertionId(Uuid::new_v4()),
        kind: AssertionKind::Relationship,
        subject: subject_id,
        predicate: pred_id,
        object: AssertionTarget::Value(LiteralValue::String("Delhi".to_string())),
    };

    let assert2 = SemanticAssertion {
        id: AssertionId(Uuid::new_v4()),
        kind: AssertionKind::Relationship,
        subject: subject_id,
        predicate: pred_id,
        object: AssertionTarget::Value(LiteralValue::String("Mumbai".to_string())),
    };

    let fact1 = FactVersion {
        id: FactVersionId(Uuid::new_v4()),
        assertion_id: assert1.id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.9).unwrap(),
        temporal: TemporalWindow::new(t1, t1, t1, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "u1".to_string() },
            derived_from: vec![],
        },
    };

    let fact2 = FactVersion {
        id: FactVersionId(Uuid::new_v4()),
        assertion_id: assert2.id,
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.9).unwrap(),
        temporal: TemporalWindow::new(t2, t2, t2, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "u1".to_string() },
            derived_from: vec![],
        },
    };

    let snapshot = MockSnapshotWithContradiction {
        predicates: vec![pred],
        assertions: vec![assert1, assert2],
        active_facts: vec![fact1.clone(), fact2.clone()],
    };

    let context = V2ReflectionContext {
        now: Timestamp::now(),
        cancellation_token: CancellationToken::new(),
        max_operations_budget: 100,
    };

    let pass = V2ContradictionPass;
    let outcome = pass.analyze(&snapshot, &context).unwrap().unwrap();
    assert_eq!(outcome.plan.reason, RewriteReason::Contradiction);
    assert_eq!(outcome.plan.operations.len(), 1);

    match &outcome.plan.operations[0] {
        RewriteOperation::SupersedeFact { old_fact_id, new_fact_id, closed_at } => {
            assert_eq!(*old_fact_id, fact1.id);
            assert_eq!(*new_fact_id, fact2.id);
            assert_eq!(*closed_at, t2);
        }
        _ => panic!("Expected SupersedeFact operation"),
    }
}
