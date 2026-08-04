use brain_domain::{Message, MessageId, MessageRole, SessionId};
use brain_tui::state::SessionViewModel;
use brain_tui::ui::search::providers::{CommandsProvider, LocalMessagesProvider, SessionsProvider};
use brain_tui::ui::search::types::{
    SearchContext, SearchEvent, SearchEventSink, SearchGeneration, SearchProvider, SearchQuery,
    SearchResultKind,
};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
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

fn create_search_context() -> SearchContext {
    let s1 = SessionViewModel {
        id: SessionId::new(),
        title: "Rust Architecture".to_string(),
        updated_at: SystemTime::now(),
        active: true,
        preview: None,
        pinned: false,
        archived: false,
    };
    let s2 = SessionViewModel {
        id: SessionId::new(),
        title: "Archived Notes".to_string(),
        updated_at: SystemTime::now(),
        active: false,
        preview: None,
        pinned: false,
        archived: true,
    };

    let m1 = Message::new(
        MessageId::new(),
        MessageRole::User,
        "Hello world rust code".to_string(),
    );
    let m2 = Message::new(
        MessageId::new(),
        MessageRole::Assistant,
        "This is an assistant response".to_string(),
    );

    SearchContext {
        sessions: vec![s1, s2],
        active_messages: vec![m1, m2],
    }
}

#[test]
fn test_commands_provider() {
    let provider = CommandsProvider;
    let sink = Arc::new(MockEventSink::new());
    let query = SearchQuery {
        generation: SearchGeneration(1),
        text: "theme".to_string(),
    };
    let context = create_search_context();

    provider.search(&query, &context, CancellationToken::new(), sink.clone());

    let events = sink.get_events();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], SearchEvent::Started { .. }));

    if let SearchEvent::Results { results, .. } = &events[1] {
        assert!(!results.is_empty());
        assert_eq!(results[0].kind, SearchResultKind::Command);
        assert!(results[0]
            .title
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains("theme"));
    } else {
        panic!("Expected results event");
    }
    assert!(matches!(events[2], SearchEvent::Finished { .. }));
}

#[test]
fn test_sessions_provider() {
    let provider = SessionsProvider;
    let sink = Arc::new(MockEventSink::new());
    let query = SearchQuery {
        generation: SearchGeneration(1),
        text: "arch".to_string(),
    };
    let context = create_search_context();

    provider.search(&query, &context, CancellationToken::new(), sink.clone());

    let events = sink.get_events();
    assert_eq!(events.len(), 3);

    if let SearchEvent::Results { results, .. } = &events[1] {
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, Some("Rust Architecture".to_string()));
        assert_eq!(results[1].title, Some("Archived Notes".to_string()));
        assert_eq!(results[0].kind, SearchResultKind::Session);
    } else {
        panic!("Expected results event");
    }
}

#[test]
fn test_local_messages_provider() {
    let provider = LocalMessagesProvider;
    let sink = Arc::new(MockEventSink::new());
    let query = SearchQuery {
        generation: SearchGeneration(1),
        text: "assistant".to_string(),
    };
    let context = create_search_context();

    provider.search(&query, &context, CancellationToken::new(), sink.clone());

    let events = sink.get_events();
    assert_eq!(events.len(), 3);

    if let SearchEvent::Results { results, .. } = &events[1] {
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].title,
            Some("This is an assistant response".to_string())
        );
        assert_eq!(results[0].kind, SearchResultKind::Message);
    } else {
        panic!("Expected results");
    }
}
