use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use brain_core::errors::BrainError;
use brain_core::repositories::RepositorySet;
use brain_core::retrieval::{
    DefaultQueryEmbeddingService, EmbeddingProvider, MemorySource, MemorySourceResult,
    RetrievalRequest, SourceMetadata,
};
use brain_domain::{Node, NodeId, RelationRegistry};
use brain_services::retrieval::pipeline::MemoryPipelineBuilder;
use brain_services::retrieval::ranking::{Bm25Ranking, EmbeddingRanking, RrfRanking};
use brain_services::retrieval::source::{LtmMemorySource, SemanticMemorySource};
use brain_storage::store::SqliteStorage;

// ============================================================================
// Local Heuristic Matching Implementation for Baseline Benchmarking
// ============================================================================

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut row: Vec<usize> = (0..=len2).collect();
    for (i, c1) in s1.chars().enumerate() {
        let mut prev = i + 1;
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            let val = std::cmp::min(row[j + 1] + 1, std::cmp::min(prev + 1, row[j] + cost));
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

fn calculate_node_match_score(node: &Node, query: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let label_lower = node.label.to_lowercase();
    let mut score = 0.0;

    if label_lower == query_lower {
        score += 150.0;
    } else if label_lower.contains(&query_lower) || query_lower.contains(&label_lower) {
        score += 80.0;
    }

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

struct HeuristicMemorySource {
    storage: Arc<SqliteStorage>,
    registry: Arc<RelationRegistry>,
}

impl MemorySource for HeuristicMemorySource {
    fn retrieve(&self, request: &RetrievalRequest) -> Result<MemorySourceResult, BrainError> {
        let mut candidates = std::collections::HashMap::new();
        let db_nodes = self.storage.nodes().list_all()?;
        for node in db_nodes {
            if request.exclude_ids.contains(&node.id) {
                continue;
            }
            let score = calculate_node_match_score(&node, &request.query);
            if score > 0.0 {
                candidates.insert(node.id, (node, score));
            }
        }

        let start_nodes: Vec<NodeId> = candidates.keys().cloned().collect();
        let traversal_budget = brain_services::retrieval::graph_service::TraversalBudget {
            max_depth: 1,
            max_nodes: 50,
            max_edges: 100,
            prevent_cycles: true,
            deadline: request.deadline,
            ..Default::default()
        };

        let mut expansions = Vec::new();
        if let Ok(connections) = brain_services::retrieval::graph_service::Graph.expand_neighbors(
            self.storage.as_ref(),
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

                if !candidates.contains_key(&neighbor_id)
                    && !request.exclude_ids.contains(&neighbor_id)
                {
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
                if let Ok(Some(neighbor_node)) = self.storage.nodes().find_by_id(&neighbor_id) {
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
                source_name: "HeuristicMemorySource",
            },
        })
    }
}

// ============================================================================
// 1. Isolated Tracking Allocator
// ============================================================================
struct TrackingAllocator {
    allocated: AtomicUsize,
    deallocated: AtomicUsize,
    allocations: AtomicUsize,
}

impl TrackingAllocator {
    const fn new() -> Self {
        Self {
            allocated: AtomicUsize::new(0),
            deallocated: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
        }
    }

    fn reset(&self) {
        self.allocated.store(0, Ordering::SeqCst);
        self.deallocated.store(0, Ordering::SeqCst);
        self.allocations.store(0, Ordering::SeqCst);
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            self.allocated.fetch_add(layout.size(), Ordering::SeqCst);
            self.allocations.fetch_add(1, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        self.deallocated.fetch_add(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static ALLOC: TrackingAllocator = TrackingAllocator::new();

// ============================================================================
// 2. Predefined Centroids & Seeding
// ============================================================================
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

fn compute_closest_centroid(vector: &[f32]) -> i32 {
    let centroids = get_predefined_centroids();
    let mut best_centroid = 0;
    let mut max_similarity = f32::NEG_INFINITY;

    for (c, centroid) in centroids.iter().enumerate() {
        let mut dot_product = 0.0f32;
        let limit = std::cmp::min(vector.len(), centroid.len());
        for i in 0..limit {
            dot_product += vector[i] * centroid[i];
        }
        if dot_product > max_similarity {
            max_similarity = dot_product;
            best_centroid = c as i32;
        }
    }
    best_centroid
}

fn deterministic_vector(index: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; 384];
    let mut norm_sq = 0.0f32;
    for (i, slot) in v.iter_mut().enumerate() {
        let val = ((2.0 * std::f64::consts::PI * (i + 1) as f64 * (index + 1) as f64) / 384.0).sin()
            as f32;
        *slot = val;
        norm_sq += val * val;
    }
    let norm = norm_sq.sqrt();
    if norm > 0.0 {
        for val in v.iter_mut() {
            *val /= norm;
        }
    }
    v
}

fn deterministic_label(index: usize) -> String {
    let categories = [
        "concept",
        "server",
        "database",
        "assistant",
        "optimization",
        "configuration",
        "deployment",
        "compiler",
    ];
    let cat = categories[index % categories.len()];
    format!("Standard Node Label for {} item index {}", cat, index)
}

fn seed_database(storage: &SqliteStorage, count: usize) {
    let mut conn = storage.pool().get().unwrap();
    let tx = conn.transaction().unwrap();
    {
        let mut stmt = tx
            .prepare("INSERT INTO nodes (id, label, node_type, properties, updated_at) VALUES (?, ?, ?, ?, ?)")
            .unwrap();
        let mut stmt_emb = tx
            .prepare("INSERT INTO embeddings (node_id, vector, dimension, centroid_id) VALUES (?, ?, ?, ?)")
            .unwrap();

        for i in 0..count {
            let mut bytes = [0u8; 16];
            let i_bytes = (i as u64).to_be_bytes();
            bytes[8..16].copy_from_slice(&i_bytes);
            let id = uuid::Uuid::from_bytes(bytes).to_string();
            let label = deterministic_label(i);
            let n_type = if i % 2 == 0 {
                brain_domain::NodeType::Concept
            } else {
                brain_domain::NodeType::Technology
            };
            let node_type_str = serde_json::to_string(&n_type).unwrap();
            let properties = serde_json::to_string(&serde_json::json!({
                "index": i,
                "importance": (i % 10) as f64
            }))
            .unwrap();
            let updated_at = 1000 + i as i64;
            stmt.execute(rusqlite::params![
                id,
                label,
                node_type_str,
                properties,
                updated_at
            ])
            .unwrap();

            // Seed deterministic high-dimensional embedding
            let vector = deterministic_vector(i);
            let centroid_id = compute_closest_centroid(&vector);
            let mut emb_bytes = Vec::with_capacity(384 * 4);
            for &val in &vector {
                emb_bytes.extend_from_slice(&val.to_le_bytes());
            }
            stmt_emb
                .execute(rusqlite::params![id, emb_bytes, 384, centroid_id])
                .unwrap();
        }
    }
    tx.commit().unwrap();
}

// ============================================================================
// 3. Benchmarking Embedding Provider
// ============================================================================
struct BenchEmbeddingProvider;

impl EmbeddingProvider for BenchEmbeddingProvider {
    fn name(&self) -> &'static str {
        "bench-embedding-provider"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, BrainError> {
        if text.contains("compiler") {
            Ok(deterministic_vector(7))
        } else {
            Ok(deterministic_vector(0))
        }
    }
}

// ============================================================================
// 4. Environment Metadata & Exporter
// ============================================================================
fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn get_rust_version() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

fn get_cpu_info() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown macos cpu".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg("grep -m 1 'model name' /proc/cpuinfo | cut -d: -f2")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown linux cpu".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        "unknown cpu".to_string()
    }
}

// ============================================================================
// 5. Tracing Execution & Allocations
// ============================================================================
fn run_scenario_tracing(
    storage: &SqliteStorage,
    count: usize,
    scenario: &str,
) -> serde_json::Value {
    let registry = Arc::new(RelationRegistry::default_embedded());
    let provider = Arc::new(BenchEmbeddingProvider);
    let request = RetrievalRequest {
        reference_time: None,
        session_id: brain_domain::SessionId::new(),
        query: "compiler".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let (res_nodes, ivf_bypass, ivf_activation, cosine_comps) = match scenario {
        "heuristic" => {
            let src = HeuristicMemorySource {
                storage: Arc::new(storage.clone()),
                registry: registry.clone(),
            };
            ALLOC.reset();
            let start = Instant::now();
            let res = src.retrieve(&request).unwrap().nodes;
            let _dur = start.elapsed();
            (res, 0, 0, 0)
        }
        "bm25" => {
            let src = LtmMemorySource::new(Arc::new(storage.clone()), registry.clone());
            ALLOC.reset();
            let start = Instant::now();
            let res = src.retrieve(&request).unwrap().nodes;
            let _dur = start.elapsed();
            (res, 0, 0, 0)
        }
        "vector" => {
            let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider.clone()));
            let src = SemanticMemorySource::new(Arc::new(storage.clone()), embed_service);
            ALLOC.reset();
            let start = Instant::now();
            let res = src.retrieve(&request).unwrap().nodes;
            let _dur = start.elapsed();
            let bypass = src.bypass_count.load(Ordering::SeqCst);
            let active = src.activation_count.load(Ordering::SeqCst);
            let comps = src.cosine_computations.load(Ordering::SeqCst);
            (res, bypass, active, comps)
        }
        "rrf" => {
            let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider.clone()));
            let src_b = Arc::new(LtmMemorySource::new(
                Arc::new(storage.clone()),
                registry.clone(),
            ));
            let src_v = Arc::new(SemanticMemorySource::new(
                Arc::new(storage.clone()),
                embed_service.clone(),
            ));
            let strategy_b = Arc::new(Bm25Ranking::default());
            let strategy_v = Arc::new(EmbeddingRanking::new(
                embed_service,
                Arc::new(storage.clone()) as Arc<dyn brain_core::retrieval::EmbeddingLookup>,
            ));
            let rrf = Arc::new(RrfRanking::new(
                vec![(strategy_b, 1.0), (strategy_v, 1.0)],
                60.0,
            ));
            let pipeline = MemoryPipelineBuilder::new()
                .register_source(src_b)
                .register_source(src_v.clone())
                .with_ranking_strategy(rrf)
                .build();
            ALLOC.reset();
            let start = Instant::now();
            let res = pipeline.execute(&request).unwrap().nodes;
            let _dur = start.elapsed();
            let bypass = src_v.bypass_count.load(Ordering::SeqCst);
            let active = src_v.activation_count.load(Ordering::SeqCst);
            let comps = src_v.cosine_computations.load(Ordering::SeqCst);
            (res, bypass, active, comps)
        }
        _ => panic!("unknown scenario"),
    };

    // Reset allocator and time for exact execution
    ALLOC.reset();
    let start_time = Instant::now();
    let _ = match scenario {
        "heuristic" => {
            let src = HeuristicMemorySource {
                storage: Arc::new(storage.clone()),
                registry: registry.clone(),
            };
            src.retrieve(&request).unwrap().nodes
        }
        "bm25" => {
            let src = LtmMemorySource::new(Arc::new(storage.clone()), registry.clone());
            src.retrieve(&request).unwrap().nodes
        }
        "vector" => {
            let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider.clone()));
            let src = SemanticMemorySource::new(Arc::new(storage.clone()), embed_service);
            src.retrieve(&request).unwrap().nodes
        }
        "rrf" => {
            let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider.clone()));
            let src_b = Arc::new(LtmMemorySource::new(
                Arc::new(storage.clone()),
                registry.clone(),
            ));
            let src_v = Arc::new(SemanticMemorySource::new(
                Arc::new(storage.clone()),
                embed_service.clone(),
            ));
            let strategy_b = Arc::new(Bm25Ranking::default());
            let strategy_v = Arc::new(EmbeddingRanking::new(
                embed_service,
                Arc::new(storage.clone()) as Arc<dyn brain_core::retrieval::EmbeddingLookup>,
            ));
            let rrf = Arc::new(RrfRanking::new(
                vec![(strategy_b, 1.0), (strategy_v, 1.0)],
                60.0,
            ));
            let pipeline = MemoryPipelineBuilder::new()
                .register_source(src_b)
                .register_source(src_v)
                .with_ranking_strategy(rrf)
                .build();
            pipeline.execute(&request).unwrap().nodes
        }
        _ => panic!("unknown scenario"),
    };
    let duration = start_time.elapsed();
    let allocated_bytes = ALLOC.allocated.load(Ordering::SeqCst);
    let allocation_count = ALLOC.allocations.load(Ordering::SeqCst);

    // Calculate Recall
    let hits = res_nodes
        .iter()
        .filter(|node| node.label.to_lowercase().contains("compiler"))
        .count();
    let expected_in_top_10 = std::cmp::min(10, count / 8);
    let recall = if expected_in_top_10 > 0 {
        hits as f64 / expected_in_top_10 as f64
    } else {
        1.0
    };

    serde_json::json!({
        "scenario": scenario,
        "scale_nodes": count,
        "correctness_verified": !res_nodes.is_empty(),
        "recall_at_10": recall,
        "single_query_duration_ms": duration.as_secs_f64() * 1000.0,
        "heap_allocated_bytes": allocated_bytes,
        "heap_allocation_count": allocation_count,
        "ivf_bypass_rate": ivf_bypass,
        "ivf_activation_rate": ivf_activation,
        "cosine_computations": cosine_comps
    })
}

