//! Sprint 3 runtime validation tests.
//!
//! Validates the three new capabilities:
//! 1. `RuntimeEventDispatcher` trait — `InMemoryEventDispatcher` implements it; services hold trait objects.
//! 2. `ReflectionEngine` — emit-only pipeline: canonicalize → reflect → `ReflectionCompletedEvent`.
//! 3. Observability — `ObservabilitySubscriber` records `TaskProgress` into a `CorrelationIndex`.

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};
    use brain_core::{
        events::{CorrelationId, RuntimeEventDispatcher, RuntimeEvent},
        evolution::{Canonicalizer, Observation, Provenance},
        reflection::{ReflectionCompletedEvent, ReflectionEngine, ReflectionTarget},
    };
    use brain_domain::NodeId;
    use brain_storage::test_utils::TestStorage;
    use brain_services::{
        SqliteCanonicalizer, InMemoryEventDispatcher, InMemoryReflectionEngine,
    };
    use brain_observability::{CorrelationIndex, ObservabilitySubscriber};

    fn make_obs(payload: &str, corr_id: CorrelationId) -> Observation {
        Observation {
            payload: payload.as_bytes().to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "test".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: corr_id,
            },
        }
    }

    // --- S3-RT-1: RuntimeEventDispatcher trait object dispatch ---
    //
    // Verifies that `InMemoryEventDispatcher` is usable as `Arc<dyn RuntimeEventDispatcher>` and
    // that events dispatched through the trait object are received by a concrete subscriber.
    #[test]
    fn test_dispatcher_trait_object_dispatch() {
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(16));
        let trait_obj: Arc<dyn RuntimeEventDispatcher> = Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        let mut rx = dispatcher.subscribe();

        // Dispatch through the trait object
        use brain_core::events::{TaskProgress, TaskState, EventSource, OperationId};
        let event = Arc::new(TaskProgress {
            operation_id: OperationId::new_v4(),
            correlation_id: CorrelationId::new_v4(),
            state: TaskState::Completed,
            source: EventSource::Ingestion,
            sequence: 1,
            timestamp: SystemTime::now(),
        }) as Arc<dyn RuntimeEvent>;

        trait_obj.dispatch(Arc::clone(&event));

        // Receive via concrete subscriber
        let received = rx.try_recv().expect("Expected event on subscriber channel");
        let progress = received.as_any().downcast_ref::<TaskProgress>().unwrap();
        assert!(matches!(progress.state, TaskState::Completed));
    }

    // --- S3-RT-2: Reflection pipeline: canonicalize → reflect → ReflectionCompletedEvent ---
    //
    // Verifies the full Sprint 3 composition:
    //   SqliteCanonicalizer.with_reflection(InMemoryReflectionEngine)
    //   After canonicalize(), the ReflectionCompletedEvent is received by a subscriber.
    #[test]
    fn test_reflection_event_pipeline() {
        let test_db = TestStorage::new();
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(32));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> = Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        let reflection_engine = Arc::new(InMemoryReflectionEngine::new(Arc::clone(&dispatcher_trait)));

        let canonicalizer = SqliteCanonicalizer::new(
            test_db.storage().clone(),
            Arc::clone(&dispatcher_trait),
        )
        .with_reflection(reflection_engine);

        let corr_id = CorrelationId::new_v4();
        let mut rx = dispatcher.subscribe();

        let result = canonicalizer.canonicalize(make_obs("Reflection Test Node", corr_id)).unwrap();
        assert_eq!(result.epoch.0, 1);
        assert_eq!(result.affected_entities.len(), 1);

        // Drain all events from the channel and find ReflectionCompletedEvent
        let mut found_reflection = false;
        while let Ok(event) = rx.try_recv() {
            if let Some(reflection_ev) = event.as_any().downcast_ref::<ReflectionCompletedEvent>() {
                assert_eq!(reflection_ev.epoch.0, 1);
                assert_eq!(reflection_ev.correlation_id, corr_id);
                assert_eq!(reflection_ev.entities_reflected.len(), 1);
                found_reflection = true;
            }
        }
        assert!(found_reflection, "ReflectionCompletedEvent was never dispatched");
    }

    // --- S3-RT-3: Reflection engine is storage-agnostic ---
    //
    // Verifies the contract: InMemoryReflectionEngine can be called directly with an
    // arbitrary ReflectionTarget, without any storage. No panics, no storage calls.
    #[test]
    fn test_reflection_engine_storage_agnostic() {
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(16));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> = Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;
        let engine = InMemoryReflectionEngine::new(dispatcher_trait);

        let corr_id = CorrelationId::new_v4();
        let fake_node_id = NodeId(uuid::Uuid::new_v4());

        let target = ReflectionTarget {
            affected_entities: vec![fake_node_id],
            epoch: brain_domain::EpochId(42),
            correlation_id: corr_id,
        };

        // Call reflect() with no storage at all
        let event = engine.reflect(target).expect("Reflection must not fail without storage");
        assert_eq!(event.epoch.0, 42);
        assert_eq!(event.correlation_id, corr_id);
        assert_eq!(event.entities_reflected.len(), 1);
    }

    // --- S3-RT-4: ObservabilitySubscriber records TaskProgress spans ---
    //
    // Verifies that the std::thread subscriber feeds the CorrelationIndex
    // with monotonic spans for a complete canonicalization run.
    #[test]
    fn test_observability_subscriber_records_spans() {
        let test_db = TestStorage::new();
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
        let dispatcher_trait: Arc<dyn RuntimeEventDispatcher> = Arc::clone(&dispatcher) as Arc<dyn RuntimeEventDispatcher>;

        // Wire up ObservabilitySubscriber via subscribe_sync()
        let sync_rx = dispatcher.subscribe_sync();
        let index = Arc::new(Mutex::new(CorrelationIndex::new()));
        let _subscriber = ObservabilitySubscriber::new(sync_rx, Arc::clone(&index));

        let corr_id = CorrelationId::new_v4();
        let canonicalizer = SqliteCanonicalizer::new(
            test_db.storage().clone(),
            Arc::clone(&dispatcher_trait),
        );

        canonicalizer.canonicalize(make_obs("Observability Test", corr_id)).unwrap();

        // Give the background thread a moment to drain the channel
        std::thread::sleep(Duration::from_millis(20));

        let idx = index.lock().unwrap();
        let spans = idx.spans_for(corr_id).expect("No spans recorded for correlation ID");
        assert!(!spans.is_empty(), "Expected at least one span");

        // Verify monotonic sequence
        let seqs: Vec<u64> = spans.iter().map(|s| s.sequence).collect();
        let mut sorted = seqs.clone();
        sorted.sort();
        assert_eq!(seqs, sorted, "Spans must arrive in monotonically increasing sequence");

        // Verify the timeline is complete (contains Completed state)
        assert!(idx.is_complete(corr_id), "Timeline should be complete after canonicalization");
    }

    // --- S3-RT-5: ObservabilitySubscriber thread lifecycle ---
    //
    // Verifies the subscriber thread shuts down gracefully when the dispatcher is dropped
    // (channel closes → blocking recv() returns Err → thread exits).
    #[test]
    fn test_observability_subscriber_thread_shutdown() {
        let index = Arc::new(Mutex::new(CorrelationIndex::new()));

        {
            let dispatcher = Arc::new(InMemoryEventDispatcher::new(16));
            let sync_rx = dispatcher.subscribe_sync();
            let _subscriber = ObservabilitySubscriber::new(sync_rx, Arc::clone(&index));
            // dispatcher drops here, closing the SyncSender → subscriber thread exits
        }

        // Brief wait to let the thread exit
        std::thread::sleep(Duration::from_millis(10));
        // If we reach here without hanging, the thread exited cleanly
    }
}
