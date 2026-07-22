use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use brain_core::errors::BrainError;
use brain_core::retrieval::{
    CacheHydrationPolicy, MemorySource, MemorySourceResult, RankingStrategy, RetrievalRequest,
    SourceMetadata,
};
use brain_domain::{Node, NodeId, NodeType, SessionId};
use brain_session::SessionCacheManager;

use brain_services::retrieval::pipeline::{MemoryPipelineBuilder, PipelineAccumulator};

struct MockMemorySource {
    name: &'static str,
    nodes: Vec<Node>,
    calls: Arc<Mutex<usize>>,
    delay: Option<Duration>,
}

impl MockMemorySource {
    fn new(name: &'static str, nodes: Vec<Node>, calls: Arc<Mutex<usize>>) -> Self {
        Self {
            name,
            nodes,
            calls,
            delay: None,
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

impl MemorySource for MockMemorySource {
    fn retrieve(&self, _request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        if let Some(delay) = self.delay {
            std::thread::sleep(delay);
        }
        let mut guard = self.calls.lock().unwrap();
        *guard += 1;
        Ok(MemorySourceResult {
            nodes: self.nodes.clone(),
            metadata: SourceMetadata {
                source_name: self.name,
            },
        })
    }
}

struct ReverseRanking;

impl RankingStrategy for ReverseRanking {
    fn rank(
        &self,
        _request: &RetrievalRequest,
        mut nodes: Vec<Node>,
    ) -> Result<Vec<Node>, BrainError> {
        nodes.reverse();
        Ok(nodes)
    }
}

#[test]
fn test_pipeline_accumulator_deduplication() {
    let node1 = Node::new(NodeId::new(), "Node 1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node 2".to_string(), NodeType::Concept);

    let mut accumulator = PipelineAccumulator::new(HashSet::new());
    accumulator.add_source_results("source1", vec![node1.clone(), node2.clone()], 1.5);
    // Add duplicates
    accumulator.add_source_results("source2", vec![node1.clone()], 0.5);

    assert_eq!(accumulator.len(), 2);
    let nodes = accumulator.nodes_ref();
    assert_eq!(nodes[0].id, node1.id);
    assert_eq!(nodes[1].id, node2.id);

    let diagnostics = accumulator.diagnostics();
    assert_eq!(diagnostics.get("source1").unwrap().raw_count, 2);
    assert_eq!(diagnostics.get("source1").unwrap().unique_count, 2);
    assert_eq!(diagnostics.get("source2").unwrap().raw_count, 1);
    assert_eq!(diagnostics.get("source2").unwrap().unique_count, 0);
}

#[test]
fn test_pipeline_deterministic_execution_and_early_exit() {
    let calls_s1 = Arc::new(Mutex::new(0));
    let calls_s2 = Arc::new(Mutex::new(0));
    let calls_s3 = Arc::new(Mutex::new(0));

    let node1 = Node::new(NodeId::new(), "Node 1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node 2".to_string(), NodeType::Concept);
    let node3 = Node::new(NodeId::new(), "Node 3".to_string(), NodeType::Concept);
    let node4 = Node::new(NodeId::new(), "Node 4".to_string(), NodeType::Concept);
    let node5 = Node::new(NodeId::new(), "Node 5".to_string(), NodeType::Concept);

    let source1 = Arc::new(MockMemorySource::new(
        "s1",
        vec![node1.clone()],
        calls_s1.clone(),
    ));
    let source2 = Arc::new(MockMemorySource::new(
        "s2",
        vec![node1.clone(), node2.clone(), node3.clone(), node4.clone()],
        calls_s2.clone(),
    ));
    let source3 = Arc::new(MockMemorySource::new(
        "s3",
        vec![node5.clone()],
        calls_s3.clone(),
    ));

    // Limit is 1. With 3 sources, the early-exit threshold is limit * 3 = 3.
    // Source 1 gives Node 1 (accumulated: Node 1 -> count = 1).
    // Source 2 gives Node 1 (duplicate), Node 2, Node 3, Node 4 (accumulated: 4 nodes).
    // Since 4 >= 3, the limit * 3 break is triggered!
    // Source 3 should NOT be queried.
    let pipeline = MemoryPipelineBuilder::new()
        .register_source(source1)
        .register_source(source2)
        .register_source(source3)
        .build();

    let request = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "test".to_string(),
        limit: 1,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let response = pipeline.execute(&request).unwrap();
    assert_eq!(response.nodes.len(), 1);
    assert_eq!(response.nodes[0].id, node1.id);

    assert_eq!(*calls_s1.lock().unwrap(), 1);
    assert_eq!(*calls_s2.lock().unwrap(), 1);
    assert_eq!(*calls_s3.lock().unwrap(), 0); // Early exit triggered!
}

#[test]
fn test_pipeline_ranking_and_truncation() {
    let calls_s1 = Arc::new(Mutex::new(0));
    let node1 = Node::new(NodeId::new(), "Node 1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node 2".to_string(), NodeType::Concept);

    let source1 = Arc::new(MockMemorySource::new(
        "s1",
        vec![node1.clone(), node2.clone()],
        calls_s1,
    ));
    let pipeline = MemoryPipelineBuilder::new()
        .register_source(source1)
        .with_ranking_strategy(Arc::new(ReverseRanking))
        .build();

    let request = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "test".to_string(),
        limit: 1,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let response = pipeline.execute(&request).unwrap();
    // Unique collected = 2. Ranked (reversed) -> [Node 2, Node 1]. Truncated to limit 1 -> [Node 2].
    assert_eq!(response.nodes.len(), 1);
    assert_eq!(response.nodes[0].id, node2.id);
}

#[test]
fn test_pipeline_cache_hydration() {
    let calls_s1 = Arc::new(Mutex::new(0));
    let node1 = Node::new(NodeId::new(), "Node 1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node 2".to_string(), NodeType::Concept);

    let source1 = Arc::new(MockMemorySource::new(
        "s1",
        vec![node1.clone(), node2.clone()],
        calls_s1,
    ));
    let cache_manager = Arc::new(SessionCacheManager::new());

    let pipeline = MemoryPipelineBuilder::new()
        .register_source(source1)
        .with_cache_manager(cache_manager.clone())
        .with_policy(CacheHydrationPolicy::OnHit)
        .build();

    let session_id = SessionId::new();
    let request = RetrievalRequest {
        reference_time: None,
        session_id,
        query: "test".to_string(),
        limit: 1, // Only first node should be in response and hydrated
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let response = pipeline.execute(&request).unwrap();
    assert_eq!(response.nodes.len(), 1);
    assert_eq!(response.nodes[0].id, node1.id);

    // Verify cache has Node 1 but NOT Node 2 (since Node 2 was truncated before hydration)
    let ctx = cache_manager.get_or_create(session_id);
    let cached = ctx.read().unwrap().query("Node");
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].node.id, node1.id);
}

#[test]
fn test_pipeline_timeout() {
    let calls_s1 = Arc::new(Mutex::new(0));
    let node1 = Node::new(NodeId::new(), "Node 1".to_string(), NodeType::Concept);

    // Source delays for 50ms
    let source1 = Arc::new(
        MockMemorySource::new("s1", vec![node1], calls_s1).with_delay(Duration::from_millis(50)),
    );

    let pipeline = MemoryPipelineBuilder::new()
        .register_source(source1)
        .build();

    let request = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "test".to_string(),
        limit: 1,
        exclude_ids: HashSet::new(),
        // Deadline is 10ms from now, so it should time out
        deadline: Some(Instant::now() + Duration::from_millis(10)),
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let result = pipeline.execute(&request);
    assert!(result.is_err());
    match result.unwrap_err() {
        BrainError::Timeout { .. } => {}
        other => panic!("Expected Timeout error, got {:?}", other),
    }
}

#[test]
fn test_stm_memory_source() {
    use brain_services::retrieval::source::StmMemorySource;

    let cache_manager = Arc::new(SessionCacheManager::new());
    let session_id = SessionId::new();
    let node = Node::new(NodeId::new(), "Cached Node".to_string(), NodeType::Concept);

    // Ingest into STM
    {
        let ctx = cache_manager.get_or_create(session_id);
        ctx.write().unwrap().ingest(node.clone());
    }

    use brain_storage::TestStorage;
    let test_store = TestStorage::new();
    let repos = Arc::new(test_store.storage().clone());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let source = StmMemorySource::new(cache_manager.clone(), repos, registry);

    // 1. Basic match
    let req = RetrievalRequest {
        reference_time: None,
        session_id,
        query: "Cached".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };
    let res = source.retrieve(&req).unwrap();
    assert_eq!(res.metadata.source_name, "StmMemorySource");
    assert_eq!(res.nodes.len(), 1);
    assert_eq!(res.nodes[0].id, node.id);

    // 2. Exclude ID
    let mut exclude_ids = HashSet::new();
    exclude_ids.insert(node.id);
    let req_exclude = RetrievalRequest {
        reference_time: None,
        session_id,
        query: "Cached".to_string(),
        limit: 10,
        exclude_ids,
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };
    let res_exclude = source.retrieve(&req_exclude).unwrap();
    assert!(res_exclude.nodes.is_empty());
}

#[test]
fn test_ltm_memory_source() {
    use brain_core::repositories::RepositorySet;
    use brain_services::retrieval::source::LtmMemorySource;
    use brain_storage::TestStorage;

    let test_store = TestStorage::new();
    let repos = Arc::new(test_store.storage().clone());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let source = LtmMemorySource::new(repos.clone(), registry);

    let node1 = Node::new(
        NodeId::new(),
        "LongTerm Node A".to_string(),
        NodeType::Concept,
    );
    let node2 = Node::new(NodeId::new(), "Ltm Node B".to_string(), NodeType::Concept);

    repos.nodes().save(&node1).unwrap();
    repos.nodes().save(&node2).unwrap();

    // 1. Match case insensitivity and search query
    let req = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "longterm".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };
    let res = source.retrieve(&req).unwrap();
    assert_eq!(res.metadata.source_name, "LtmMemorySource");
    assert_eq!(res.nodes.len(), 1);
    assert_eq!(res.nodes[0].id, node1.id);

    // 2. Keyword match "Node" (both match)
    let req_both = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "node".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };
    let res_both = source.retrieve(&req_both).unwrap();
    assert_eq!(res_both.nodes.len(), 2);

    // 3. Exclude IDs
    let mut exclude = HashSet::new();
    exclude.insert(node1.id);
    let req_exclude = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "node".to_string(),
        limit: 10,
        exclude_ids: exclude,
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };
    let res_exclude = source.retrieve(&req_exclude).unwrap();
    assert_eq!(res_exclude.nodes.len(), 1);
    assert_eq!(res_exclude.nodes[0].id, node2.id);
}
