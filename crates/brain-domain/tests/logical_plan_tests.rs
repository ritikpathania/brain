use brain_domain::query::logical_plan::*;
use brain_domain::query::scan_target::*;

#[test]
fn test_logical_plan_tree() {
    let scan = LogicalPlan::Scan {
        target: ScanTarget::ActiveFacts,
    };
    let limit = LogicalPlan::Limit {
        count: 10,
        input: Box::new(scan),
    };

    match limit {
        LogicalPlan::Limit { count, input } => {
            assert_eq!(count, 10);
            assert!(matches!(*input, LogicalPlan::Scan { target: ScanTarget::ActiveFacts }));
        }
        _ => panic!("Expected Limit plan"),
    }
}
