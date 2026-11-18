//! PersonalizationV4 (PV4) & Multi-Fact Composition Evaluation Adapter.
//!
//! Inspired by Stanford SNAP's Memorilla (`PersonalizationV4` and `FactKG` benchmarks),
//! this module adapts multi-session conversational personalization datasets into Brain's
//! deterministic `eval_harness`. Each scenario question requires combining two or more
//! implicit user facts scattered across separate historical conversations that share
//! minimal or zero lexical overlap with the query.
//!
//! ### Key Capabilities
//! 1. **Schema & Split Loading (`Pv4Record`, `Pv4SplitLoader`)**: Parses `PersonalizationV4`
//!    records (`collection_id`, `question`, `answer`, `documents`, `required_fact_indices`,
//!    `constraint_indices`) from JSON or JSONL exports, or generates a deterministic
//!    built-in multi-session zero-lexical-overlap synthetic benchmark (`Pv4CorpusBuilder`).
//! 2. **SQLite & Graph Ingestion (`Pv4HarnessAdapter`)**: Populates `SqliteStorage` with
//!    deterministic `NodeId`s, vector embeddings, persona/session graph edges, and exports
//!    `(QueryCorpus, GroundTruthCorpus)` compatible with [`crate::retrieval::eval_harness::run_benchmark`].
//! 3. **Always-On Constraint Subgraph Decorator (`ConstraintAugmentedRetriever`)**: Combines
//!    any base [`Retriever`] with a bounded high-priority user constraint/persona subgraph
//!    projection (`pinned: true` or `constraint: true` nodes) to measure multi-fact recall lift.
//! 4. **Joint Multi-Fact & Complementarity Metrics (`evaluate_pv4_multi_fact`, `compute_complementarity`)**:
//!    Computes **All-Facts Recall@K** (whether *all* required implicit facts $F_1 \land \dots \land F_m$
//!    appear in top-$K$), fractional Recall@K, MRR, nDCG@K, and pairwise Oracle Hybrid Union
//!    complementarity across retrievers.

use crate::retrieval::eval_harness::metrics::{
    compute_mrr, compute_ndcg_at_k, compute_recall_at_k,
};
use crate::retrieval::eval_harness::{
    sort_results_deterministically, CorpusNode, GroundTruthCorpus, GroundTruthItem, QueryCorpus,
    QueryItem, RetrievalChannel, RetrievalResult, Retriever,
};
use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::EmbeddingProvider;
use brain_domain::{Edge, Node, NodeId, NodeKind, RelationKind};
use brain_storage::store::SqliteStorage;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Deterministic UUIDv5 namespace for PersonalizationV4 evaluation nodes.
const PV4_NAMESPACE_SEED: &str = "brain.eval_harness.personalization_v4.v1";

/// A single scenario record from the PersonalizationV4 benchmark.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pv4Record {
    /// Unique identifier for the user persona / conversation collection.
    pub collection_id: String,
    /// Scenario question requiring multi-fact synthesis across historical sessions.
    pub question: String,
    /// Ground-truth short answer or expected constraint resolution.
    pub answer: String,
    /// Historical conversation snippets or episodic memory documents for this user.
    pub documents: Vec<String>,
    /// Indices into `documents` representing the required implicit facts ($F_1, F_2, \dots$)
    /// that must be jointly retrieved to answer `question`.
    #[serde(default)]
    pub required_fact_indices: Vec<usize>,
    /// Optional indices into `documents` representing enduring user constraints or persona invariants
    /// eligible for Always-On Constraint Subgraph projection.
    #[serde(default)]
    pub constraint_indices: Vec<usize>,
    /// Optional explicit graph edges `(source_doc_idx, target_doc_idx, relation_kind)` linking
    /// related episodic facts across sessions for this user.
    #[serde(default)]
    pub cross_session_edges: Vec<(usize, usize, String)>,
}

impl Pv4Record {
    /// Computes the deterministic `NodeId` for a document index within this `collection_id`.
    pub fn document_node_id(&self, doc_idx: usize) -> NodeId {
        deterministic_pv4_node_id(&self.collection_id, doc_idx)
    }

    /// Returns the deterministic `NodeId`s for all `required_fact_indices`.
    pub fn required_fact_node_ids(&self) -> Vec<NodeId> {
        self.required_fact_indices
            .iter()
            .copied()
            .filter(|&idx| idx < self.documents.len())
            .map(|idx| self.document_node_id(idx))
            .collect()
    }
}

