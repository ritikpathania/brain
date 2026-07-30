use brain_domain::bkf::*;
use brain_services::reflection::conflict_resolver::*;
use brain_services::reflection::executor::*;
use brain_services::reflection::pass_context::*;
use brain_services::reflection::passes::canonicalization::*;
use brain_services::reflection::passes::confidence_recalculation::*;
use brain_services::reflection::passes::contradiction::*;
use brain_services::reflection::passes::duplicate_consolidation::*;
use brain_services::reflection::passes::stale_knowledge::*;
use brain_services::reflection::registry_dag::*;
use tokio_util::sync::CancellationToken;

struct FullSnapshot {
    entities: Vec<KnowledgeEntity>,
    assertions: Vec<SemanticAssertion>,
    predicates: Vec<Predicate>,
    active_facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for FullSnapshot {
    fn entities(&self) -> &[KnowledgeEntity] {
        &self.entities
    }
    fn assertions(&self) -> &[SemanticAssertion] {
        &self.assertions
    }
    fn predicates(&self) -> &[Predicate] {
        &self.predicates
    }
    fn active_facts(&self) -> &[FactVersion] {
        &self.active_facts
    }
}

#[test]
fn test_end_to_end_v2_reflection_pipeline() {
    let mut registry = PassRegistryV2::new();
    registry.register(Box::new(CanonicalizationPass)).unwrap();
    registry.register(Box::new(V2ContradictionPass)).unwrap();
    registry
        .register(Box::new(V2DuplicateConsolidationPass))
        .unwrap();
    registry.register(Box::new(V2StaleKnowledgePass)).unwrap();
    registry
        .register(Box::new(V2ConfidenceRecalculationPass))
        .unwrap();

    let passes = registry.resolve_execution_order().unwrap();
    assert_eq!(passes.len(), 5);

    // Verify DAG order
    let order: Vec<String> = passes.iter().map(|p| p.id().as_str().to_string()).collect();
    assert_eq!(order[0], "canonicalization");
    assert_eq!(order[4], "confidence_recalculation");

    let snapshot = FullSnapshot {
        entities: vec![],
        assertions: vec![],
        predicates: vec![],
        active_facts: vec![],
    };

    let context = V2ReflectionContext {
        now: Timestamp::now(),
        cancellation_token: CancellationToken::new(),
        max_operations_budget: 1000,
    };

    // Execute each pass in topological order
    let mut proposed_plans = Vec::new();
    for pass in passes {
        if let Some(outcome) = pass.analyze(&snapshot, &context).unwrap() {
            proposed_plans.push(outcome.plan);
        }
    }

    // Resolve conflicts into single merged plan
    let merged_plan = ConflictResolver::resolve(proposed_plans).unwrap();

    // Validate merged plan
    assert!(RewriteValidator::validate(&merged_plan, &snapshot).is_ok());

    // Lower to events
    let events = V2RewriteExecutor::lower_plan_to_events(&merged_plan).unwrap();
    assert_eq!(events.len(), 0);
}
