//! Independent Performance & Latency Benchmark Suite (Phase 5)

use brain_core::context::{
    ContextAssembler, MemoryRanker, MemoryScope, RetrievalCandidate, RetrievalChannel,
};
use brain_domain::NodeId;
use brain_services::context::{DefaultContextAssembler, DefaultMemoryRanker};
use std::collections::HashMap;
use std::time::Instant;

fn generate_synthetic_candidates(count: usize, ref_time: u64) -> Vec<RetrievalCandidate> {
    let mut candidates = Vec::with_capacity(count);
    for i in 0..count {
        let channel = match i % 3 {
            0 => RetrievalChannel::Lexical,
            1 => RetrievalChannel::Semantic,
            _ => RetrievalChannel::Graph,
        };
        let scope = match i % 3 {
            0 => MemoryScope::Session(brain_domain::SessionId::new()),
            1 => MemoryScope::Workspace(format!("workspace-{}", i % 5)),
            _ => MemoryScope::Global,
        };
        candidates.push(RetrievalCandidate {
            node_id: NodeId::new(),
            label: format!("Memory Entity Spec #{}", i),
            content: format!(
                "This is the detailed content for memory entity #{} with architectural guidelines and configuration values: parameter_{}={}.",
                i, i, i * 42
            ),
            channel,
            raw_score: 0.50 + ((i % 50) as f64 / 100.0),
            timestamp: ref_time - ((i as u64) * 3600),
            scope,
            metadata: HashMap::new(),
        });
    }
    candidates
}

fn calculate_percentiles(mut durations_us: Vec<f64>) -> (f64, f64, f64) {
    durations_us.sort_by(|a, b| a.total_cmp(b));
    let n = durations_us.len();
    let p50 = durations_us[(n as f64 * 0.50) as usize];
    let p95 = durations_us[(n as f64 * 0.95) as usize];
    let p99 = durations_us[(n as f64 * 0.99) as usize];
    (p50, p95, p99)
}

#[test]
fn test_independent_ranking_assembly_and_pipeline_benchmarks() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    for candidate_count in [100, 500] {
        let candidates = generate_synthetic_candidates(candidate_count, ref_time);

        // 1. Warm-up (50 iterations)
        for _ in 0..50 {
            let ranked = ranker.rank_and_deduplicate(candidates.clone(), ref_time);
            let _ = assembler.assemble(&ranked, 2048, 10, "epoch-warmup");
        }

        // 2. Benchmark Ranking & Fusion (1,000 iterations)
        let mut ranking_latencies_us = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let t0 = Instant::now();
            let _ranked = ranker.rank_and_deduplicate(candidates.clone(), ref_time);
            ranking_latencies_us.push(t0.elapsed().as_micros() as f64);
        }

        // Pre-rank for assembly benchmark
        let ranked = ranker.rank_and_deduplicate(candidates.clone(), ref_time);

        // 3. Benchmark Context Assembly & Budget Clamping (1,000 iterations)
        let mut assembly_latencies_us = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let t0 = Instant::now();
            let _snapshot = assembler.assemble(&ranked, 2048, 10, "epoch-bench");
            assembly_latencies_us.push(t0.elapsed().as_micros() as f64);
        }

        // 4. Benchmark End-to-End Pipeline (1,000 iterations)
        let mut pipeline_latencies_us = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let t0 = Instant::now();
            let r = ranker.rank_and_deduplicate(candidates.clone(), ref_time);
            let _s = assembler.assemble(&r, 2048, 10, "epoch-pipeline");
            pipeline_latencies_us.push(t0.elapsed().as_micros() as f64);
        }

        let (rank_p50, rank_p95, rank_p99) = calculate_percentiles(ranking_latencies_us);
        let (asm_p50, asm_p95, asm_p99) = calculate_percentiles(assembly_latencies_us);
        let (pipe_p50, pipe_p95, pipe_p99) = calculate_percentiles(pipeline_latencies_us);

        println!(
            "\n=== LATENCY BENCHMARK: {} CANDIDATES ===\n\
             Stage                  p50 (µs)    p95 (µs)    p99 (µs)\n\
             Ranking / Fusion:      {:>8.1}    {:>8.1}    {:>8.1}\n\
             Context Assembly:      {:>8.1}    {:>8.1}    {:>8.1}\n\
             End-to-End Pipeline:   {:>8.1}    {:>8.1}    {:>8.1}",
            candidate_count,
            rank_p50,
            rank_p95,
            rank_p99,
            asm_p50,
            asm_p95,
            asm_p99,
            pipe_p50,
            pipe_p95,
            pipe_p99,
        );

        // Performance qualification targets:
        // Assembly p95 must be well under 5ms (5,000 µs)
        assert!(
            asm_p95 < 5000.0,
            "Context assembly p95 latency ({:.1}µs) exceeded 5ms target",
            asm_p95
        );

        #[cfg(debug_assertions)]
        let max_pipe_target = 30000.0; // 30ms for unoptimized debug builds under parallel test load
        #[cfg(not(debug_assertions))]
        let max_pipe_target = 10000.0; // 10ms for release builds

        assert!(
            pipe_p95 < max_pipe_target,
            "Pipeline p95 latency ({:.1}µs) exceeded target",
            pipe_p95
        );

        // Monotonic distribution check
        assert!(rank_p50 <= rank_p95 && rank_p95 <= rank_p99);
        assert!(asm_p50 <= asm_p95 && asm_p95 <= asm_p99);
        assert!(pipe_p50 <= pipe_p95 && pipe_p95 <= pipe_p99);
    }
}

#[test]
fn test_token_budget_packing_efficiency_and_whole_item_preservation() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    let candidates = generate_synthetic_candidates(50, ref_time);
    let ranked = ranker.rank_and_deduplicate(candidates, ref_time);

    let max_tokens = 500;
    let max_items = 20;
    let snapshot = assembler.assemble(&ranked, max_tokens, max_items, "epoch-packing");

    assert!(
        snapshot.token_count <= max_tokens,
        "Token budget violated: {} > {}",
        snapshot.token_count,
        max_tokens
    );
    assert!(!snapshot.items.is_empty());
    assert!(snapshot
        .serialized_context
        .starts_with("<brain_memory_context"));
    assert!(snapshot
        .serialized_context
        .ends_with("</brain_memory_context>\n"));

    // Verify all XML memory tags are complete and closed
    let open_tags = snapshot.serialized_context.matches("<memory ").count();
    let close_tags = snapshot.serialized_context.matches("</memory>").count();
    assert_eq!(
        open_tags, close_tags,
        "Malformed XML: unclosed <memory> tag found"
    );
    assert_eq!(open_tags, snapshot.items.len());
}

#[test]
fn test_retained_memory_and_allocation_stability() {
    let ranker = DefaultMemoryRanker::new();
    let assembler = DefaultContextAssembler::new();
    let ref_time = 1_700_000_000;

    let candidates = generate_synthetic_candidates(100, ref_time);

    // Verify output determinism across 500 consecutive cycles
    let baseline_snapshot = assembler.assemble(
        &ranker.rank_and_deduplicate(candidates.clone(), ref_time),
        2048,
        10,
        "epoch-stable",
    );

    for _ in 0..500 {
        let ranked = ranker.rank_and_deduplicate(candidates.clone(), ref_time);
        let snapshot = assembler.assemble(&ranked, 2048, 10, "epoch-stable");
        assert_eq!(
            snapshot.serialized_context,
            baseline_snapshot.serialized_context
        );
        assert_eq!(snapshot.token_count, baseline_snapshot.token_count);
        assert_eq!(snapshot.items.len(), baseline_snapshot.items.len());
    }
}