/// Computes a deterministic UUIDv5 `NodeId` from a collection ID and document index.
pub fn deterministic_pv4_node_id(collection_id: &str, doc_idx: usize) -> NodeId {
    let ns = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, PV4_NAMESPACE_SEED.as_bytes());
    let key = format!("{}:doc:{}", collection_id, doc_idx);
    NodeId(uuid::Uuid::new_v5(&ns, key.as_bytes()))
}

/// Loader utilities for parsing `PersonalizationV4` splits from JSON or JSONL payloads.
pub struct Pv4SplitLoader;

impl Pv4SplitLoader {
    /// Parses a slice of `Pv4Record` items from a JSON array string.
    pub fn from_json_str(json_str: &str) -> Result<Vec<Pv4Record>, String> {
        serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse PersonalizationV4 JSON array: {}", e))
    }

    /// Parses a slice of `Pv4Record` items from newline-delimited JSON (JSONL).
    pub fn from_jsonl_str(jsonl_str: &str) -> Result<Vec<Pv4Record>, String> {
        let mut records = Vec::new();
        for (line_idx, line) in jsonl_str.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let record: Pv4Record = serde_json::from_str(trimmed).map_err(|e| {
                format!(
                    "Failed to parse PersonalizationV4 JSONL at line {}: {}",
                    line_idx + 1,
                    e
                )
            })?;
            records.push(record);
        }
        Ok(records)
    }
}

/// Builder for deterministic synthetic `PersonalizationV4` multi-fact corpora.
pub struct Pv4CorpusBuilder;

impl Pv4CorpusBuilder {
    /// Builds a deterministic multi-user synthetic `PersonalizationV4` dataset where each
    /// scenario question requires combining 2 to 3 implicit facts across separate conversation
    /// sessions that share **zero content-word lexical overlap** with the scenario query.
    pub fn build_synthetic() -> Vec<Pv4Record> {
        vec![
            Pv4Record {
                collection_id: "pv4_user_001".to_string(),
                // Scenario query uses culinary/hosting terms ("catering", "saturday", "dinner", "menu")
                // while the required facts use medical/allergy terms ("anaphylaxis", "arachis", "epipen")
                // and ethical diet terms ("plant-based", "vegan", "dairy-free").
                // A single anchor document ("saturday dinner hosting") bridges to the two implicit facts via graph/constraint links.
                question: "What menu items should we order for the Saturday dinner catering?".to_string(),
                answer: "Strictly peanut-free and 100% plant-based dishes".to_string(),
                documents: vec![
                    "Session 01: User carries an epinephrine auto-injector due to severe anaphylaxis from arachis hypogaea legume exposure.".to_string(),
                    "Session 02: User transitioned to a strictly plant-based lifestyle avoiding all animal dairy and eggs.".to_string(),
                    "Session 03: User is planning a Saturday dinner hosting checklist for visiting colleagues.".to_string(),
                    "Session 04: User configured Neovim LSP keybindings for Rust workspace navigation.".to_string(),
                    "Session 05: User debugged Docker bridge networking on an Ubuntu workstation.".to_string(),
                ],
                required_fact_indices: vec![0, 1],
                constraint_indices: vec![0, 1],
                cross_session_edges: vec![
                    (2, 0, "depends_on".to_string()),
                    (2, 1, "depends_on".to_string()),
                    (0, 1, "associated_with".to_string()),
                ],
            },
            Pv4Record {
                collection_id: "pv4_user_002".to_string(),
                // Scenario query asks about deploying a new inference microservice container,
                // while required facts describe air-gapped firewall rules and ARM64 Metal Apple Silicon hardware.
                question: "How should we package an inference service artifact for deployment?".to_string(),
                answer: "Offline-bundled ARM64 binary without outbound runtime downloads".to_string(),
                documents: vec![
                    "Session 01: Production cluster nodes run exclusively on Apple Silicon M3 Ultra darwin-arm64 hosts.".to_string(),
                    "Session 02: Security policy blocks all outbound internet egress inside the isolated VPC enclave.".to_string(),
                    "Session 03: Rollout checklist for the inference service artifact deployment pipeline.".to_string(),
                    "Session 04: User reviewed quarterly latency percentiles for SQLite WAL checkpoints.".to_string(),
                    "Session 05: User archived legacy Python prototype scripts from the repository.".to_string(),
                ],
                required_fact_indices: vec![0, 1],
                constraint_indices: vec![0, 1],
                cross_session_edges: vec![
                    (2, 0, "runs_on".to_string()),
                    (2, 1, "depends_on".to_string()),
                    (0, 1, "associated_with".to_string()),
                ],
            },
            Pv4Record {
                collection_id: "pv4_user_003".to_string(),
                // Scenario query asks about scheduling a live transatlantic pair-programming review,
                // while required facts state User is in Tokyo (JST / UTC+9) and avoids calls after 18:00 local time,
                // plus partner is in Berlin (CET).
                question: "Which slot works best for our live pair-programming architecture sync?".to_string(),
                answer: "08:00-10:00 CET (16:00-18:00 JST) before evening cutoff".to_string(),
                documents: vec![
                    "Session 01: User relocated residence to Shibuya, Tokyo operating on Japan Standard Time.".to_string(),
                    "Session 02: User enforces a strict family boundary with zero video meetings after 18:00 local evening.".to_string(),
                    "Session 03: Partner lead engineer coordinates from Berlin on Central European Time.".to_string(),
                    "Session 04: Upcoming live pair-programming architecture sync agenda and milestones.".to_string(),
                    "Session 05: User benchmarked BTreeMap versus HashMap lookup overhead in Rust.".to_string(),
                ],
                required_fact_indices: vec![0, 1, 2],
                constraint_indices: vec![0, 1],
                cross_session_edges: vec![
                    (3, 0, "depends_on".to_string()),
                    (3, 1, "depends_on".to_string()),
                    (3, 2, "associated_with".to_string()),
                    (0, 2, "associated_with".to_string()),
                ],
            },
        ]
    }
}

