//! Context orchestrator coordinating memory scoping, candidate retrieval, ranking, and snapshotting.

use brain_core::context::{
    ContextAssembler, ContextAssemblyRequest, ContextOrchestrator, ContextSnapshot, MemoryRanker,
    MemoryScope, RankedMemory, RetrievalCandidate,
};
use brain_core::errors::BrainError;
use brain_core::repositories::{EdgeRepository, NodeRepository};
use brain_core::retrieval::RetrievalRequest;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::context::assembler::DefaultContextAssembler;
use crate::context::ranker::DefaultMemoryRanker;
use crate::context::ALWAYS_ON_CONSTRAINT;
use crate::retrieval::source::SemanticMemorySource;

/// Default implementation of `ContextOrchestrator`.
#[derive(Clone)]
pub struct DefaultContextOrchestrator {
    ranker: Arc<dyn MemoryRanker>,
    assembler: Arc<dyn ContextAssembler>,
    node_repo: Option<Arc<dyn NodeRepository + Send + Sync>>,
    edge_repo: Option<Arc<dyn EdgeRepository + Send + Sync>>,
    semantic_source: Option<Arc<SemanticMemorySource>>,
    candidates: Arc<RwLock<Vec<RetrievalCandidate>>>,
    deleted_nodes: Arc<RwLock<std::collections::HashSet<brain_domain::NodeId>>>,
}

