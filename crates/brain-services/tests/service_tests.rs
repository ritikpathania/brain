use brain_core::repositories::RepositorySet;
use brain_core::services::{RetrievalService, SessionService};
use brain_domain::{
    Conversation, ConversationId, Edge, MemoryDTO, Message, MessageRole, Node, NodeDTO, NodeId,
    NodeType,
};
use brain_services::{
    RetrievalServiceImpl, SessionServiceImpl, StubRetrievalService, StubSessionService,
};
use brain_session::SessionCacheManager;
use brain_storage::TestStorage;
use std::sync::Arc;

#[test]
fn test_session_service_lifecycle() {
    let test_store = TestStorage::new();
    let repos = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());

    let service = SessionServiceImpl::new(repos.clone(), cache_manager.clone());

    // 1. Create session
    let session_id = service.create_session().unwrap();
    assert!(service.session_exists(&session_id).unwrap());

    // 2. Load empty session
    let conversation = service.load_session(&session_id).unwrap();
    assert!(conversation.messages.is_empty());

    // 3. Save conversation history
    let message = Message::new(
        brain_domain::MessageId::new(),
        MessageRole::User,
        "Hello".to_string(),
    );
    let history = Conversation::new(ConversationId(session_id.0)).with_messages(vec![message]);
    service.save_session(&session_id, &history).unwrap();

    let loaded = service.load_session(&session_id).unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "Hello");

    // 4. Ingest node
    let node_id = NodeId::new();
    let node = Node::new(node_id, "Memory Node".to_string(), NodeType::Concept);
    service.ingest_node(&session_id, node).unwrap();

    // Verify it is saved in storage
    let stored_node = repos.nodes().find_by_id(&node_id).unwrap().unwrap();
    assert_eq!(stored_node.label, "Memory Node");

    // Verify it is cached in STM
    let ctx = cache_manager.get_or_create(session_id);
    let cached_nodes = ctx.read().unwrap().query("Memory");
    assert_eq!(cached_nodes.len(), 1);
    assert_eq!(cached_nodes[0].node.label, "Memory Node");

    // 5. Delete session
    service.delete_session(&session_id).unwrap();
    assert!(!service.session_exists(&session_id).unwrap());
    assert!(!cache_manager.exists(&session_id));
}

#[test]
fn test_retrieval_service_stm_and_ltm() {
    let test_store = TestStorage::new();
    let repos = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());

    let session_service = SessionServiceImpl::new(repos.clone(), cache_manager.clone());
    let retrieval_service = RetrievalServiceImpl::new(repos.clone(), cache_manager.clone());

    let session_id = session_service.create_session().unwrap();

    // Setup: Node A in STM and LTM, Node B in LTM only
    let node_a_id = NodeId::new();
    let node_a = Node::new(node_a_id, "Apple Juice".to_string(), NodeType::Concept);

    let node_b_id = NodeId::new();
    let node_b = Node::new(node_b_id, "Orange Juice".to_string(), NodeType::Concept);

    // Save Node B to LTM directly (not in cache)
    repos.nodes().save(&node_b).unwrap();

    // Ingest Node A (this saves it to LTM AND caches it in STM)
    session_service.ingest_node(&session_id, node_a).unwrap();

    // Add some edge connections
    let edge = Edge::new(node_a_id, node_b_id, "related".to_string(), 0.8);
    repos.edges().save(&edge).unwrap();

    // Query for "Juice". Should get Node A (from STM) and Node B (from LTM)
    let results = retrieval_service
        .retrieve(&session_id, "Juice", 10)
        .unwrap();
    assert_eq!(results.len(), 2);

    // Node A is first (since STM is queried first)
    assert_eq!(results[0].node.label, "Apple Juice");
    assert_eq!(results[0].outgoing_edges.len(), 1);
    assert_eq!(results[0].outgoing_edges[0].target, node_b_id.to_string());

    // Node B is second (ltm)
    assert_eq!(results[1].node.label, "Orange Juice");
    assert_eq!(results[1].incoming_edges.len(), 1);
    assert_eq!(results[1].incoming_edges[0].source, node_a_id.to_string());

    // Limit check
    let results_limited = retrieval_service.retrieve(&session_id, "Juice", 1).unwrap();
    assert_eq!(results_limited.len(), 1);
    assert_eq!(results_limited[0].node.label, "Apple Juice");
}

