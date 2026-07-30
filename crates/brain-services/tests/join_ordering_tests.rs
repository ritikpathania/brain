use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;

#[test]
fn test_join_ordering_stable_tie_break() {
    let join_plan = LogicalPlan::Join {
        left: Box::new(LogicalPlan::Scan {
            target: ScanTarget::Entities,
        }),
        right: Box::new(LogicalPlan::Scan {
            target: ScanTarget::ActiveFacts,
        }),
    };

    let opt1 = LogicalOptimizer::optimize(join_plan.clone()).unwrap();
    let opt2 = LogicalOptimizer::optimize(join_plan.clone()).unwrap();

    assert_eq!(opt1, opt2);
}