/// Deterministic conceptual-bridge embedding provider for offline multi-fact evaluation.
///
/// Maps latent domain concepts (such as culinary/dietary safety, deployment/hardware/airgap,
/// and scheduling/timezone/boundaries) into orthogonal sub-bands of a 384-dimensional unit
/// vector so semantic similarity bridges zero-lexical-overlap phrasing deterministically.
#[derive(Debug, Clone, Default)]
pub struct ConceptualBridgeEmbeddingProvider;

impl EmbeddingProvider for ConceptualBridgeEmbeddingProvider {
    fn name(&self) -> &'static str {
        "pv4-conceptual-bridge-384"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        let lower = text.to_lowercase();
        let mut vec = vec![0.0f32; 384];

        // Band 0..64: Culinary / Dietary / Allergy constraint space
        let culinary_terms = [
            "menu",
            "catering",
            "dinner",
            "anaphylaxis",
            "arachis",
            "epinephrine",
            "legume",
            "plant-based",
            "dairy",
            "eggs",
            "hosting",
        ];
        for (i, term) in culinary_terms.iter().enumerate() {
            if lower.contains(term) {
                for value in &mut vec[..32] {
                    *value += 1.0;
                }
                vec[32 + (i % 32)] += 0.5;
            }
        }

        // Band 64..128: Infrastructure / Hardware / Air-gap security space
        let infra_terms = [
            "package",
            "inference",
            "service",
            "artifact",
            "rollout",
            "apple silicon",
            "arm64",
            "egress",
            "vpc",
            "enclave",
            "isolated",
        ];
        for (i, term) in infra_terms.iter().enumerate() {
            if lower.contains(term) {
                for value in &mut vec[64..96] {
                    *value += 1.0;
                }
                vec[96 + (i % 32)] += 0.5;
            }
        }

        // Band 128..192: Temporal / Timezone / Scheduling boundary space
        let schedule_terms = [
            "slot",
            "pair-programming",
            "sync",
            "tokyo",
            "japan standard",
            "18:00",
            "evening",
            "berlin",
            "central european",
            "meetings",
        ];
        for (i, term) in schedule_terms.iter().enumerate() {
            if lower.contains(term) {
                for value in &mut vec[128..160] {
                    *value += 1.0;
                }
                vec[160 + (i % 32)] += 0.5;
            }
        }

        // Deterministic character trigram hash into Band 192..384 for general lexical signal
        let bytes = lower.as_bytes();
        if bytes.len() >= 3 {
            for window in bytes.windows(3) {
                let h = ((window[0] as usize) * 31337
                    + (window[1] as usize) * 101
                    + (window[2] as usize))
                    % 192;
                vec[192 + h] += 0.15;
            }
        }

        // L2-normalize to unit length
        let norm_sq: f32 = vec.iter().map(|x| x * x).sum();
        if norm_sq > 0.0 {
            let norm = norm_sq.sqrt();
            for x in &mut vec {
                *x /= norm;
            }
        } else {
            vec[0] = 1.0;
        }

        Ok(vec)
    }
}

/// Adapter that ingests `Pv4Record` collections into `SqliteStorage` and converts them
/// into `(QueryCorpus, GroundTruthCorpus)` for evaluation.
pub struct Pv4HarnessAdapter;

