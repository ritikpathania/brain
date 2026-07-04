use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{MemorySource, MemorySourceResult, RetrievalRequest, SourceMetadata};
use brain_session::SessionCacheManager;
use std::sync::Arc;

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    if len1 == 0 { return len2; }
    if len2 == 0 { return len1; }

    let mut row: Vec<usize> = (0..=len2).collect();
    for (i, c1) in s1.chars().enumerate() {
        let mut prev = i + 1;
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            let val = std::cmp::min(
                row[j + 1] + 1,
                std::cmp::min(prev + 1, row[j] + cost),
            );
            row[j] = prev;
            prev = val;
        }
        row[len2] = prev;
    }
    row[len2]
}

fn word_similarity(q: &str, word: &str) -> f32 {
    let q_lower = q.to_lowercase();
    let w_lower = word.to_lowercase();
    if q_lower == w_lower {
        return 1.0;
    }
    if w_lower.contains(&q_lower) {
        return q_lower.len() as f32 / w_lower.len() as f32;
    }
    let dist = levenshtein_distance(&q_lower, &w_lower);
    let max_len = std::cmp::max(q_lower.len(), w_lower.len());
    if max_len > 0 {
        let sim = 1.0 - (dist as f32 / max_len as f32);
        if sim >= 0.7 {
            return sim;
        }
    }
    0.0
}

fn tokenize(text: &str) -> std::collections::HashSet<String> {
    let stop_words: std::collections::HashSet<&str> = [
        "a", "an", "the", "and", "or", "but", "is", "are", "was", "were", "to", "of", "in", "on",
        "at", "for", "with", "by", "about", "as", "this", "that", "these", "those", "it", "its",
        "you", "your", "my", "up", "down", "out", "off",
    ]
    .iter()
    .cloned()
    .collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty() && s.len() > 1 && !stop_words.contains(s))
        .map(|s| s.to_string())
        .collect()
}

pub(crate) fn calculate_node_match_score(node: &brain_domain::Node, query: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let label_lower = node.label.to_lowercase();
    let mut score = 0.0;

    // 1. Phrase Boosting
    if label_lower == query_lower {
        score += 150.0;
    } else if label_lower.contains(&query_lower) || query_lower.contains(&label_lower) {
        score += 80.0;
    }

    // 2. Token-level matching (OR semantics, partial, fuzzy)
    let query_tokens = tokenize(query);
    let label_tokens = tokenize(&node.label);

    for q_tok in &query_tokens {
        let mut best_sim = 0.0f32;
        for l_tok in &label_tokens {
            let sim = word_similarity(q_tok, l_tok);
            if sim > best_sim {
                best_sim = sim;
            }
        }
        // Also scan properties (string values)
        for val in node.properties.values() {
            if let serde_json::Value::String(s) = val {
                let prop_tokens = tokenize(s);
                for p_tok in prop_tokens {
                    let sim = word_similarity(q_tok, &p_tok);
                    if sim > best_sim {
                        best_sim = sim;
                    }
                }
            }
        }

        if best_sim > 0.0 {
            if best_sim == 1.0 {
                score += 20.0;
            } else {
                score += 10.0 * best_sim;
            }
        }
    }
    score
}

/// Short-term memory (STM) source querying the active session cache.
pub struct StmMemorySource {
    cache_manager: Arc<SessionCacheManager>,
    repos: Arc<dyn RepositorySet>,
    registry: Arc<brain_domain::RelationRegistry>,
}

impl StmMemorySource {
    /// Creates a new StmMemorySource.
    pub fn new(
        cache_manager: Arc<SessionCacheManager>,
        repos: Arc<dyn RepositorySet>,
        registry: Arc<brain_domain::RelationRegistry>,
    ) -> Self {
        Self {
            cache_manager,
            repos,
            registry,
        }
    }
}

impl MemorySource for StmMemorySource {
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        let context_lock = self.cache_manager.get_or_create(request.session_id);
        let context = context_lock.read().map_err(|e| BrainError::Internal {
            message: format!("Failed to acquire cache lock: {}", e),
        })?;

        let stm_nodes: Vec<brain_domain::Node> = context.iter().map(|n| n.node.clone()).collect();
        let mut candidates = std::collections::HashMap::new();

        for (idx, node) in stm_nodes.into_iter().enumerate() {
            if request.exclude_ids.contains(&node.id) {
                continue;
            }
            let score = calculate_node_match_score(&node, &request.query);
            if score > 0.0 {
                candidates.insert(node.id, (node, score, idx));
            }
        }

        // Graph Expansion using GraphTraversalService
        let start_nodes: Vec<brain_domain::NodeId> = candidates.keys().cloned().collect();
        let traversal_budget = crate::retrieval::graph_service::TraversalBudget {
            max_depth: 1,
            max_nodes: 50,
            max_edges: 100,
            prevent_cycles: true,
            deadline: request.deadline,
            ..Default::default()
        };

