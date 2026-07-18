//! Pipeline timing benchmark: measures wall-clock latency for the full
//! token → typewriter → drain → layout → render cycle with realistic data.

use brain_tui::state::{
    GenerationState, TypewriterQueue, UiState,
};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::Instant;

/// Simulates a realistic daemon response: header + N matches + relations.
fn generate_realistic_response(num_matches: usize, relations_per_match: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    chunks.push(format!(
        "Found {} matches via Hybrid Retrieval:",
        num_matches
    ));
    for i in 0..num_matches {
        chunks.push(format!(
            "\n  • [node-{}] source='STM' score=0.95 label='Google' type='entity' attributes='{{}}'",
            i
        ));
        for j in 0..relations_per_match {
            chunks.push(format!(
                "\n    └── [Graph Relation]: [Google] --(associated_with)--> [Target-{}-{}]",
                i, j
            ));
        }
    }
    chunks
}

/// Counts word tokens from push_chunk output for a set of chunks.
fn count_word_tokens(chunks: &[String]) -> usize {
    let mut tokenizer = brain_tui::state::IncrementalTokenizer::new();
    let mut total = 0;
    for chunk in chunks {
        total += tokenizer.push_chunk(chunk).len();
    }
    total += tokenizer.flush().len();
    total
}

#[test]
fn bench_typewriter_drain_timing() {
    // Simulate a response with 3 matches × 2 relations = 9 chunks
    let chunks = generate_realistic_response(3, 2);
    let token_count = count_word_tokens(&chunks);

    println!("\n=== Typewriter Drain Benchmark ===");
    println!("Chunks: {}", chunks.len());
    println!("Word tokens: {}", token_count);

    // Measure: push all tokens then drain at 30ms rate (pre-Finished)
    let mut queue = TypewriterQueue::new();
    let mut tokenizer = brain_tui::state::IncrementalTokenizer::new();
    for chunk in &chunks {
        for tok in tokenizer.push_chunk(chunk) {
            queue.push(tok);
        }
    }
    for tok in tokenizer.flush() {
        queue.push(tok);
    }

    let t0 = Instant::now();
    let mut tick_count = 0;
    let mut total_drained = 0;
    let mut sim_time = t0;

    // Simulate 10ms ticks, draining at 30ms rate (streaming mode, not finished)
    while !queue.is_empty() && tick_count < 10000 {
        sim_time += std::time::Duration::from_millis(10);
        let res = queue.drain_for_tick(sim_time);
        total_drained += res.emitted.len();
        tick_count += 1;
    }

    let elapsed_real = t0.elapsed();
    let simulated_wall = tick_count as u64 * 10; // ms
    println!("Streaming mode (30ms/token):");
    println!("  Ticks: {}", tick_count);
    println!("  Drained: {}", total_drained);
    println!("  Simulated wall time: {}ms", simulated_wall);
    println!(
        "  Real CPU time: {:.3}ms",
        elapsed_real.as_secs_f64() * 1000.0
    );

    // Measure: with backend_finished (flush mode)
    let mut queue2 = TypewriterQueue::new();
    let mut tokenizer2 = brain_tui::state::IncrementalTokenizer::new();
    for chunk in &chunks {
        for tok in tokenizer2.push_chunk(chunk) {
            queue2.push(tok);
        }
    }
    for tok in tokenizer2.flush() {
        queue2.push(tok);
    }
    queue2.finish_backend();

    let t1 = Instant::now();
    let res = queue2.drain_for_tick(t1);
    let elapsed_flush = t1.elapsed();
    println!("\nFlush mode (backend_finished):");
    println!("  Drained in 1 tick: {}", res.emitted.len());
    println!(
        "  Real CPU time: {:.3}ms",
        elapsed_flush.as_secs_f64() * 1000.0
    );
    println!("  Finished: {}", res.finished);
}