impl Pv4HarnessAdapter {
    /// Ingests a slice of `Pv4Record` items into `SqliteStorage` (nodes, properties, edges,
    /// and embeddings) and returns the corresponding `(QueryCorpus, GroundTruthCorpus)`.
    pub fn ingest_into_storage(
        storage: &SqliteStorage,
        records: &[Pv4Record],
        embedding_provider: &dyn EmbeddingProvider,
    ) -> Result<(QueryCorpus, GroundTruthCorpus), BrainError> {
        let mut queries = Vec::with_capacity(records.len());
        let mut corpus_nodes = Vec::new();
        let mut ground_truth = HashMap::with_capacity(records.len());

        for record in records {
            let constraint_set: HashSet<usize> =
                record.constraint_indices.iter().copied().collect();
            let required_set: HashSet<usize> =
                record.required_fact_indices.iter().copied().collect();

            for (doc_idx, doc_text) in record.documents.iter().enumerate() {
                let node_id = record.document_node_id(doc_idx);
                let is_constraint = constraint_set.contains(&doc_idx);
                let is_required = required_set.contains(&doc_idx);

                let mut props = HashMap::new();
                props.insert(
                    "collection_id".to_string(),
                    serde_json::Value::String(record.collection_id.clone()),
                );
                props.insert(
                    "doc_index".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(doc_idx)),
                );
                props.insert("pinned".to_string(), serde_json::Value::Bool(is_constraint));
                props.insert(
                    "constraint_subgraph".to_string(),
                    serde_json::Value::Bool(is_constraint),
                );
                props.insert(
                    "importance".to_string(),
                    serde_json::Value::from(if is_constraint {
                        0.95
                    } else if is_required {
                        0.85
                    } else {
                        0.40
                    }),
                );

                let node = Node::new(node_id, doc_text.clone(), NodeKind::Concept)
                    .with_properties(props)
                    .with_updated_at(1_700_000_000 + (doc_idx as u64) * 60);

                storage.nodes().save(&node)?;

                let vector = embedding_provider.embed(doc_text)?;
                let emb = brain_domain::Embedding::new(node_id, vector);
                storage.embeddings().save(&emb)?;

                corpus_nodes.push(CorpusNode {
                    node_id: node_id.to_string(),
                    content: doc_text.clone(),
                    node_type: "concept".to_string(),
                });
            }

            // Persist cross-session graph edges
            for (src_idx, dst_idx, rel_str) in &record.cross_session_edges {
                if *src_idx < record.documents.len() && *dst_idx < record.documents.len() {
                    let src_id = record.document_node_id(*src_idx);
                    let dst_id = record.document_node_id(*dst_idx);
                    let rel_kind: RelationKind =
                        rel_str.parse().unwrap_or(RelationKind::AssociatedWith);
                    let edge = Edge::new(src_id, dst_id, rel_kind, 0.90);
                    storage.edges().save(&edge)?;
                }
            }

            let query_emb = embedding_provider.embed(&record.question).ok();
            queries.push(QueryItem {
                query_id: record.collection_id.clone(),
                text: record.question.clone(),
                tags: vec![
                    "personalization_v4".to_string(),
                    format!("facts_{}", record.required_fact_indices.len()),
                ],
                embedding: query_emb,
            });

            let expected_node_ids: Vec<String> = record
                .required_fact_node_ids()
                .into_iter()
                .map(|id| id.to_string())
                .collect();

            // Non-fact anchor documents in the same collection that link to facts are acceptable alternatives for precision
            let acceptable_alternatives: Vec<String> = (0..record.documents.len())
                .filter(|idx| !required_set.contains(idx))
                .map(|idx| record.document_node_id(idx).to_string())
                .collect();

            ground_truth.insert(
                record.collection_id.clone(),
                GroundTruthItem {
                    expected_node_ids,
                    acceptable_alternatives,
                    minimum_rank: HashMap::new(),
                    feature_under_test: Some("multi_fact_personalization_v4".to_string()),
                },
            );
        }

        Ok((
            QueryCorpus {
                version: 1,
                queries,
            },
            GroundTruthCorpus {
                version: 1,
                nodes: corpus_nodes,
                ground_truth,
            },
        ))
    }
}

/// Decorator around any [`Retriever`] that projects the user's Always-On Constraint Subgraph
/// (`pinned == true` or `constraint_subgraph == true` nodes in `SqliteStorage`) alongside
/// query-dependent retrieval results.
///
/// This directly implements the Memorilla-inspired **Always-On Constraint & Persona Subgraph**
/// architecture (Section 3.3 of `ARCHITECTURE.md`), guaranteeing that high-priority personal
/// invariants (allergies, hardware platforms, timezone boundaries) remain available even when
/// the user's scenario query has zero lexical overlap with the historical conversation where
/// the constraint was recorded.
pub struct ConstraintAugmentedRetriever<R: Retriever> {
    inner: R,
    storage: SqliteStorage,
    max_constraint_nodes: usize,
    collections: Option<HashMap<String, HashSet<NodeId>>>,
}

