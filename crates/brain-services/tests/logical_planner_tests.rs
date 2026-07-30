use brain_domain::query::*;
use brain_services::query::logical_planner::*;
use brain_services::query::semantic_binder::*;

#[test]
fn test_logical_planner_builds_plan() {
    let query = Query::builder()
        .filter(QueryFilter::EntityKind("Person".to_string()))
        .limit(5)
        .build();

    let bound = SemanticBinder::bind(&query).unwrap();
    let plan = LogicalPlanner::plan(&bound).unwrap();

    match plan {
        LogicalPlan::Limit { count, input } => {
            assert_eq!(count, 5);
            assert!(matches!(*input, LogicalPlan::Filter { .. }));
        }
        _ => panic!("Expected Limit root plan"),
    }
}