impl Default for DefaultContextOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultContextOrchestrator {
    /// Creates a new `DefaultContextOrchestrator` with default ranker and assembler.
    pub fn new() -> Self {
        Self {
            ranker: Arc::new(DefaultMemoryRanker::new()),
            assembler: Arc::new(DefaultContextAssembler::new()),
            node_repo: None,
            edge_repo: None,
            semantic_source: None,
            candidates: Arc::new(RwLock::new(Vec::new())),
            deleted_nodes: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    /// Creates an orchestrator with custom ranker and assembler.
    pub fn with_components(
        ranker: Arc<dyn MemoryRanker>,
        assembler: Arc<dyn ContextAssembler>,
    ) -> Self {
        Self {
            ranker,
            assembler,
            ..Self::new()
        }
    }

    /// Creates an orchestrator backed by production storage repositories.
    pub fn with_storage(
        node_repo: Arc<dyn NodeRepository + Send + Sync>,
        edge_repo: Arc<dyn EdgeRepository + Send + Sync>,
    ) -> Self {
        Self {
            node_repo: Some(node_repo),
            edge_repo: Some(edge_repo),
            ..Self::new()
        }
    }

    /// Creates an orchestrator with full control over storage and ranking components.
    pub fn with_all(
        node_repo: Option<Arc<dyn NodeRepository + Send + Sync>>,
        edge_repo: Option<Arc<dyn EdgeRepository + Send + Sync>>,
        ranker: Arc<dyn MemoryRanker>,
        assembler: Arc<dyn ContextAssembler>,
    ) -> Self {
        Self {
            ranker,
            assembler,
            node_repo,
            edge_repo,
            ..Self::new()
        }
    }

    /// Enables semantic recall using the configured vector source.
    pub fn with_semantic_source(mut self, source: Arc<SemanticMemorySource>) -> Self {
        self.semantic_source = Some(source);
        self
    }

    /// Normalizes raw SQLite FTS5 BM25 score to a monotonic float in [0.0, 1.0).
    ///
    /// Handles both pre-negated positive relevance scores (from SqliteStorage::find_by_fts)
    /// and raw negative SQLite bm25() floats. Applies the continuous saturation function
    /// `pos / (1.0 + pos)` mapping [0, inf) smoothly to [0.0, 1.0).
    pub fn normalize_bm25_score(score: f64) -> f64 {
        let pos = score.abs();
        if pos == 0.0 {
            0.0
        } else {
            (pos / (1.0 + pos)).clamp(0.0, 0.999)
        }
    }

    /// Registers a memory candidate into the orchestrator's in-memory index.
    pub async fn register_candidate(&self, candidate: RetrievalCandidate) {
        let mut list = self.candidates.write().await;
        list.push(candidate);
    }

    /// Marks a node as deleted/archived for immediate in-flight candidate exclusion.
    pub async fn mark_node_deleted(&self, node_id: brain_domain::NodeId) {
        let mut set = self.deleted_nodes.write().await;
        set.insert(node_id);
    }

    /// Removes deletion tombstone on a node (used for rollback on storage failure).
    pub async fn unmark_node_deleted(&self, node_id: brain_domain::NodeId) {
        let mut set = self.deleted_nodes.write().await;
        set.remove(&node_id);
    }

    /// Helper to evaluate candidate scope against request scope.
    fn is_in_scope(candidate_scope: &MemoryScope, request: &ContextAssemblyRequest) -> bool {
        match candidate_scope {
            MemoryScope::Session(sess_id) => sess_id == &request.session_id,
            MemoryScope::Workspace(ws_id) => {
                if let Some(req_ws) = &request.workspace_id {
                    ws_id == req_ws
                } else {
                    false
                }
            }
            MemoryScope::Global => true,
        }
    }

    fn visible_node_scope(
        node: &brain_domain::Node,
        request: &ContextAssemblyRequest,
    ) -> Option<MemoryScope> {
        let scope = match node.properties.get("scope") {
            None => MemoryScope::Global,
            Some(value) => match value.as_str()? {
                value if value.starts_with("session:") => {
                    MemoryScope::Session(value.strip_prefix("session:")?.parse().ok()?)
                }
                value if value.starts_with("workspace:") => {
                    MemoryScope::Workspace(value.strip_prefix("workspace:")?.to_owned())
                }
                value if value.contains(':') => return None,
                // Legacy unqualified labels carry no workspace or session identity.
                _ => MemoryScope::Global,
            },
        };
        Self::is_in_scope(&scope, request).then_some(scope)
    }

    async fn fetch_semantic_candidates(
        &self,
        request: &ContextAssemblyRequest,
    ) -> Result<Vec<RetrievalCandidate>, BrainError> {
        let Some(source) = self.semantic_source.clone() else {
            return Ok(Vec::new());
        };
        let retrieval = RetrievalRequest {
            session_id: request.session_id,
            query: request.prompt.clone(),
            limit: if request.max_items == 0 {
                10
            } else {
                request.max_items
            },
            exclude_ids: self.deleted_nodes.read().await.clone(),
            deadline: None,
            explain: false,
            graph_depth: Some(0),
            expand_relations: false,
            reference_time: request.reference_time,
        };
        let request = request.clone();
        tokio::task::spawn_blocking(move || {
            let nodes = source.retrieve_scored(&retrieval, false, |node| {
                Self::visible_node_scope(node, &request).is_some()
            })?;
            Ok(nodes
                .into_iter()
                .filter_map(|(node, score)| {
                    let scope = Self::visible_node_scope(&node, &request)?;
                    let content = Self::extract_node_content(&node);
                    Some(RetrievalCandidate {
                        node_id: node.id,
                        label: node.label,
                        content,
                        channel: brain_core::context::RetrievalChannel::Semantic,
                        raw_score: f64::from(score),
                        timestamp: node.updated_at,
                        scope,
                        metadata: Default::default(),
                    })
                })
                .collect())
        })
        .await
        .map_err(|error| BrainError::Internal {
            message: format!("Semantic context worker failed: {error}"),
        })?
    }

    /// Helper to extract content text from node properties.
    fn extract_node_content(node: &brain_domain::Node) -> String {
        if let Some(val) = node
            .properties
            .get("content")
            .or_else(|| node.properties.get("excerpt"))
        {
            if let Some(s) = val.as_str() {
                if !s.trim().is_empty() {
                    return s.to_string();
                }
            }
        }
        node.label.clone()
    }

    /// Fetches and filters candidates matching the request scope and query.
    async fn fetch_scoped_candidates(
        &self,
        request: &ContextAssemblyRequest,
    ) -> Result<Vec<RetrievalCandidate>, BrainError> {
        let (mut matched, semantic_error) = match self.fetch_semantic_candidates(request).await {
            Ok(candidates) => (candidates, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let deleted = self.deleted_nodes.read().await;
        matched.retain(|candidate| !deleted.contains(&candidate.node_id));
        let mut seen_nodes = std::collections::HashSet::new();

        // 1. Storage-backed retrieval via FTS
        if let Some(ref node_repo) = self.node_repo {
            let fts_results = node_repo.find_by_fts(&request.prompt)?;

            // Collect top parent candidates for bounded graph expansion
            let mut top_parents: Vec<(brain_domain::Node, f64)> = Vec::new();
            let max_expansion_parents = request.max_items.clamp(1, 5);

            for (node, bm25_score) in fts_results {
                if deleted.contains(&node.id) {
                    continue;
                }
                let Some(scope) = Self::visible_node_scope(&node, request) else {
                    continue;
                };

                let raw_score = Self::normalize_bm25_score(bm25_score);
                if raw_score <= 0.0 {
                    continue;
                }

                let content = Self::extract_node_content(&node);
                seen_nodes.insert(node.id);
                matched.push(RetrievalCandidate {
                    node_id: node.id,
                    label: node.label.clone(),
                    content,
                    channel: brain_core::context::RetrievalChannel::Lexical,
                    raw_score,
                    timestamp: node.updated_at,
                    scope,
                    metadata: std::collections::HashMap::new(),
                });

                if top_parents.len() < max_expansion_parents {
                    top_parents.push((node, raw_score));
                }
            }

            // 2. Bounded 1-hop graph neighbor expansion
            if let Some(ref edge_repo) = self.edge_repo {
                for (parent_node, parent_raw_score) in top_parents {
                    let connections = edge_repo.get_connections(&parent_node.id)?;
                    let mut sorted_edges = connections;
                    sorted_edges.sort_by(|a, b| {
                        b.weight
                            .total_cmp(&a.weight)
                            .then_with(|| a.target.to_string().cmp(&b.target.to_string()))
                    });

                    // Expand up to 5 neighbors per parent
                    for edge in sorted_edges.into_iter().take(5) {
                        let target_id = if edge.source == parent_node.id {
                            edge.target
                        } else {
                            edge.source
                        };

                        if seen_nodes.contains(&target_id) || deleted.contains(&target_id) {
                            continue;
                        }

                        if let Some(target_node) = node_repo.find_by_id(&target_id)? {
                            let Some(target_scope) =
                                Self::visible_node_scope(&target_node, request)
                            else {
                                continue;
                            };

                            // Decay weight: parent_raw_score * 0.5 * clamped_weight
                            let decayed_score =
                                parent_raw_score * 0.5 * edge.weight.clamp(0.0, 1.0);
                            if decayed_score <= 0.0 {
                                continue;
                            }

                            seen_nodes.insert(target_id);
                            let target_content = Self::extract_node_content(&target_node);
                            matched.push(RetrievalCandidate {
                                node_id: target_id,
                                label: target_node.label.clone(),
                                content: target_content,
                                channel: brain_core::context::RetrievalChannel::Graph,
                                raw_score: decayed_score,
                                timestamp: target_node.updated_at,
                                scope: target_scope,
                                metadata: std::collections::HashMap::from([(
                                    "relation".to_string(),
                                    edge.relation.to_string(),
                                )]),
                            });
                        }
                    }
                }
            }

            let mut constraints = std::collections::HashMap::new();
            for node in node_repo.list_context_constraints()? {
                if deleted.contains(&node.id) {
                    continue;
                }
                let Some(scope) = Self::visible_node_scope(&node, request) else {
                    continue;
                };
                let content = Self::extract_node_content(&node);
                seen_nodes.insert(node.id);
                constraints.insert(
                    node.id,
                    RetrievalCandidate {
                        node_id: node.id,
                        label: node.label,
                        content,
                        channel: brain_core::context::RetrievalChannel::Graph,
                        raw_score: 1.0,
                        timestamp: node.updated_at,
                        scope,
                        metadata: std::collections::HashMap::from([(
                            ALWAYS_ON_CONSTRAINT.to_string(),
                            "true".to_string(),
                        )]),
                    },
                );
            }
            matched.retain(|candidate| !constraints.contains_key(&candidate.node_id));
            matched.extend(constraints.into_values());
        }

        // 3. Merge in-memory registered candidates (e.g. from tests or dynamic runtime)
        seen_nodes.extend(matched.iter().map(|candidate| candidate.node_id));
        let memory_candidates = self.candidates.read().await;
        if !memory_candidates.is_empty() {
            let query_lower = request.prompt.to_lowercase();
            let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

            for item in memory_candidates.iter() {
                if deleted.contains(&item.node_id) || seen_nodes.contains(&item.node_id) {
                    continue;
                }
                if !Self::is_in_scope(&item.scope, request) {
                    continue;
                }

                let content_lower = item.content.to_lowercase();
                let label_lower = item.label.to_lowercase();

                let mut matched_terms = 0;
                for term in &query_terms {
                    if content_lower.contains(term) || label_lower.contains(term) {
                        matched_terms += 1;
                    }
                }

                let mut scored_candidate = item.clone();
                if !query_terms.is_empty() && matched_terms > 0 {
                    let overlap_ratio = (matched_terms as f64) / (query_terms.len() as f64);
                    scored_candidate.raw_score =
                        (item.raw_score * 0.5 + overlap_ratio * 0.5).min(1.0);
                    matched.push(scored_candidate);
                } else if query_terms.is_empty() {
                    matched.push(scored_candidate);
                }
            }
        }

        if let Some(error) = semantic_error {
            if matched.is_empty() {
                return Err(error);
            }
            tracing::warn!(%error, "Semantic context unavailable; using other retrieval channels");
        }
        Ok(matched)
    }
}

#[async_trait::async_trait]
impl ContextOrchestrator for DefaultContextOrchestrator {
    async fn assemble_context(
        &self,
        request: ContextAssemblyRequest,
    ) -> Result<ContextSnapshot, BrainError> {
        let reference_time = request.reference_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        // 1. Fetch scoped candidates with explicit storage error propagation
        let candidates = self.fetch_scoped_candidates(&request).await?;

        // 2. Rank and deduplicate candidates
        let ranked = self.ranker.rank_and_deduplicate(candidates, reference_time);

        // 3. Generate unique immutable epoch ID for this snapshot
        let epoch_id = format!("epoch-{}", uuid::Uuid::new_v4());

        // 4. Assemble context snapshot with strict budget clamping
        let max_tokens = if request.max_tokens == 0 {
            2048
        } else {
            request.max_tokens
        };
        let max_items = if request.max_items == 0 {
            10
        } else {
            request.max_items
        };

        let snapshot = self
            .assembler
            .assemble(&ranked, max_tokens, max_items, &epoch_id);
        Ok(snapshot)
    }

    async fn retrieve_memories(
        &self,
        request: ContextAssemblyRequest,
    ) -> Result<Vec<RankedMemory>, BrainError> {
        let reference_time = request.reference_time.unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });

        let candidates = self.fetch_scoped_candidates(&request).await?;
        let ranked = self.ranker.rank_and_deduplicate(candidates, reference_time);

        let max_items = if request.max_items == 0 {
            10
        } else {
            request.max_items
        };
        Ok(ranked.into_iter().take(max_items).collect())
    }

    async fn mark_node_deleted(&self, node_id: brain_domain::NodeId) {
        let mut set = self.deleted_nodes.write().await;
        set.insert(node_id);
    }

    async fn unmark_node_deleted(&self, node_id: brain_domain::NodeId) {
        let mut set = self.deleted_nodes.write().await;
        set.remove(&node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_bm25_score_semantics() {
        // Zero returns 0.0
        assert_eq!(DefaultContextOrchestrator::normalize_bm25_score(0.0), 0.0);

        // Positive or negative BM25 magnitudes produce strictly positive, increasing score
        let score_weak = DefaultContextOrchestrator::normalize_bm25_score(1.0);
        let score_weak_neg = DefaultContextOrchestrator::normalize_bm25_score(-1.0);
        let score_mid = DefaultContextOrchestrator::normalize_bm25_score(4.0);
        let score_strong = DefaultContextOrchestrator::normalize_bm25_score(9.0);
        let score_extreme = DefaultContextOrchestrator::normalize_bm25_score(99.0);

        assert_eq!(score_weak, 0.5); // 1.0 / 2.0
        assert_eq!(score_weak_neg, 0.5);
        assert_eq!(score_mid, 0.8); // 4.0 / 5.0
        assert_eq!(score_strong, 0.9); // 9.0 / 10.0
        assert!(score_weak < score_mid);
        assert!(score_mid < score_strong);
        assert!(score_strong < score_extreme);

        // Always strictly < 1.0
        assert!(score_extreme < 1.0);
    }
}