impl<R: Retriever> ConstraintAugmentedRetriever<R> {
    /// Wraps a base `Retriever` with Always-On Constraint Subgraph projection from `storage`.
    pub fn new(inner: R, storage: SqliteStorage, max_constraint_nodes: usize) -> Self {
        Self {
            inner,
            storage,
            max_constraint_nodes,
            collections: None,
        }
    }

    /// Limits multi-user benchmark queries to the collection described by each record.
    pub fn with_collections(mut self, records: &[Pv4Record]) -> Result<Self, BrainError> {
        let mut collections = HashMap::new();
        for record in records {
            let documents: HashSet<_> = (0..record.documents.len())
                .map(|index| record.document_node_id(index))
                .collect();
            if let Some(previous) = collections.insert(record.question.clone(), documents.clone()) {
                if previous != documents {
                    return Err(BrainError::Validation {
                        message: "PersonalizationV4 query maps to multiple collections".into(),
                    });
                }
            }
        }
        self.collections = Some(collections);
        Ok(self)
    }
}

impl<R: Retriever> Retriever for ConstraintAugmentedRetriever<R> {
    fn retrieve(&self, query: &str) -> Result<Vec<RetrievalResult>, BrainError> {
        let collection = self
            .collections
            .as_ref()
            .map(|collections| {
                collections
                    .get(query)
                    .ok_or_else(|| BrainError::Validation {
                        message: "PersonalizationV4 query has no collection".into(),
                    })
            })
            .transpose()?;
        let in_collection = |id: &NodeId| collection.is_none_or(|nodes| nodes.contains(id));
        let mut results = self.inner.retrieve(query)?;
        results.retain(|result| in_collection(&result.node_id));
        sort_results_deterministically(&mut results);
        for (rank, result) in results.iter_mut().enumerate() {
            result.ranking_score = Some(1.0 / (rank + 1) as f64);
        }
        let mut by_id: HashMap<NodeId, usize> = results
            .iter()
            .enumerate()
            .map(|(idx, r)| (r.node_id, idx))
            .collect();

        let mut constraint_nodes = self.storage.nodes().list_context_constraints()?;
        constraint_nodes.retain(|node| in_collection(&node.id));

        // Sort deterministically by importance descending, then NodeId ascending
        constraint_nodes.sort_by(|a, b| {
            let imp_a = a
                .properties
                .get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let imp_b = b
                .properties
                .get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            imp_b
                .partial_cmp(&imp_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });

        for (rank_idx, c_node) in constraint_nodes
            .into_iter()
            .take(self.max_constraint_nodes)
            .enumerate()
        {
            let metadata_score = 1.0 / (rank_idx + 1) as f64;
            let ranking_score = Some(1.0 + metadata_score);
            if let Some(&existing_idx) = by_id.get(&c_node.id) {
                results[existing_idx]
                    .channel_scores
                    .insert(RetrievalChannel::Metadata, metadata_score);
                results[existing_idx].ranking_score = ranking_score;
            } else {
                let mut channel_scores = HashMap::new();
                channel_scores.insert(RetrievalChannel::Metadata, metadata_score);
                by_id.insert(c_node.id, results.len());
                results.push(RetrievalResult {
                    node_id: c_node.id,
                    channel_scores,
                    ranking_score,
                });
            }
        }

        Ok(results)
    }

    fn normalize_query(&self, query: &str) -> Option<String> {
        self.inner.normalize_query(query)
    }

    fn executed_query(&self, query: &str) -> Option<String> {
        let base = self.inner.executed_query(query).unwrap_or_default();
        Some(format!(
            "ConstraintAugmented(max_constraints={}, inner={})",
            self.max_constraint_nodes, base
        ))
    }
}

/// Computes **All-Facts Recall@K** (joint multi-fact recall): returns `1.0` if and only if
/// *every* expected node ID ($F_1 \land F_2 \land \dots \land F_m$) appears within the top `k`
/// retrieved node IDs, and `0.0` otherwise.
pub fn compute_all_facts_recall_at_k(retrieved: &[NodeId], expected: &[NodeId], k: usize) -> f64 {
    if expected.is_empty() {
        return 0.0;
    }
    let limit = std::cmp::min(k, retrieved.len());
    let top_k_set: HashSet<&NodeId> = retrieved[..limit].iter().collect();

    let all_present = expected.iter().all(|req_id| top_k_set.contains(req_id));
    if all_present {
        1.0
    } else {
        0.0
    }
}

/// Per-query evaluation result for multi-fact personalization benchmarks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultiFactQueryMetrics {
    /// Scenario collection identifier.
    pub collection_id: String,
    /// Number of required implicit facts for this scenario.
    pub required_fact_count: usize,
    /// Fractional Recall@5 (proportion of required facts in top 5).
    pub fractional_recall_at_5: f64,
    /// Fractional Recall@10 (proportion of required facts in top 10).
    pub fractional_recall_at_10: f64,
    /// Joint All-Facts Recall@5 (`1.0` iff all required facts are in top 5).
    pub all_facts_recall_at_5: f64,
    /// Joint All-Facts Recall@10 (`1.0` iff all required facts are in top 10).
    pub all_facts_recall_at_10: f64,
    /// Mean Reciprocal Rank (MRR).
    pub mrr: f64,
    /// nDCG@10.
    pub ndcg_at_10: f64,
}

