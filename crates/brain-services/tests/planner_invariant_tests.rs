use brain_domain::query::*;
use brain_services::query::logical_optimizer::*;
use brain_services::query::logical_planner::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_logical_optimizer_idempotence() {
    let query = Query::builder()
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(10)
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    let plan1 = LogicalPlanner::plan(&bound).unwrap();
    let opt1 = LogicalOptimizer::optimize(plan1).unwrap();
    let opt2 = LogicalOptimizer::optimize(opt1.clone()).unwrap();

    assert_eq!(opt1, opt2);
}

#[test]
fn test_logical_plan_snapshot_determinism() {
    let query = Query::builder()
        .pattern(Pattern::triple(
            QueryVar::new("p"),
            brain_domain::bkf::PredicateName::new("LivesIn").unwrap(),
            QueryVar::new("c"),
        ))
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(10)
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    let plan = LogicalPlanner::plan(&bound).unwrap();
    let opt = LogicalOptimizer::optimize(plan).unwrap();

    assert!(matches!(opt, LogicalPlan::Limit { .. }));
}
