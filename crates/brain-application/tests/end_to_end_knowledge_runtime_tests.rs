use brain_events::{
    KnowledgeCompiledPayload, RuntimeCompilationMode, RuntimeEvent, RuntimeEventBus,
    RuntimeEventSubscriber,
};
use brain_services::graph::ProjectionService;
use brain_services::reflection::ReflectionService;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

struct MockFaultySubscriber {
    pub called: Arc<AtomicBool>,
}

impl RuntimeEventSubscriber for MockFaultySubscriber {
    fn name(&self) -> &'static str {
        "MockFaultySubscriber"
    }

    fn handle_event<'a>(
        &'a self,
        _event: Arc<RuntimeEvent>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        let called = Arc::clone(&self.called);
        Box::pin(async move {
            called.store(true, Ordering::Release);
            panic!("Simulated subscriber error for failure isolation test");
        })
    }
}

#[tokio::test]
async fn test_end_to_end_runtime_event_bus_and_subscriber_isolation() {
    let bus = Arc::new(RuntimeEventBus::new());

    let reflection_service = Arc::new(ReflectionService::new(Some(Arc::clone(&bus))));
    let projection_service = Arc::new(ProjectionService::new(
        "search_projection",
        Some(Arc::clone(&bus)),
    ));

    let faulty_called = Arc::new(AtomicBool::new(false));
    let faulty_sub = Arc::new(MockFaultySubscriber {
        called: Arc::clone(&faulty_called),
    });

    bus.subscribe(faulty_sub);
    bus.subscribe(reflection_service.clone());
    bus.subscribe(projection_service.clone());

    let mut changed_entities = HashSet::new();
    changed_entities.insert("entity_rust".to_string());

    let mut changed_facts = HashSet::new();
    changed_facts.insert("fact_type".to_string());

    let event = RuntimeEvent::KnowledgeCompiled(KnowledgeCompiledPayload {
        compilation_id: Uuid::new_v4(),
        graph_version: 5,
        mode: RuntimeCompilationMode::Incremental,
        changed_entities,
        changed_facts,
        timestamp_ms: 1700000000000,
    });

    // Publish event
    bus.publish(event).await;

    // Give async subscribers time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 1. Verify faulty subscriber was invoked without crashing event bus
    assert!(faulty_called.load(Ordering::Acquire));

    // 2. Verify ReflectionService reacted to KnowledgeCompiled event
    assert_eq!(reflection_service.last_evaluated_version(), 5);
    assert_eq!(reflection_service.total_sweeps_executed(), 1);

    // 3. Verify ProjectionService caught up to epoch 5 and invariant ProjectionVersion == GraphVersion holds
    assert_eq!(projection_service.projection_version(), 5);
    assert!(projection_service.is_synchronized(5));

    // 4. Verify EventBus telemetry metrics
    let metrics = bus.metrics();
    assert!(metrics.total_events_published >= 1);
}