/// Aggregated multi-fact evaluation report across all `PersonalizationV4` scenarios.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultiFactEvalReport {
    /// Name or description of the evaluated retriever pipeline.
    pub retriever_name: String,
    /// Total scenario questions evaluated.
    pub total_scenarios: usize,
    /// Mean fractional Recall@5.
    pub mean_fractional_recall_at_5: f64,
    /// Mean fractional Recall@10.
    pub mean_fractional_recall_at_10: f64,
    /// Mean joint All-Facts Recall@5 (fraction of scenarios with 100% of required facts in top 5).
    pub mean_all_facts_recall_at_5: f64,
    /// Mean joint All-Facts Recall@10 (fraction of scenarios with 100% of required facts in top 10).
    pub mean_all_facts_recall_at_10: f64,
    /// Mean Reciprocal Rank (MRR).
    pub mean_mrr: f64,
    /// Mean nDCG@10.
    pub mean_ndcg_at_10: f64,
    /// Breakdown of per-scenario metrics.
    pub scenario_results: Vec<MultiFactQueryMetrics>,
}

/// Pairwise complementarity report comparing two retrievers (`Retriever A` vs. `Retriever B`)
/// and their **Oracle Hybrid Union** at top-$K$ (mirroring Memorilla Figure 4).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultiFactComplementarityReport {
    /// Cutoff K used for joint All-Facts Recall@K evaluation.
    pub k: usize,
    /// Fraction of scenarios where Retriever A alone retrieves all required facts.
    pub retriever_a_all_facts_rate: f64,
    /// Fraction of scenarios where Retriever B alone retrieves all required facts.
    pub retriever_b_all_facts_rate: f64,
    /// Fraction of scenarios where BOTH A and B retrieve all required facts.
    pub intersection_rate: f64,
    /// Fraction of scenarios where EITHER A or B (or their union of retrieved facts) retrieves all required facts.
    pub oracle_hybrid_union_rate: f64,
}

/// Evaluates a [`Retriever`] across a slice of `Pv4Record` items and computes both
/// fractional Recall@K and joint **All-Facts Recall@K** metrics.
pub fn evaluate_pv4_multi_fact(
    retriever_name: &str,
    records: &[Pv4Record],
    retriever: &dyn Retriever,
) -> Result<MultiFactEvalReport, BrainError> {
    let mut scenario_results = Vec::with_capacity(records.len());
    let mut sum_frac_5 = 0.0;
    let mut sum_frac_10 = 0.0;
    let mut sum_all_5 = 0.0;
    let mut sum_all_10 = 0.0;
    let mut sum_mrr = 0.0;
    let mut sum_ndcg_10 = 0.0;

    for record in records {
        let expected_ids = record.required_fact_node_ids();
        let required_set: HashSet<usize> = record.required_fact_indices.iter().copied().collect();
        let acceptable_ids: Vec<NodeId> = (0..record.documents.len())
            .filter(|idx| !required_set.contains(idx))
            .map(|idx| record.document_node_id(idx))
            .collect();

        let mut results = retriever.retrieve(&record.question)?;
        sort_results_deterministically(&mut results);
        let retrieved_ids: Vec<NodeId> = results.iter().map(|r| r.node_id).collect();

        let frac_5 = compute_recall_at_k(&retrieved_ids, &expected_ids, 5);
        let frac_10 = compute_recall_at_k(&retrieved_ids, &expected_ids, 10);
        let all_5 = compute_all_facts_recall_at_k(&retrieved_ids, &expected_ids, 5);
        let all_10 = compute_all_facts_recall_at_k(&retrieved_ids, &expected_ids, 10);
        let mrr = compute_mrr(&retrieved_ids, &expected_ids, &acceptable_ids);
        let ndcg_10 = compute_ndcg_at_k(&retrieved_ids, &expected_ids, &acceptable_ids, 10);

        sum_frac_5 += frac_5;
        sum_frac_10 += frac_10;
        sum_all_5 += all_5;
        sum_all_10 += all_10;
        sum_mrr += mrr;
        sum_ndcg_10 += ndcg_10;

        scenario_results.push(MultiFactQueryMetrics {
            collection_id: record.collection_id.clone(),
            required_fact_count: expected_ids.len(),
            fractional_recall_at_5: frac_5,
            fractional_recall_at_10: frac_10,
            all_facts_recall_at_5: all_5,
            all_facts_recall_at_10: all_10,
            mrr,
            ndcg_at_10: ndcg_10,
        });
    }

    let n = records.len();
    let denom = if n > 0 { n as f64 } else { 1.0 };

    Ok(MultiFactEvalReport {
        retriever_name: retriever_name.to_string(),
        total_scenarios: n,
        mean_fractional_recall_at_5: sum_frac_5 / denom,
        mean_fractional_recall_at_10: sum_frac_10 / denom,
        mean_all_facts_recall_at_5: sum_all_5 / denom,
        mean_all_facts_recall_at_10: sum_all_10 / denom,
        mean_mrr: sum_mrr / denom,
        mean_ndcg_at_10: sum_ndcg_10 / denom,
        scenario_results,
    })
}

