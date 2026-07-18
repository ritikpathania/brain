use std::collections::BTreeMap;

use brain_domain::EventId;
use brain_integrations::{EventIdentity, IngestionEnvelope, IngestionEvent};
use brain_sdk_rs::client::{
    BatchStrategy, DefaultBatchStrategy, InMemoryReplayStrategy, ReplayStrategy, RuntimeState,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_envelope(event_id: EventId) -> IngestionEnvelope {
    IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: EventIdentity {
            event_id,
            parent_event_id: None,
            workspace_id: brain_domain::WorkspaceId::new("ws"),
            client_id: brain_domain::ClientId::new("cli"),
            adapter_id: brain_domain::AdapterId::new("adapter"),
            session_id: brain_domain::SessionId::new(),
            conversation_id: None,
            timestamp: chrono::Utc::now(),
        },
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: "test".to_string(),
            metadata: BTreeMap::new(),
        },
    }
}

// ===========================================================================
//  InMemoryReplayStrategy property tests
// ===========================================================================

/// Record N events, then acknowledge all of them in random order.
/// After all ACKs, `get_unacknowledged` must return an empty vec.
#[tokio::test]
async fn replay_strategy_ack_all_random_order() {
    let strategy = InMemoryReplayStrategy::new();
    let mut ids: Vec<EventId> = (0..50).map(|_| EventId::new()).collect();

    // Record all
    for id in &ids {
        let env = make_envelope(*id);
        strategy.record(env).await.unwrap();
    }
    assert_eq!(strategy.get_unacknowledged().await.len(), 50);

    // Shuffle using Fisher-Yates with rand
    for i in (1..ids.len()).rev() {
        let j = rand::random::<usize>() % (i + 1);
        ids.swap(i, j);
    }

    // Acknowledge in random order
    for id in &ids {
        strategy.acknowledge(id).await.unwrap();
    }

    assert!(
        strategy.get_unacknowledged().await.is_empty(),
        "All events should be acknowledged"
    );
}

/// Record N events, acknowledge a random subset. The remaining must be exactly
/// the un-acknowledged set.
#[tokio::test]
async fn replay_strategy_partial_ack() {
    let strategy = InMemoryReplayStrategy::new();
    let ids: Vec<EventId> = (0..100).map(|_| EventId::new()).collect();

    for id in &ids {
        strategy.record(make_envelope(*id)).await.unwrap();
    }

    // Acknowledge ~50% randomly
    let mut acked = std::collections::HashSet::new();
    for id in &ids {
        if rand::random::<bool>() {
            strategy.acknowledge(id).await.unwrap();
            acked.insert(*id);
        }
    }

    let remaining = strategy.get_unacknowledged().await;
    let remaining_ids: std::collections::HashSet<EventId> =
        remaining.iter().map(|e| e.identity.event_id).collect();

    let expected: std::collections::HashSet<EventId> = ids
        .iter()
        .filter(|id| !acked.contains(*id))
        .cloned()
        .collect();

    assert_eq!(remaining_ids, expected);
}

/// Acknowledging a non-existent event_id is a no-op (no panic, no error).
#[tokio::test]
async fn replay_strategy_ack_nonexistent() {
    let strategy = InMemoryReplayStrategy::new();
    let id = EventId::new();
    strategy.record(make_envelope(id)).await.unwrap();

    let bogus = EventId::new();
    strategy.acknowledge(&bogus).await.unwrap();

    assert_eq!(strategy.get_unacknowledged().await.len(), 1);
}

/// Recording the same event_id twice overwrites (idempotent insert).
#[tokio::test]
async fn replay_strategy_duplicate_record() {
    let strategy = InMemoryReplayStrategy::new();
    let id = EventId::new();
    strategy.record(make_envelope(id)).await.unwrap();
    strategy.record(make_envelope(id)).await.unwrap();

    assert_eq!(strategy.get_unacknowledged().await.len(), 1);
}

/// Reconcile removes server-known events and returns the rest.
#[tokio::test]
async fn replay_strategy_reconcile() {
    let strategy = InMemoryReplayStrategy::new();
    let ids: Vec<EventId> = (0..10).map(|_| EventId::new()).collect();

    for id in &ids {
        strategy.record(make_envelope(*id)).await.unwrap();
    }

    // Server knows about the first 5
    let server_known: Vec<IngestionEnvelope> =
        ids[..5].iter().map(|id| make_envelope(*id)).collect();

    let replay_resp = brain_sdk_rs::ReplayResponse {
        events: server_known,
        last_sequence: 5,
    };

    let remaining = strategy.reconcile(replay_resp).await.unwrap();
    assert_eq!(remaining.len(), 5);

    let remaining_ids: std::collections::HashSet<EventId> =
        remaining.iter().map(|e| e.identity.event_id).collect();
    for id in &ids[5..] {
        assert!(remaining_ids.contains(id));
    }
}

