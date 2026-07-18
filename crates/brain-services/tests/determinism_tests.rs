use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{CacheHydrationPolicy, RetrievalRequest};
use brain_domain::{Edge, Node, NodeId, NodeType, RelationKind, SessionId};
use brain_services::retrieval::pipeline::MemoryPipelineBuilder;
use brain_services::retrieval::source::{LtmMemorySource, StmMemorySource};
use brain_session::SessionCacheManager;
use brain_storage::TestStorage;

#[test]
fn test_query_and_sorting_determinism() {
    use std::str::FromStr;

    let run_query = || {
        let test_storage = TestStorage::new();
        let store = test_storage.store();
        let cache_manager = Arc::new(SessionCacheManager::new());

        let session_id = SessionId::from_str("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();

        // Set up database with fixed IDs to ensure determinism
        let brain_id = NodeId::from_str("00000000-0000-0000-0000-000000000002").unwrap();
        let sqlite_id = NodeId::from_str("00000000-0000-0000-0000-000000000003").unwrap();
        let duckdb_id = NodeId::from_str("00000000-0000-0000-0000-000000000004").unwrap();

        let brain = Node::new(brain_id, "Brain".to_string(), NodeType::Project);
        let sqlite = Node::new(sqlite_id, "SQLite".to_string(), NodeType::Database);
        let duckdb = Node::new(duckdb_id, "DuckDB".to_string(), NodeType::Database);

        store.nodes().save(&brain).unwrap();
        store.nodes().save(&sqlite).unwrap();
        store.nodes().save(&duckdb).unwrap();

        store
            .edges()
            .save(&Edge::new(brain_id, sqlite_id, RelationKind::Uses, 1.0))
            .unwrap();
        store
            .edges()
            .save(&Edge::new(brain_id, duckdb_id, RelationKind::Uses, 1.0))
            .unwrap();

        let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
        let pipeline = MemoryPipelineBuilder::new()
            .register_source(Arc::new(StmMemorySource::new(
                cache_manager.clone(),
                store.clone(),
                registry.clone(),
            )))
            .register_source(Arc::new(LtmMemorySource::new(store.clone(), registry)))
            .with_policy(CacheHydrationPolicy::OnHit)
            .with_cache_manager(cache_manager)
            .build();

        let req = RetrievalRequest {
            session_id,
            query: "Brain".to_string(),
            limit: 10,
            exclude_ids: HashSet::new(),
            deadline: None,
        };

        pipeline.execute(&req).unwrap()
    };

    // Execute on identical starting states
    let res1 = run_query();
    let res2 = run_query();

    assert_eq!(res1.nodes.len(), res2.nodes.len());
    for (n1, n2) in res1.nodes.iter().zip(res2.nodes.iter()) {
        assert_eq!(n1.id, n2.id);
        assert_eq!(n1.label, n2.label);
        assert_eq!(n1.node_type, n2.node_type);
    }
}

#[test]
fn test_property_merging_determinism() {
    let test_storage = TestStorage::new();
    let store = test_storage.store();

    let node_id = NodeId::new();

    // Insert 1: Base properties
    let mut props1 = HashMap::new();
    props1.insert("version".to_string(), serde_json::json!(1.0));
    props1.insert(
        "desc".to_string(),
        serde_json::json!("original description"),
    );
    let node1 = Node::new(node_id, "Test Node".to_string(), NodeType::Concept)
        .with_properties(props1)
        .with_updated_at(100);

    store.nodes().save(&node1).unwrap();

    // Insert 2: Conflict update
    let mut props2 = HashMap::new();
    props2.insert("version".to_string(), serde_json::json!(2.0));
    props2.insert("author".to_string(), serde_json::json!("developer"));
    let node2 = Node::new(node_id, "Test Node Updated".to_string(), NodeType::Concept)
        .with_properties(props2)
        .with_updated_at(200);

    store.nodes().save(&node2).unwrap();

    // Ingest duplicate node2 again (idempotent write verification)
    store.nodes().save(&node2).unwrap();

    let fetched = store.nodes().find_by_id(&node_id).unwrap().unwrap();

    // Verify deterministic merge output
    assert_eq!(
        fetched.properties.get("version").unwrap(),
        &serde_json::json!(2.0)
    );
    assert_eq!(
        fetched.properties.get("desc").unwrap(),
        &serde_json::json!("original description")
    );
    assert_eq!(
        fetched.properties.get("author").unwrap(),
        &serde_json::json!("developer")
    );
    assert_eq!(fetched.label, "Test Node Updated");
}

#[test]
fn test_graph_traversal_determinism() {
    let test_storage = TestStorage::new();
    let store = test_storage.store();

    let n1 = NodeId::new();
    let n2 = NodeId::new();
    let n3 = NodeId::new();

    store
        .nodes()
        .save(&Node::new(n1, "N1".to_string(), NodeType::Concept))
        .unwrap();
    store
        .nodes()
        .save(&Node::new(n2, "N2".to_string(), NodeType::Concept))
        .unwrap();
    store
        .nodes()
        .save(&Node::new(n3, "N3".to_string(), NodeType::Concept))
        .unwrap();

    // Add edges in specific order
    store
        .edges()
        .save(&Edge::new(n1, n2, RelationKind::Uses, 0.5))
        .unwrap();
    store
        .edges()
        .save(&Edge::new(n2, n3, RelationKind::Uses, 0.8))
        .unwrap();

    let traversal_service = brain_services::retrieval::graph_service::Graph;
    let budget = brain_services::retrieval::graph_service::TraversalBudget {
        max_depth: 2,
        max_nodes: 50,
        max_edges: 100,
        prevent_cycles: true,
        deadline: None,
        respect_directionality: false,
    };

    let registry = brain_domain::RelationRegistry::default_embedded();
    let edges1 = traversal_service
        .expand_neighbors(store.as_ref(), &registry, &[n1], &budget)
        .unwrap();
    let edges2 = traversal_service
        .expand_neighbors(store.as_ref(), &registry, &[n1], &budget)
        .unwrap();

    assert_eq!(edges1.len(), edges2.len());
    for (e1, e2) in edges1.iter().zip(edges2.iter()) {
        assert_eq!(e1.source, e2.source);
        assert_eq!(e1.target, e2.target);
        assert_eq!(e1.relation, e2.relation);
        assert_eq!(e1.weight, e2.weight);
    }
}
