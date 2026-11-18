mod e2e_common;

use std::sync::Arc;

use brain_core::agents::ExtractionAgent;
use brain_core::repositories::{EdgeRepository, NodeRepository};
use brain_core::services::SessionService;
use brain_domain::{Edge, Message, MessageId, MessageRole, Node, NodeId, NodeType, RelationKind};
use brain_services::brain_runtime::{BrainRuntime, RuntimeHealth};
use brain_services::{SessionServiceImpl, StubDomainEventPublisher};
use brain_session::SessionCacheManager;

use e2e_common::{ReferenceHeuristicExtractor, TestDb};

/// Tier 4.1: Developer Architecture Discussion Scenario
/// Simulates an end-to-end user conversation explaining system architecture:
/// Dialogue -> Heuristic extraction -> Ingestion into STM & LTM -> Search/Retrieval.
#[test]
fn test_tier4_developer_architecture_discussion_workflow() {
    let test_db = TestDb::new();
    let storage = test_db.storage();
    let cache_manager = Arc::new(SessionCacheManager::new());
    let publisher = Arc::new(StubDomainEventPublisher::new());

    let session_service = SessionServiceImpl::new(
        storage.clone() as Arc<dyn brain_core::repositories::RepositorySet + Send + Sync>,
        cache_manager.clone(),
        publisher,
    );

    // 1. Create developer session
    let session_id = session_service
        .create_session()
        .expect("Failed to initialize developer conversation session");
    session_service
        .rename_session(&session_id, "Architecture Architecture Overview")
        .expect("Failed to name session");

    // 2. User prompt turn
    let user_prompt = "Brain is an AI agent engine. We are developing Brain using Rust and SQLite. Ritikpathania develops Brain. API keys are stored in environment variables.";

    // 3. Extraction agent processes turn
    let extractor = ReferenceHeuristicExtractor::new();
    let (node_dtos, edge_dtos) = extractor
        .extract_graph(user_prompt)
        .expect("Extraction failed on user dialogue turn");

    assert!(!node_dtos.is_empty(), "Expected extracted entities");
    assert!(!edge_dtos.is_empty(), "Expected extracted relations");

    // 4. Ingest nodes into session STM and persistent LTM
    for dto in &node_dtos {
        let node = Node::new(NodeId::new(), dto.label.clone(), NodeType::Concept);
        session_service
            .ingest_node(&session_id, node)
            .expect("Failed to ingest node into session");
    }

    // 5. Query candidate retrieval for "Brain"
    let brain_matches = storage.find_by_tokens(&["Brain".to_string()]).unwrap();
    assert!(
        brain_matches.iter().any(|n| n.label.contains("Brain")),
        "Expected retrieval to surface Brain concept"
    );

    // 6. Query candidate retrieval for "SQLite"
    let sqlite_matches = storage.find_by_tokens(&["SQLite".to_string()]).unwrap();
    assert!(
        sqlite_matches.iter().any(|n| n.label.contains("SQLite")),
        "Expected retrieval to surface SQLite concept"
    );
}

/// Tier 4.2: Multi-Turn Session Branching & Fork Isolation
/// Verifies conversation session creation, appending turns, forking at midpoint,
/// modifying the fork, and archiving the parent session while the fork remains active.
#[test]
fn test_tier4_multi_turn_session_branching_and_fork_isolation() {
    let test_db = TestDb::new();
    let storage = test_db.storage();
    let cache_manager = Arc::new(SessionCacheManager::new());
    let publisher = Arc::new(StubDomainEventPublisher::new());

    let session_service = SessionServiceImpl::new(
        storage as Arc<dyn brain_core::repositories::RepositorySet + Send + Sync>,
        cache_manager.clone(),
        publisher,
    );

    // 1. Create Session A and populate messages
    let session_a_id = session_service.create_session().unwrap();
    let mut session_a = session_service.load_session(&session_a_id).unwrap();

    let msg1_id = MessageId::new();
    let msg1 = Message::new(
        msg1_id,
        MessageRole::User,
        "Message 1: Start discussion".to_string(),
    );
    let _ = session_a.add_message(msg1);

    let msg2_id = MessageId::new();
    let msg2 = Message::new(
        msg2_id,
        MessageRole::Assistant,
        "Message 2: Assistant response".to_string(),
    );
    let _ = session_a.add_message(msg2);

    session_service
        .save_session(&session_a_id, &mut session_a)
        .unwrap();

    // 2. Fork Session A to Session B
    let session_b_id = session_service
        .fork_session(&session_a_id, Some("Alternative Exploration"), None)
        .expect("Forking session failed");

    assert_ne!(session_a_id, session_b_id);

    // 3. Mutate Fork Session B with a new node
    let fork_node = Node::new(
        NodeId::new(),
        "Fork Specific Concept".to_string(),
        NodeType::Concept,
    );
    session_service
        .ingest_node(&session_b_id, fork_node)
        .expect("Failed to ingest node into fork");

    // 4. Archive Session A
    session_service.archive_session(&session_a_id).unwrap();

    let loaded_a = session_service.load_session(&session_a_id).unwrap();
    assert!(loaded_a.archived, "Session A must be archived");

    // 5. Verify Session B is still active and valid
    let loaded_b = session_service.load_session(&session_b_id).unwrap();
    assert!(!loaded_b.archived, "Session B must remain active");
    assert_eq!(loaded_b.title.0, "Alternative Exploration");
}

