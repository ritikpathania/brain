use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::plugins::{EmbeddingProvider, RankingStrategy, RetrievalAlgorithm, StorageBackend};
use crate::retrieval::embeddings::EmbeddingsRetrieval;
use crate::stm::{STMIndex, TempNode};
use crate::storage::ExtractedEdge;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct QueryResultNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub content: String,
    pub attributes: serde_json::Value,
    pub score: i64,
    pub source: String, // "STM" or "LTM"
    pub connections: Vec<ExtractedEdge>,
}

pub fn run_retrieval_pipeline(
    query: &str,
    index: &STMIndex,
    window: &[TempNode],
    retrieval: &dyn RetrievalAlgorithm,
    ranking: &dyn RankingStrategy,
    storage: Option<&dyn StorageBackend>,
    embedding_provider: Option<&dyn EmbeddingProvider>,
) -> Result<Vec<QueryResultNode>, String> {
    // 1. Retrieve STM lexical candidates
    let stm_candidates = retrieval.retrieve(query, index, window)?;

    // 2. Retrieve LTM lexical candidates
    let mut ltm_lexical_candidates = Vec::new();
    let mut ltm_nodes_map = HashMap::new();

    if let Some(storage_backend) = storage {
        if let Ok(graph_nodes) = storage_backend.query_graph(query) {
            let mut ltm_temp_nodes = Vec::new();
            for (node, _) in graph_nodes {
                let content = format!(
                    "{} {} ({}) {}",
                    node.id,
                    node.label,
                    node.node_type,
                    serde_json::to_string(&node.attributes).unwrap_or_default()
                );
                ltm_temp_nodes.push(TempNode {
                    id: node.id.clone(),
                    epoch: 0,
                    content,
                    timestamp: 0,
                });
                ltm_nodes_map.insert(node.id.clone(), node);
            }
            let bm25 = crate::retrieval::bm25::Bm25Retrieval::default();
            ltm_lexical_candidates = bm25.score_corpus(query, &ltm_temp_nodes);
        }
    }

    // 3. Retrieve LTM semantic candidates
    let mut semantic_candidates = Vec::new();
    if let (Some(storage_backend), Some(emb_provider)) = (storage, embedding_provider) {
        if emb_provider.name() != "noop" {
            if let Ok(sem_res) =
                EmbeddingsRetrieval::retrieve_ltm_semantic(query, emb_provider, storage_backend, 20)
            {
                semantic_candidates = sem_res;
            }
        }
    }

    // 4. Merge candidates using Reciprocal Rank Fusion (RRF)
    let mut stm_candidates_sorted = stm_candidates.clone();
    stm_candidates_sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
    let stm_ids: Vec<String> = stm_candidates_sorted
        .iter()
        .map(|c| c.0.id.clone())
        .collect();

    let mut ltm_lexical_sorted = ltm_lexical_candidates.clone();
    ltm_lexical_sorted.sort_by_key(|b| std::cmp::Reverse(b.1));
    let ltm_lexical_ids: Vec<String> = ltm_lexical_sorted.iter().map(|c| c.0.id.clone()).collect();

    let mut semantic_sorted = semantic_candidates.clone();
    semantic_sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let semantic_ids: Vec<String> = semantic_sorted.iter().map(|c| c.0.clone()).collect();

    let mut rrf_scores = HashMap::new();
    let add_to_rrf = |list_ids: &[String], rrf_scores: &mut HashMap<String, f64>| {
        for (rank, id) in list_ids.iter().enumerate() {
            let r = (rank + 1) as f64;
            let score = 1.0 / (60.0 + r);
            *rrf_scores.entry(id.clone()).or_insert(0.0) += score;
        }
    };

    add_to_rrf(&stm_ids, &mut rrf_scores);
    add_to_rrf(&ltm_lexical_ids, &mut rrf_scores);
    add_to_rrf(&semantic_ids, &mut rrf_scores);

    // Fetch missing LTM node details
    if let Some(storage_backend) = storage {
        let mut missing_ltm_ids = Vec::new();
        for id in rrf_scores.keys() {
            if !window.iter().any(|n| n.id == *id) && !ltm_nodes_map.contains_key(id) {
                missing_ltm_ids.push(id.clone());
            }
        }
        if !missing_ltm_ids.is_empty() {
            if let Ok(fetched_nodes) = storage_backend.get_nodes_by_ids(&missing_ltm_ids) {
                for node in fetched_nodes {
                    ltm_nodes_map.insert(node.id.clone(), node);
                }
            }
        }
    }

    // Construct QueryResultNode list
    let mut candidate_nodes = Vec::new();
    for (id, rrf_score) in rrf_scores {
        let score_val = (rrf_score * 10000.0) as i64;
        if let Some(stm_node) = window.iter().find(|n| n.id == id) {
            candidate_nodes.push(QueryResultNode {
                id: stm_node.id.clone(),
                label: stm_node.content.clone(),
                node_type: "session_context".to_string(),
                content: stm_node.content.clone(),
                attributes: serde_json::json!({
                    "epoch": stm_node.epoch,
                    "timestamp": stm_node.timestamp
                }),
                score: score_val,
                source: "STM".to_string(),
                connections: Vec::new(),
            });
        } else if let Some(ltm_node) = ltm_nodes_map.get(&id) {
            let content = format!(
                "{} ({}) {}",
                ltm_node.label,
                ltm_node.node_type,
                serde_json::to_string(&ltm_node.attributes).unwrap_or_default()
            );
            candidate_nodes.push(QueryResultNode {
                id: ltm_node.id.clone(),
                label: ltm_node.label.clone(),
                node_type: ltm_node.node_type.clone(),
                content,
                attributes: ltm_node.attributes.clone(),
                score: score_val,
                source: "LTM".to_string(),
                connections: Vec::new(),
            });
        }
    }

    // 5. Graph Expansion
    if let Some(storage_backend) = storage {
        candidate_nodes.sort_by_key(|b| std::cmp::Reverse(b.score));
        let top_ids: Vec<String> = candidate_nodes
            .iter()
            .take(10)
            .map(|n| n.id.clone())
            .collect();
        if !top_ids.is_empty() {
            if let Ok(edges) = storage_backend.get_connections(&top_ids) {
                // Populate connections for candidates
                for node in &mut candidate_nodes {
                    node.connections = edges
                        .iter()
                        .filter(|e| e.source == node.id || e.target == node.id)
                        .cloned()
                        .collect();
                }

                // Identify 1-hop neighbors
                let mut neighbors_to_add = HashMap::new();
                for edge in &edges {
                    let source_is_candidate = top_ids.contains(&edge.source);
                    let target_is_candidate = top_ids.contains(&edge.target);

                    if source_is_candidate
                        && !target_is_candidate
                        && !candidate_nodes.iter().any(|n| n.id == edge.target)
                    {
                        let parent_score = candidate_nodes
                            .iter()
                            .find(|n| n.id == edge.source)
                            .map(|n| n.score)
                            .unwrap_or(0);
                        let neighbor_score = (parent_score as f64 * 0.5) as i64;
                        let entry = neighbors_to_add.entry(edge.target.clone()).or_insert(0);
                        if neighbor_score > *entry {
                            *entry = neighbor_score;
                        }
                    } else if target_is_candidate
                        && !source_is_candidate
                        && !candidate_nodes.iter().any(|n| n.id == edge.source)
                    {
                        let parent_score = candidate_nodes
                            .iter()
                            .find(|n| n.id == edge.target)
                            .map(|n| n.score)
                            .unwrap_or(0);
                        let neighbor_score = (parent_score as f64 * 0.5) as i64;
                        let entry = neighbors_to_add.entry(edge.source.clone()).or_insert(0);
                        if neighbor_score > *entry {
                            *entry = neighbor_score;
                        }
                    }
                }

                if !neighbors_to_add.is_empty() {
                    let neighbor_ids: Vec<String> = neighbors_to_add.keys().cloned().collect();
                    let mut neighbor_nodes_map = HashMap::new();
                    if let Ok(fetched) = storage_backend.get_nodes_by_ids(&neighbor_ids) {
                        for node in fetched {
                            neighbor_nodes_map.insert(node.id.clone(), node);
                        }
                    }

                    for (nid, score) in neighbors_to_add {
                        if let Some(nnode) = neighbor_nodes_map.remove(&nid) {
                            let n_edges = edges
                                .iter()
                                .filter(|e| e.source == nid || e.target == nid)
                                .cloned()
                                .collect();

                            let content = format!(
                                "{} {} ({}) {}",
                                nnode.id,
                                nnode.label,
                                nnode.node_type,
                                serde_json::to_string(&nnode.attributes).unwrap_or_default()
                            );
                            candidate_nodes.push(QueryResultNode {
                                id: nnode.id.clone(),
                                label: nnode.label.clone(),
                                node_type: nnode.node_type.clone(),
                                content,
                                attributes: nnode.attributes.clone(),
                                score,
                                source: "LTM".to_string(),
                                connections: n_edges,
                            });
                        }
                    }
                }
            }
        }
    }

    // 6. Reranking using RankingStrategy
    let mut temp_candidates: Vec<(TempNode, i64)> = candidate_nodes
        .iter()
        .map(|n| {
            (
                TempNode {
                    id: n.id.clone(),
                    epoch: 0,
                    content: n.content.clone(),
                    timestamp: 0,
                },
                n.score,
            )
        })
        .collect();

    ranking.rank(query, &mut temp_candidates)?;

    let mut score_map = HashMap::new();
    for (tn, score) in temp_candidates {
        score_map.insert(tn.id, score);
    }
    for node in &mut candidate_nodes {
        if let Some(&new_score) = score_map.get(&node.id) {
            node.score = new_score;
        }
    }

    candidate_nodes.sort_by_key(|b| std::cmp::Reverse(b.score));

    Ok(candidate_nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::StorageBackend;
    use crate::retrieval::fuzzy::FuzzyRetrieval;
    use crate::retrieval::reranker::DefaultRanking;
    use crate::stm::STMIndex;
    use crate::storage::sqlite::LtmDatabase;
    use crate::storage::ExtractedNode;

    #[test]
    fn test_hybrid_retrieval_pipeline() {
        let retrieval = FuzzyRetrieval;
        let ranking = DefaultRanking;

        // 1. Create a dummy STM window
        let window = vec![TempNode {
            id: "stm-node-1".to_string(),
            epoch: 1,
            content: "rust compiler optimization flags".to_string(),
            timestamp: 1000,
        }];
        let mut index = STMIndex::new();
        index.add("stm-node-1".to_string(), "rust compiler optimization flags");

        // 2. Create in-memory SQLite storage
        let storage = LtmDatabase::new_in_memory().unwrap();

        // Insert nodes and relations
        let ltm_nodes = vec![
            ExtractedNode {
                id: "ltm-node-1".to_string(),
                label: "Rust Programming".to_string(),
                node_type: "language".to_string(),
                attributes: serde_json::json!({"level": "advanced"}),
            },
            ExtractedNode {
                id: "ltm-node-2".to_string(),
                label: "LLVM".to_string(),
                node_type: "compiler-backend".to_string(),
                attributes: serde_json::json!({}),
            },
        ];
        let ltm_edges = vec![crate::storage::ExtractedEdge {
            source: "ltm-node-1".to_string(),
            target: "ltm-node-2".to_string(),
            relation: "compiled_with".to_string(),
        }];
        storage
            .upsert_nodes_and_edges(&ltm_nodes, &ltm_edges)
            .unwrap();

        // Write mock embedding for ltm-node-1
        storage
            .write_embeddings(&[("ltm-node-1".to_string(), vec![0.5, 0.5, 0.0])])
            .unwrap();

        // 3. Create mock embedding provider
        struct MockEmbeddingProvider;
        impl EmbeddingProvider for MockEmbeddingProvider {
            fn name(&self) -> &str {
                "mock-embed"
            }
            fn embed(&self, _text: &str) -> Result<Vec<f32>, String> {
                // Match the mock embedding of ltm-node-1
                Ok(vec![0.5, 0.5, 0.0])
            }
        }
        let emb_provider = MockEmbeddingProvider;

        // 4. Run the hybrid retrieval pipeline
        let results = run_retrieval_pipeline(
            "rust compiler",
            &index,
            &window,
            &retrieval,
            &ranking,
            Some(&storage),
            Some(&emb_provider),
        )
        .unwrap();

        // We expect stm-node-1, ltm-node-1 (lexical & semantic), and ltm-node-2 (via graph expansion)
        assert!(!results.is_empty());

        // Check that stm-node-1 is retrieved
        assert!(results
            .iter()
            .any(|r| r.id == "stm-node-1" && r.source == "STM"));

        // Check that ltm-node-1 is retrieved
        assert!(results
            .iter()
            .any(|r| r.id == "ltm-node-1" && r.source == "LTM"));

        // Check that ltm-node-2 is retrieved via graph expansion (its score should be dampened)
        let expanded = results.iter().find(|r| r.id == "ltm-node-2");
        assert!(expanded.is_some());
        let expanded_node = expanded.unwrap();
        assert_eq!(expanded_node.source, "LTM");
        assert!(!expanded_node.connections.is_empty());
        assert_eq!(expanded_node.connections[0].relation, "compiled_with");
    }
}
