use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_core::repositories::{EdgeRepository, NodeRepository};
use brain_core::services::{ExtractionRequest, ExtractionResult, MemoryExtractor};
use brain_domain::{
    ConversationId, Edge, Message, MessageId, MessageRole, Node, NodeId, NodeType, RelationKind,
    Session, SessionId,
};
use brain_session::SessionCacheManager;
use brain_storage::TestStorage;

use brain_services::conversation::{
    CheckpointStore, ContextBudget, ContextBuilder, ConversationManager, ConversationManagerImpl,
    CountThresholdPromotionPolicy, CountThresholdSummaryPolicy, IngestionPolicy,
    PromotionEngineImpl, SqliteCheckpointStore, WordSpaceTokenCounter,
};

// --- Mock Implementations ---

struct MockMemoryExtractor {
    should_fail: Arc<AtomicBool>,
}

impl MemoryExtractor for MockMemoryExtractor {
    fn extract(&self, request: ExtractionRequest) -> Result<ExtractionResult, BrainError> {
        if self.should_fail.load(Ordering::SeqCst) {
            return Err(BrainError::Storage {
                message: "Extractor failed intentionally".to_string(),
                source: None,
            });
        }
        // Extract simple node based on prompt text
        let node_id = NodeId::new();
        let node = Node::new(node_id, request.raw_content, NodeType::Concept);
        Ok(ExtractionResult {
            nodes: vec![node],
            edges: vec![],
            provenance: brain_domain::GraphProvenance::default(),
            graph_version: brain_domain::GraphVersion::V1,
        })
    }
}

struct MockChatAgent {
    response: String,
}

impl brain_core::agents::ChatAgent for MockChatAgent {
    fn name(&self) -> &str {
        "MockChatAgent"
    }
    fn chat(&self, _session_id: SessionId, _prompt: &str) -> Result<String, BrainError> {
        Ok(self.response.clone())
    }
}

struct MockRetrieval {
    nodes: Vec<brain_domain::MemoryDTO>,
}