/// Computes the pairwise complementarity and Oracle Hybrid Union rate between two retrievers
/// at cutoff `k` across a slice of `Pv4Record` items.
pub fn compute_complementarity(
    records: &[Pv4Record],
    retriever_a: &dyn Retriever,
    retriever_b: &dyn Retriever,
    k: usize,
) -> Result<MultiFactComplementarityReport, BrainError> {
    if records.is_empty() {
        return Ok(MultiFactComplementarityReport {
            k,
            retriever_a_all_facts_rate: 0.0,
            retriever_b_all_facts_rate: 0.0,
            intersection_rate: 0.0,
            oracle_hybrid_union_rate: 0.0,
        });
    }

    let mut count_a = 0usize;
    let mut count_b = 0usize;
    let mut count_both = 0usize;
    let mut count_union = 0usize;

    for record in records {
        let expected_ids = record.required_fact_node_ids();

        let mut res_a = retriever_a.retrieve(&record.question)?;
        sort_results_deterministically(&mut res_a);
        let ids_a: Vec<NodeId> = res_a.iter().map(|r| r.node_id).collect();

        let mut res_b = retriever_b.retrieve(&record.question)?;
        sort_results_deterministically(&mut res_b);
        let ids_b: Vec<NodeId> = res_b.iter().map(|r| r.node_id).collect();

        let solved_a = compute_all_facts_recall_at_k(&ids_a, &expected_ids, k) >= 1.0;
        let solved_b = compute_all_facts_recall_at_k(&ids_b, &expected_ids, k) >= 1.0;

        // Union of top-K candidates from A and top-K candidates from B
        let limit_a = std::cmp::min(k, ids_a.len());
        let limit_b = std::cmp::min(k, ids_b.len());
        let mut union_set: HashSet<NodeId> = ids_a[..limit_a].iter().copied().collect();
        union_set.extend(ids_b[..limit_b].iter().copied());
        let solved_union = !expected_ids.is_empty()
            && expected_ids.iter().all(|req_id| union_set.contains(req_id));

        if solved_a {
            count_a += 1;
        }
        if solved_b {
            count_b += 1;
        }
        if solved_a && solved_b {
            count_both += 1;
        }
        if solved_union {
            count_union += 1;
        }
    }

    let total = records.len() as f64;
    Ok(MultiFactComplementarityReport {
        k,
        retriever_a_all_facts_rate: count_a as f64 / total,
        retriever_b_all_facts_rate: count_b as f64 / total,
        intersection_rate: count_both as f64 / total,
        oracle_hybrid_union_rate: count_union as f64 / total,
    })
}

/// Convenience helper that runs an end-to-end multi-fact benchmark comparing:
/// 1. Lexical `FtsRetriever` (BM25 only),
/// 2. `GraphAwareRetriever` (BM25 + multi-hop graph traversal over cross-session edges),
/// 3. `HybridRetriever` + `ConstraintAugmentedRetriever` (BM25 + Semantic + Always-On Constraint Subgraph).
pub fn run_pv4_comparative_suite(
    records: &[Pv4Record],
) -> Result<
    (
        MultiFactEvalReport,
        MultiFactEvalReport,
        MultiFactEvalReport,
        MultiFactComplementarityReport,
    ),
    BrainError,
