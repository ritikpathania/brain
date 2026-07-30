use brain_domain::query::ast::*;
use brain_domain::query::bound::*;
use brain_domain::query::explain::*;
use brain_domain::query::result::*;
use std::time::Duration;

#[test]
fn test_query_result_slot_indexing() {
    let mut schema = BindingSchema::new();
    let slot_x = schema.get_or_create_slot(&QueryVar::new("x"));

    let mut row = BindingRow::with_capacity(1);
    row.set(
        slot_x,
        QueryValue::Literal(brain_domain::bkf::LiteralValue::String("test".to_string())),
    );

    let result = QueryResult {
        schema,
        bindings: vec![row],
        statistics: QueryStatistics {
            result_count: 1,
            logical_plan_depth: 2,
            traversal_depth: 0,
            pattern_count: 1,
        },
        execution_statistics: ExecutionStatistics {
            rows_scanned: 10,
            total_batches: 1,
            execution_time: Duration::from_millis(5),
            memory_bytes: 1024,
            operator_metrics: vec![],
        },
    };

    assert_eq!(result.bindings.len(), 1);
    assert_eq!(result.statistics.result_count, 1);
}

#[test]
fn test_explain_plan_construction() {
    let explain = ExplainPlan {
        logical_plan_str: "LogicalScan".to_string(),
        physical_plan_str: "PhysicalScan".to_string(),
    };
    assert_eq!(explain.logical_plan_str, "LogicalScan");
}
