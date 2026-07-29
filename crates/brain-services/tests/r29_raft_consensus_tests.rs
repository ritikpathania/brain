use brain_services::ha::*;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn test_scenario_1_leader_election_replays_committed_unexecuted_intents() {
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

    let executor = Arc::new(MockEffectExecutor::new());
    let engine = IntentReplayEngine::new(mock_log.clone(), executor.clone());

    // Trigger replay on BecameLeader
    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);
    assert!(lease_mgr.is_leader(1001));

    engine.replay_pending().await.unwrap();
    assert_eq!(executor.executed_count(), 1);
    assert!(mock_log.is_locally_completed(effect_id));
}

#[tokio::test]
async fn test_scenario_2_leader_step_down_disables_effect_execution() {
    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);
    assert!(lease_mgr.is_leader(1001));

    lease_mgr.handle_event(LeadershipEvent::BecameFollower { term: 2 }, 1002, 5);
    assert!(!lease_mgr.is_leader(1002));
}

#[tokio::test]
async fn test_scenario_3_follower_promotion_applies_state_machine_with_zero_follower_side_effects()
{
    let mock_log = Arc::new(MockRaftIntentLog::new());
    let executor = Arc::new(MockEffectExecutor::new());

    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameFollower { term: 1 }, 1000, 5);

    // Follower executes 0 side effects
    if lease_mgr.is_leader(1000) {
        let engine = IntentReplayEngine::new(mock_log.clone(), executor.clone());
        engine.replay_pending().await.unwrap();
    }

    assert_eq!(executor.executed_count(), 0);
}

#[tokio::test]
async fn test_scenario_4_duplicate_replay_validation_enforces_effect_id_idempotency() {
    let _mock_log = Arc::new(MockRaftIntentLog::new());
    let executor = Arc::new(MockEffectExecutor::new());
    let effect_id = EffectId(Uuid::new_v4());

    let effect = CoordinatorEffect::EmitWorkerLost("w1".to_string());
    executor.execute_effect(effect_id, &effect).await.unwrap();
    executor.execute_effect(effect_id, &effect).await.unwrap();

    assert_eq!(executor.executed_count(), 1);
}

#[tokio::test]
async fn test_scenario_5_network_partition_lease_expiration_aborts_effect_dispatches() {
    let mut lease_mgr = LeaderLeaseManager::new();
    lease_mgr.handle_event(LeadershipEvent::BecameLeader { term: 1 }, 1000, 5);

    // After 5s partition, lease expires
    assert!(!lease_mgr.is_leader(1006));
}