> {
    let storage = SqliteStorage::new(":memory:", 1, false)?;
    let provider: Arc<dyn EmbeddingProvider> = Arc::new(ConceptualBridgeEmbeddingProvider);

    Pv4HarnessAdapter::ingest_into_storage(&storage, records, provider.as_ref())?;

    let fts = crate::retrieval::eval_harness::FtsRetriever::new(storage.pool().clone());
    let registry = Arc::new(brain_domain::RelationRegistry::default_embedded());
    let graph_ret = crate::retrieval::eval_harness::GraphAwareRetriever::new(
        Arc::new(storage.clone()),
        registry,
        2,
    );
    let sem_ret = crate::retrieval::eval_harness::SemanticRetriever::new(
        storage.pool().clone(),
        provider.clone(),
    );
    let hybrid = crate::retrieval::eval_harness::HybridRetriever::new(
        crate::retrieval::eval_harness::FtsRetriever::new(storage.pool().clone()),
        sem_ret,
    );
    let constraint_hybrid =
        ConstraintAugmentedRetriever::new(hybrid, storage.clone(), 10).with_collections(records)?;

    let fts_report = evaluate_pv4_multi_fact("FtsRetriever (BM25)", records, &fts)?;
    let graph_report =
        evaluate_pv4_multi_fact("GraphAwareRetriever (depth=2)", records, &graph_ret)?;
    let constraint_hybrid_report = evaluate_pv4_multi_fact(
        "ConstraintAugmented(HybridRetriever)",
        records,
        &constraint_hybrid,
    )?;

    let complementarity = compute_complementarity(records, &fts, &constraint_hybrid, 5)?;

    Ok((
        fts_report,
        graph_report,
        constraint_hybrid_report,
        complementarity,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::eval_harness::run_benchmark;

    #[test]
    fn test_pv4_synthetic_comparative_suite_and_complementarity() {
        let records = Pv4CorpusBuilder::build_synthetic();
        assert_eq!(records.len(), 3);

        let (fts_rep, graph_rep, hybrid_constraint_rep, comp) =
            run_pv4_comparative_suite(&records).expect("PV4 comparative suite should succeed");

        // Lexical BM25 alone cannot retrieve zero-lexical-overlap implicit facts
        assert_eq!(fts_rep.mean_all_facts_recall_at_5, 0.0);

        // Graph expansion (depth=2) traverses from the lexical anchor session node to the linked implicit fact nodes!
        assert!(
            graph_rep.mean_all_facts_recall_at_5 >= 0.66,
            "Expected GraphAwareRetriever to recover multi-hop facts, got {}",
            graph_rep.mean_all_facts_recall_at_5
        );

        // Hybrid + Always-On Constraint Subgraph achieves 100% joint All-Facts Recall@5
        assert_eq!(hybrid_constraint_rep.mean_all_facts_recall_at_5, 1.0);
        assert_eq!(comp.oracle_hybrid_union_rate, 1.0);
    }

    #[test]
    fn test_pv4_adapter_integrates_with_standard_run_benchmark() {
        let storage = SqliteStorage::new(":memory:", 1, false).unwrap();
        let records = Pv4CorpusBuilder::build_synthetic();
        let provider = ConceptualBridgeEmbeddingProvider;

        let (queries, truth) =
            Pv4HarnessAdapter::ingest_into_storage(&storage, &records, &provider).unwrap();
        let queries_json = serde_json::to_string(&queries).unwrap();
        let truth_json = serde_json::to_string(&truth).unwrap();

        let sem = crate::retrieval::eval_harness::SemanticRetriever::new(
            storage.pool().clone(),
            Arc::new(provider),
        );
        let augmented = ConstraintAugmentedRetriever::new(sem, storage, 10)
            .with_collections(&records)
            .unwrap();

        let report =
            run_benchmark(&queries_json, &truth_json, &augmented, "warm").expect("run_benchmark");
        assert_eq!(report.stable.metrics.total_queries, 3);
        assert_eq!(report.stable.metrics.failed_queries, 0);
        assert!(report.stable.metrics.mean_recall_at_5 >= 0.99);
    }

    #[test]
    fn test_pv4_jsonl_loader_roundtrip() {
        let records = Pv4CorpusBuilder::build_synthetic();
        let jsonl = records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .collect::<Vec<_>>()
            .join("\n");

        let parsed = Pv4SplitLoader::from_jsonl_str(&jsonl).expect("JSONL parse");
        assert_eq!(parsed, records);
    }
}
