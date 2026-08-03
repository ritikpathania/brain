use async_trait::async_trait;
use brain_core::errors::BrainError;
use brain_domain::{Message, SessionId};
use brain_tui::client::{
    Confidence, EventReceiver, ExecutionClient, ExecutionRequest, SearchCandidate, SessionSummary,
};
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

    async fn search_candidates(
        &self,
        query: &str,
    ) -> Result<Vec<SearchCandidate>, BrainError> {
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        Ok(vec![SearchCandidate {
            entity_id: "test-entity-id".to_string(),
            title: Some(format!("Found knowledge: {}", query)),
            summary: Some("Test summary".to_string()),
            score: 0.85,
            confidence: Confidence::High,
        }])
    }

    async fn inspect_entity(
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

    async fn list_memories(
        &self,
        _filter: brain_domain::MemoryFilter,
    ) -> Result<Vec<brain_domain::MemorySummary>, BrainError> {
        Ok(vec![])
    }

    async fn mutate_memory(
        &self,
        _id: &str,
        _mutation: brain_domain::MemoryMutation,
    ) -> Result<(), BrainError> {
        Ok(())
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
    tokio::time::sleep(Duration::from_millis(150)).await;

    let events = sink.get_events();
    assert_eq!(events.len(), 3, "Expected Started + Results + Finished");
    assert!(matches!(events[0], SearchEvent::Started { .. }));

    if let SearchEvent::Results { results, .. } = &events[1] {
        assert_eq!(results.len(), 1);
        // Knowledge graph entities use Knowledge kind — NOT Message
        assert_eq!(results[0].kind, SearchResultKind::Knowledge);
        // Title is preserved as Some — ViewModel resolves None at the presentation boundary
        assert_eq!(
            results[0].title,
            Some("Found knowledge: target query".to_string())
        );
        // entity_id must be propagated
        assert_eq!(results[0].entity_id, "test-entity-id");
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

#[tokio::test]
async fn test_mutation_activity_log_emission_on_success_vs_rollback() {
    let mut model = brain_domain::query::inspector::InspectorModel {
        entity: brain_domain::dtos::NodeDTO::new(
            "mem_test".to_string(),
            "Test Entity".to_string(),
            "Memory".to_string(),
            serde_json::Value::Null,
        ),
        metadata: std::collections::HashMap::new(),
        relationships: vec![],
        provenance: brain_domain::query::inspector::ProvenanceDTO {
            source: "Test Engine".to_string(),
            location: "Memory Store".to_string(),
            timestamp: 100,
            extra_info: std::collections::HashMap::new(),
        },
        retrieval_explanation: None,
        recent_activity: vec![],
    };

    // Simulated successful mutation appends activity entry
    model
        .recent_activity
        .push(brain_domain::query::inspector::ActivityLogEntry {
            timestamp: 101,
            action: "Pinned".to_string(),
            details: "Memory pinned to active context by user".to_string(),
        });

    let vm = brain_tui::ui::view_models::InspectorViewModel::from_domain(&model);
    let activity_section = vm
        .sections
        .iter()
        .find(|s| s.id() == brain_tui::ui::view_models::EntitySectionId::ActivityFeed);
    assert!(activity_section.is_some());

    if let Some(brain_tui::ui::view_models::EntitySection::ActivityFeed { entries }) =
        activity_section
    {
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "Pinned");
        assert_eq!(
            entries[0].details,
            "Memory pinned to active context by user"
        );
    } else {
        panic!("Expected ActivityFeed section");
    }

    // Simulated rolled back mutation -> remove entry, leaving feed consistent
    model.recent_activity.pop();
    let rolled_back_vm = brain_tui::ui::view_models::InspectorViewModel::from_domain(&model);
    let rolled_back_activity_section = rolled_back_vm
        .sections
        .iter()
        .find(|s| s.id() == brain_tui::ui::view_models::EntitySectionId::ActivityFeed);
    assert!(rolled_back_activity_section.is_none());
}
