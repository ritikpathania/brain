use brain_domain::bkf::*;
use brain_services::reflection::pass_context::*;
use brain_services::reflection::passes::stale_knowledge::*;
use std::time::{Duration, SystemTime};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct MockSnapshotWithExpiredFact {
    active_facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for MockSnapshotWithExpiredFact {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &self.active_facts }
}

#[test]
fn test_stale_knowledge_id_and_deps() {
    let pass = V2StaleKnowledgePass;
    assert_eq!(pass.id().as_str(), "stale_knowledge");
    assert_eq!(pass.dependencies().len(), 3);
}

#[test]
fn test_stale_knowledge_identifies_expired_temporal_windows() {
    let base = SystemTime::UNIX_EPOCH;
    let t_past = Timestamp(base + Duration::from_secs(100));
    let t_expired = Timestamp(base + Duration::from_secs(200));
    let t_now = Timestamp(base + Duration::from_secs(300));

    let fact_id = FactVersionId(Uuid::new_v4());

    let expired_fact = FactVersion {
        id: fact_id,
        assertion_id: AssertionId(Uuid::new_v4()),
        lifecycle: FactLifecycle::Verified,
        confidence: Confidence::new(0.8).unwrap(),
        temporal: TemporalWindow::new(t_past, t_past, t_past, Some(t_expired)).unwrap(),
        supersedes: None,
        provenance: FactProvenance {
            source: FactProvenanceSource::Manual { user_id: "u1".to_string() },
            derived_from: vec![],
        },
    };

    let snapshot = MockSnapshotWithExpiredFact {
        active_facts: vec![expired_fact],
    };

    let context = V2ReflectionContext {
        now: t_now,
        cancellation_token: CancellationToken::new(),
        max_operations_budget: 100,
    };

    let pass = V2StaleKnowledgePass;
    let outcome = pass.analyze(&snapshot, &context).unwrap().unwrap();
    assert_eq!(outcome.plan.reason, RewriteReason::TemporalExpiration);
    assert_eq!(outcome.plan.operations.len(), 1);

    match &outcome.plan.operations[0] {
        RewriteOperation::ArchiveFact { fact_id: target_id, archived_at } => {
            assert_eq!(*target_id, fact_id);
            assert_eq!(*archived_at, t_now);
        }
        _ => panic!("Expected ArchiveFact operation"),
    }
}
