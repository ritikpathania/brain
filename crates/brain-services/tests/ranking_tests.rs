use std::collections::HashMap;
use std::sync::Arc;

use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{EmbeddingLookup, EmbeddingProvider, RankingStrategy, RetrievalRequest};
use brain_domain::{Edge, Node, NodeId, NodeType, SessionId};
use brain_services::retrieval::ranking::{
    Bm25Ranking, EmbeddingRanking, GraphRanking, RrfRanking,
};
use brain_storage::TestStorage;

struct MockEmbeddingProvider {
    name: &'static str,
    embedding: Vec<f32>,
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn embed(&self, _text: &str) -> Result<Vec<f32>, BrainError> {
        Ok(self.embedding.clone())
    }
}

struct MockEmbeddingLookup {
    embeddings: HashMap<NodeId, Vec<f32>>,
}

impl EmbeddingLookup for MockEmbeddingLookup {
    fn lookup(&self, node_id: &NodeId) -> Result<Option<Vec<f32>>, BrainError> {
        Ok(self.embeddings.get(node_id).cloned())
    }
}

#[test]
fn test_bm25_ranking() {
    let request = RetrievalRequest {
        session_id: SessionId::new(),
        query: "rust testing python".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let node1 = Node::new(NodeId::new(), "Rust is a programming language with testing frameworks".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Python is popular for data science".to_string(), NodeType::Concept);
    let node3 = Node::new(NodeId::new(), "JavaScript is used in web development".to_string(), NodeType::Concept);

    let ranking = Bm25Ranking::default();
    let ranked = ranking.rank(&request, vec![node3.clone(), node2.clone(), node1.clone()]).unwrap();

    assert_eq!(ranked.len(), 3);
    // node1 contains "rust" and "testing" -> should rank first
    assert_eq!(ranked[0].id, node1.id);
    // node2 contains "python" -> should rank second
    assert_eq!(ranked[1].id, node2.id);
    // node3 contains neither -> should rank third
    assert_eq!(ranked[2].id, node3.id);
}

#[test]
fn test_embedding_ranking() {
    let request = RetrievalRequest {
        session_id: SessionId::new(),
        query: "query text".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let node1 = Node::new(NodeId::new(), "Node 1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node 2".to_string(), NodeType::Concept);

    // query embedding: [1.0, 0.0]
    let provider = Arc::new(MockEmbeddingProvider {
        name: "mock",
        embedding: vec![1.0, 0.0],
    });

    // node1 embedding: [1.0, 0.0] (similarity 1.0)
    // node2 embedding: [0.0, 1.0] (similarity 0.0)
    let mut lookup_map = HashMap::new();
    lookup_map.insert(node1.id, vec![1.0, 0.0]);
    lookup_map.insert(node2.id, vec![0.0, 1.0]);

    let lookup = Arc::new(MockEmbeddingLookup {
        embeddings: lookup_map,
    });

    let ranking = EmbeddingRanking::new(provider, lookup);
    let ranked = ranking.rank(&request, vec![node2.clone(), node1.clone()]).unwrap();

    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].id, node1.id);
    assert_eq!(ranked[1].id, node2.id);
}

#[test]
fn test_graph_ranking() {
    let test_store = TestStorage::new();
    let repos = Arc::new(test_store.storage().clone());

    let node1 = Node::new(NodeId::new(), "Node 1".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Node 2".to_string(), NodeType::Concept);
    let node3 = Node::new(NodeId::new(), "Node 3".to_string(), NodeType::Concept);

    repos.nodes().save(&node1).unwrap();
    repos.nodes().save(&node2).unwrap();
    repos.nodes().save(&node3).unwrap();

    // Node 1 has connection to Node 2 (weight 5.0) and Node 3 (weight 2.5) -> sum = 7.5
    // Node 2 has connection to Node 1 (weight 1.0) -> sum = 1.0
    // Node 3 has no outgoing connections -> sum = 0.0
    let edge1 = Edge::new(node1.id, node2.id, "links".to_string(), 5.0);
    let edge2 = Edge::new(node1.id, node3.id, "links".to_string(), 2.5);
    let edge3 = Edge::new(node2.id, node1.id, "links".to_string(), 1.0);

    repos.edges().save(&edge1).unwrap();
    repos.edges().save(&edge2).unwrap();
    repos.edges().save(&edge3).unwrap();

    let request = RetrievalRequest {
        session_id: SessionId::new(),
        query: "ignored".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let ranking = GraphRanking::new(repos);
    let ranked = ranking.rank(&request, vec![node3.clone(), node2.clone(), node1.clone()]).unwrap();

    assert_eq!(ranked.len(), 3);
    assert_eq!(ranked[0].id, node1.id);
    assert_eq!(ranked[1].id, node2.id);
    assert_eq!(ranked[2].id, node3.id);
}

#[test]
fn test_rrf_ranking() {
    let request = RetrievalRequest {
        session_id: SessionId::new(),
        query: "rust python".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
    };

    let node1 = Node::new(NodeId::new(), "Rust language".to_string(), NodeType::Concept);
    let node2 = Node::new(NodeId::new(), "Python language".to_string(), NodeType::Concept);
    let node3 = Node::new(NodeId::new(), "C++ language".to_string(), NodeType::Concept);

    // Strategy 1 (weight 1.0): ranks node1, node2, node3
    // Strategy 2 (weight 0.5): ranks node2, node3, node1
    struct DummyStrategy {
        order: Vec<NodeId>,
    }
    impl RankingStrategy for DummyStrategy {
        fn rank(&self, _req: &RetrievalRequest, nodes: Vec<Node>) -> Result<Vec<Node>, BrainError> {
            let mut res = Vec::new();
            for &id in &self.order {
                if let Some(n) = nodes.iter().find(|n| n.id == id) {
                    res.push(n.clone());
                }
            }
            Ok(res)
        }
    }

    let s1 = Arc::new(DummyStrategy {
        order: vec![node1.id, node2.id, node3.id],
    });
    let s2 = Arc::new(DummyStrategy {
        order: vec![node2.id, node3.id, node1.id],
    });

    // RRF score logic:
    // For k = 1.0:
    // node1 score:
    // - s1 rank 1 -> 1.0 / (1.0 + 1.0) = 0.5
    // - s2 rank 3 -> 0.5 / (1.0 + 3.0) = 0.125
    // -> sum = 0.625
    // node2 score:
    // - s1 rank 2 -> 1.0 / (1.0 + 2.0) = 0.333
    // - s2 rank 1 -> 0.5 / (1.0 + 1.0) = 0.25
    // -> sum = 0.583
    // node3 score:
    // - s1 rank 3 -> 1.0 / (1.0 + 3.0) = 0.25
    // - s2 rank 2 -> 0.5 / (1.0 + 2.0) = 0.167
    // -> sum = 0.417
    // Ordered: node1, node2, node3

    let ranking = RrfRanking::new(vec![(s1, 1.0), (s2, 0.5)], 1.0);
    let ranked = ranking.rank(&request, vec![node3.clone(), node2.clone(), node1.clone()]).unwrap();

    assert_eq!(ranked.len(), 3);
    assert_eq!(ranked[0].id, node1.id);
    assert_eq!(ranked[1].id, node2.id);
    assert_eq!(ranked[2].id, node3.id);
}
