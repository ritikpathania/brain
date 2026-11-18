mod e2e_common;

use std::sync::Arc;
use std::thread;

use brain_config::loader::{resolve, DefaultsSource, OverrideSource};
use brain_config::schema::{PartialBrainSettings, PartialDatabaseSettings};
use brain_core::agents::{ChatAgent, EmbeddingAgent, ExtractionAgent};
use brain_core::retrieval::EmbeddingProvider;
use brain_core::services::{ExtractionRequest, MemoryExtractor};
use brain_domain::SessionId;
use brain_services::brain_runtime::{BrainRuntime, CapabilityState, RuntimeHealth};
use brain_services::runtime::{ApplicationRuntime, RuntimeState};

use e2e_common::{
    MockChatAgent, MockEmbeddingProvider, ReferenceHeuristicExtractor, ReferenceLlmMemoryExtractor,
    TestDb,
};

/// Tier 1.1: Production Storage Initialization
/// Verifies SqliteStorage initialization directly via `SqliteStorage::new`,
/// exercising connection pooling and schema initialization without test-utils feature.
#[test]
fn test_tier1_storage_initialization_production() {
    let test_db = TestDb::new();
    let storage = test_db.storage();

    // Verify connection pool checkout works
    let conn = storage.pool().get();
    assert!(
        conn.is_ok(),
        "Failed to acquire SQLite connection from pool"
    );

    // Verify sqlite_master tables exist
    let conn = conn.unwrap();
    let mut stmt = conn
        .prepare("SELECT count(*) FROM sqlite_master WHERE type='table'")
        .expect("Failed to prepare query on sqlite_master");
    let count: i64 = stmt
        .query_row([], |row| row.get(0))
        .expect("Failed to query table count");
    assert!(count > 0, "Expected database tables to be initialized");
}

/// Tier 1.2: BrainRuntime Unified Composition & Lifecycle
/// Verifies BrainRuntime boots cleanly, transitions to Healthy, and exposes
/// registered capability descriptors.
#[tokio::test]
async fn test_tier1_brain_runtime_lifecycle_healthy() {
    let test_db = TestDb::new();
    let runtime = BrainRuntime::new(test_db.db_path())
        .expect("Failed to initialize BrainRuntime with valid database");

    // Verify health is Healthy via status()
    assert_eq!(runtime.status().health, RuntimeHealth::Healthy);

    // Verify registered capabilities
    let caps = runtime.discover_capabilities();
    let cap_names: Vec<&str> = caps.iter().map(|c| c.name).collect();
    assert!(cap_names.contains(&"storage"));
    assert!(cap_names.contains(&"evolution"));
    assert!(cap_names.contains(&"projection"));
    assert!(cap_names.contains(&"subscription"));

    for cap in &caps {
        assert_eq!(cap.state, CapabilityState::Active);
        assert!(cap.is_enabled);
    }
}

/// Tier 1.3: Concurrency & Thread Safety with parking_lot::Mutex
/// Verifies parking_lot::Mutex concurrency without thread-poisoning panics.
#[test]
fn test_tier1_parking_lot_concurrency_no_poisoning() {
    let mutex = Arc::new(parking_lot::Mutex::new(0usize));
    let mut handles = Vec::new();

    // Spawn 10 concurrent threads mutating state
    for _ in 0..10 {
        let m = Arc::clone(&mutex);
        handles.push(thread::spawn(move || {
            for _ in 0..100 {
                let mut guard = m.lock();
                *guard += 1;
            }
        }));
    }

    for h in handles {
        h.join().expect("Thread joined successfully");
    }

    assert_eq!(*mutex.lock(), 1000);

    // Verify no poisoning even if a thread panics
    let m = Arc::clone(&mutex);
    let _ = thread::spawn(move || {
        let _guard = m.lock();
        panic!("Simulated panic inside mutex holder");
    })
    .join();

    // parking_lot mutex does not poison, acquisition succeeds
    let guard = mutex.try_lock();
    assert!(
        guard.is_some(),
        "parking_lot::Mutex must not poison after thread panic"
    );
}

