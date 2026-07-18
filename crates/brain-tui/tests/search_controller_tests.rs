use brain_tui::ui::search::controller::SearchController;
use brain_tui::ui::search::types::{
    ProviderId, SearchContext, SearchEvent, SearchEventSink, SearchGeneration, SearchProvider,
    SearchQuery, PROVIDER_LOCAL_MESSAGES, PROVIDER_REMOTE_MESSAGES,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

struct MockEventSink {
    events: Arc<Mutex<Vec<SearchEvent>>>,
}

impl MockEventSink {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_events(&self) -> Vec<SearchEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl SearchEventSink for MockEventSink {
    fn submit(&self, event: SearchEvent) {
        self.events.lock().unwrap().push(event);
    }
}

struct DummyImmediateProvider;
impl SearchProvider for DummyImmediateProvider {
    fn provider_id(&self) -> ProviderId {
        PROVIDER_LOCAL_MESSAGES
    }
    fn search(
        &self,
        query: &SearchQuery,
        _context: &SearchContext,
        _cancellation: CancellationToken,
        sink: Arc<dyn SearchEventSink>,
    ) {
        sink.submit(SearchEvent::Started {
            generation: query.generation,
            provider: self.provider_id(),
        });
        sink.submit(SearchEvent::Finished {
            generation: query.generation,
            provider: self.provider_id(),
        });
    }
}

struct DummyAsyncProvider;
impl SearchProvider for DummyAsyncProvider {
    fn provider_id(&self) -> ProviderId {
        PROVIDER_REMOTE_MESSAGES
    }
    fn search(
        &self,
        query: &SearchQuery,
        _context: &SearchContext,
        cancellation: CancellationToken,
        sink: Arc<dyn SearchEventSink>,
    ) {
        if cancellation.is_cancelled() {
            return;
        }
        sink.submit(SearchEvent::Started {
            generation: query.generation,
            provider: self.provider_id(),
        });
        sink.submit(SearchEvent::Finished {
            generation: query.generation,
            provider: self.provider_id(),
        });
    }
}

fn create_empty_context() -> SearchContext {
    SearchContext {
        sessions: Vec::new(),
        active_messages: Vec::new(),
    }
}

#[tokio::test]
async fn test_controller_cancellation_and_debounce() {
    let sink = Arc::new(MockEventSink::new());
    let p_imm = Arc::new(DummyImmediateProvider);
    let p_async = Arc::new(DummyAsyncProvider);

    let mut controller = SearchController::new(
        vec![p_imm.clone()],
        vec![p_async.clone()], // async providers to be debounced
        sink.clone(),
    );

    let context = create_empty_context();

    // Query 1
    controller.search("hello".to_string(), &context);

    // Immediate provider runs synchronously
    let events = sink.get_events();
    assert!(events.iter().any(|e| matches!(
        e,
        SearchEvent::Started {
            generation: SearchGeneration(1),
            ..
        }
    )));

    // async provider should not have run yet due to debounce
    assert!(!events.iter().any(
        |e| matches!(e, SearchEvent::Started { provider, .. } if *provider == p_async.provider_id())
    ));

    // Immediately trigger Query 2 (which cancels Query 1)
    controller.search("world".to_string(), &context);

    // Sleep longer than the 150ms debounce delay
    tokio::time::sleep(Duration::from_millis(200)).await;

    let final_events = sink.get_events();

    // Query 1's async provider must NOT have started (because it was cancelled during debounce)
    assert!(!final_events.iter().any(|e| matches!(e, SearchEvent::Started { generation: SearchGeneration(1), provider } if *provider == p_async.provider_id())));

    // Query 2's async provider MUST have started and completed successfully
    assert!(final_events.iter().any(|e| matches!(e, SearchEvent::Started { generation: SearchGeneration(2), provider } if *provider == p_async.provider_id())));
    assert!(final_events.iter().any(|e| matches!(e, SearchEvent::Finished { generation: SearchGeneration(2), provider } if *provider == p_async.provider_id())));
}
