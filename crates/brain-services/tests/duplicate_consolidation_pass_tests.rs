use brain_domain::bkf::*;
use brain_services::reflection::pass_context::*;
use brain_services::reflection::passes::duplicate_consolidation::*;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct MockSnapshotWithDuplicates {
    entities: Vec<KnowledgeEntity>,
    assertions: Vec<SemanticAssertion>,
    predicates: Vec<Predicate>,
    active_facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for MockSnapshotWithDuplicates {
    fn entities(&self) -> &[KnowledgeEntity] { &self.entities }
    fn assertions(&self) -> &[SemanticAssertion] { &self.assertions }
    fn predicates(&self) -> &[Predicate] { &self.predicates }
    fn active_facts(&self) -> &[FactVersion] { &self.active_facts }
}

#[test]
fn test_duplicate_consolidation_id_and_deps() {
    let pass = V2DuplicateConsolidationPass;
    assert_eq!(pass.id().as_str(), "duplicate_consolidation");
    assert_eq!(pass.dependencies().len(), 2);
    assert_eq!(pass.dependencies()[0].as_str(), "canonicalization");
    assert_eq!(pass.dependencies()[1].as_str(), "contradiction");
}

#[test]
fn test_duplicate_consolidation_identifies_duplicate_facts() {
    let t1 = Timestamp::now();
    let id1 = FactVersionId(Uuid::new_v4());
    let id2 = FactVersionId(Uuid::new_v4());
    let assertion_id = AssertionId(Uuid::new_v4());

    let fact1 = FactVersion {
        id: id1,
        assertion_id,
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
        id: id2,
        assertion_id, // Same assertion!
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.9).unwrap(),
        temporal: TemporalWindow::new(t1, t1, t1, None).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "u1".to_string() },
            derived_from: vec![],
        },
    };

    let snapshot = MockSnapshotWithDuplicates {
        entities: vec![],
        assertions: vec![],
        predicates: vec![],
        active_facts: vec![fact1, fact2],
    };

    let context = V2ReflectionContext {
        now: Timestamp::now(),
        cancellation_token: CancellationToken::new(),
        max_operations_budget: 100,
    };

    let pass = V2DuplicateConsolidationPass;
    let outcome = pass.analyze(&snapshot, &context).unwrap().unwrap();
    assert_eq!(outcome.plan.reason, RewriteReason::Duplicate);
    assert_eq!(outcome.plan.operations.len(), 1);

    match &outcome.plan.operations[0] {
        RewriteOperation::MergeFacts { source_fact_ids, target_fact_id } => {
            assert_eq!(source_fact_ids.len(), 1);
            assert!(*target_fact_id == id1 || *target_fact_id == id2);
        }
        _ => panic!("Expected MergeFacts operation"),
    }
}
