use brain_domain::bkf::*;
use brain_services::reflection::pass_context::*;
use brain_services::reflection::passes::canonicalization::*;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

struct MockSnapshotWithUnnormalizedEntity {
    entities: Vec<KnowledgeEntity>,
}

impl KnowledgeSnapshotView for MockSnapshotWithUnnormalizedEntity {
    fn entities(&self) -> &[KnowledgeEntity] {
        &self.entities
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
fn test_canonicalization_pass_id_and_deps() {
    let pass = CanonicalizationPass;
    assert_eq!(pass.id().as_str(), "canonicalization");
    assert!(pass.dependencies().is_empty());
}

#[test]
fn test_canonicalization_pass_identifies_unnormalized_names() {
    let pass = CanonicalizationPass;
    let snapshot = MockSnapshotWithUnnormalizedEntity {
        entities: vec![KnowledgeEntity {
            id: KnowledgeEntityId(Uuid::new_v4()),
            name: EntityName::new("  john  doe  ").unwrap(),
            kind: KnowledgeEntityKind::new("person").unwrap(),
        }],
    };

    let context = V2ReflectionContext {
        now: Timestamp::now(),
        cancellation_token: CancellationToken::new(),
        max_operations_budget: 100,
    };

    let outcome = pass.analyze(&snapshot, &context).unwrap().unwrap();
    assert_eq!(outcome.plan.reason, RewriteReason::Canonicalization);
}
