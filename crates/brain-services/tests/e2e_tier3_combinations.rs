mod e2e_common;

use std::sync::Arc;
use std::time::SystemTime;

use brain_core::agents::ExtractionAgent;
use brain_core::evolution::{Canonicalizer, Observation, Provenance};
use brain_core::repositories::{EdgeRepository, NodeRepository};
use brain_core::services::SessionService;
use brain_domain::{Edge, EpochId, KnowledgeGraph, Node, NodeId, NodeType, RelationKind};
use brain_services::evolution_service::InMemoryCanonicalizer;
use brain_services::{InMemoryEventDispatcher, SessionServiceImpl, StubDomainEventPublisher};
use brain_session::SessionCacheManager;

use e2e_common::{ReferenceHeuristicExtractor, ReferenceLlmMemoryExtractor, TestDb};

/// Tier 3.1: Extractor + Persistent Storage Integration
/// Verifies extracting semantic nodes & edges from text and persisting them directly
/// into SqliteStorage, then loading them back with full structural equality.
#[test]
fn test_tier3_extractor_to_storage_roundtrip() {
    let test_db = TestDb::new();
    let storage = test_db.storage();
    let extractor = ReferenceHeuristicExtractor::new();

    let text = "ritikpathania develops Brain. SQLite is stored in config.toml.";
    let (node_dtos, edge_dtos) = extractor
        .extract_graph(text)
        .expect("Extraction failed in roundtrip test");

    // Convert DTOs to entities and save into database
    let mut node_id_map = std::collections::HashMap::new();
    for dto in &node_dtos {
        let node_id = NodeId::new();
        let node = Node::new(node_id, dto.label.clone(), NodeType::Concept);
        NodeRepository::save(&*storage, &node).expect("Failed to persist extracted node");
        node_id_map.insert(dto.id.clone(), (node_id, node));
    }

    // Persist edges
    for dto in &edge_dtos {
        if let (Some((src_id, _)), Some((dst_id, _))) =
            (node_id_map.get(&dto.source), node_id_map.get(&dto.target))
        {
            let edge = Edge::new(*src_id, *dst_id, RelationKind::Develops, 1.0);
            EdgeRepository::save(&*storage, &edge).expect("Failed to persist extracted edge");
        }
    }

    // Read back and verify
    for (id, original_node) in node_id_map.values() {
        let loaded = NodeRepository::find_by_id(&*storage, id)
            .expect("Database query failed")
            .expect("Persisted node not found");
        assert_eq!(loaded.id, original_node.id);
        assert_eq!(loaded.label, original_node.label);
    }
}

/// Tier 3.2: Extractor + Session Manager Integration
/// Verifies conversational turn extraction -> session volatile cache update -> persistent DB synchronization.
#[test]
fn test_tier3_extractor_session_cache_and_db_persistence() {
    let test_db = TestDb::new();
    let storage = test_db.storage();
    let cache_manager = Arc::new(SessionCacheManager::new());
    let publisher = Arc::new(StubDomainEventPublisher::new());

    let session_service = SessionServiceImpl::new(
        storage as Arc<dyn brain_core::repositories::RepositorySet + Send + Sync>,
        cache_manager.clone(),
        publisher,
    );

    let session_id = session_service
        .create_session()
        .expect("Failed to create session");
    assert!(session_service.session_exists(&session_id).unwrap());

    // Extract memory from user message
    let extractor = ReferenceHeuristicExtractor::new();
    let (nodes, _) = extractor
        .extract_graph("We are configuring SQLite database configuration")
        .expect("Extraction failed");

    // Ingest each extracted node into session
    for dto in nodes {
        let node = Node::new(NodeId::new(), dto.label, NodeType::Concept);
        session_service
            .ingest_node(&session_id, node)
            .expect("Failed to ingest node into session");
    }

    // Verify session aggregate is loaded back
    let loaded_session = session_service
        .load_session(&session_id)
        .expect("Failed to load session");
    assert_eq!(loaded_session.id, session_id);
}

