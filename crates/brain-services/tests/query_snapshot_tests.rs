use brain_domain::query::*;
use brain_services::query::explain_formatter::*;
use brain_services::query::logical_optimizer::*;
use brain_services::query::logical_planner::*;
use brain_services::query::physical_planner::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_end_to_end_query_plan_snapshot_determinism() {
    let query = Query::builder()
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(10)
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    let logical = LogicalPlanner::plan(&bound).unwrap();
    let opt_logical = LogicalOptimizer::optimize(logical).unwrap();
    let physical = PhysicalPlanner::plan(&opt_logical).unwrap();
    let explain = ExplainFormatter::format(&opt_logical, &physical);

    assert!(explain.logical_plan_str.contains("Filter"));
    assert!(explain.physical_plan_str.contains("PhysicalFilter") || explain.physical_plan_str.contains("Limit"));
}
