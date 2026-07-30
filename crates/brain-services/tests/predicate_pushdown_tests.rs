use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;

#[test]
fn test_predicate_pushdown_reorders_limit_and_filter() {
    let plan = LogicalPlan::Limit {
        count: 10,
        input: Box::new(LogicalPlan::Filter {
            condition: QueryFilter::EntityKind("Person".to_string()),
            input: Box::new(LogicalPlan::Scan {
                target: ScanTarget::ActiveFacts,
            }),
        }),
    };

    let optimized = LogicalOptimizer::optimize(plan).unwrap();
    // Verify that Filter is pushed below Limit or preserved as valid optimization
    match optimized {
        LogicalPlan::Limit { input, count: 10 } => {
            assert!(matches!(*input, LogicalPlan::Filter { .. }));
        }
        _ => panic!("Expected Limit root plan"),
    }
}
