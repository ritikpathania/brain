use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::span::{Attributes, Record};
use tracing::{Event, Id, Metadata};

use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{DefaultQueryEmbeddingService, EmbeddingProvider, RetrievalRequest};
use brain_domain::{Embedding, Node, NodeId, NodeType};
use brain_services::conversation::{ContextBudget, ContextBuilder, WordSpaceTokenCounter};
use brain_services::RetrievalServiceImpl;
use brain_session::SessionCacheManager;
use brain_storage::TestStorage;

// A custom thread-safe tracing subscriber to capture structured telemetry events.
struct TelemetryCollector {
    events: Mutex<Vec<(String, HashMap<String, String>)>>,
}

impl tracing::Subscriber for TelemetryCollector {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target() == "brain::telemetry::retrieval"
    }

    // Return Interest::always() for our target so that this subscriber, when active
    // as the thread-local default, registers matching callsites with a permissive
    // interest level instead of letting the global NoSubscriber cache them as
    // Interest::never().
    fn register_callsite(
        &self,
        metadata: &'static Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        if metadata.target() == "brain::telemetry::retrieval" {
            tracing::subscriber::Interest::always()
        } else {
            tracing::subscriber::Interest::never()
        }
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = HashMap::new();
        struct Visitor<'a>(&'a mut HashMap<String, String>);
        impl<'a> tracing::field::Visit for Visitor<'a> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0
                    .insert(field.name().to_string(), format!("{:?}", value));
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.0.insert(field.name().to_string(), value.to_string());
            }
        }
        event.record(&mut Visitor(&mut fields));
        let stage = fields.get("stage").cloned().unwrap_or_default();
        // Remove quotes around string values if present from record_debug format
        let clean_stage = stage.trim_matches('"').to_string();
        self.events.lock().unwrap().push((clean_stage, fields));
    }

    fn enter(&self, _span: &Id) {}
    fn exit(&self, _span: &Id) {}
}

/// Install a fresh `TelemetryCollector` as the thread-local tracing subscriber,
/// run `f`, then return the collector so callers can inspect captured events.
///
/// # Why rebuild the interest cache?
///
/// `tracing` caches each callsite's `Interest` globally the first time it is
/// reached.  If a callsite fires while the global default is `NoSubscriber`
/// (e.g. during another test that does not install a subscriber), it gets
/// permanently cached as `Interest::never()`.  The hot-path in the `tracing!`
/// macro then short-circuits *before* consulting the thread-local dispatcher,
/// so events are silently dropped even when our `TelemetryCollector` is active.
///
/// `rebuild_interest_cache` resets all cached interests and forces
/// re-registration using the currently active (thread-local) subscriber,
/// making telemetry capture deterministic regardless of test execution order.
fn with_test_collector<F, R>(f: F) -> (Arc<TelemetryCollector>, R)
where
    F: FnOnce() -> R,
{
    let collector = Arc::new(TelemetryCollector {
        events: Mutex::new(Vec::new()),
    });
    let result = tracing::subscriber::with_default(collector.clone(), || {
        tracing::callsite::rebuild_interest_cache();
        f()
    });
    (collector, result)
}

// A mock embedding provider that yields deterministic vectors
#[derive(Debug)]
struct MockQueryEmbeddingProvider {
    fail_on_query: Option<String>,
}

impl EmbeddingProvider for MockQueryEmbeddingProvider {
    fn name(&self) -> &'static str {
        "mock-validation-provider"
    }

    fn embed(&self, query: &str) -> Result<Vec<f32>, BrainError> {
        if let Some(ref fail) = self.fail_on_query {
            if query == fail {
                return Err(BrainError::Internal {
                    message: "Mock embedding failure occurred".to_string(),
                });
            }
        }

        // Return deterministic vector based on characters
        let mut vec = vec![0.0f32; 8];
        for (i, c) in query.chars().enumerate() {
            vec[i % 8] += c as u32 as f32;
        }
        // Normalize
        let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in vec.iter_mut() {
                *val /= norm;
            }
        }
        Ok(vec)
    }
}