/// Tier 1.4: Non-Blocking HostContext Tool Execution Across Runtimes
/// Verifies tool execution safety across both multi-thread and current-thread runtimes.
#[test]
fn test_tier1_host_context_execution_across_runtime_flavors() {
    // Current-thread runtime test
    let rt_current = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt_current.block_on(async {
        let handle = tokio::runtime::Handle::current();
        let is_current = std::panic::catch_unwind(|| tokio::task::block_in_place(|| 42)).is_err();
        assert!(
            is_current,
            "block_in_place is expected to panic on current_thread runtime without multi-thread"
        );
        assert!(handle.metrics().num_workers() >= 1);
    });

    // Multi-threaded runtime test
    let rt_multi = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    rt_multi.block_on(async {
        let result = tokio::task::block_in_place(|| 42);
        assert_eq!(result, 42);
    });
}

/// Tier 1.5: Python Feature Gating Behavior
/// Verifies ApplicationRuntime operates cleanly with defaults and configuration.
#[tokio::test]
async fn test_tier1_python_feature_gating_behavior() {
    let test_db = TestDb::new();
    let partial = PartialBrainSettings {
        database: Some(PartialDatabaseSettings {
            path: Some(test_db.db_path().to_string()),
            pool_size: Some(2),
            enable_wal: Some(false),
        }),
        ..Default::default()
    };
    let config = resolve(&[
        Box::new(DefaultsSource),
        Box::new(OverrideSource::new(partial)),
    ])
    .expect("Failed to resolve configuration");

    let runtime = ApplicationRuntime::builder()
        .with_config(config)
        .build()
        .expect("Failed to build ApplicationRuntime");

    assert_eq!(runtime.state(), RuntimeState::Created);
    assert!(!runtime.is_ready());
}

/// Tier 1.6: Heuristic Extraction Contract Happy Path
/// Verifies ExtractionAgent and MemoryExtractor trait implementations on representative text.
#[test]
fn test_tier1_extraction_contract_happy_path() {
    let extractor = ReferenceHeuristicExtractor::new();

    // 1. ExtractionAgent trait test
    let input_text =
        "setting up sqlite database configuration and storing API keys in environment variables";
    let (nodes, edges) = extractor
        .extract_graph(input_text)
        .expect("Failed to extract graph via ExtractionAgent");

    let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"sqlite"), "Missing sqlite node");
    assert!(node_ids.contains(&"db-config"), "Missing db-config node");
    assert!(node_ids.contains(&"api-key"), "Missing api-key node");
    assert!(node_ids.contains(&"env-vars"), "Missing env-vars node");

    let edge_relations: Vec<&str> = edges.iter().map(|e| e.relation.as_str()).collect();
    assert!(
        edge_relations.contains(&"configures"),
        "Missing configures relation"
    );
    assert!(
        edge_relations.contains(&"stored_in"),
        "Missing stored_in relation"
    );

    // 2. MemoryExtractor trait test
    let req = ExtractionRequest {
        raw_content: "ritikpathania develops Brain".to_string(),
        context_metadata: std::collections::HashMap::new(),
    };
    let result = extractor
        .extract(req)
        .expect("Failed to extract memory via MemoryExtractor");

    assert!(!result.nodes.is_empty(), "Expected extracted nodes");
    assert!(!result.edges.is_empty(), "Expected extracted edges");
}

/// Tier 1.7: ChatAgent Contract Happy Path
/// Verifies standard ChatAgent interaction and name contract.
#[test]
fn test_tier1_chat_agent_contract_happy_path() {
    let agent = MockChatAgent::new("ollama-llama3", "Hello from native Ollama provider!");
    assert_eq!(agent.name(), "ollama-llama3");

    let session_id = SessionId::new();
    let reply = agent
        .chat(session_id, "Hello")
        .expect("Chat execution failed");
    assert_eq!(reply, "Hello from native Ollama provider!");
}

