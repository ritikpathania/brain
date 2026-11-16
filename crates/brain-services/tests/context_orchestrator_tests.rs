use brain_core::context::{
    ContextAssembler, ContextAssemblyRequest, ContextOrchestrator, MemoryRanker, MemoryScope,
    RetrievalCandidate, RetrievalChannel,
};
use brain_domain::{NodeId, SessionId};
use brain_services::context::{
    DefaultContextAssembler, DefaultContextOrchestrator, DefaultMemoryRanker,
};
use std::collections::HashMap;

#[tokio::test]
async fn test_ranker_deterministic_tie_breaking() {
    let ranker = DefaultMemoryRanker::new();
    let ref_time = 1_700_000_000;

    let node1 = NodeId::new();
    let node2 = NodeId::new();

    // Ensure node1 < node2 string comparison
    let (n_small, n_large) = if node1.to_string() < node2.to_string() {
        (node1, node2)
    } else {
        (node2, node1)
    };

    let candidates = vec![
        RetrievalCandidate {
            node_id: n_large,
            label: "Large".to_string(),
            content: "Content Large".to_string(),
            channel: RetrievalChannel::Lexical,
            raw_score: 0.8,
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: n_small,
            label: "Small".to_string(),
            content: "Content Small".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.8,
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);
    assert_eq!(ranked.len(), 2);
    assert_eq!(
        ranked[0].node_id, n_small,
        "Deterministic tie-breaking by node_id ASC failed"
    );
    assert_eq!(ranked[1].node_id, n_large);
}

#[tokio::test]
async fn test_ranker_channel_deduplication_and_fusion() {
    let ranker = DefaultMemoryRanker::new();
    let ref_time = 1_700_000_000;
    let node_id = NodeId::new();

    let candidates = vec![
        RetrievalCandidate {
            node_id,
            label: "Duplicate Node".to_string(),
            content: "Content".to_string(),
            channel: RetrievalChannel::Lexical,
            raw_score: 0.7,
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id,
            label: "Duplicate Node".to_string(),
            content: "Content".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.8,
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);
    assert_eq!(
        ranked.len(),
        1,
        "Duplicate node across channels was not fused"
    );
    assert_eq!(ranked[0].channel, RetrievalChannel::Hybrid);
    assert!(
        ranked[0].final_score > 0.8,
        "Hybrid fusion bonus was not applied"
    );
}

#[tokio::test]
async fn test_assembler_budget_clamping_and_whole_item_preservation() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    let mut candidates = Vec::new();
    for i in 0..10 {
        candidates.push(RetrievalCandidate {
            node_id: NodeId::new(),
            label: format!("Node {}", i),
            content: "A".repeat(100), // ~25 tokens each
            channel: RetrievalChannel::Lexical,
            raw_score: 0.9 - (i as f64 * 0.05),
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        });
    }

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);

    // Limit budget to 120 tokens (~480 chars)
    let snapshot = assembler.assemble(&ranked, 120, 10, "test-epoch");

    assert!(
        snapshot.token_count <= 120,
        "Token count {} exceeded budget of 120",
        snapshot.token_count
    );
    assert!(!snapshot.items.is_empty());
    assert!(
        snapshot.items.len() < 10,
        "Did not clamp item count to budget"
    );
    assert!(snapshot
        .serialized_context
        .contains("</brain_memory_context>"));
}

#[tokio::test]
async fn test_orchestrator_workspace_isolation() {
    let orchestrator = DefaultContextOrchestrator::new();
    let _sess_a = SessionId::new();
    let sess_b = SessionId::new();

    let node_ws1 = NodeId::new();
    let node_ws2 = NodeId::new();

    orchestrator
        .register_candidate(RetrievalCandidate {
            node_id: node_ws1,
            label: "Secret WS1 Doc".to_string(),
            content: "Confidential Project Alpha".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.95,
            timestamp: 1_700_000_000,
            scope: MemoryScope::Workspace("workspace-alpha".to_string()),
            metadata: HashMap::new(),
        })
        .await;

    orchestrator
        .register_candidate(RetrievalCandidate {
            node_id: node_ws2,
            label: "Public WS2 Doc".to_string(),
            content: "Project Beta Details".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: 1_700_000_000,
            scope: MemoryScope::Workspace("workspace-beta".to_string()),
            metadata: HashMap::new(),
        })
        .await;

    // Request scoped strictly to workspace-beta
    let req_beta = ContextAssemblyRequest {
        prompt: "Project".to_string(),
        session_id: sess_b,
        workspace_id: Some("workspace-beta".to_string()),
        max_tokens: 500,
        max_items: 5,
        reference_time: Some(1_700_000_000),
    };

    let snapshot_beta = orchestrator.assemble_context(req_beta).await.unwrap();
    assert_eq!(snapshot_beta.items.len(), 1);
    assert_eq!(snapshot_beta.items[0].node_id, node_ws2);
    assert!(
        !snapshot_beta.serialized_context.contains("Alpha"),
        "Workspace A leaked into Workspace B"
    );
}

