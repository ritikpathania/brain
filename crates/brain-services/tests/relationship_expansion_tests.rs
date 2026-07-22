use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{DefaultQueryEmbeddingService, EmbeddingProvider, RetrievalRequest};
use brain_domain::{Edge, Node, NodeId, NodeType, RelationKind};
use brain_services::retrieval::relationship_expander::RelationshipExpander;
use brain_storage::TestStorage;
use std::collections::HashSet;
use std::sync::Arc;

struct CustomEmbeddingProvider {
    emb: Vec<f32>,
}
impl EmbeddingProvider for CustomEmbeddingProvider {
    fn name(&self) -> &'static str {
        "custom"
    }
    fn embed(&self, _text: &str) -> Result<Vec<f32>, brain_core::errors::BrainError> {
        Ok(self.emb.clone())
    }
}

fn node(id: NodeId, label: &str) -> Node {
    Node::new(id, label.to_string(), NodeType::Concept)
}

fn base_request(query: &str) -> RetrievalRequest {
    RetrievalRequest {
        session_id: brain_domain::SessionId::new(),
        query: query.to_string(),
        limit: 20,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
        reference_time: None,
    }
}

fn build_test_graph() -> (
    TestStorage,
    NodeId, // rust
    NodeId, // memory
    NodeId, // heap
) {
    let test_store = TestStorage::new();
    let store = test_store.store();

    let rust_id = NodeId::new();
    let memory_id = NodeId::new();
    let heap_id = NodeId::new();

    store.nodes().save(&node(rust_id, "rust")).unwrap();
    store.nodes().save(&node(memory_id, "memory")).unwrap();
    store.nodes().save(&node(heap_id, "heap")).unwrap();

    // rust -> memory (RelationKind::Uses)
    store
        .edges()
        .save(&Edge::new(rust_id, memory_id, RelationKind::Uses, 1.0))
        .unwrap();
    // memory -> heap (RelationKind::Uses)
    store
        .edges()
        .save(&Edge::new(memory_id, heap_id, RelationKind::Uses, 0.8))
        .unwrap();

    (test_store, rust_id, memory_id, heap_id)
}

#[test]
fn test_expander_empty_input() {
    let test_store = TestStorage::new();
    let expander = RelationshipExpander::new(test_store.store());

    let results = expander.expand(&[]).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_expander_correct_enrichment() {
    let (test_store, rust_id, memory_id, heap_id) = build_test_graph();
    let expander = RelationshipExpander::new(test_store.store());

    let nodes = vec![
        node(rust_id, "rust"),
        node(memory_id, "memory"),
        node(heap_id, "heap"),
    ];

    let results = expander.expand(&nodes).unwrap();
    assert_eq!(results.len(), 3);

    // 1. rust node: outgoing edge to memory, no incoming
    let rust_expansion = results
        .iter()
        .find(|r| r.node_id == rust_id.0.to_string())
        .unwrap();
    assert_eq!(rust_expansion.incoming.len(), 0);
    assert_eq!(rust_expansion.outgoing.len(), 1);
    assert_eq!(rust_expansion.outgoing[0].target, memory_id.0.to_string());
    assert_eq!(rust_expansion.outgoing[0].relation, "uses");
    assert_eq!(rust_expansion.outgoing[0].weight, 1.0);

    // 2. memory node: incoming from rust, outgoing to heap
    let memory_expansion = results
        .iter()
        .find(|r| r.node_id == memory_id.0.to_string())
        .unwrap();
    assert_eq!(memory_expansion.incoming.len(), 1);
    assert_eq!(memory_expansion.incoming[0].source, rust_id.0.to_string());
    assert_eq!(memory_expansion.outgoing.len(), 1);
    assert_eq!(memory_expansion.outgoing[0].target, heap_id.0.to_string());
    assert_eq!(memory_expansion.outgoing[0].weight, 0.8);

    // 3. heap node: incoming from memory, no outgoing
    let heap_expansion = results
        .iter()
        .find(|r| r.node_id == heap_id.0.to_string())
        .unwrap();
    assert_eq!(heap_expansion.incoming.len(), 1);
    assert_eq!(heap_expansion.incoming[0].source, memory_id.0.to_string());
    assert_eq!(heap_expansion.outgoing.len(), 0);
}

#[test]
fn test_retrieval_service_incorporates_relationships() {
    let (test_store, rust_id, memory_id, _heap_id) = build_test_graph();
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let provider = Arc::new(CustomEmbeddingProvider {
        emb: vec![0.0; 384],
    });
    let query_embedding_service = Arc::new(DefaultQueryEmbeddingService::new(provider));

    let service = brain_services::retrieval::RetrievalServiceImpl::new(
        test_store.store(),
        Arc::new(brain_session::SessionCacheManager::new()),
        registry,
        query_embedding_service,
    );

    // Default expand_relations is false
    let req_default = base_request("rust");
    let resp_default = service.execute_pipeline(&req_default).unwrap();
    assert!(resp_default.relationships.is_none());

    // Explicit expand_relations is true
    let req_expanded = RetrievalRequest {
        reference_time: None,
        expand_relations: true,
        ..base_request("rust")
    };
    let resp_expanded = service.execute_pipeline(&req_expanded).unwrap();
    let relationships = resp_expanded
        .relationships
        .expect("relationships should be populated");

    // The query "rust" retrieves "rust" and "memory"
    assert!(resp_expanded.nodes.iter().any(|n| n.id == rust_id));
    assert!(resp_expanded.nodes.iter().any(|n| n.id == memory_id));

    // Relationships should have expansions for the retrieved nodes
    assert_eq!(relationships.len(), resp_expanded.nodes.len());
    let rust_rel = relationships
        .iter()
        .find(|r| r.node_id == rust_id.0.to_string())
        .unwrap();
    assert_eq!(rust_rel.outgoing.len(), 1);
    assert_eq!(rust_rel.outgoing[0].target, memory_id.0.to_string());
}
