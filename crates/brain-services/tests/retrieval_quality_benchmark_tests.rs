//! Retrieval Quality & Precision/Recall Benchmark Evaluation Suite (Phase 4)

use brain_core::context::{
    ContextAssembler, MemoryRanker, MemoryScope, RetrievalCandidate, RetrievalChannel,
};
use brain_domain::{NodeId, SessionId};
use brain_services::context::{DefaultContextAssembler, DefaultMemoryRanker};
use std::collections::HashMap;

struct GroundTruthBenchmark {
    _query: &'static str,
    expected_node_ids: Vec<NodeId>,
    candidates: Vec<RetrievalCandidate>,
}

#[test]
fn test_precision_and_recall_on_curated_dataset() {
    let ranker = DefaultMemoryRanker::new();
    let ref_time = 1_700_000_000;

    let node_auth = NodeId::new();
    let node_postgres = NodeId::new();
    let node_redis = NodeId::new();
    let node_routes = NodeId::new();
    let node_irrelevant = NodeId::new();

    let benchmark = GroundTruthBenchmark {
        _query: "How do we authenticate requests and connect to database?",
        expected_node_ids: vec![node_auth, node_postgres],
        candidates: vec![
            RetrievalCandidate {
                node_id: node_auth,
                label: "Authentication RFC".to_string(),
                content: "All requests authenticate using Bearer JWT tokens with RSA256 signature."
                    .to_string(),
                channel: RetrievalChannel::Semantic,
                raw_score: 0.95,
                timestamp: ref_time - 3600,
                scope: MemoryScope::Workspace("backend".to_string()),
                metadata: HashMap::new(),
            },
            RetrievalCandidate {
                node_id: node_postgres,
                label: "Postgres Database Config".to_string(),
                content: "Connect to Postgres on port 5432 using connection pool size 20."
                    .to_string(),
                channel: RetrievalChannel::Semantic,
                raw_score: 0.92,
                timestamp: ref_time - 7200,
                scope: MemoryScope::Workspace("backend".to_string()),
                metadata: HashMap::new(),
            },
            RetrievalCandidate {
                node_id: node_redis,
                label: "Redis Cache Policy".to_string(),
                content: "Session tokens are cached in Redis with 1 hour TTL.".to_string(),
                channel: RetrievalChannel::Lexical,
                raw_score: 0.70,
                timestamp: ref_time - 10000,
                scope: MemoryScope::Workspace("backend".to_string()),
                metadata: HashMap::new(),
            },
            RetrievalCandidate {
                node_id: node_routes,
                label: "API Gateway Routes".to_string(),
                content: "Route table definition for /api/v1 endpoints.".to_string(),
                channel: RetrievalChannel::Lexical,
                raw_score: 0.60,
                timestamp: ref_time - 20000,
                scope: MemoryScope::Workspace("backend".to_string()),
                metadata: HashMap::new(),
            },
            RetrievalCandidate {
                node_id: node_irrelevant,
                label: "Unrelated CSS Theme".to_string(),
                content: "Dark mode background color is #1e1e1e.".to_string(),
                channel: RetrievalChannel::Lexical,
                raw_score: 0.35,
                timestamp: ref_time - 50000,
                scope: MemoryScope::Workspace("backend".to_string()),
                metadata: HashMap::new(),
            },
        ],
    };

    let ranked = ranker.rank_and_deduplicate(benchmark.candidates, ref_time);
    let top_5: Vec<NodeId> = ranked.iter().take(5).map(|r| r.node_id).collect();

    // Check Precision & Recall
    let relevant_retrieved = benchmark
        .expected_node_ids
        .iter()
        .filter(|id| top_5.contains(id))
        .count();

    let recall = relevant_retrieved as f64 / benchmark.expected_node_ids.len() as f64;
    let precision = relevant_retrieved as f64 / 2.0; // Top-2 retrieved

    assert!(
        recall >= 0.85,
        "Recall@5 ({}) fell below 85% target",
        recall
    );
    assert!(
        precision >= 0.80,
        "Precision@2 ({}) fell below 80% target",
        precision
    );

    // Top two should be exactly auth and postgres
    assert_eq!(top_5[0], node_auth);
    assert_eq!(top_5[1], node_postgres);
}

