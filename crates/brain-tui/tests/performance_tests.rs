mod common;

use brain_domain::{Message, MessageId, MessageRole};
use brain_tui::ui::interaction::markdown::{
    CachedMessageLayout, KeywordSyntaxHighlighter, MarkdownLayout, MarkdownParser, MessageRevision,
    ViewportIndex,
};
use brain_tui::ui::scheduler::{RenderInvalidation, RenderReason, RenderRequest};
use std::time::Instant;

#[test]
fn test_viewport_index_binary_search_efficiency() {
    // Rebuild index for 10,000 messages of varying line heights
    let mut heights = Vec::new();
    for i in 0..10000 {
        heights.push((i % 5) + 1); // heights between 1 and 5
    }

    let start = Instant::now();
    let index = ViewportIndex::rebuild(&heights);
    let rebuild_duration = start.elapsed();
    assert!(
        rebuild_duration.as_millis() < 5,
        "Rebuilding 10,000 heights must take < 5ms"
    );

    // Perform O(log n) visibility query
    let start_query = Instant::now();
    let res = index.find_offset(15000);
    let query_duration = start_query.elapsed();

    assert!(res.is_some());
    assert!(
        query_duration.as_micros() < 50,
        "Binary search lookup must take < 50us"
    );
}

#[test]
fn test_cold_vs_warm_cache_latency() {
    let mut messages = Vec::new();
    for i in 0..100 {
        messages.push(Message::new(
            MessageId::new(),
            MessageRole::User,
            format!(
                "This is message number {} containing some markdown **bold** and `code` keywords.",
                i
            ),
        ));
    }

    let width = 80;
    let highlighter = KeywordSyntaxHighlighter::new();

    // 1. Cold Cache Render (simulate first layout pass)
    let start_cold = Instant::now();
    let mut cached_layouts = Vec::new();
    for msg in &messages {
        let ast = MarkdownParser::parse(&msg.content);
        let lines = MarkdownLayout::layout(&ast, width, &highlighter);
        let height = lines.len();
        cached_layouts.push(CachedMessageLayout {
            revision: MessageRevision(1),
            visual_lines: lines,
            height,
        });
    }
    let cold_duration = start_cold.elapsed();

    // 2. Warm Cache Render (simulate subsequent frames reusing the cache)
    let start_warm = Instant::now();
    let mut warm_layouts = Vec::new();
    for (i, msg) in messages.iter().enumerate() {
        let cached = &cached_layouts[i];
        let revision = MessageRevision(1);
        let layout = if cached.revision == revision {
            cached.clone()
        } else {
            let ast = MarkdownParser::parse(&msg.content);
            let lines = MarkdownLayout::layout(&ast, width, &highlighter);
            let height = lines.len();
            CachedMessageLayout {
                revision,
                visual_lines: lines,
                height,
            }
        };
        warm_layouts.push(layout);
    }
    let warm_duration = start_warm.elapsed();

    assert_eq!(cached_layouts, warm_layouts);
    assert!(
        warm_duration < cold_duration,
        "Warm cache rendering ({:?}) must be faster than cold rendering ({:?})",
        warm_duration,
        cold_duration
    );
}

#[test]
fn test_rendering_scaling_virtualization() {
    let highlighter = KeywordSyntaxHighlighter::new();

    // Verify virtualization scaling under 100, 1000, and 5000 messages
    let scales = vec![100, 1000, 5000];
    let viewport_height = 20;

    for scale in scales {
        let mut messages = Vec::new();
        for i in 0..scale {
            messages.push(Message::new(
                MessageId::new(),
                MessageRole::User,
                format!("Line {} content string.", i),
            ));
        }

        // Cache precalculated layouts
        let mut cached_layouts = Vec::new();
        let mut heights = Vec::new();
        for msg in &messages {
            let ast = MarkdownParser::parse(&msg.content);
            let lines = MarkdownLayout::layout(&ast, 80, &highlighter);
            let height = lines.len();
            cached_layouts.push(CachedMessageLayout {
                revision: MessageRevision(0),
                visual_lines: lines,
                height,
            });
            heights.push(1 + height + 1);
        }

        let index = ViewportIndex::rebuild(&heights);

        // Render visible slice
        let start = Instant::now();
        let start_offset = (scale as u32 * 2).saturating_sub(10); // scroll near tail
        let mut visible_lines = Vec::new();

        if let Some((mut msg_idx, mut local_line)) = index.find_offset(start_offset) {
            let mut lines_collected = 0u32;
            while msg_idx < messages.len() && lines_collected < viewport_height {
                let msg = &messages[msg_idx];
                let layout = &cached_layouts[msg_idx];
                let block_height = heights[msg_idx] as u32;

                while local_line < block_height && lines_collected < viewport_height {
                    if local_line == 0 {
                        visible_lines.push(match msg.role {
                            MessageRole::User => "User".to_string(),
                            MessageRole::Assistant => "Assistant".to_string(),
                            MessageRole::System => "System".to_string(),
                        });
                    } else if local_line <= layout.height as u32 {
                        let _span = &layout.visual_lines[(local_line - 1) as usize];
                    }
                    local_line += 1;
                    lines_collected += 1;
                }
                msg_idx += 1;
                local_line = 0;
            }
        }
        let duration = start.elapsed();

        assert!(
            duration.as_millis() < 5,
            "Slicing visible region for scale {} must take < 5ms (took {:?})",
            scale,
            duration
        );
    }
}

#[test]
fn test_scheduler_coalescing_rapid_events() {
    let r1 = RenderRequest {
        reason: RenderReason::StreamToken,
        invalidation: RenderInvalidation::ConversationStale,
    };
    let r2 = RenderRequest {
        reason: RenderReason::Input,
        invalidation: RenderInvalidation::EditorStale,
    };

    // Coalesce sequence checks
    let coalesced = r1.coalesce(r2);
    assert_eq!(coalesced.reason, RenderReason::Input);
    assert_eq!(coalesced.invalidation, RenderInvalidation::EverythingStale);
}