#[tokio::test]
async fn test_orchestrator_deleted_entity_exclusion() {
    let orchestrator = DefaultContextOrchestrator::new();
    let sess_id = SessionId::new();
    let node_id = NodeId::new();

    orchestrator
        .register_candidate(RetrievalCandidate {
            node_id,
            label: "Deleted Note".to_string(),
            content: "Should not appear anywhere".to_string(),
            channel: RetrievalChannel::Lexical,
            raw_score: 0.99,
            timestamp: 1_700_000_000,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        })
        .await;

    // Mark as deleted
    orchestrator.mark_node_deleted(node_id).await;

    let req = ContextAssemblyRequest {
        prompt: "Deleted Note".to_string(),
        session_id: sess_id,
        workspace_id: None,
        max_tokens: 500,
        max_items: 5,
        reference_time: Some(1_700_000_000),
    };

    let snapshot = orchestrator.assemble_context(req.clone()).await.unwrap();
    assert_eq!(
        snapshot.items.len(),
        0,
        "Deleted entity appeared in assembled context"
    );

    // Rollback tombstone (unmark)
    orchestrator.unmark_node_deleted(node_id).await;
    let snapshot_restored = orchestrator.assemble_context(req).await.unwrap();
    assert_eq!(
        snapshot_restored.items.len(),
        1,
        "Unmarked entity was not restored to eligibility"
    );
    assert_eq!(snapshot_restored.items[0].node_id, node_id);
}

#[tokio::test]
async fn test_orchestrator_prioritizes_only_visible_context_constraints() {
    use brain_core::repositories::NodeRepository;
    use brain_domain::{Node, NodeKind};
    use brain_storage::SqliteStorage;
    use std::sync::Arc;

    let storage = Arc::new(SqliteStorage::new(":memory:", 1, false).unwrap());
    let reference_time = 1_700_000_000;
    let session_id = SessionId::new();
    let other_session = format!("session:{}", SessionId::new());
    let save = |label: &str, flag: &str, value: serde_json::Value, scope: &str, updated_at| {
        let id = NodeId::new();
        let node = Node::new(id, label.to_string(), NodeKind::Concept)
            .with_properties(HashMap::from([
                (flag.to_string(), value),
                ("scope".to_string(), serde_json::json!(scope)),
            ]))
            .with_updated_at(updated_at);
        NodeRepository::save(storage.as_ref(), &node).unwrap();
        id
    };
    let visible = save("Gluten restriction", "pinned", true.into(), "global", 1);
    save(
        "Private account",
        "pinned",
        true.into(),
        &other_session,
        reference_time,
    );
    save(
        "Workspace account",
        "pinned",
        true.into(),
        "workspace:private",
        reference_time,
    );
    save(
        "String flag",
        "pinned",
        "true".into(),
        "global",
        reference_time,
    );
    save(
        "Compiler options",
        "pinned",
        false.into(),
        "global",
        reference_time,
    );
    let deleted = save(
        "Timezone restriction",
        "constraint_subgraph",
        true.into(),
        "global",
        reference_time,
    );

    let orchestrator = DefaultContextOrchestrator::with_storage(storage.clone(), storage);
    orchestrator.mark_node_deleted(deleted).await;
    let request = ContextAssemblyRequest {
        prompt: "Compiler options".to_string(),
        session_id,
        workspace_id: None,
        max_tokens: 500,
        max_items: 1,
        reference_time: Some(reference_time),
    };
    let snapshot = orchestrator.assemble_context(request).await.unwrap();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].node_id, visible);
}