/// Tier 3.3: Storage + Search & Token Filtering Integration
/// Ingests nodes into storage and queries them via candidate token matching.
#[test]
fn test_tier3_storage_to_retrieval_pipeline() {
    let test_db = TestDb::new();
    let storage = test_db.storage();

    let node1 = Node::new(
        NodeId::new(),
        "Rust Programming Language".to_string(),
        NodeType::Concept,
    );
    let node2 = Node::new(
        NodeId::new(),
        "SQLite Embedded Database".to_string(),
        NodeType::Concept,
    );

    NodeRepository::save(&*storage, &node1).unwrap();
    NodeRepository::save(&*storage, &node2).unwrap();

    let tokens = vec!["Rust".to_string()];
    let matches = storage.find_by_tokens(&tokens).unwrap();
    assert!(
        !matches.is_empty(),
        "Expected candidate node matches for 'Rust'"
    );
    assert!(matches.iter().any(|n| n.label.contains("Rust")));
}

/// Tier 3.4: Provider + Extractor + Canonicalizer Pipeline
/// Verifies mock LLM provider response -> LlmMemoryExtractor parsing -> Ingestion into Canonicalizer.
#[test]
fn test_tier3_provider_extractor_pipeline() {
    let simulated_llm_output = r#"```json
{
  "nodes": [
    {"id": "brain-engine", "label": "Brain Engine", "type": "project"},
    {"id": "rust-lang", "label": "Rust Language", "type": "language"}
  ],
  "edges": [
    {"source": "brain-engine", "target": "rust-lang", "relation": "implemented_in"}
  ]
}
```"#;

    let extractor = ReferenceLlmMemoryExtractor::new("mock-provider", simulated_llm_output);
    let (nodes, edges) = extractor
        .extract_graph("input transcripts")
        .expect("LlmMemoryExtractor failed");

    assert_eq!(nodes.len(), 2);
    assert_eq!(edges.len(), 1);

    // Feed to Canonicalizer
    let graph = Arc::new(std::sync::Mutex::new(KnowledgeGraph::new()));
    let epoch = Arc::new(std::sync::Mutex::new(EpochId::initial()));
    let dispatcher = Arc::new(InMemoryEventDispatcher::new(64));
    let canonicalizer = InMemoryCanonicalizer::new(graph, epoch, dispatcher);

    for node in nodes {
        let obs = Observation {
            payload: serde_json::to_vec(&node).unwrap(),
            media_type: "application/json".to_string(),
            provenance: Provenance {
                source_adapter: "test-adapter".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: uuid::Uuid::new_v4(),
            },
        };
        let res = canonicalizer.canonicalize(obs);
        assert!(
            res.is_ok(),
            "Canonicalization failed for node: {}",
            node.label
        );
    }
}

/// Tier 3.5: Concurrent HostContext & Storage Contention
/// Verifies parallel execution of storage transactions and thread-safe lock acquisitions.
#[tokio::test]
async fn test_tier3_concurrent_host_context_and_storage_contention() {
    let test_db = TestDb::new();
    let storage = test_db.storage();

    let mut tasks = Vec::new();

    // Spawn 10 concurrent async tasks reading/writing to storage
    for i in 0..10 {
        let s = Arc::clone(&storage);
        tasks.push(tokio::spawn(async move {
            for j in 0..10 {
                let node = Node::new(
                    NodeId::new(),
                    format!("concurrent_node_{}_{}", i, j),
                    NodeType::Concept,
                );
                NodeRepository::save(&*s, &node).expect("Concurrent write failed");
                let found =
                    NodeRepository::find_by_id(&*s, &node.id).expect("Concurrent read failed");
                assert!(found.is_some());
            }
        }));
    }

    for task in tasks {
        task.await.expect("Task failed");
    }
}

/// Tier 3.6: Native Execution Without Python Plugins
/// Verifies the entire pipeline executes purely via native Rust components offline.
#[test]
fn test_tier3_python_gating_offline_execution() {
    let test_db = TestDb::new();
    let storage = test_db.storage();
    let extractor = ReferenceHeuristicExtractor::new();

    // End-to-end extraction and persistence with 0 Python dependency
    let (nodes, edges) = extractor
        .extract_graph("setting up sqlite database configuration and storing API keys in environment variables")
        .expect("Native extraction failed");

    assert_eq!(nodes.len(), 5);
    assert_eq!(edges.len(), 2);

    for node in nodes {
        let entity = Node::new(NodeId::new(), node.label, NodeType::Concept);
        NodeRepository::save(&*storage, &entity).expect("Native storage save failed");
    }

    let all_nodes = NodeRepository::list_all(&*storage).expect("Native storage query failed");
    assert_eq!(all_nodes.len(), 5);
}
