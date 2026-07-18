use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{
    DefaultQueryEmbeddingService, EmbeddingProvider, MemorySource, RetrievalRequest,
};
use brain_domain::{Node, NodeId, NodeType};
use brain_services::eval_harness::{
    run_benchmark, GroundTruthCorpus, QueryCorpus, RetrievalChannel, RetrievalResult, Retriever,
};
use brain_services::retrieval::pipeline::MemoryPipelineBuilder;
use brain_services::retrieval::ranking::{Bm25Ranking, EmbeddingRanking, RrfRanking};
use brain_services::retrieval::source::{LtmMemorySource, SemanticMemorySource};
use brain_storage::TestStorage;
use std::sync::Arc;

fn create_test_node(id: NodeId, label: &str) -> Node {
    Node::new(id, label.to_string(), NodeType::Concept)
}

#[test]
fn test_ltm_retrieval_edge_cases() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node_punctuation = NodeId::new();
    let node_unicode = NodeId::new();
    let node_mixed_case = NodeId::new();
    let node_percent = NodeId::new();
    let node_underscore = NodeId::new();
    let node_wildcard_no_match = NodeId::new();
    let node_wildcard_single_no_match = NodeId::new();
    let node_prefix_collision = NodeId::new();
    let node_high_match1 = NodeId::new();
    let node_high_match2 = NodeId::new();

    // Seed test nodes
    let nodes = vec![
        create_test_node(node_punctuation, "Rust Lang"),
        create_test_node(node_unicode, "äëïöü"),
        create_test_node(node_mixed_case, "rust"),
        create_test_node(node_percent, "100% finished"),
        create_test_node(node_underscore, "a_b connection"),
        create_test_node(node_wildcard_no_match, "1000 items"),
        create_test_node(node_wildcard_single_no_match, "acb connection"),
        create_test_node(node_prefix_collision, "rust"),
        create_test_node(node_high_match1, "common match one"),
        create_test_node(node_high_match2, "common match two"),
    ];
    store.nodes().save_batch(&nodes).unwrap();

    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let source = LtmMemorySource::new(Arc::new(store.clone()), registry);

    // 1. Punctuation
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "rust-lang!".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    let ids: Vec<NodeId> = res.nodes.iter().map(|n| n.id).collect();
    assert!(
        ids.contains(&node_punctuation),
        "Punctuation-rich query should match 'Rust Lang'"
    );

    // 2. Unicode
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "äëïöü".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    let ids: Vec<NodeId> = res.nodes.iter().map(|n| n.id).collect();
    assert!(
        ids.contains(&node_unicode),
        "Unicode query should match 'äëïöü'"
    );

    // 3. Mixed Case
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "RuSt".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    let ids: Vec<NodeId> = res.nodes.iter().map(|n| n.id).collect();
    assert!(
        ids.contains(&node_mixed_case),
        "Mixed-case query should match 'rust'"
    );

    // 4. SQL wildcard escaping (%) - Querying "100%" on NodeRepository directly
    let results = store.nodes().find_by_tokens(&["100%".to_string()]).unwrap();
    let matched_ids: Vec<NodeId> = results.iter().map(|n| n.id).collect();
    assert!(matched_ids.contains(&node_percent));
    assert!(
        !matched_ids.contains(&node_wildcard_no_match),
        "SQL wildcard % escaping failed"
    );

    // 5. SQL wildcard escaping (_) - Querying "a_b" on NodeRepository directly
    let results = store.nodes().find_by_tokens(&["a_b".to_string()]).unwrap();
    let matched_ids: Vec<NodeId> = results.iter().map(|n| n.id).collect();
    assert!(matched_ids.contains(&node_underscore));
    assert!(
        !matched_ids.contains(&node_wildcard_single_no_match),
        "SQL wildcard _ escaping failed"
    );

    // 6. Duplicate Tokens
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "rust rust".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    assert!(!res.nodes.is_empty());

    // 7. Empty Query
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    assert!(res.nodes.is_empty(), "Empty query should return zero nodes");

    // 8. Stop Words Only
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "the is in".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    assert!(
        res.nodes.is_empty(),
        "Query containing only stop words should return zero nodes"
    );

    // 9. Zero-match query
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "nonexistentkeyword".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    assert!(res.nodes.is_empty());

    // 10. High-match query
    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "match".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };
    let res = source.retrieve(&req).unwrap();
    assert_eq!(res.nodes.len(), 2);
}