#[tokio::test]
async fn test_orchestrator_recalls_semantic_matches_within_scope_before_cutoff() {
    use brain_core::errors::BrainError;
    use brain_core::repositories::{EmbeddingRepository, NodeRepository};
    use brain_core::retrieval::QueryEmbeddingService;
    use brain_domain::{Embedding, Node, NodeKind};
    use brain_services::retrieval::source::SemanticMemorySource;
    use brain_storage::SqliteStorage;
    use std::sync::Arc;

    struct TestEmbeddings;
    impl QueryEmbeddingService for TestEmbeddings {
        fn embed_query(&self, _query: &str) -> Result<Vec<f32>, BrainError> {
            Ok(vec![1.0, 0.0])
        }
    }

    let storage = Arc::new(SqliteStorage::new(":memory:", 1, false).unwrap());
    let visible = NodeId::new();
    let reference_time = 1_700_000_000;
    for (id, scope, vector) in [
        (NodeId::new(), "workspace:private", vec![1.0, 0.0]),
        (NodeId::new(), "workspace:private", vec![1.0, 0.0]),
        (visible, "global", vec![0.75, 0.25]),
    ] {
        let node = Node::new(id, "Cargo manifests".to_string(), NodeKind::Concept)
            .with_properties(HashMap::from([("scope".to_string(), scope.into())]))
            .with_updated_at(reference_time);
        NodeRepository::save(storage.as_ref(), &node).unwrap();
        EmbeddingRepository::save(storage.as_ref(), &Embedding::new(id, vector)).unwrap();
    }
    let semantic_source = Arc::new(SemanticMemorySource::new(
        storage.clone(),
        Arc::new(TestEmbeddings),
    ));
    let orchestrator = DefaultContextOrchestrator::with_storage(storage.clone(), storage)
        .with_semantic_source(semantic_source);
    let snapshot = orchestrator
        .assemble_context(ContextAssemblyRequest {
            prompt: "dependency declarations".to_string(),
            session_id: SessionId::new(),
            workspace_id: None,
            max_tokens: 500,
            max_items: 1,
            reference_time: Some(reference_time),
        })
        .await
        .unwrap();

    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].node_id, visible);
    assert_eq!(snapshot.items[0].channel, RetrievalChannel::Semantic);
}

#[tokio::test]
async fn test_assembler_skips_oversized_constraint_without_spending_an_item_slot() {
    let fit = NodeId::new();
    let large = NodeId::new();
    let time = 1_700_000_000;
    let make_candidate = |node_id, content, metadata| RetrievalCandidate {
        node_id,
        label: "Context".into(),
        content,
        channel: RetrievalChannel::Graph,
        raw_score: 1.0,
        timestamp: time,
        scope: MemoryScope::Global,
        metadata,
    };
    let ranked = DefaultMemoryRanker::new().rank_and_deduplicate(
        vec![
            make_candidate(
                large,
                "Oversized ".repeat(1_000),
                HashMap::from([("always_on_constraint".into(), "true".into())]),
            ),
            make_candidate(fit, "Small context".into(), HashMap::new()),
        ],
        time,
    );
    assert_eq!(ranked[0].node_id, large);

    let snapshot = DefaultContextAssembler::new().assemble(&ranked, 125, 1, "e1");
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].node_id, fit);
    assert!(snapshot.token_count <= 125);
}

#[tokio::test]
async fn test_orchestrator_can_use_lexical_results_after_semantic_error() {
    use brain_core::errors::BrainError;
    use brain_core::repositories::{EmbeddingRepository, NodeRepository};
    use brain_core::retrieval::QueryEmbeddingService;
    use brain_domain::{Embedding, Node, NodeKind};
    use brain_services::retrieval::source::SemanticMemorySource;
    use brain_storage::SqliteStorage;
    use std::sync::Arc;

    struct UnavailableEmbeddings;
    impl QueryEmbeddingService for UnavailableEmbeddings {
        fn embed_query(&self, _query: &str) -> Result<Vec<f32>, BrainError> {
            Err(BrainError::Internal {
                message: "provider unavailable".into(),
            })
        }
    }

    let storage = Arc::new(SqliteStorage::new(":memory:", 1, false).unwrap());
    let visible = NodeId::new();
    let reference_time = 1_700_000_000;
    for (id, scope) in [
        (visible, serde_json::json!("old-label")),
        (NodeId::new(), serde_json::json!("session:invalid")),
        (NodeId::new(), serde_json::json!(true)),
    ] {
        let node = Node::new(id, "Compiler inputs".into(), NodeKind::Concept)
            .with_properties(HashMap::from([("scope".into(), scope)]))
            .with_updated_at(reference_time);
        NodeRepository::save(storage.as_ref(), &node).unwrap();
        EmbeddingRepository::save(storage.as_ref(), &Embedding::new(id, vec![1.0, 0.0])).unwrap();
    }
    let source = Arc::new(SemanticMemorySource::new(
        storage.clone(),
        Arc::new(UnavailableEmbeddings),
    ));
    let orchestrator = DefaultContextOrchestrator::with_storage(storage.clone(), storage)
        .with_semantic_source(source);
    let request = ContextAssemblyRequest {
        prompt: "Compiler".into(),
        session_id: SessionId::new(),
        workspace_id: None,
        max_tokens: 500,
        max_items: 3,
        reference_time: Some(reference_time),
    };
    let snapshot = orchestrator
        .assemble_context(request.clone())
        .await
        .unwrap();
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(snapshot.items[0].node_id, visible);
    assert_eq!(snapshot.items[0].channel, RetrievalChannel::Lexical);

    let failure = orchestrator
        .assemble_context(ContextAssemblyRequest {
            prompt: "nonexistent".into(),
            ..request
        })
        .await
        .unwrap_err();
    assert!(failure.to_string().contains("provider unavailable"));
}

