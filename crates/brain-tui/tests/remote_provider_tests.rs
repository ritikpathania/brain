use async_trait::async_trait;
use brain_core::errors::BrainError;
use brain_domain::{Message, MessageId as DomainMessageId, MessageRole, SessionId};
use brain_tui::client::{EventReceiver, ExecutionClient, ExecutionRequest, SessionSummary};
use brain_tui::ui::search::providers::RemoteMessagesProvider;
use brain_tui::ui::search::types::{
    SearchContext, SearchEvent, SearchEventSink, SearchGeneration, SearchProvider, SearchQuery,
    SearchResultKind,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

struct MockSearchClient {
    delay_ms: u64,
}

#[async_trait]
impl ExecutionClient for MockSearchClient {
    async fn execute(&self, _req: ExecutionRequest) -> Result<EventReceiver, BrainError> {
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok(EventReceiver::new(rx, CancellationToken::new()))
    }
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError> {
        Ok(vec![])
    }
    async fn load_session(&self, _id: SessionId) -> Result<Vec<Message>, BrainError> {
        Ok(vec![])
    }
    async fn delete_session(&self, _id: SessionId) -> Result<(), BrainError> {
        Ok(())
    }
    async fn approve_tool_call(
        &self,
        _call_id: brain_core::events::ToolCallId,
        _approved: bool,
    ) -> Result<(), BrainError> {
        Ok(())
    }

    async fn search_messages(&self, query: &str) -> Result<Vec<Message>, BrainError> {
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        let m1 = Message::new(
            DomainMessageId::new(),
            MessageRole::User,
            format!("Found message: {}", query),
        );
        Ok(vec![m1])
    }

    async fn inspect_node(
        &self,
        id: brain_domain::NodeId,
    ) -> Result<brain_domain::query::inspector::InspectorModel, BrainError> {
        let entity = brain_domain::dtos::NodeDTO::new(
            id.to_string(),
            "Mock Node".to_string(),
            "Technology".to_string(),
            serde_json::Value::Null,
        );
        Ok(brain_domain::query::inspector::InspectorModel {
            entity,
            metadata: std::collections::HashMap::new(),
            relationships: vec![],
            provenance: brain_domain::query::inspector::ProvenanceDTO {
                source: "Mock".to_string(),
                location: "Mock Location".to_string(),
                timestamp: 0,
                extra_info: std::collections::HashMap::new(),
            },
            retrieval_explanation: None,
            recent_activity: vec![],
        })
    }
}

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

#[tokio::test]
async fn test_remote_messages_provider_success() {
    let client = Arc::new(MockSearchClient { delay_ms: 50 });
    let provider = RemoteMessagesProvider::new(client);
    let sink = Arc::new(MockEventSink::new());

    let query = SearchQuery {
        generation: SearchGeneration(1),
        text: "target query".to_string(),
    };
    let context = SearchContext {
        sessions: vec![],
        active_messages: vec![],
    };

    provider.search(&query, &context, CancellationToken::new(), sink.clone());

    // Sleep to let async task complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    let events = sink.get_events();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], SearchEvent::Started { .. }));

    if let SearchEvent::Results { results, .. } = &events[1] {
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Found message: target query");
        assert_eq!(results[0].kind, SearchResultKind::Message);
    } else {
        panic!("Expected results event");
    }
    assert!(matches!(events[2], SearchEvent::Finished { .. }));
}

#[tokio::test]
async fn test_remote_messages_provider_cancellation() {
    let client = Arc::new(MockSearchClient { delay_ms: 100 });
    let provider = RemoteMessagesProvider::new(client);
    let sink = Arc::new(MockEventSink::new());

    let query = SearchQuery {
        generation: SearchGeneration(1),
        text: "cancel me".to_string(),
    };
    let context = SearchContext {
        sessions: vec![],
        active_messages: vec![],
    };

    let token = CancellationToken::new();
    provider.search(&query, &context, token.clone(), sink.clone());

    // Cancel immediately
    token.cancel();

    // Sleep to let task run
    tokio::time::sleep(Duration::from_millis(150)).await;

    let events = sink.get_events();

    // Only Started event should have been emitted before cancellation,
    // and Results/Finished must not be emitted.
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], SearchEvent::Started { .. }));
}