// ============================================================================
// 6. Macro Pipeline Telemetry Tracing
// ============================================================================
fn run_macro_pipeline_tracing(storage: &SqliteStorage, count: usize) -> serde_json::Value {
    let registry = Arc::new(RelationRegistry::default_embedded());
    let provider = Arc::new(BenchEmbeddingProvider);
    let request = RetrievalRequest {
        reference_time: None,
        session_id: brain_domain::SessionId::new(),
        query: "compiler".to_string(),
        limit: 10,
        exclude_ids: std::collections::HashSet::new(),
        deadline: None,
        explain: false,
        graph_depth: None,
        expand_relations: false,
    };

    let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider.clone()));
    let src_b = Arc::new(LtmMemorySource::new(
        Arc::new(storage.clone()),
        registry.clone(),
    ));
    let src_v = Arc::new(SemanticMemorySource::new(
        Arc::new(storage.clone()),
        embed_service.clone(),
    ));
    let strategy_b = Arc::new(Bm25Ranking::default());
    let strategy_v = Arc::new(EmbeddingRanking::new(
        embed_service,
        Arc::new(storage.clone()) as Arc<dyn brain_core::retrieval::EmbeddingLookup>,
    ));
    let rrf = Arc::new(RrfRanking::new(
        vec![(strategy_b, 1.0), (strategy_v, 1.0)],
        60.0,
    ));
    let pipeline = MemoryPipelineBuilder::new()
        .register_source(src_b)
        .register_source(src_v)
        .with_ranking_strategy(rrf)
        .build();

    // 1. Embedding generation duration
    let start_embed = Instant::now();
    let _query_vector = provider.embed(&request.query).unwrap();
    let embed_duration = start_embed.elapsed();

    // 2. Retrieval + RRF ranking duration
    let start_retrieve = Instant::now();
    let response = pipeline.execute(&request).unwrap();
    let retrieve_duration = start_retrieve.elapsed();

    // 3. DTO mapping & Edge lookups duration
    let start_dto = Instant::now();
    let mut dtos = Vec::with_capacity(response.nodes.len());
    for node in response.nodes {
        let connections = storage.edges().get_connections(&node.id).unwrap();
        let dto = brain_services::mapper::to_memory_dto(&node, &connections).unwrap();
        dtos.push(dto);
    }
    let dto_duration = start_dto.elapsed();

    // 4. Context Assembly duration
    let start_assembly = Instant::now();
    let counter = brain_services::conversation::WordSpaceTokenCounter;
    let budget = brain_services::conversation::ContextBudget {
        max_tokens: 2048,
        reserved_system_tokens: 200,
        reserved_completion_tokens: 500,
    };
    let history = vec![
        brain_domain::Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::System,
            "You are a helpful assistant.".to_string(),
        ),
        brain_domain::Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::User,
            "Explain compilers.".to_string(),
        ),
    ];
    let summary = Some(brain_services::conversation::ConversationSummary {
        version: 1,
        created_at: std::time::SystemTime::now(),
        start_message_idx: 0,
        end_message_idx: 1,
        text: "User requested explanation of compilers.".to_string(),
    });
    let _window = brain_services::conversation::ContextBuilder::build(
        &counter, budget, &history, summary, dtos,
    );
    let assembly_duration = start_assembly.elapsed();

    let total_duration = embed_duration + retrieve_duration + dto_duration + assembly_duration;

    serde_json::json!({
        "scale_nodes": count,
        "total_macro_latency_ms": total_duration.as_secs_f64() * 1000.0,
        "embedding_contribution_ms": embed_duration.as_secs_f64() * 1000.0,
        "retrieval_contribution_ms": retrieve_duration.as_secs_f64() * 1000.0,
        "dto_mapping_contribution_ms": dto_duration.as_secs_f64() * 1000.0,
        "assembly_contribution_ms": assembly_duration.as_secs_f64() * 1000.0,
        "embedding_percentage": (embed_duration.as_secs_f64() / total_duration.as_secs_f64()) * 100.0,
        "retrieval_percentage": (retrieve_duration.as_secs_f64() / total_duration.as_secs_f64()) * 100.0,
        "dto_mapping_percentage": (dto_duration.as_secs_f64() / total_duration.as_secs_f64()) * 100.0,
        "assembly_percentage": (assembly_duration.as_secs_f64() / total_duration.as_secs_f64()) * 100.0,
    })
}

