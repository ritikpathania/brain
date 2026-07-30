use brain_domain::query::*;
use brain_services::query::batch::*;
use brain_services::query::context::*;

#[test]
fn test_binding_batch_capacity_and_operations() {
    let mut batch = BindingBatch::new(10);
    assert_eq!(batch.capacity(), 10);
    assert_eq!(batch.len(), 0);

    let row = BindingRow::with_capacity(1);
    batch.append(row);
    assert_eq!(batch.len(), 1);

    batch.clear();
    assert_eq!(batch.len(), 0);
}

#[test]
fn test_execution_config_and_state_split() {
    let config = ExecutionConfig::new();
    let state = ExecutionState::new();

    assert_eq!(config.batch_size, 100);
    assert_eq!(state.total_rows_scanned, 0);
}