#[tokio::test]
async fn test_orchestrator_storage_backed_retrieval_and_neighbor_expansion() {
    use brain_core::repositories::{EdgeRepository, NodeRepository};
    use brain_domain::{Edge, Node, NodeKind, RelationKind};
    use brain_storage::SqliteStorage;
    use std::sync::Arc;

    let storage = Arc::new(SqliteStorage::new(":memory:", 1, false).unwrap());

    // 1. Insert parent node (matching FTS query)
    let parent_id = NodeId::new();
    let mut parent_props = std::collections::HashMap::new();
    parent_props.insert(
        "content".to_string(),
        serde_json::json!("The rustc compiler uses query-based demand evaluation."),
    );
    let parent_node = Node::new(
        parent_id,
        "Rust Compiler Architecture".to_string(),
        NodeKind::Concept,
    )
    .with_properties(parent_props)
    .with_updated_at(1_700_000_000);
    NodeRepository::save(&*storage, &parent_node).unwrap();

    // 2. Insert neighbor node (connected by edge)
    let neighbor_id = NodeId::new();
    let mut neighbor_props = std::collections::HashMap::new();
    neighbor_props.insert(
        "content".to_string(),
        serde_json::json!("LLVM translates intermediate representation to target assembly."),
    );
    let neighbor_node = Node::new(
        neighbor_id,
        "LLVM Backend".to_string(),
        NodeKind::Concept,
    )
    .with_properties(neighbor_props)
    .with_updated_at(1_700_000_000);
    NodeRepository::save(&*storage, &neighbor_node).unwrap();

    // 3. Insert edge connecting parent to neighbor
    let edge = Edge::new(
        parent_id,
        neighbor_id,
        RelationKind::DependsOn,
        0.9,
    );
    EdgeRepository::save(&*storage, &edge).unwrap();

    // 4. Insert unrelated node
    let unrelated_id = NodeId::new();
    let mut unrelated_props = std::collections::HashMap::new();
    unrelated_props.insert(
        "content".to_string(),
        serde_json::json!("Sourdough fermentation requires wild yeast cultures."),
    );
    let unrelated_node = Node::new(
        unrelated_id,
        "Baking Science".to_string(),
        NodeKind::Concept,
    )
    .with_properties(unrelated_props)
    .with_updated_at(1_700_000_000);
    NodeRepository::save(&*storage, &unrelated_node).unwrap();

    // Create storage-backed orchestrator
    let orchestrator = DefaultContextOrchestrator::with_storage(
        Arc::clone(&storage) as Arc<dyn NodeRepository>,
        Arc::clone(&storage) as Arc<dyn EdgeRepository>,
    );

    // 5. Query matching parent topic
    let req = ContextAssemblyRequest {
        prompt: "rustc compiler architecture evaluation".to_string(),
        session_id: SessionId::new(),
        workspace_id: None,
        max_tokens: 2048,
        max_items: 10,
        reference_time: Some(1_700_000_000),
    };

    let snapshot = orchestrator.assemble_context(req).await.unwrap();

    // Verify parent and neighbor are retrieved, while unrelated node is excluded
    assert_eq!(snapshot.items.len(), 2);
    let item_ids: Vec<NodeId> = snapshot.items.iter().map(|it| it.node_id).collect();
    assert!(item_ids.contains(&parent_id));
    assert!(item_ids.contains(&neighbor_id));
    assert!(!item_ids.contains(&unrelated_id));

    // Verify parent has higher score than 1-hop decayed neighbor
    let parent_item = snapshot.items.iter().find(|it| it.node_id == parent_id).unwrap();
    let neighbor_item = snapshot.items.iter().find(|it| it.node_id == neighbor_id).unwrap();
    assert!(parent_item.score > neighbor_item.score);

    // Verify formatted serialized context framing
    assert!(snapshot.serialized_context.contains("<brain_memory_context"));
    assert!(snapshot.serialized_context.contains("Rust Compiler Architecture"));
    assert!(snapshot.serialized_context.contains("LLVM Backend"));
    assert!(!snapshot.serialized_context.contains("Sourdough"));

    // 6. Negative case: Query with zero matching terms yields empty context
    let negative_req = ContextAssemblyRequest {
        prompt: "astronomy galaxies telescope optical".to_string(),
        session_id: SessionId::new(),
        workspace_id: None,
        max_tokens: 2048,
        max_items: 10,
        reference_time: Some(1_700_000_000),
    };

    let negative_snapshot = orchestrator.assemble_context(negative_req).await.unwrap();
    assert_eq!(negative_snapshot.items.len(), 0);
    assert_eq!(negative_snapshot.serialized_context, "");
}