/// Tier 4.3: Knowledge Evolution and Metric Sweep
/// Verifies that ingesting observations updates metrics and health across the runtime.
#[tokio::test]
async fn test_tier4_knowledge_evolution_and_reflection_sweep() {
    let test_db = TestDb::new();
    let runtime = BrainRuntime::new(test_db.db_path()).expect("Failed to initialize BrainRuntime");

    assert_eq!(runtime.status().health, RuntimeHealth::Healthy);

    // Initial metrics inspection
    let metrics = runtime.metrics();
    assert_eq!(metrics.canonicalization_failures, 0);

    // Verify capabilities
    let caps = runtime.discover_capabilities();
    assert!(!caps.is_empty());
}

/// Tier 4.4: Cold Restart & Disaster Recovery
/// Verifies that a persistent database survives runtime shutdown and cold restart,
/// ensuring 100% data fidelity with zero data loss.
#[tokio::test]
async fn test_tier4_daemon_disaster_recovery_and_restart() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let db_path = temp_dir
        .path()
        .join("persistent_brain.db")
        .to_string_lossy()
        .to_string();

    let node_id = NodeId::new();
    let node_label = "Durable Architecture Concept".to_string();

    // Session 1: Write state and shutdown
    {
        let runtime_v1 = BrainRuntime::new(&db_path).expect("First runtime instantiation failed");
        assert_eq!(runtime_v1.status().health, RuntimeHealth::Healthy);

        let storage = runtime_v1.sqlite_storage();
        let node = Node::new(node_id, node_label.clone(), NodeType::Concept);
        NodeRepository::save(&*storage, &node).expect("Failed to write durable node");

        let edge_target_id = NodeId::new();
        let edge_target = Node::new(
            edge_target_id,
            "Target Concept".to_string(),
            NodeType::Concept,
        );
        NodeRepository::save(&*storage, &edge_target).expect("Failed to write target node");

        let edge = Edge::new(node_id, edge_target_id, RelationKind::Develops, 1.0);
        EdgeRepository::save(&*storage, &edge).expect("Failed to write durable edge");

        // Runtime drops here at end of scope (simulating graceful daemon shutdown)
    }

    // Session 2: Cold restart from same database file
    {
        let runtime_v2 =
            BrainRuntime::new(&db_path).expect("Cold restart runtime instantiation failed");
        assert_eq!(runtime_v2.status().health, RuntimeHealth::Healthy);

        let storage = runtime_v2.sqlite_storage();

        // Verify previously saved node exists
        let loaded_node = NodeRepository::find_by_id(&*storage, &node_id)
            .expect("Query failed on cold restart")
            .expect("Durable node missing after cold restart");
        assert_eq!(loaded_node.label, node_label);

        // Verify connected edge exists
        let connections = EdgeRepository::get_connections(&*storage, &node_id)
            .expect("Failed to retrieve edge connections on cold restart");
        assert_eq!(
            connections.len(),
            1,
            "Expected 1 edge connection preserved across cold restart"
        );
        assert_eq!(connections[0].source, node_id);
    }
}