#[test]
fn test_retrieval_determinism_validation() {
    let test_store = TestStorage::new();
    let store = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());

    let provider = Arc::new(MockQueryEmbeddingProvider {
        fail_on_query: None,
    });
    let query_embedding_service = Arc::new(DefaultQueryEmbeddingService::new(provider));
    let retrieval_service = RetrievalServiceImpl::new(
        store.clone(),
        cache_manager.clone(),
        registry.clone(),
        query_embedding_service,
    );

    // Seed database with a few nodes
    let node1 = Node::new(
        NodeId::new(),
        "Machine learning compiler optimization".to_string(),
        NodeType::Concept,
    );
    let node2 = Node::new(
        NodeId::new(),
        "Database storage engine layout".to_string(),
        NodeType::Concept,
    );
    let node3 = Node::new(
        NodeId::new(),
        "Lexical search vs vector recall".to_string(),
        NodeType::Concept,
    );
    store.nodes().save(&node1).unwrap();
    store.nodes().save(&node2).unwrap();
    store.nodes().save(&node3).unwrap();

    // Save corresponding mock embeddings
    store
        .embeddings()
        .save(&Embedding::new(
            node1.id,
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ))
        .unwrap();
    store
        .embeddings()
        .save(&Embedding::new(
            node2.id,
            vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ))
        .unwrap();
    store
        .embeddings()
        .save(&Embedding::new(
            node3.id,
            vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ))
        .unwrap();

    let session_id = brain_domain::SessionId::new();
    let request = RetrievalRequest {
        session_id,
        query: "compiler optimization".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let budget = ContextBudget {
        max_tokens: 4096,
        reserved_system_tokens: 512,
        reserved_completion_tokens: 512,
    };

    // Run query 1
    let response1 = retrieval_service.execute_pipeline(&request).unwrap();
    let mut memories1 = Vec::new();
    for node in response1.nodes.clone() {
        let connections = store.edges().get_connections(&node.id).unwrap();
        memories1.push(brain_services::mapper::to_memory_dto(&node, &connections).unwrap());
    }
    let context1 = ContextBuilder::build(&WordSpaceTokenCounter, budget, &[], None, memories1);

    // Run query 2
    let response2 = retrieval_service.execute_pipeline(&request).unwrap();
    let mut memories2 = Vec::new();
    for node in response2.nodes.clone() {
        let connections = store.edges().get_connections(&node.id).unwrap();
        memories2.push(brain_services::mapper::to_memory_dto(&node, &connections).unwrap());
    }
    let context2 = ContextBuilder::build(&WordSpaceTokenCounter, budget, &[], None, memories2);

    // Assert absolute identical results
    assert_eq!(response1.nodes.len(), response2.nodes.len());
    for (n1, n2) in response1.nodes.iter().zip(response2.nodes.iter()) {
        assert_eq!(n1.id, n2.id);
        assert_eq!(n1.label, n2.label);
    }
    assert_eq!(context1.messages().len(), context2.messages().len());
    assert_eq!(
        context1.retrieved_memories().len(),
        context2.retrieved_memories().len()
    );
    for (m1, m2) in context1
        .retrieved_memories()
        .iter()
        .zip(context2.retrieved_memories().iter())
    {
        assert_eq!(m1.node.id, m2.node.id);
    }
}

#[test]
fn test_structured_telemetry_emission_contract() {
    let test_store = TestStorage::new();
    let store = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());

    let provider = Arc::new(MockQueryEmbeddingProvider {
        fail_on_query: None,
    });
    let query_embedding_service = Arc::new(DefaultQueryEmbeddingService::new(provider));
    let retrieval_service = RetrievalServiceImpl::new(
        store.clone(),
        cache_manager.clone(),
        registry.clone(),
        query_embedding_service,
    );

    // Seed database with a matching node
    let node = Node::new(
        NodeId::new(),
        "telemetry validation query".to_string(),
        NodeType::Concept,
    );
    store.nodes().save(&node).unwrap();
    store
        .embeddings()
        .save(&Embedding::new(node.id, vec![1.0; 8]))
        .unwrap();

    let session_id = brain_domain::SessionId::new();
    let request = RetrievalRequest {
        session_id,
        query: "telemetry validation query".to_string(),
        limit: 5,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let (collector, _) = with_test_collector(|| {
        retrieval_service.execute_pipeline(&request).unwrap();
    });

    let events = collector.events.lock().unwrap();
    assert!(!events.is_empty(), "No telemetry events captured!");

    // Check that we captured the expected stages
    let stages: Vec<&str> = events.iter().map(|(stage, _)| stage.as_str()).collect();
    println!("Captured stages: {:?}", stages);
    assert!(
        stages.contains(&"embedding"),
        "Missing embedding stage telemetry"
    );
    assert!(
        stages.contains(&"candidate_counts"),
        "Missing candidate counts stage telemetry"
    );
    assert!(stages.contains(&"BM25"), "Missing BM25 stage telemetry");
    assert!(stages.contains(&"vector"), "Missing vector stage telemetry");
    assert!(
        stages.contains(&"RRF"),
        "Missing RRF fusion stage telemetry"
    );
    assert!(
        stages.contains(&"pipeline"),
        "Missing overall pipeline stage telemetry"
    );
}