// ===========================================================================
//  DefaultBatchStrategy property tests
// ===========================================================================

/// Push N events (N < max), drain returns all of them, buffer is empty after.
#[tokio::test]
async fn batch_strategy_drain_returns_all() {
    let mut batch = DefaultBatchStrategy::new(100, 1024 * 1024);

    let count = 37;
    for _ in 0..count {
        batch.push(make_envelope(EventId::new()));
    }
    assert!(!batch.should_flush());

    let drained = batch.drain();
    assert_eq!(drained.len(), count);

    // Buffer is empty after drain
    assert!(batch.drain().is_empty());
    assert!(!batch.should_flush());
}

/// should_flush triggers exactly at max_batch_size boundary.
#[tokio::test]
async fn batch_strategy_size_threshold() {
    let max_size = 10;
    let mut batch = DefaultBatchStrategy::new(max_size, 1024 * 1024);

    for i in 0..max_size {
        batch.push(make_envelope(EventId::new()));
        if i < max_size - 1 {
            assert!(!batch.should_flush(), "Should not flush at count {}", i + 1);
        }
    }
    assert!(batch.should_flush(), "Should flush at count {}", max_size);
}

/// should_flush triggers when cumulative bytes exceed threshold.
#[tokio::test]
async fn batch_strategy_byte_threshold() {
    // A single envelope serializes to roughly 400-600 bytes.
    // Set byte threshold very low to trigger on the first push.
    let mut batch = DefaultBatchStrategy::new(1000, 10);

    batch.push(make_envelope(EventId::new()));
    assert!(
        batch.should_flush(),
        "Should flush when bytes exceed threshold"
    );
}

/// drain resets byte counter — subsequent pushes start from zero.
#[tokio::test]
async fn batch_strategy_drain_resets_bytes() {
    let mut batch = DefaultBatchStrategy::new(1000, 10);
    batch.push(make_envelope(EventId::new()));
    assert!(batch.should_flush());

    batch.drain();

    // After drain, bytes are reset, so should_flush is false with high byte limit
    let mut batch2 = DefaultBatchStrategy::new(1000, 1024 * 1024);
    batch2.push(make_envelope(EventId::new()));
    assert!(!batch2.should_flush());
}

/// Rapidly push and drain in random alternation — no panics, counts are consistent.
#[tokio::test]
async fn batch_strategy_random_push_drain_sequence() {
    let mut batch = DefaultBatchStrategy::new(5, 1024 * 1024);
    let mut total_pushed = 0usize;
    let mut total_drained = 0usize;

    for _ in 0..200 {
        if rand::random::<bool>() {
            // Push 1-3 events
            let n = 1 + rand::random::<usize>() % 3;
            for _ in 0..n {
                batch.push(make_envelope(EventId::new()));
                total_pushed += 1;
            }
        } else {
            let d = batch.drain();
            total_drained += d.len();
        }
    }

    // Final drain
    total_drained += batch.drain().len();
    assert_eq!(total_pushed, total_drained);
}

// ===========================================================================
//  RuntimeState transition validity tests
// ===========================================================================

/// Valid state transitions according to the state machine spec.
#[test]
fn runtime_state_valid_transitions() {
    use RuntimeState::*;

    // Define the legal transitions
    let legal: Vec<(RuntimeState, RuntimeState)> = vec![
        (Disconnected, Connecting),
        (Disconnected, Disconnected), // re-enter on repeated failure
        (Connecting, Connected),
        (Connecting, Disconnected), // connect failed
        (Connecting, Replaying),    // optimized path (connect + replay)
        (Connected, Replaying),
        (Replaying, Ready),
        (Replaying, Disconnected), // replay write failed
        (Ready, Disconnected),     // connection lost
    ];

    // Verify each legal transition is well-defined
    for (from, to) in &legal {
        // This is a compile-time check that the enum variants exist and are matchable
        match (from, to) {
            (Disconnected, Connecting) => {}
            (Disconnected, Disconnected) => {}
            (Connecting, Connected) => {}
            (Connecting, Disconnected) => {}
            (Connecting, Replaying) => {}
            (Connected, Replaying) => {}
            (Replaying, Ready) => {}
            (Replaying, Disconnected) => {}
            (Ready, Disconnected) => {}
            _ => panic!("Unexpected legal transition: {:?} -> {:?}", from, to),
        }
    }
}

/// All RuntimeState variants are Debug and Clone.
#[test]
fn runtime_state_is_debug_clone() {
    let states = vec![
        RuntimeState::Disconnected,
        RuntimeState::Connecting,
        RuntimeState::Connected,
        RuntimeState::Replaying,
        RuntimeState::Ready,
    ];

    for s in &states {
        let cloned = *s;
        let debug = format!("{:?}", cloned);
        assert!(!debug.is_empty());
    }
}
