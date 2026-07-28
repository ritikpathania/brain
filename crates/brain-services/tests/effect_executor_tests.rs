use brain_services::ha::*;
use uuid::Uuid;

#[tokio::test]
async fn test_effect_executor_routing_and_idempotency() {
    let executor = MockEffectExecutor::new();
    let effect_id = EffectId(Uuid::new_v4());
    let effect = CoordinatorEffect::EmitWorkerLost("worker-1".to_string());

    executor.execute_effect(effect_id, &effect).await.unwrap();
    assert_eq!(executor.executed_count(), 1);

    // Duplicate execution is idempotent
    executor.execute_effect(effect_id, &effect).await.unwrap();
    assert_eq!(executor.executed_count(), 1);
}