#[test]
fn test_cache_precedence_over_ltm() {
    let test_store = TestStorage::new();
    let repos = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());

    let session_service = SessionServiceImpl::new(repos.clone(), cache_manager.clone());
    let retrieval_service = RetrievalServiceImpl::new(repos.clone(), cache_manager.clone());

    let session_id = session_service.create_session().unwrap();

    // Create a node with same ID but different label in cache (STM) and DB (LTM)
    let node_id = NodeId::new();

    // Save Node A (DB version) to DB
    let node_db = Node::new(
        node_id,
        "Node A (DB Version)".to_string(),
        NodeType::Concept,
    );
    repos.nodes().save(&node_db).unwrap();

    // Ingest Node A (Cache version) to STM
    let node_cache = Node::new(
        node_id,
        "Node A (Cache Version)".to_string(),
        NodeType::Concept,
    );
    session_service
        .ingest_node(&session_id, node_cache)
        .unwrap();

    // Query should return the cache version
    let res = retrieval_service
        .retrieve(&session_id, "Node A", 5)
        .unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].node.label, "Node A (Cache Version)");
}

#[test]
fn test_retrieval_cache_miss_db_hit_populates_cache() {
    let test_store = TestStorage::new();
    let repos = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());

    let session_service = SessionServiceImpl::new(repos.clone(), cache_manager.clone());
    let retrieval_service = RetrievalServiceImpl::new(repos.clone(), cache_manager.clone());

    let session_id = session_service.create_session().unwrap();

    // 1. Save Node directly in DB (LTM) only - not in STM cache
    let node_id = NodeId::new();
    let node = Node::new(
        node_id,
        "Python Data Science".to_string(),
        NodeType::Concept,
    );
    repos.nodes().save(&node).unwrap();

    // Verify cache is empty initially
    {
        let ctx = cache_manager.get_or_create(session_id);
        let ctx_read = ctx.read().unwrap();
        assert_eq!(ctx_read.len(), 0);
    }

    // 2. Perform retrieval, which should hit the DB (LTM)
    let res = retrieval_service
        .retrieve(&session_id, "Python", 5)
        .unwrap();
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].node.label, "Python Data Science");

    // 3. Verify that the cache (STM) is now populated with the node from DB hit!
    {
        let ctx = cache_manager.get_or_create(session_id);
        let ctx_read = ctx.read().unwrap();
        assert_eq!(ctx_read.len(), 1);
        assert_eq!(
            ctx_read.iter().next().unwrap().node.label,
            "Python Data Science"
        );
    }
}

#[test]
fn test_stubs() {
    // 1. StubSessionService
    let session_service = StubSessionService::new();
    let session_id = session_service.create_session().unwrap();
    assert!(session_service.session_exists(&session_id).unwrap());

    let node = Node::new(NodeId::new(), "Test Node".to_string(), NodeType::Concept);
    session_service.ingest_node(&session_id, node).unwrap();

    let conversation = session_service.load_session(&session_id).unwrap();
    assert!(conversation.messages.is_empty());

    // 2. StubRetrievalService
    let retrieval_service = StubRetrievalService::new();
    let res = retrieval_service
        .retrieve(&session_id, "anything", 5)
        .unwrap();
    assert_eq!(res.len(), 1);
    assert!(res[0].node.label.contains("Stub result for"));

    // Set mock results
    let mock_node = NodeDTO::new(
        NodeId::new().to_string(),
        "Custom Mock".to_string(),
        "Concept".to_string(),
        serde_json::json!({}),
    );
    let mock_memory = MemoryDTO::new(mock_node, Vec::new(), Vec::new());
    retrieval_service.set_results(vec![mock_memory]);

    let res_mocked = retrieval_service.retrieve(&session_id, "mock", 5).unwrap();
    assert_eq!(res_mocked.len(), 1);
    assert_eq!(res_mocked[0].node.label, "Custom Mock");
}