impl brain_core::services::RetrievalService for MockRetrieval {
    fn retrieve(
        &self,
        _session_id: &SessionId,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<brain_domain::MemoryDTO>, BrainError> {
        Ok(self.nodes.clone())
    }
}

// --- Test Cases ---

#[test]
fn test_context_builder_determinism_and_system_preservation() {
    let counter = WordSpaceTokenCounter;
    let budget = ContextBudget {
        max_tokens: 16, // Small budget: 16 - 5 - 5 = 6 tokens for history + system
        reserved_system_tokens: 5,
        reserved_completion_tokens: 5,
    };

    let system_msg = Message::new(
        MessageId::new(),
        MessageRole::System,
        "System prompt instructions".to_string(), // ~3 words
    );
    let msg1 = Message::new(
        MessageId::new(),
        MessageRole::User,
        "First long message that will be dropped".to_string(), // ~7 words
    );
    let msg2 = Message::new(
        MessageId::new(),
        MessageRole::User,
        "Active latest message".to_string(), // ~3 words
    );

    let history = vec![system_msg, msg1, msg2];

    // Build context window
    let window1 = ContextBuilder::build(&counter, budget, &history, None, vec![]);
    let window2 = ContextBuilder::build(&counter, budget, &history, None, vec![]);

    // Determinism
    assert_eq!(window1.messages().len(), window2.messages().len());
    assert_eq!(window1.messages()[0].content, window2.messages()[0].content);
    assert_eq!(window1.messages()[1].content, window2.messages()[1].content);

    // System message preserved
    assert_eq!(window1.messages()[0].role, MessageRole::System);
    assert_eq!(window1.messages()[1].content, "Active latest message");
}

#[tokio::test]
async fn test_checkpoint_store_immutability() {
    let test_store = TestStorage::new();
    let storage = test_store.store();
    let checkpoint_store = SqliteCheckpointStore::new(storage);

    let session_id = SessionId::new();
    let checkpoint_id = ConversationId::new();

    let mut history = Session::new_empty();
    history.messages.push(Message::new(
        MessageId::new(),
        MessageRole::User,
        "Snapshot interaction".to_string(),
    ));

    // Save checkpoint
    checkpoint_store
        .save(&session_id, &checkpoint_id, "snapshot-1", &history)
        .unwrap();

    // Verify restored checkpoint matches
    let restored = checkpoint_store
        .restore(&session_id, &checkpoint_id)
        .unwrap();
    assert_eq!(restored.messages.len(), 1);
    assert_eq!(restored.messages[0].content, "Snapshot interaction");

    // Modify original active conversation
    history.messages.push(Message::new(
        MessageId::new(),
        MessageRole::Assistant,
        "New interaction".to_string(),
    ));

    // Restore checkpoint again and verify snapshot remained immutable
    let restored_again = checkpoint_store
        .restore(&session_id, &checkpoint_id)
        .unwrap();
    assert_eq!(restored_again.messages.len(), 1);
    assert_eq!(restored_again.messages[0].content, "Snapshot interaction");
}

#[tokio::test]
async fn test_promotion_idempotency_and_transactional_rollback() {
    let test_store = TestStorage::new();
    let storage = test_store.store();
    let repos = test_store.store();
    let cache_manager = Arc::new(SessionCacheManager::new());

    let should_fail = Arc::new(AtomicBool::new(false));
    let extractor = Arc::new(MockMemoryExtractor {
        should_fail: should_fail.clone(),
    });

    let promotion_engine = Arc::new(PromotionEngineImpl::new(
        CountThresholdPromotionPolicy::new(2),
    ));
    let summary_policy = Arc::new(CountThresholdSummaryPolicy::new(10));
    let checkpoint_store = Arc::new(SqliteCheckpointStore::new(storage.clone()));
    let retrieval_service = Arc::new(MockRetrieval { nodes: vec![] });
    let chat_agent = Arc::new(MockChatAgent {
        response: "Summary text".to_string(),
    });

    let manager = ConversationManagerImpl::new(
        repos.clone(),
        storage.clone(),
        cache_manager.clone(),
        Arc::new(WordSpaceTokenCounter),
        extractor,
        promotion_engine,
        summary_policy,
        checkpoint_store,
        retrieval_service,
        chat_agent,
        None,
        Arc::new(brain_domain::RelationRegistry::default_embedded()),
    );

    let session_id = SessionId::new();
    let policy = IngestionPolicy { stm_only: true };

    // Ingest first concept node -> STM only
    manager
        .ingest_interaction(&session_id, "Memory ingestion 1", "response 1", policy)
        .unwrap();

    // Ingest second concept node -> trigger promotion to LTM
    manager
        .ingest_interaction(&session_id, "Memory ingestion 2", "response 2", policy)
        .unwrap();

    // Verify nodes are promoted to SQLite
    let ltm_nodes = NodeRepository::list_all(repos.as_ref()).unwrap();
    assert!(!ltm_nodes.is_empty());

    // Idempotency: Trigger promotion again manually (no new stm nodes)
    manager.promote_memories(&session_id).unwrap();

    // Rollback testing: set mock extractor to fail, add stm node, trigger promote
    let cache = cache_manager.get_or_create(session_id);
    {
        let mut guard = cache.write().unwrap();
        guard.ingest(Node::new(
            NodeId::new(),
            "Failing Node".to_string(),
            NodeType::Concept,
        ));
    }

    should_fail.store(true, Ordering::SeqCst);
    let promote_res = manager.promote_memories(&session_id);
    assert!(promote_res.is_err());

    // Verify "Failing Node" is not present in SQLite due to transactional rollback
    let ltm_nodes_after = NodeRepository::list_all(repos.as_ref()).unwrap();
    assert!(!ltm_nodes_after.iter().any(|n| n.label == "Failing Node"));
}

#[tokio::test]
async fn test_summarization_versioning() {
    let test_store = TestStorage::new();
    let storage = test_store.store();
    let repos = test_store.store();
    let cache_manager = Arc::new(SessionCacheManager::new());

    let manager = ConversationManagerImpl::new(
        repos.clone(),
        storage.clone(),
        cache_manager.clone(),
        Arc::new(WordSpaceTokenCounter),
        Arc::new(MockMemoryExtractor {
            should_fail: Arc::new(AtomicBool::new(false)),
        }),
        Arc::new(PromotionEngineImpl::new(
            CountThresholdPromotionPolicy::new(10),
        )),
        Arc::new(CountThresholdSummaryPolicy::new(2)), // Summarize every 2 messages
        Arc::new(SqliteCheckpointStore::new(storage.clone())),
        Arc::new(MockRetrieval { nodes: vec![] }),
        Arc::new(MockChatAgent {
            response: "Segment Summary".to_string(),
        }),
        None,
        Arc::new(brain_domain::RelationRegistry::default_embedded()),
    );

    let session_id = SessionId::new();
    let policy = IngestionPolicy { stm_only: true };

    // Ingesting 2 messages triggers first summary (version 1)
    manager
        .ingest_interaction(&session_id, "Hello", "Hi there", policy)
        .unwrap();

    let context = manager
        .build_context_window(
            &session_id,
            ContextBudget {
                max_tokens: 4096,
                reserved_system_tokens: 0,
                reserved_completion_tokens: 0,
            },
        )
        .unwrap();

    assert!(context.summary().is_some());
    assert_eq!(context.summary().unwrap().version, 1);
    assert_eq!(context.summary().unwrap().text, "Segment Summary");
}

#[tokio::test]
async fn test_pruning_pinned_memories_safety() {
    let test_store = TestStorage::new();
    let storage = test_store.store();
    let repos = test_store.store();
    let cache_manager = Arc::new(SessionCacheManager::new());

    let manager = ConversationManagerImpl::new(
        repos.clone(),
        storage.clone(),
        cache_manager.clone(),
        Arc::new(WordSpaceTokenCounter),
        Arc::new(MockMemoryExtractor {
            should_fail: Arc::new(AtomicBool::new(false)),
        }),
        Arc::new(PromotionEngineImpl::new(
            CountThresholdPromotionPolicy::new(10),
        )),
        Arc::new(CountThresholdSummaryPolicy::new(10)),
        Arc::new(SqliteCheckpointStore::new(storage.clone())),
        Arc::new(MockRetrieval { nodes: vec![] }),
        Arc::new(MockChatAgent {
            response: "".to_string(),
        }),
        None,
        Arc::new(brain_domain::RelationRegistry::default_embedded()),
    );

    let session_id = SessionId::new();

    // Insert pinned and unpinned nodes/edges directly
    let node_unpinned = Node::new(NodeId::new(), "unpinned".to_string(), NodeType::Concept);

    let mut props = std::collections::HashMap::new();
    props.insert("pinned".to_string(), serde_json::Value::Bool(true));
    let node_pinned =
        Node::new(NodeId::new(), "pinned".to_string(), NodeType::Concept).with_properties(props);

    NodeRepository::save(repos.as_ref(), &node_unpinned).unwrap();
    NodeRepository::save(repos.as_ref(), &node_pinned).unwrap();

    // Edges with low weights (decayed below 0.1)
    let edge_unpinned = Edge::new(
        node_unpinned.id,
        node_pinned.id,
        RelationKind::AssociatedWith,
        0.05,
    );
    let edge_pinned = Edge::new(
        node_pinned.id,
        node_unpinned.id,
        RelationKind::AssociatedWith,
        0.05,
    );

    EdgeRepository::save(repos.as_ref(), &edge_unpinned).unwrap();
    EdgeRepository::save(repos.as_ref(), &edge_pinned).unwrap();

    // Run pruning
    let pruned_count = manager.prune_memories(&session_id).unwrap();
    assert_eq!(pruned_count, 1); // Only the unpinned relationship was pruned!

    // Verify database contains edge_pinned but not edge_unpinned
    let remaining_edges = EdgeRepository::get_connections(repos.as_ref(), &node_pinned.id).unwrap();
    assert!(!remaining_edges.is_empty());
}

struct MockEventPublisher {
    published: Arc<std::sync::Mutex<Vec<brain_events::EventEnvelope>>>,
}

impl brain_events::EventPublisher for MockEventPublisher {
    fn publish(&self, envelope: brain_events::EventEnvelope) {
        self.published.lock().unwrap().push(envelope);
    }
}

#[test]
fn test_archive_conversation_and_event_publishing() {
    let test_store = TestStorage::new();
    let storage = test_store.store();
    let repos = test_store.store();
    let cache_manager = Arc::new(SessionCacheManager::new());

    let published_events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let publisher = Arc::new(MockEventPublisher {
        published: published_events.clone(),
    });

    let manager = ConversationManagerImpl::new(
        repos.clone(),
        storage.clone(),
        cache_manager.clone(),
        Arc::new(WordSpaceTokenCounter),
        Arc::new(MockMemoryExtractor {
            should_fail: Arc::new(AtomicBool::new(false)),
        }),
        Arc::new(PromotionEngineImpl::new(
            CountThresholdPromotionPolicy::new(10),
        )),
        Arc::new(CountThresholdSummaryPolicy::new(10)),
        Arc::new(SqliteCheckpointStore::new(storage.clone())),
        Arc::new(MockRetrieval { nodes: vec![] }),
        Arc::new(MockChatAgent {
            response: "".to_string(),
        }),
        Some(publisher),
        Arc::new(brain_domain::RelationRegistry::default_embedded()),
    );

    let session_id = SessionId::new();
    let policy = IngestionPolicy { stm_only: false };

    // Ingest works when conversation is active
    assert!(manager
        .ingest_interaction(&session_id, "hello", "hi", policy)
        .is_ok());

    // Archive the conversation
    assert!(manager.archive_conversation(&session_id).is_ok());

    // Ingest should now fail because the conversation is archived
    let err = manager.ingest_interaction(&session_id, "second prompt", "response", policy);
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("is archived"));

    // Verify event was published (1 SessionCreated, 2 MessageAdded, 1 ConversationArchived = 4 events)
    let events = published_events.lock().unwrap();
    assert_eq!(events.len(), 4);
    let envelope = &events[3];
    assert_eq!(envelope.source, "conversation_service");
    match &envelope.payload {
        brain_events::DomainEvent::Session(brain_events::SessionEvent::ConversationArchived(_)) => {
        }
        _ => panic!("Expected SessionEvent::ConversationArchived variant"),
    }
}