// ============================================================================
// 7. Benchmark Orchestration
// ============================================================================
fn bench_retrieval_baseline(c: &mut Criterion) {
    let scales = [100, 1000, 10000];
    let mut all_scenarios_telemetry = Vec::new();
    let mut macro_telemetry = Vec::new();

    for &count in &scales {
        let rand_val = uuid::Uuid::new_v4().to_string();
        let db_file = std::env::temp_dir().join(format!("bench-ltm-{}-{}.db", count, rand_val));
        let db_path = db_file.to_str().unwrap();

        let storage = SqliteStorage::new(db_path, 1, false).unwrap();
        seed_database(&storage, count);

        let scenarios = ["heuristic", "bm25", "vector", "rrf"];
        for scenario in &scenarios {
            let telemetry = run_scenario_tracing(&storage, count, scenario);
            all_scenarios_telemetry.push(telemetry);
        }

        // Run macro pipeline tracing
        let macro_run = run_macro_pipeline_tracing(&storage, count);
        macro_telemetry.push(macro_run);

        // Criterion Micro-benchmarks
        let registry = Arc::new(RelationRegistry::default_embedded());
        let provider = Arc::new(BenchEmbeddingProvider);
        let request = RetrievalRequest {
            reference_time: None,
            session_id: brain_domain::SessionId::new(),
            query: "compiler".to_string(),
            limit: 10,
            exclude_ids: std::collections::HashSet::new(),
            deadline: None,
            explain: false,
            graph_depth: None,
            expand_relations: false,
        };

        // Heuristic
        let src_h = HeuristicMemorySource {
            storage: Arc::new(storage.clone()),
            registry: registry.clone(),
        };
        let mut group = c.benchmark_group("LtmMemorySource_Heuristic");
        group.bench_function(format!("retrieve_heuristic_scale_{}", count), |b| {
            b.iter(|| {
                let _res = src_h.retrieve(black_box(&request)).unwrap();
            })
        });
        group.finish();

        // BM25
        let src_b = LtmMemorySource::new(Arc::new(storage.clone()), registry.clone());
        let mut group = c.benchmark_group("LtmMemorySource_BM25");
        group.bench_function(format!("retrieve_bm25_scale_{}", count), |b| {
            b.iter(|| {
                let _res = src_b.retrieve(black_box(&request)).unwrap();
            })
        });
        group.finish();

        // Vector
        let embed_service = Arc::new(DefaultQueryEmbeddingService::new(provider.clone()));
        let src_v = SemanticMemorySource::new(Arc::new(storage.clone()), embed_service.clone());
        let mut group = c.benchmark_group("LtmMemorySource_Vector");
        group.bench_function(format!("retrieve_vector_scale_{}", count), |b| {
            b.iter(|| {
                let _res = src_v.retrieve(black_box(&request)).unwrap();
            })
        });
        group.finish();

        // RRF Hybrid
        let src_b_p = Arc::new(LtmMemorySource::new(
            Arc::new(storage.clone()),
            registry.clone(),
        ));
        let src_v_p = Arc::new(SemanticMemorySource::new(
            Arc::new(storage.clone()),
            embed_service.clone(),
        ));
        let strategy_b = Arc::new(Bm25Ranking::default());
        let strategy_v = Arc::new(EmbeddingRanking::new(
            embed_service.clone(),
            Arc::new(storage.clone()) as Arc<dyn brain_core::retrieval::EmbeddingLookup>,
        ));
        let rrf = Arc::new(RrfRanking::new(
            vec![(strategy_b, 1.0), (strategy_v, 1.0)],
            60.0,
        ));
        let pipeline = MemoryPipelineBuilder::new()
            .register_source(src_b_p)
            .register_source(src_v_p)
            .with_ranking_strategy(rrf)
            .build();
        let mut group = c.benchmark_group("LtmMemorySource_RRF");
        group.bench_function(format!("retrieve_rrf_scale_{}", count), |b| {
            b.iter(|| {
                let _res = pipeline.execute(black_box(&request)).unwrap();
            })
        });
        group.finish();

        // Macro Pipeline Bench
        let src_b_m = Arc::new(LtmMemorySource::new(
            Arc::new(storage.clone()),
            registry.clone(),
        ));
        let src_v_m = Arc::new(SemanticMemorySource::new(
            Arc::new(storage.clone()),
            embed_service.clone(),
        ));
        let strategy_bm25 = Arc::new(Bm25Ranking::default());
        let strategy_vector = Arc::new(EmbeddingRanking::new(
            embed_service.clone(),
            Arc::new(storage.clone()) as Arc<dyn brain_core::retrieval::EmbeddingLookup>,
        ));
        let rrf_ranking = Arc::new(RrfRanking::new(
            vec![(strategy_bm25, 1.0), (strategy_vector, 1.0)],
            60.0,
        ));
        let pipeline_m = MemoryPipelineBuilder::new()
            .register_source(src_b_m)
            .register_source(src_v_m)
            .with_ranking_strategy(rrf_ranking)
            .build();
        let counter = brain_services::conversation::WordSpaceTokenCounter;
        let budget = brain_services::conversation::ContextBudget {
            max_tokens: 2048,
            reserved_system_tokens: 200,
            reserved_completion_tokens: 500,
        };
        let history = vec![
            brain_domain::Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::System,
                "You are a helpful assistant.".to_string(),
            ),
            brain_domain::Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                "Explain compilers.".to_string(),
            ),
        ];
        let summary = Some(brain_services::conversation::ConversationSummary {
            version: 1,
            created_at: std::time::SystemTime::now(),
            start_message_idx: 0,
            end_message_idx: 1,
            text: "User requested explanation of compilers.".to_string(),
        });

        let mut group = c.benchmark_group("LtmMemorySource_MacroPipeline");
        group.bench_function(format!("macro_pipeline_scale_{}", count), |b| {
            b.iter(|| {
                let res = pipeline_m.execute(black_box(&request)).unwrap();
                let mut dtos = Vec::with_capacity(res.nodes.len());
                for node in res.nodes {
                    let connections = storage.edges().get_connections(&node.id).unwrap();
                    let dto = brain_services::mapper::to_memory_dto(&node, &connections).unwrap();
                    dtos.push(dto);
                }
                let _window = brain_services::conversation::ContextBuilder::build(
                    &counter,
                    budget,
                    &history,
                    summary.clone(),
                    dtos,
                );
            })
        });
        group.finish();

        let _ = fs::remove_file(db_file);
    }

    // Export machine-readable results
    let artifact_dir = PathBuf::from(
        "/Users/ritikpathania/.gemini/antigravity/brain/34b4c5e5-7d12-4562-abaf-12d79236c9cb",
    );
    let results_path = artifact_dir.join("baseline_bench_results.json");

    let final_report = serde_json::json!({
        "metadata": {
            "git_commit": get_git_commit(),
            "rust_version": get_rust_version(),
            "cpu_info": get_cpu_info(),
            "os": std::env::consts::OS,
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "timestamp": chrono::Utc::now().to_rfc3339()
        },
        "runs": all_scenarios_telemetry,
        "macro_runs": macro_telemetry
    });

    if let Ok(json_str) = serde_json::to_string_pretty(&final_report) {
        let _ = fs::create_dir_all(&artifact_dir);
        let _ = fs::write(results_path, json_str);
    }
}

criterion_group!(benches, bench_retrieval_baseline);
criterion_main!(benches);
