use brain_domain::bkf::*;
use brain_domain::query::*;
use brain_services::query::context::*;
use brain_services::query::execution_engine::*;
use brain_services::query::physical_plan::*;

struct MockSnapshotWithFacts {
    facts: Vec<FactVersion>,
}

impl KnowledgeSnapshotView for MockSnapshotWithFacts {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &self.facts }
}

#[test]
fn property_test_query_execution_determinism() {
    let snapshot = MockSnapshotWithFacts { facts: vec![] };
    let plan = PhysicalPlan {
        root: PhysicalPlanNode::Scan { target: ScanTarget::ActiveFacts },
    };

    let config1 = ExecutionConfig::new();
    let mut state1 = ExecutionState::new();
    let res1 = V2ExecutionEngine::execute(&plan, &snapshot, &config1, &mut state1).unwrap();

    let config2 = ExecutionConfig::new();
    let mut state2 = ExecutionState::new();
    let res2 = V2ExecutionEngine::execute(&plan, &snapshot, &config2, &mut state2).unwrap();

    assert_eq!(res1.bindings, res2.bindings);
    assert_eq!(res1.statistics, res2.statistics);
}

#[test]
fn property_test_batch_size_invariance() {
    let snapshot = MockSnapshotWithFacts { facts: vec![] };
    let plan = PhysicalPlan {
        root: PhysicalPlanNode::Scan { target: ScanTarget::ActiveFacts },
    };

    let mut config1 = ExecutionConfig::new();
    config1.batch_size = 1;
    let mut state1 = ExecutionState::new();
    let res1 = V2ExecutionEngine::execute(&plan, &snapshot, &config1, &mut state1).unwrap();

    let mut config2 = ExecutionConfig::new();
    config2.batch_size = 100;
    let mut state2 = ExecutionState::new();
    let res2 = V2ExecutionEngine::execute(&plan, &snapshot, &config2, &mut state2).unwrap();

    assert_eq!(res1.bindings, res2.bindings);
}