#[test]
fn test_retrieval_timeout_graceful_handling() {
    let test_store = TestStorage::new();
    let store = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());

    let provider = Arc::new(MockQueryEmbeddingProvider {
        fail_on_query: None,
    });
    let query_embedding_service = Arc::new(DefaultQueryEmbeddingService::new(provider));
    let retrieval_service = RetrievalServiceImpl::new(
        store.clone(),
        cache_manager.clone(),
        registry.clone(),
        query_embedding_service,
    );

    let session_id = brain_domain::SessionId::new();
    // Use an expired deadline (already in the past) to trigger immediate timeout
    let deadline = std::time::Instant::now() - std::time::Duration::from_millis(50);

    let request = RetrievalRequest {
        session_id,
        query: "timeout check query".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: Some(deadline),
    };

    let result = retrieval_service.execute_pipeline(&request);
    assert!(result.is_err());
    match result.err().unwrap() {
        BrainError::Timeout { .. } => {}
        other => panic!("Expected BrainError::Timeout, got {:?}", other),
    }
}

#[test]
fn test_embedding_failure_propagation() {
    let test_store = TestStorage::new();
    let store = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());

    let provider = Arc::new(MockQueryEmbeddingProvider {
        fail_on_query: Some("unsupported search terms".to_string()),
    });
    let query_embedding_service = Arc::new(DefaultQueryEmbeddingService::new(provider));
    let retrieval_service = RetrievalServiceImpl::new(
        store.clone(),
        cache_manager.clone(),
        registry.clone(),
        query_embedding_service,
    );

    let session_id = brain_domain::SessionId::new();
    let request = RetrievalRequest {
        session_id,
        query: "unsupported search terms".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let (collector, result) = with_test_collector(|| retrieval_service.execute_pipeline(&request));

    assert!(result.is_err());

    let events = collector.events.lock().unwrap();
    let embedding_event = events.iter().find(|(stage, _)| stage == "embedding");
    assert!(embedding_event.is_some());
    let (_, fields) = embedding_event.unwrap();
    assert_eq!(fields.get("success").map(|s| s.as_str()), Some("false"));
}

#[test]
fn test_retrieval_concurrency_lock_safety() {
    let test_store = TestStorage::new();
    let store = Arc::new(test_store.storage().clone());
    let cache_manager = Arc::new(SessionCacheManager::new());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());

    let provider = Arc::new(MockQueryEmbeddingProvider {
        fail_on_query: None,
    });
    let query_embedding_service = Arc::new(DefaultQueryEmbeddingService::new(provider));
    let retrieval_service = Arc::new(RetrievalServiceImpl::new(
        store.clone(),
        cache_manager.clone(),
        registry.clone(),
        query_embedding_service,
    ));

    // Seed database
    let node = Node::new(
        NodeId::new(),
        "shared node content".to_string(),
        NodeType::Concept,
    );
    store.nodes().save(&node).unwrap();

    let mut handles = Vec::new();
    for i in 0..10 {
        let retrieval_service = retrieval_service.clone();
        let handle = std::thread::spawn(move || {
            let session_id = brain_domain::SessionId::new();
            let request = RetrievalRequest {
                session_id,
                query: format!("shared query {}", i),
                limit: 10,
                exclude_ids: std::collections::HashSet::new(),
                deadline: None,
            };
            let response = retrieval_service.execute_pipeline(&request).unwrap();
            assert!(response.nodes.len() <= 1);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
