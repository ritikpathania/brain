use brain_domain::bkf::*;
use brain_domain::query::*;
use brain_services::query::batch::*;
use brain_services::query::context::*;
use brain_services::query::operators::*;

struct EmptySnapshot;

impl KnowledgeSnapshotView for EmptySnapshot {
    fn entities(&self) -> &[KnowledgeEntity] { &[] }
    fn assertions(&self) -> &[SemanticAssertion] { &[] }
    fn predicates(&self) -> &[Predicate] { &[] }
    fn active_facts(&self) -> &[FactVersion] { &[] }
}

#[test]
fn test_scan_operator_pulls_empty_batch() {
    let snapshot = EmptySnapshot;
    let config = ExecutionConfig::new();
    let mut state = ExecutionState::new();
    let mut batch = BindingBatch::new(10);
    let mut op = ScanOperator::new(ScanTarget::ActiveFacts);

    let status = op.next_batch(&snapshot, &config, &mut state, &mut batch).unwrap();
    assert!(matches!(status, BatchStatus::Exhausted));
    assert_eq!(batch.len(), 0);
}
