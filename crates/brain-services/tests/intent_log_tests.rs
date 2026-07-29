use brain_services::ha::*;
use rusqlite::Connection;
use uuid::Uuid;

#[tokio::test]
async fn test_sqlite_intent_log_append_status_update_and_pending_scan() {
    let conn = Connection::open_in_memory().unwrap();
    let log = SqliteIntentLog::new(conn);
    log.init_schema().unwrap();

    let effect_id = EffectId(Uuid::new_v4());
    let record = IntentRecord {
        sequence: SequenceNumber(1),
        event_id: EventId(Uuid::new_v4()),
        effect_id,
        created_at: 1000,
        effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        status: IntentStatus::Persisted,
    };

    log.append_record(&record).await.unwrap();

    let pending = log.scan_pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].effect_id, effect_id);

    log.update_status(effect_id, IntentStatus::Completed)
        .await
        .unwrap();
    let pending_after = log.scan_pending().await.unwrap();
    assert_eq!(pending_after.len(), 0);
}