#[test]
fn bench_layout_and_draw_timing() {
    let chunks = generate_realistic_response(3, 2);

    // Build the full response string
    let full_response: String = chunks.join("");
    println!("\n=== Layout + Draw Benchmark ===");
    println!("Response length: {} chars", full_response.len());

    let renderer = AppRenderer::new();
    let theme = Theme::default();
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    // Measure draw with empty response
    let mut state = UiState::new();
    let t0 = Instant::now();
    terminal
        .draw(|f| {
            renderer.draw(f, f.size(), &state, &theme);
        })
        .unwrap();
    let empty_draw = t0.elapsed();
    println!("Empty draw: {:.3}ms", empty_draw.as_secs_f64() * 1000.0);

    // Measure draw with full response
    state.active_response = full_response.clone();
    state.active_response_revision += 1;
    state.generation_state = GenerationState::Streaming {
        started_at: std::time::SystemTime::now(),
    };
    // Need a timeline entry for the response to be rendered
    state.timeline.push((
        brain_tui::ui::interaction::timeline::EventOrdinal(1),
        brain_tui::ui::interaction::timeline::TimelineItem::Message(
            brain_tui::ui::interaction::MessageId(0),
        ),
    ));

    let t1 = Instant::now();
    terminal
        .draw(|f| {
            renderer.draw(f, f.size(), &state, &theme);
        })
        .unwrap();
    let full_draw = t1.elapsed();
    println!(
        "Full response draw: {:.3}ms",
        full_draw.as_secs_f64() * 1000.0
    );

    // Measure 100 consecutive draws (simulating 1 second of ticks)
    let t2 = Instant::now();
    for _ in 0..100 {
        terminal
            .draw(|f| {
                renderer.draw(f, f.size(), &state, &theme);
            })
            .unwrap();
    }
    let hundred_draws = t2.elapsed();
    println!(
        "100 consecutive draws: {:.3}ms ({:.3}ms avg)",
        hundred_draws.as_secs_f64() * 1000.0,
        hundred_draws.as_secs_f64() * 10.0
    );

    // Measure layout cache hit vs miss
    state.active_response_revision += 1; // force cache miss
    let t3 = Instant::now();
    terminal
        .draw(|f| {
            renderer.draw(f, f.size(), &state, &theme);
        })
        .unwrap();
    let cache_miss = t3.elapsed();

    // This draw should be a cache hit (same revision)
    let t4 = Instant::now();
    terminal
        .draw(|f| {
            renderer.draw(f, f.size(), &state, &theme);
        })
        .unwrap();
    let cache_hit = t4.elapsed();
    println!(
        "Cache miss draw: {:.3}ms",
        cache_miss.as_secs_f64() * 1000.0
    );
    println!("Cache hit draw: {:.3}ms", cache_hit.as_secs_f64() * 1000.0);
}

#[test]
fn bench_large_response_scaling() {
    println!("\n=== Response Size Scaling ===");
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    for (matches, rels) in [(3, 2), (10, 5), (50, 10), (100, 20)] {
        let chunks = generate_realistic_response(matches, rels);
        let full_response: String = chunks.join("");
        let token_count = count_word_tokens(&chunks);

        // Measure typewriter drain time in streaming mode
        let mut queue = TypewriterQueue::new();
        let mut tokenizer = brain_tui::state::IncrementalTokenizer::new();
        for chunk in &chunks {
            for tok in tokenizer.push_chunk(chunk) {
                queue.push(tok);
            }
        }
        for tok in tokenizer.flush() {
            queue.push(tok);
        }

        let mut tick_count = 0u64;
        let sim_start = Instant::now();
        let mut sim_time = sim_start;
        while !queue.is_empty() && tick_count < 100000 {
            sim_time += std::time::Duration::from_millis(10);
            queue.drain_for_tick(sim_time);
            tick_count += 1;
        }
        let simulated_ms = tick_count * 10;

        // Measure draw time
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = UiState::new();
        state.active_response = full_response.clone();
        state.active_response_revision += 1;
        state.generation_state = GenerationState::Streaming {
            started_at: std::time::SystemTime::now(),
        };
        state.timeline.push((
            brain_tui::ui::interaction::timeline::EventOrdinal(1),
            brain_tui::ui::interaction::timeline::TimelineItem::Message(
                brain_tui::ui::interaction::MessageId(0),
            ),
        ));

        let t0 = Instant::now();
        terminal
            .draw(|f| {
                renderer.draw(f, f.size(), &state, &theme);
            })
            .unwrap();
        let draw_ms = t0.elapsed().as_secs_f64() * 1000.0;

        println!(
            "{}m×{}r: {} chars, {} tokens, streaming={:.1}s, draw={:.1}ms",
            matches,
            rels,
            full_response.len(),
            token_count,
            simulated_ms as f64 / 1000.0,
            draw_ms
        );
    }
}
