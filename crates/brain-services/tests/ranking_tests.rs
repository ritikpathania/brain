use std::collections::HashSet;
use std::sync::Arc;

use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{
    DefaultQueryEmbeddingService, EmbeddingProvider, RankingStrategy, RetrievalRequest,
};
use brain_domain::{Edge, Embedding, Node, NodeId, RelationKind, SessionId};
use brain_services::retrieval::ranking::{Bm25Ranking, EmbeddingRanking, GraphRanking, RrfRanking};
use brain_storage::TestStorage;

fn make_node(id: u64, content: &str) -> Node {
    let node_id = NodeId(uuid::Uuid::from_u64_pair(0, id));
    Node::new(
        node_id,
        content.to_string(),
        brain_domain::NodeType::Concept,
    )
}

#[test]
fn test_bm25_ranking() {
    let strategy = Bm25Ranking::default();
    let request = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "sqlite database".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let node1 = make_node(1, "database setup and sqlite configuration");
    let node2 = make_node(2, "writing simple rust code without databases");
    let node3 = make_node(3, "sqlite is a file based lightweight relational database");

    let nodes = vec![node2.clone(), node1.clone(), node3.clone()];
    let ranked = strategy.rank(&request, nodes).unwrap();

    assert_eq!(ranked.len(), 3);
    // node1 and node3 both contain "sqlite" and "database", node2 contains neither (databases is different token)
    assert_eq!(ranked[2].id, node2.id);
}

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

#[test]
fn test_embedding_ranking() {
    let test_storage = TestStorage::new();
    let store = test_storage.store();

    // Query vector
    let provider = Arc::new(CustomEmbeddingProvider {
        emb: vec![1.0, 0.0],
    });
    let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider));
    let strategy = EmbeddingRanking::new(
        embed_service,
        store.clone() as Arc<dyn brain_core::retrieval::EmbeddingLookup>,
    );

    let node1 = make_node(1, "first node");
    let node2 = make_node(2, "second node");
    let node3 = make_node(3, "third node");

    store.nodes().save(&node1).unwrap();
    store.nodes().save(&node2).unwrap();
    store.nodes().save(&node3).unwrap();

    // Save embeddings
    // node1 is perfectly aligned (similarity = 1.0)
    store
        .embeddings()
        .save(&Embedding::new(node1.id, vec![1.0, 0.0]))
        .unwrap();
    // node2 is perpendicular (similarity = 0.0)
    store
        .embeddings()
        .save(&Embedding::new(node2.id, vec![0.0, 1.0]))
        .unwrap();
    // node3 has no embedding (similarity = 0.0)

    let request = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "query text".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let ranked = strategy
        .rank(&request, vec![node2.clone(), node3.clone(), node1.clone()])
        .unwrap();
    assert_eq!(ranked[0].id, node1.id);
    assert_eq!(ranked[1].id, node2.id);
    assert_eq!(ranked[2].id, node3.id);

    test_storage.assert_clean();
}

#[test]
fn test_graph_ranking() {
    let test_storage = TestStorage::new();
    let store = test_storage.store();
    let strategy = GraphRanking::new(store.clone());

    let node1 = make_node(1, "node 1");
    let node2 = make_node(2, "node 2");
    let node3 = make_node(3, "node 3");

    store.nodes().save(&node1).unwrap();
    store.nodes().save(&node2).unwrap();
    store.nodes().save(&node3).unwrap();

    // node1 has total weight 5.0
    store
        .edges()
        .save(&Edge::new(
            node1.id,
            node2.id,
            RelationKind::AssociatedWith,
            5.0,
        ))
        .unwrap();
    // node2 has total weight 7.0 (5.0 + 2.0)
    store
        .edges()
        .save(&Edge::new(
            node2.id,
            node3.id,
            RelationKind::AssociatedWith,
            2.0,
        ))
        .unwrap();

    let request = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "query text".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let ranked = strategy
        .rank(&request, vec![node1.clone(), node3.clone(), node2.clone()])
        .unwrap();
    assert_eq!(ranked[0].id, node2.id); // Weight 7.0
    assert_eq!(ranked[1].id, node1.id); // Weight 5.0
    assert_eq!(ranked[2].id, node3.id); // Weight 2.0

    test_storage.assert_clean();
}

#[test]
fn test_rrf_ranking() {
    struct ReverseStrategy;
    impl RankingStrategy for ReverseStrategy {
        fn rank(
            &self,
            _req: &RetrievalRequest,
            mut nodes: Vec<Node>,
        ) -> Result<Vec<Node>, brain_core::errors::BrainError> {
            nodes.reverse();
            Ok(nodes)
        }
    }

    struct IdentityStrategy;
    impl RankingStrategy for IdentityStrategy {
        fn rank(
            &self,
            _req: &RetrievalRequest,
            nodes: Vec<Node>,
        ) -> Result<Vec<Node>, brain_core::errors::BrainError> {
            Ok(nodes)
        }
    }

    let strategy_a = Arc::new(ReverseStrategy);
    let strategy_b = Arc::new(IdentityStrategy);

    let rrf = RrfRanking::new(vec![(strategy_a, 1.5), (strategy_b, 0.5)], 60.0);

    let node1 = make_node(1, "first");
    let node2 = make_node(2, "second");

    let request = RetrievalRequest {
        reference_time: None,
        session_id: SessionId::new(),
        query: "query text".to_string(),
        limit: 10,
        exclude_ids: HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let ranked = rrf
        .rank(&request, vec![node1.clone(), node2.clone()])
        .unwrap();
    // ReverseStrategy outputs [node2, node1] -> node2 rank = 1, node1 rank = 2
    // IdentityStrategy outputs [node1, node2] -> node1 rank = 1, node2 rank = 2
    // node1 score = 1.5 / (60+2) + 0.5 / (60+1) = 1.5/62 + 0.5/61 = 0.02419 + 0.00819 = 0.03238
    // node2 score = 1.5 / (60+1) + 0.5 / (60+2) = 1.5/61 + 0.5/62 = 0.02459 + 0.00806 = 0.03265
    // node2 has higher score, so node2 should rank first.
    assert_eq!(ranked[0].id, node2.id);
}