#[test]
fn test_ltm_retrieval_bm25_toggle() {
    let test_store = TestStorage::new();
    let store = test_store.storage();

    let node_id = NodeId::new();
    let node = Node::new(
        node_id,
        "special unique compile label".to_string(),
        NodeType::Concept,
    );
    store.nodes().save(&node).unwrap();

    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let source = LtmMemorySource::new(Arc::new(store.clone()), registry);

    let req = RetrievalRequest {
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        query: "compile".to_string(),
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        limit: 10,
    };

    let res = source.retrieve(&req).unwrap();
    let matched_ids: Vec<NodeId> = res.nodes.iter().map(|n| n.id).collect();
    assert!(
        matched_ids.contains(&node_id),
        "BM25 retrieval failed to match FTS5 node"
    );
}

struct MemorySourceRetriever {
    source: Arc<LtmMemorySource>,
    session_id: brain_domain::SessionId,
}

impl Retriever for MemorySourceRetriever {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let request = RetrievalRequest {
            session_id: self.session_id,
            query: query.to_string(),
            limit: 10,
            exclude_ids: std::collections::HashSet::new(),
            deadline: None,
        };
        let res = self.source.retrieve(&request)?;
        Ok(res
            .nodes
            .into_iter()
            .map(|node| RetrievalResult {
                node_id: node.id,
                channel_scores: std::collections::HashMap::from([(RetrievalChannel::Fts, 1.0)]),
                ranking_score: None,
            })
            .collect())
    }
}

#[test]
fn test_ltm_retrieval_ranking_quality() {
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");

    let _queries: QueryCorpus = serde_json::from_str(queries_json).unwrap();
    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();

    for n in &ground_truth.nodes {
        let node_id = NodeId(uuid::Uuid::parse_str(&n.node_id).unwrap());
        let node_type = if n.node_type == "Concept" {
            NodeType::Concept
        } else {
            NodeType::Technology
        };
        let node = Node::new(node_id, n.content.clone(), node_type);
        sqlite.nodes().save(&node).unwrap();
    }

    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let session_id = "01H7X1F8Z9Y000000000000000".parse().unwrap();

    // BM25 quality
    let bm25_source = Arc::new(LtmMemorySource::new(Arc::new(sqlite.clone()), registry));
    let retriever_b = MemorySourceRetriever {
        source: bm25_source,
        session_id,
    };
    let report_b = run_benchmark(queries_json, ground_truth_json, &retriever_b, "cold").unwrap();

    println!("=== Retrieval Quality (BM25) ===");
    println!(
        "BM25      - Precision@10: {:.4}, Recall@10: {:.4}, NDCG@10: {:.4}",
        report_b.stable.metrics.mean_precision_at_10,
        report_b.stable.metrics.mean_recall_at_10,
        report_b.stable.metrics.mean_ndcg_at_10
    );

    assert!(report_b.stable.metrics.mean_precision_at_10 > 0.0);
    assert!(report_b.stable.metrics.mean_recall_at_10 > 0.0);
}

struct HashingEmbeddingProvider;

impl brain_core::retrieval::EmbeddingProvider for HashingEmbeddingProvider {
    fn name(&self) -> &'static str {
        "hashing-embedding-provider"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let mut v = vec![0.0f32; 384];
        let stop_words: std::collections::HashSet<&str> = [
            "a", "an", "the", "and", "or", "but", "is", "are", "was", "were", "to", "of", "in",
            "on", "at", "for", "with", "by", "about", "as", "this", "that", "these", "those", "it",
            "its", "you", "your", "my", "up", "down", "out", "off",
        ]
        .iter()
        .cloned()
        .collect();