#[test]
fn test_orthogonal_query_noise_suppression() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    // Candidates scored for an out-of-domain query (e.g. baking cake in software repo)
    // Quality threshold: discard candidates with final_score < 0.65
    let candidates = vec![
        RetrievalCandidate {
            node_id: NodeId::new(),
            label: "Rust Compiler Flags".to_string(),
            content: "RUSTFLAGS=-C opt-level=3".to_string(),
            channel: RetrievalChannel::Lexical,
            raw_score: 0.22, // Below threshold
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: NodeId::new(),
            label: "Docker Compose".to_string(),
            content: "version: '3.8'".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.18, // Below threshold
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);
    let quality_filtered: Vec<_> = ranked
        .into_iter()
        .filter(|r| r.final_score >= 0.65)
        .collect();

    let result = assembler.assemble(&quality_filtered, 2048, 5, "epoch-noise-test");

    assert_eq!(result.items.len(), 0);
    assert_eq!(result.token_count, 0);
    assert!(
        result.serialized_context.is_empty(),
        "Noise was not suppressed"
    );
    assert_eq!(result.provenance.count, 0);
}

#[test]
fn test_temporal_decay_quality_ranking() {
    let ranker = DefaultMemoryRanker::with_half_life(86400.0); // 1-day half life for test
    let ref_time = 1_700_000_000;

    let fresh_node = NodeId::new();
    let stale_node = NodeId::new();

    let candidates = vec![
        RetrievalCandidate {
            node_id: stale_node,
            label: "Database Host (Stale)".to_string(),
            content: "Postgres host: db-v14.legacy.internal".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.95,
            timestamp: ref_time - (86400 * 5), // 5 days old (5 half-lives -> ~3% score)
            scope: MemoryScope::Workspace("ws".to_string()),
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: fresh_node,
            label: "Database Host (Fresh)".to_string(),
            content: "Postgres host: db-v16.primary.internal".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: ref_time - 3600, // 1 hour old (~97% score)
            scope: MemoryScope::Workspace("ws".to_string()),
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);

    assert_eq!(ranked.len(), 2);
    assert_eq!(
        ranked[0].node_id, fresh_node,
        "Fresh memory should rank above decayed stale memory"
    );
    assert!(ranked[0].final_score > ranked[1].final_score);
}

#[test]
fn test_semantic_deduplication_and_channel_fusion() {
    let ranker = DefaultMemoryRanker::new();
    let ref_time = 1_700_000_000;

    let shared_node = NodeId::new();

    // Same node discovered across Lexical and Semantic channels
    let candidates = vec![
        RetrievalCandidate {
            node_id: shared_node,
            label: "API Architecture".to_string(),
            content: "REST endpoints use OpenAPI 3.1 specification.".to_string(),
            channel: RetrievalChannel::Lexical,
            raw_score: 0.80,
            timestamp: ref_time - 100,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: shared_node,
            label: "API Architecture".to_string(),
            content: "REST endpoints use OpenAPI 3.1 specification.".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.88,
            timestamp: ref_time - 100,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);

    assert_eq!(
        ranked.len(),
        1,
        "Duplicate node across channels must be fused into 1 entry"
    );
    assert_eq!(ranked[0].node_id, shared_node);
    assert_eq!(ranked[0].channel, RetrievalChannel::Hybrid);
    assert!(ranked[0].final_score > 0.70);
}

#[test]
fn test_hierarchical_scope_conflict_resolution() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    let session_id = SessionId::new();

    let node_global = NodeId::new();
    let node_workspace = NodeId::new();
    let node_session = NodeId::new();

    // 3 conflicting facts with equal base relevance
    let candidates = vec![
        RetrievalCandidate {
            node_id: node_global,
            label: "Timeout Setting (Global)".to_string(),
            content: "Request timeout = 30s".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: ref_time,
            scope: MemoryScope::Global,
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: node_workspace,
            label: "Timeout Setting (Workspace)".to_string(),
            content: "Request timeout = 60s".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: ref_time,
            scope: MemoryScope::Workspace("ws-app".to_string()),
            metadata: HashMap::new(),
        },
        RetrievalCandidate {
            node_id: node_session,
            label: "Timeout Setting (Session)".to_string(),
            content: "Request timeout = 10s".to_string(),
            channel: RetrievalChannel::Semantic,
            raw_score: 0.90,
            timestamp: ref_time,
            scope: MemoryScope::Session(session_id),
            metadata: HashMap::new(),
        },
    ];

    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);

    // Assert ranking tier hierarchy: Session (1.2x) > Workspace (1.0x) > Global (0.85x)
    assert_eq!(
        ranked[0].node_id, node_session,
        "Session scope must win conflict"
    );
    assert_eq!(
        ranked[1].node_id, node_workspace,
        "Workspace scope must rank second"
    );
    assert_eq!(
        ranked[2].node_id, node_global,
        "Global scope must rank third"
    );

    // Assembled context puts the winning Session fact first
    let assembled = assembler.assemble(&ranked, 2048, 3, "epoch-conflict-test");
    assert_eq!(assembled.items[0].node_id, node_session);
    assert!(
        assembled
            .serialized_context
            .find("Request timeout = 10s")
            .unwrap()
            < assembled
                .serialized_context
                .find("Request timeout = 60s")
                .unwrap()
    );
}
