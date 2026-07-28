use brain_services::ha::*;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn test_raft_intent_log_commit_notifier_and_status_tracking() {
    let mock_log = Arc::new(MockRaftIntentLog::new());

    let effect_id = EffectId(Uuid::new_v4());
    let record = IntentRecord {
        sequence: SequenceNumber(1),
        event_id: EventId(Uuid::new_v4()),
        effect_id,
        created_at: 1000,
        effect: CoordinatorEffect::EmitWorkerLost("w1".to_string()),
        status: IntentStatus::Persisted,
    };

    mock_log.append_record(&record).await.unwrap();

    let committed = mock_log.wait_for_commit(SequenceNumber(1)).await.unwrap();
    assert_eq!(committed.effect_id, effect_id);

    // update_status updates local execution tracker, not consensus log
    mock_log.update_status(effect_id, IntentStatus::Completed).await.unwrap();
    assert!(mock_log.is_locally_completed(effect_id));
}