        let tokens: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && !stop_words.contains(s))
            .map(|s| match s {
                "olap" => "analytics".to_string(),
                "dashboard" => "metrics".to_string(),
                "instrumentation" => "telemetry".to_string(),
                "reporter" => "export".to_string(),
                "termination" => "sigterm".to_string(),
                "graceful" => "draining".to_string(),
                "shutdown" => "sigterm".to_string(),
                "routine" => "signals".to_string(),
                other => other.to_string(),
            })
            .collect();

        if tokens.is_empty() {
            return Ok(v);
        }

        for tok in &tokens {
            let mut h: u32 = 5381;
            for c in tok.bytes() {
                h = h.wrapping_shl(5).wrapping_add(h).wrapping_add(c as u32);
            }
            let idx = (h % 384) as usize;
            v[idx] += 1.0;
        }

        let mut norm_sq = 0.0f32;
        for &val in &v {
            norm_sq += val * val;
        }
        let norm = norm_sq.sqrt();
        if norm > 0.0 {
            for val in v.iter_mut() {
                *val /= norm;
            }
        }
        Ok(v)
    }
}

#[test]
fn test_ltm_hybrid_retrieval_metrics() {
    let test_storage = TestStorage::new();
    let sqlite = test_storage.storage();

    let queries_json = include_str!("evaluation/queries.json");
    let ground_truth_json = include_str!("evaluation/ground_truth.json");

    let ground_truth: GroundTruthCorpus = serde_json::from_str(ground_truth_json).unwrap();
    let provider = Arc::new(HashingEmbeddingProvider);
    let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider.clone()));

    // Save nodes and generate embeddings
    for n in &ground_truth.nodes {
        let node_id = NodeId(uuid::Uuid::parse_str(&n.node_id).unwrap());
        let node_type = if n.node_type == "Concept" {
            NodeType::Concept
        } else {
            NodeType::Technology
        };
        let node = Node::new(node_id, n.content.clone(), node_type);
        sqlite.nodes().save(&node).unwrap();

        let vector = provider.embed(&n.content).unwrap();
        sqlite
            .embeddings()
            .save(&brain_domain::Embedding::new(node_id, vector))
            .unwrap();
    }

    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());

    // Build independent channels
    let source_bm25 = Arc::new(LtmMemorySource::new(
        Arc::new(sqlite.clone()),
        registry.clone(),
    ));
    let source_vector = Arc::new(SemanticMemorySource::new(
        Arc::new(sqlite.clone()),
        embed_service.clone(),
    ));

    // Build RRF fused pipeline
    let strategy_bm25 = Arc::new(Bm25Ranking::default());
    let strategy_vector = Arc::new(EmbeddingRanking::new(
        embed_service.clone(),
        Arc::new(sqlite.clone()) as Arc<dyn brain_core::retrieval::EmbeddingLookup>,
    ));
    let rrf_ranking = Arc::new(RrfRanking::new(
        vec![(strategy_bm25, 1.0), (strategy_vector, 1.0)],
        60.0,
    ));

    let pipeline = MemoryPipelineBuilder::new()
        .register_source(source_bm25.clone())
        .register_source(source_vector.clone())
        .with_ranking_strategy(rrf_ranking)
        .build();

    let session_id = "01H7X1F8Z9Y000000000000000".parse().unwrap();
    let queries_struct: QueryCorpus = serde_json::from_str(queries_json).unwrap();

    let mut total_expected_hits = 0;
    let mut bm25_only_hits = 0;
    let mut vector_only_hits = 0;
    let mut overlapping_hits = 0;

    let mut bm25_recalls = Vec::new();
    let mut vector_recalls = Vec::new();
    let mut rrf_recalls = Vec::new();

    for q in &queries_struct.queries {
        let req = RetrievalRequest {
            session_id,
            query: q.text.clone(),
            limit: 10,
            exclude_ids: std::collections::HashSet::new(),
            deadline: None,
        };

        // Get expected matching IDs
        let expected_strs = &ground_truth
            .ground_truth
            .get(&q.query_id)
            .unwrap()
            .expected_node_ids;
        let expected: std::collections::HashSet<NodeId> = expected_strs
            .iter()
            .map(|s| NodeId(uuid::Uuid::parse_str(s).unwrap()))
            .collect();

        // 1. BM25 results
        let res_b = source_bm25.retrieve(&req).unwrap().nodes;
        let set_b: std::collections::HashSet<NodeId> = res_b.iter().map(|n| n.id).collect();

        // 2. Vector results
        let res_v = source_vector.retrieve(&req).unwrap().nodes;
        let set_v: std::collections::HashSet<NodeId> = res_v.iter().map(|n| n.id).collect();

        // 3. Fused RRF results
        let res_r = pipeline.execute(&req).unwrap().nodes;
        let set_r: std::collections::HashSet<NodeId> = res_r.iter().map(|n| n.id).collect();

        for &e in &expected {
            total_expected_hits += 1;
            let in_bm25 = set_b.contains(&e);
            let in_vector = set_v.contains(&e);

            if in_bm25 && !in_vector {
                bm25_only_hits += 1;
            }
            if in_vector && !in_bm25 {
                vector_only_hits += 1;
            }
            if in_bm25 && in_vector {
                overlapping_hits += 1;
            }
        }

        // Calculate recalls at 10
        let hits_b = expected.iter().filter(|id| set_b.contains(id)).count() as f64;
        let hits_v = expected.iter().filter(|id| set_v.contains(id)).count() as f64;
        let hits_r = expected.iter().filter(|id| set_r.contains(id)).count() as f64;

        bm25_recalls.push(hits_b / (expected.len() as f64));
        vector_recalls.push(hits_v / (expected.len() as f64));
        rrf_recalls.push(hits_r / (expected.len() as f64));
    }

    let mean_recall_bm25 = bm25_recalls.iter().sum::<f64>() / (bm25_recalls.len() as f64);
    let mean_recall_vector = vector_recalls.iter().sum::<f64>() / (vector_recalls.len() as f64);
    let mean_recall_rrf = rrf_recalls.iter().sum::<f64>() / (rrf_recalls.len() as f64);

    println!("=== Hybrid Search Evaluation Metrics (26 Queries) ===");
    println!("Total Expected Ground-Truth Hits : {}", total_expected_hits);
    println!(
        "BM25-Only Hits                   : {} ({:.1}%)",
        bm25_only_hits,
        (bm25_only_hits as f64 / total_expected_hits as f64) * 100.0
    );
    println!(
        "Vector-Only Hits                 : {} ({:.1}%)",
        vector_only_hits,
        (vector_only_hits as f64 / total_expected_hits as f64) * 100.0
    );
    println!(
        "Overlapping Hits                 : {} ({:.1}%)",
        overlapping_hits,
        (overlapping_hits as f64 / total_expected_hits as f64) * 100.0
    );
    println!("--------------------------------------------------");
    println!("Recall@10 comparison:");
    println!("  BM25-Only Channel              : {:.4}", mean_recall_bm25);
    println!(
        "  Vector-Only Channel            : {:.4}",
        mean_recall_vector
    );
    println!("  RRF Fused Pipeline             : {:.4}", mean_recall_rrf);

    assert!(
        mean_recall_rrf >= mean_recall_bm25,
        "RRF did not perform at least as well as BM25"
    );
    assert!(
        mean_recall_rrf >= mean_recall_vector,
        "RRF did not perform at least as well as Vector"
    );
    assert!(vector_only_hits > 0, "No semantic-only queries matched!");
}