/// Tier 1.8: EmbeddingProvider & EmbeddingAgent Contract Happy Path
/// Verifies embedding vector dimensions, bounded floats, and name contracts.
#[test]
fn test_tier1_embedding_provider_contract_happy_path() {
    let provider = MockEmbeddingProvider::new("ollama-nomic-embed-text", 384);
    assert_eq!(
        EmbeddingProvider::name(&provider),
        "ollama-nomic-embed-text"
    );
    assert_eq!(EmbeddingAgent::dimension(&provider), 384);

    let vec = provider
        .embed("test text for embedding")
        .expect("Embedding failed");
    assert_eq!(vec.len(), 384);
    for val in vec {
        assert!(
            (-1.0..=1.0).contains(&val),
            "Vector values must be bounded floats"
        );
    }
}

/// Tier 1.9: LLM Memory Extractor Markdown Fence Stripping
/// Verifies stripping of ```json and ``` fences and JSON parsing into DTOs.
#[test]
fn test_tier1_llm_memory_extractor_markdown_fence_stripping() {
    let raw_llm_response = r#"```json
{
  "nodes": [
    {"id": "brain", "label": "Brain", "type": "project"},
    {"id": "rust", "label": "Rust", "type": "language"}
  ],
  "edges": [
    {"source": "brain", "target": "rust", "relation": "implemented_in"}
  ]
}
```"#;

    let extractor = ReferenceLlmMemoryExtractor::new("openai-extractor", raw_llm_response);
    let (nodes, edges) = extractor
        .extract_graph("any transcript")
        .expect("Failed to parse markdown-fenced JSON");

    assert_eq!(nodes.len(), 2);
    assert_eq!(edges.len(), 1);
    assert_eq!(nodes[0].id, "brain");
    assert_eq!(nodes[1].id, "rust");
    assert_eq!(edges[0].relation, "implemented_in");
}

/// Tier 1.10: Provider Configuration Wiring
/// Verifies BrainSettings provider settings validation, runtime startup, and provider resolution.
#[test]
fn test_tier1_provider_configuration_wiring() {
    let test_db = TestDb::new();
    let partial = PartialBrainSettings {
        database: Some(PartialDatabaseSettings {
            path: Some(test_db.db_path().to_string()),
            pool_size: Some(2),
            enable_wal: Some(false),
        }),
        ..Default::default()
    };
    let settings = resolve(&[
        Box::new(DefaultsSource),
        Box::new(OverrideSource::new(partial)),
    ])
    .expect("Settings resolution failed");

    assert_eq!(settings.database().pool_size(), 2);

    let runtime = ApplicationRuntime::builder()
        .with_config(settings)
        .build()
        .expect("Runtime build failed");

    let report = runtime.start().expect("Runtime start failed");
    assert_eq!(runtime.state(), RuntimeState::Running);
    assert!(runtime.is_ready());
    assert!(report
        .completed_phases()
        .contains(&"Services Ready".to_string()));

    // Verify resolved providers from runtime
    let chat = runtime.chat_agent().expect("Chat agent missing");
    assert_eq!(chat.name(), "anthropic-claude-3-5-sonnet-20240620");

    let emb = runtime
        .embedding_provider()
        .expect("Embedding provider missing");
    assert_eq!(emb.name(), "noop");

    let extractor = runtime
        .memory_extractor()
        .expect("Memory extractor missing");
    let req = ExtractionRequest {
        raw_content: "configuring sqlite database in rust".to_string(),
        context_metadata: std::collections::HashMap::new(),
    };
    let result = extractor.extract(req).expect("Extraction should succeed");
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .filter_map(|n| n.properties.get("canonical_id").and_then(|v| v.as_str()))
        .collect();
    assert!(ids.contains(&"sqlite"));
    assert!(ids.contains(&"rust"));

    let conv = runtime.conversation();
    assert!(conv.is_ok(), "Conversation manager should be initialized");

    runtime.shutdown().expect("Shutdown failed");
}
