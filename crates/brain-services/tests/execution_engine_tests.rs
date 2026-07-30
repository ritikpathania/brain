use brain_domain::bkf::*;
use brain_domain::query::*;
use brain_services::query::context::*;
use brain_services::query::execution_engine::*;
use brain_services::query::explain_formatter::*;
use brain_services::query::physical_plan::*;

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
fn test_execution_engine_runs_physical_plan() {
    let snapshot = MockEmptySnapshot;
    let config = ExecutionConfig::new();
    let mut state = ExecutionState::new();
    let plan = PhysicalPlan {
        root: PhysicalPlanNode::Scan {
            target: ScanTarget::ActiveFacts,
        },
    };

    let result = V2ExecutionEngine::execute(&plan, &snapshot, &config, &mut state).unwrap();
    assert_eq!(result.bindings.len(), 0);
}

#[test]
fn test_explain_formatter_generates_plan_strings() {
    let logical = LogicalPlan::Scan {
        target: ScanTarget::ActiveFacts,
    };
    let physical = PhysicalPlan {
        root: PhysicalPlanNode::Scan {
            target: ScanTarget::ActiveFacts,
        },
    };

    let explain = ExplainFormatter::format(&logical, &physical);
    assert!(explain.logical_plan_str.contains("ActiveFacts"));
    assert!(explain.physical_plan_str.contains("ActiveFacts"));
}
