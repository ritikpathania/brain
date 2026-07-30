use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;

#[test]
fn test_logical_optimizer_normalization_pass() {
    let raw_plan = LogicalPlan::Scan {
        target: ScanTarget::ActiveFacts,
    };

    let optimized = LogicalOptimizer::optimize(raw_plan.clone()).unwrap();
    assert_eq!(optimized, raw_plan);
}