        let mut expansions = Vec::new();
        if let Ok(connections) = crate::retrieval::graph_service::Graph.expand_neighbors(
            self.repos.as_ref(),
            self.registry.as_ref(),
            &start_nodes,
            &traversal_budget,
        ) {
            for edge in connections {
                let (matched_id, neighbor_id) = if candidates.contains_key(&edge.source) {
                    (edge.source, edge.target)
                } else if candidates.contains_key(&edge.target) {
                    (edge.target, edge.source)
                } else {
                    continue;
                };

                if !candidates.contains_key(&neighbor_id) && !request.exclude_ids.contains(&neighbor_id) {
                    if let Some((_, parent_score, _)) = candidates.get(&matched_id) {
                        expansions.push((neighbor_id, parent_score * 0.5));
                    }
                }
            }
        }

        for (neighbor_id, exp_score) in expansions {
            if let Some((_, existing_score, _)) = candidates.get_mut(&neighbor_id) {
                if exp_score > *existing_score {
                    *existing_score = exp_score;
                }
            } else {
                if let Ok(Some(neighbor_node)) = self.repos.nodes().find_by_id(&neighbor_id) {
                    candidates.insert(neighbor_id, (neighbor_node, exp_score, usize::MAX));
                }
            }
        }

        let mut sorted_candidates: Vec<_> = candidates.into_values().collect();
        sorted_candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.2.cmp(&b.2))
        });

        let nodes = sorted_candidates.into_iter().map(|(n, _, _)| n).collect();

        Ok(MemorySourceResult {
            nodes,
            metadata: SourceMetadata {
                source_name: "StmMemorySource",
            },
        })
    }
}

/// Long-term memory (LTM) source querying the database.
pub struct LtmMemorySource {
    repos: Arc<dyn RepositorySet>,
    registry: Arc<brain_domain::RelationRegistry>,
}

impl LtmMemorySource {
    /// Creates a new LtmMemorySource.
    pub fn new(repos: Arc<dyn RepositorySet>, registry: Arc<brain_domain::RelationRegistry>) -> Self {
        Self { repos, registry }
    }
}

impl MemorySource for LtmMemorySource {
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        let db_nodes = self.repos.nodes().list_all()?;
        let mut candidates = std::collections::HashMap::new();

        for node in db_nodes {
            if request.exclude_ids.contains(&node.id) {
                continue;
            }
            let score = calculate_node_match_score(&node, &request.query);
            if score > 0.0 {
                candidates.insert(node.id, (node, score));
            }
        }

        // Graph Expansion using GraphTraversalService
        let start_nodes: Vec<brain_domain::NodeId> = candidates.keys().cloned().collect();
        let traversal_budget = crate::retrieval::graph_service::TraversalBudget {
            max_depth: 1,
            max_nodes: 50,
            max_edges: 100,
            prevent_cycles: true,
            deadline: request.deadline,
            ..Default::default()
        };

        let mut expansions = Vec::new();
        if let Ok(connections) = crate::retrieval::graph_service::Graph.expand_neighbors(
            self.repos.as_ref(),
            self.registry.as_ref(),
            &start_nodes,
            &traversal_budget,
        ) {
            for edge in connections {
                let (matched_id, neighbor_id) = if candidates.contains_key(&edge.source) {
                    (edge.source, edge.target)
                } else if candidates.contains_key(&edge.target) {
                    (edge.target, edge.source)
                } else {
                    continue;
                };

                if !candidates.contains_key(&neighbor_id) && !request.exclude_ids.contains(&neighbor_id) {
                    if let Some((_, parent_score)) = candidates.get(&matched_id) {
                        expansions.push((neighbor_id, parent_score * 0.5));
                    }
                }
            }
        }

        for (neighbor_id, exp_score) in expansions {
            if let Some((_, existing_score)) = candidates.get_mut(&neighbor_id) {
                if exp_score > *existing_score {
                    *existing_score = exp_score;
                }
            } else {
                if let Ok(Some(neighbor_node)) = self.repos.nodes().find_by_id(&neighbor_id) {
                    candidates.insert(neighbor_id, (neighbor_node, exp_score));
                }
            }
        }

        let mut sorted_candidates: Vec<_> = candidates.into_values().collect();
        sorted_candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.id.0.cmp(&b.0.id.0))
        });

        let nodes = sorted_candidates.into_iter().map(|(n, _)| n).collect();

        Ok(MemorySourceResult {
            nodes,
            metadata: SourceMetadata {
                source_name: "LtmMemorySource",
            },
        })
    }
}
