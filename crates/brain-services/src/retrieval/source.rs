use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{MemorySource, MemorySourceResult, RetrievalRequest, SourceMetadata};
use brain_session::SessionCacheManager;
use std::sync::Arc;

pub(crate) fn calculate_token_overlap_score(node: &brain_domain::Node, query: &str) -> f32 {
    let query_tokens: std::collections::HashSet<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let label_tokens: std::collections::HashSet<String> = node
        .label
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let mut overlap = 0;
    for q_tok in &query_tokens {
        if label_tokens.contains(q_tok) {
            overlap += 1;
        }
    }
    overlap as f32
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
            let score = calculate_token_overlap_score(&node, &request.query);
            if score > 0.0 {
                candidates.insert(node.id, (node, score, idx));
            }
        }

        // ── Graph Expansion ───────────────────────────────────────────────────
        // graph_depth controls the traversal horizon. None = default depth 1
        // (v0.7-compatible). Some(0) = flat retrieval, skip expansion entirely.
        //
        // Multi-hop correctness: BFS returns edges in breadth-first order, so
        // depth-N edges reference depth-(N-1) nodes that may not yet be in
        // `candidates`. We maintain `all_known_scores` — a superset of
        // `candidates` that also tracks nodes found during this expansion pass —
        // so each edge can always find its parent's score.
        let depth = request.graph_depth.unwrap_or(1);
        let start_nodes: Vec<brain_domain::NodeId> = candidates.keys().cloned().collect();
        if depth > 0 && !start_nodes.is_empty() {
            let traversal_budget = crate::retrieval::graph_service::TraversalBudget {
                max_depth: depth,
                max_nodes: 50,
                max_edges: 100,
                prevent_cycles: true,
                deadline: request.deadline,
                ..Default::default()
            };

            // Seed the running score map from initial candidates.
            let mut all_known_scores: std::collections::HashMap<brain_domain::NodeId, f32> =
                candidates
                    .iter()
                    .map(|(&id, (_, score, _))| (id, *score))
                    .collect();
            let mut expansions: Vec<(brain_domain::NodeId, f32)> = Vec::new();

            if let Ok(connections) = crate::retrieval::graph_service::Graph.expand_neighbors(
                self.repos.as_ref(),
                self.registry.as_ref(),
                &start_nodes,
                &traversal_budget,
            ) {
                for edge in connections {
                    // Identify which endpoint is already known and which is new.
                    let (matched_id, neighbor_id) = if all_known_scores.contains_key(&edge.source) {
                        (edge.source, edge.target)
                    } else if all_known_scores.contains_key(&edge.target) {
                        (edge.target, edge.source)
                    } else {
                        continue;
                    };

                    if !all_known_scores.contains_key(&neighbor_id)
                        && !request.exclude_ids.contains(&neighbor_id)
                    {
                        let parent_score = all_known_scores[&matched_id];
                        let exp_score = parent_score * 0.5;
                        expansions.push((neighbor_id, exp_score));
                        // Register immediately so subsequent depth-N edges can
                        // use this node as their parent.
                        all_known_scores.insert(neighbor_id, exp_score);
                    }
                }
            }

            for (neighbor_id, exp_score) in expansions {
                if let Some((_, existing_score, _)) = candidates.get_mut(&neighbor_id) {
                    if exp_score > *existing_score {
                        *existing_score = exp_score;
                    }
                } else if let Ok(Some(neighbor_node)) = self.repos.nodes().find_by_id(&neighbor_id)
                {
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
    pub fn new(
        repos: Arc<dyn RepositorySet>,
        registry: Arc<brain_domain::RelationRegistry>,
    ) -> Self {
        Self { repos, registry }
    }
}

impl MemorySource for LtmMemorySource {
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        let start_lexical = std::time::Instant::now();
        let mut candidates = std::collections::HashMap::new();

        let fts_nodes = self.repos.nodes().find_by_fts(&request.query)?;
        for (node, score) in fts_nodes {
            if request.exclude_ids.contains(&node.id) {
                continue;
            }
            candidates.insert(node.id, (node, score as f32));
        }

        let lexical_duration = start_lexical.elapsed();
        tracing::info!(
            target: "brain::telemetry::retrieval",
            stage = "BM25",
            duration_ms = lexical_duration.as_millis(),
            candidate_count = candidates.len(),
            "Retrieval stage completed: BM25"
        );

        // ── Graph Expansion ───────────────────────────────────────────────────
        // graph_depth controls the traversal horizon. None = default depth 1
        // (v0.7-compatible). Some(0) = flat retrieval, skip expansion entirely.
        //
        // Multi-hop correctness: BFS returns edges in breadth-first order, so
        // depth-N edges reference depth-(N-1) nodes that may not yet be in
        // `candidates`. We maintain `all_known_scores` — a superset of
        // `candidates` that also tracks nodes found during this expansion pass —
        // so each edge can always find its parent's score.
        let depth = request.graph_depth.unwrap_or(1);
        let start_nodes: Vec<brain_domain::NodeId> = candidates.keys().cloned().collect();
        if depth > 0 && !start_nodes.is_empty() {
            let traversal_budget = crate::retrieval::graph_service::TraversalBudget {
                max_depth: depth,
                max_nodes: 50,
                max_edges: 100,
                prevent_cycles: true,
                deadline: request.deadline,
                ..Default::default()
            };

            let start_expand = std::time::Instant::now();
            // Seed the running score map from initial candidates.
            let mut all_known_scores: std::collections::HashMap<brain_domain::NodeId, f32> =
                candidates
                    .iter()
                    .map(|(&id, (_, score))| (id, *score))
                    .collect();
            let mut expansions: Vec<(brain_domain::NodeId, f32)> = Vec::new();
            let mut connections_count = 0;

            if let Ok(connections) = crate::retrieval::graph_service::Graph.expand_neighbors(
                self.repos.as_ref(),
                self.registry.as_ref(),
                &start_nodes,
                &traversal_budget,
            ) {
                connections_count = connections.len();
                for edge in connections {
                    // Identify which endpoint is already known and which is new.
                    let (matched_id, neighbor_id) = if all_known_scores.contains_key(&edge.source) {
                        (edge.source, edge.target)
                    } else if all_known_scores.contains_key(&edge.target) {
                        (edge.target, edge.source)
                    } else {
                        continue;
                    };

                    if !all_known_scores.contains_key(&neighbor_id)
                        && !request.exclude_ids.contains(&neighbor_id)
                    {
                        let parent_score = all_known_scores[&matched_id];
                        let exp_score = parent_score * 0.5;
                        expansions.push((neighbor_id, exp_score));
                        // Register immediately so subsequent depth-N edges can
                        // use this node as their parent.
                        all_known_scores.insert(neighbor_id, exp_score);
                    }
                }
            }

            for (neighbor_id, exp_score) in expansions {
                if let Some((_, existing_score)) = candidates.get_mut(&neighbor_id) {
                    if exp_score > *existing_score {
                        *existing_score = exp_score;
                    }
                } else if let Ok(Some(neighbor_node)) = self.repos.nodes().find_by_id(&neighbor_id)
                {
                    candidates.insert(neighbor_id, (neighbor_node, exp_score));
                }
            }
            let expand_duration = start_expand.elapsed();
            tracing::info!(
                target: "brain::telemetry::retrieval",
                stage = "graph_expansion",
                depth = depth,
                duration_ms = expand_duration.as_millis(),
                input_nodes_count = start_nodes.len(),
                found_connections_count = connections_count,
                "Retrieval stage completed: graph expansion"
            );
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

/// Predefined centroids for IVF vector partitioning.
fn get_predefined_centroids() -> &'static [Vec<f32>] {
    static CENTROIDS: std::sync::OnceLock<Vec<Vec<f32>>> = std::sync::OnceLock::new();
    CENTROIDS.get_or_init(|| {
        let mut centroids = Vec::with_capacity(8);
        for c in 0..8 {
            let mut v = vec![0.0f32; 384];
            let mut norm_sq = 0.0f32;
            for (i, slot) in v.iter_mut().enumerate() {
                let val = ((2.0 * std::f64::consts::PI * (i + 1) as f64 * (c + 1) as f64) / 384.0)
                    .sin() as f32;
                *slot = val;
                norm_sq += val * val;
            }
            let norm = norm_sq.sqrt();
            if norm > 0.0 {
                for val in v.iter_mut() {
                    *val /= norm;
                }
            }
            centroids.push(v);
        }
        centroids
    })
}

/// Helper to compute cosine similarity between two normalized vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

/// Semantic memory source implementing high-dimensional vector similarity retrieval.
pub struct SemanticMemorySource {
    repos: Arc<dyn RepositorySet>,
    query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
    /// Metric tracking IVF activations.
    pub activation_count: std::sync::atomic::AtomicUsize,
    /// Metric tracking IVF bypasses.
    pub bypass_count: std::sync::atomic::AtomicUsize,
    /// Metric tracking cosine computation count.
    pub cosine_computations: std::sync::atomic::AtomicUsize,
}

impl SemanticMemorySource {
    /// Creates a new `SemanticMemorySource`.
    pub fn new(
        repos: Arc<dyn RepositorySet>,
        query_embedding_service: Arc<dyn brain_core::retrieval::QueryEmbeddingService>,
    ) -> Self {
        Self {
            repos,
            query_embedding_service,
            activation_count: std::sync::atomic::AtomicUsize::new(0),
            bypass_count: std::sync::atomic::AtomicUsize::new(0),
            cosine_computations: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl MemorySource for SemanticMemorySource {
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        let start_vector = std::time::Instant::now();
        // 1. Check early for empty query
        if request.query.trim().is_empty() {
            return Ok(MemorySourceResult {
                nodes: vec![],
                metadata: SourceMetadata {
                    source_name: "SemanticMemorySource",
                },
            });
        }

        // 2. Generate embedding for query
        let query_vector = self.query_embedding_service.embed_query(&request.query)?;

        // 3. Fetch all embeddings to determine search strategy
        let all_embeddings = self.repos.embeddings().list_all_embeddings()?;
        let total_count = all_embeddings.len();

        let comps_this_query;
        let candidates = if total_count < 2000 {
            // Bypass IVF: Flat brute force scan
            self.bypass_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.cosine_computations
                .fetch_add(total_count, std::sync::atomic::Ordering::SeqCst);
            comps_this_query = total_count;
            tracing::info!(
                target: "brain::telemetry::retrieval",
                stage = "ivf_activation",
                ivf_active = false,
                probe_size = 0,
                target_centroids = 0,
                total_count = total_count,
                "Retrieval stage: IVF activation check"
            );
            all_embeddings
        } else {
            // IVF Probing: Query centroids and retrieve top 2 partition subsets
            self.activation_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let centroids = get_predefined_centroids();
            let mut centroid_similarities = Vec::with_capacity(8);
            for (c_id, centroid) in centroids.iter().enumerate() {
                let sim = cosine_similarity(&query_vector, centroid);
                centroid_similarities.push((c_id as i32, sim));
            }
            self.cosine_computations
                .fetch_add(8, std::sync::atomic::Ordering::SeqCst);

            centroid_similarities
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_2_centroid_ids = vec![centroid_similarities[0].0, centroid_similarities[1].0];

            let partitioned = self
                .repos
                .embeddings()
                .find_by_centroids(&top_2_centroid_ids)?;
            self.cosine_computations
                .fetch_add(partitioned.len(), std::sync::atomic::Ordering::SeqCst);
            comps_this_query = 8 + partitioned.len();
            tracing::info!(
                target: "brain::telemetry::retrieval",
                stage = "ivf_activation",
                ivf_active = true,
                probe_size = 2,
                target_centroids = 8,
                total_count = total_count,
                "Retrieval stage: IVF activation check"
            );
            partitioned
        };

        // 4. Calculate cosine similarity for candidates
        let mut node_scores = Vec::with_capacity(candidates.len());
        for emb in candidates {
            if request.exclude_ids.contains(&emb.node_id) {
                continue;
            }
            let sim = cosine_similarity(&query_vector, &emb.vector);
            if sim > 0.0 {
                node_scores.push((emb.node_id, sim));
            }
        }

        // 5. Sort candidates by score descending and limit results
        node_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let limited_candidates: Vec<_> = node_scores.into_iter().take(request.limit * 2).collect();

        // 6. Load matching nodes from database
        let mut nodes = Vec::with_capacity(limited_candidates.len());
        for (node_id, _) in limited_candidates {
            if let Some(node) = self.repos.nodes().find_by_id(&node_id)? {
                nodes.push(node);
            }
        }

        let duration = start_vector.elapsed();
        tracing::info!(
            target: "brain::telemetry::retrieval",
            stage = "vector",
            duration_ms = duration.as_millis(),
            candidate_count = nodes.len(),
            cosine_computations = comps_this_query,
            "Retrieval stage completed: vector"
        );

        Ok(MemorySourceResult {
            nodes,
            metadata: SourceMetadata {
                source_name: "SemanticMemorySource",
            },
        })
    }
}
