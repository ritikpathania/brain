use brain_services::ha::*;
use rusqlite::Connection;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn test_end_to_end_intent_replay_engine_crash_recovery() {
    let conn = Connection::open_in_memory().unwrap();
    let log = Arc::new(SqliteIntentLog::new(conn));
    log.init_schema().unwrap();

    let effect_id = EffectId(Uuid::new_v4());
    let record = IntentRecord {
        sequence: SequenceNumber(1),
        event_id: EventId(Uuid::new_v4()),
        effect_id,
        created_at: 1000,
        effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        status: IntentStatus::Executing, // Interrupted state
    };

    log.append_record(&record).await.unwrap();

    let executor = Arc::new(MockEffectExecutor::new());
    let engine = IntentReplayEngine::new(log.clone(), executor.clone());

    // Replay pending executing records
    engine.replay_pending().await.unwrap();

    assert_eq!(executor.executed_count(), 1);

    // Verify record transitioned to Completed
    let pending = log.scan_pending().await.unwrap();
    assert_eq!(pending.len(), 0);
}
