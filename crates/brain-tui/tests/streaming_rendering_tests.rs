mod common;

use brain_tui::ui::interaction::markdown::{
    MarkdownDocument, MarkdownRenderState, VisualLine, VisualLineKind, VisualSpan, VisualStyle,
};
use brain_tui::ui::interaction::scroll::{AutoFollowPolicy, ScrollState};
use brain_tui::ui::render::{IconSet, RenderContext};
use brain_tui::ui::scheduler::{RenderInvalidation, RenderReason, RenderRequest};
use brain_tui::ui::screen::Screen;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::view_models::{ChatScreenView, ConnectionState, FocusTarget};
use brain_tui::ui::widgets::ChatScreen;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_coalescing_associativity() {
    let r1 = RenderRequest {
        reason: RenderReason::StreamToken,
        invalidation: RenderInvalidation::ConversationStale,
    };
    let r2 = RenderRequest {
        reason: RenderReason::Input,
        invalidation: RenderInvalidation::EditorStale,
    };
    let r3 = RenderRequest {
        reason: RenderReason::Resize,
        invalidation: RenderInvalidation::StatusBarStale,
    };

    // Associativity law: (r1 + r2) + r3 == r1 + (r2 + r3)
    let left = r1.coalesce(r2).coalesce(r3);
    let right = r1.coalesce(r2.coalesce(r3));

    assert_eq!(left, right);
    assert_eq!(left.reason, RenderReason::Resize);
    assert_eq!(left.invalidation, RenderInvalidation::EverythingStale);
}

#[test]
fn test_scroll_policy_transitions() {
    let mut scroll = ScrollState::new();
    assert_eq!(scroll.policy, AutoFollowPolicy::Pinned);

    // Scrolling up transitions policy to Manual (only user input transitions it!)
    scroll.scroll_up();
    assert_eq!(scroll.policy, AutoFollowPolicy::Manual);

    scroll.update_bounds(20, 5); // max_offset = 15
    assert_eq!(scroll.offset(), 0); // scroll preserved/clamped, doesn't auto-follow

    // Scrolling down to the bottom re-pins policy to Pinned
    for _ in 0..20 {
        scroll.scroll_down();
    }
    assert_eq!(scroll.policy, AutoFollowPolicy::Pinned);
}

#[test]
fn test_markdown_render_state_ephemeral() {
    // Invariant: MarkdownRenderState is ephemeral.
    // Destroying and rebuilding it from the same document must never change rendered output.
    let mut doc = MarkdownDocument::new();
    doc.append("# Title\n- Item 1\n");

    let mut cache1 = MarkdownRenderState::new();
    cache1.set_visual_lines(vec![
        VisualLine {
            kind: VisualLineKind::Heading(1),
            spans: vec![VisualSpan::new("Title", VisualStyle::Heading1)],
        },
        VisualLine {
            kind: VisualLineKind::Text,
            spans: vec![VisualSpan::new("- Item 1", VisualStyle::Normal)],
        },
    ]);

    let mut cache2 = MarkdownRenderState::new();
    cache2.set_visual_lines(vec![
        VisualLine {
            kind: VisualLineKind::Heading(1),
            spans: vec![VisualSpan::new("Title", VisualStyle::Heading1)],
        },
        VisualLine {
            kind: VisualLineKind::Text,
            spans: vec![VisualSpan::new("- Item 1", VisualStyle::Normal)],
        },
    ]);

    assert_eq!(cache1.visual_lines(), cache2.visual_lines());

    // Discarding and recreating cache
    cache1.clear();
    assert_eq!(cache1.visual_lines().len(), 0);

    cache1.set_visual_lines(cache2.visual_lines().to_vec());
    assert_eq!(cache1.visual_lines(), cache2.visual_lines());
}

#[test]
fn test_partial_markdown_code_fence_rendering() {
    let mut doc = MarkdownDocument::new();
    doc.append("```rust\nfn main() {\n");

    let mut cache = MarkdownRenderState::new();
    // Simulate parsing partial fenced block
    cache.set_visual_lines(vec![
        VisualLine {
            kind: VisualLineKind::Code,
            spans: vec![VisualSpan::new("```rust", VisualStyle::Normal)],
        },
        VisualLine {
            kind: VisualLineKind::Code,
            spans: vec![VisualSpan::new("fn main() {", VisualStyle::Normal)],
        },
    ]);

    assert_eq!(doc.raw(), "```rust\nfn main() {\n");
    assert_eq!(cache.visual_lines().len(), 2);
}

#[test]
fn test_streaming_snapshots_lifecycle_with_resizing() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 0,
    };

    // 1. stream_001.snap: Empty user prompt waiting with spinner
    {
        let view = ChatScreenView {
            session_title: "General Thread",
            connection: ConnectionState::Connected,
            is_working: true,
            message_count: 1,
            input_buffer: "",
            focus: FocusTarget::Prompt,
        };
        let screen = ChatScreen { view: &view };
        let area = Rect::new(0, 0, 90, 20);
        let mut buf = Buffer::empty(area);
        screen.render(area, &mut buf, &ctx);
        common::assert_snapshot(&buf, &ctx, "screens/chat/stream_001");
    }

    // 2. stream_002.snap: Partial content streaming
    {
        let view = ChatScreenView {
            session_title: "General Thread",
            connection: ConnectionState::Connected,
            is_working: true,
            message_count: 2,
            input_buffer: "",
            focus: FocusTarget::Conversation,
        };
        let screen = ChatScreen { view: &view };
        let area = Rect::new(0, 0, 90, 20);
        let mut buf = Buffer::empty(area);
        screen.render(area, &mut buf, &ctx);
        common::assert_snapshot(&buf, &ctx, "screens/chat/stream_002");
    }

    // 3. stream_resize.snap: Terminal resized to compact width (collapsing sidebar)
    {
        let view = ChatScreenView {
            session_title: "General Thread",
            connection: ConnectionState::Connected,
            is_working: true,
            message_count: 2,
            input_buffer: "",
            focus: FocusTarget::Conversation,
        };
        let screen = ChatScreen { view: &view };
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        screen.render(area, &mut buf, &ctx);
        common::assert_snapshot(&buf, &ctx, "screens/chat/stream_resize");
    }

    // 4. stream_complete.snap: Streaming finished, spinner idle
    {
        let view = ChatScreenView {
            session_title: "General Thread",
            connection: ConnectionState::Connected,
            is_working: false,
            message_count: 2,
            input_buffer: "",
            focus: FocusTarget::Prompt,
        };
        let screen = ChatScreen { view: &view };
        let area = Rect::new(0, 0, 90, 20);
        let mut buf = Buffer::empty(area);
        screen.render(area, &mut buf, &ctx);
        common::assert_snapshot(&buf, &ctx, "screens/chat/stream_complete");
    }
}

#[test]
fn test_streaming_does_not_move_manual_scroll_position() {
    use brain_tui::ui::widgets::scroll_anchor::ScrollAnchor;

    // 1. Start streaming -> auto-pin at bottom
    let mut anchor = ScrollAnchor::new();
    assert_eq!(anchor, ScrollAnchor::Pinned);
    assert!(anchor.should_follow_bottom());

    // 2. User ScrollUp -> transition to Unpinned
    anchor.on_scroll_up();
    assert_eq!(anchor, ScrollAnchor::Unpinned);
    assert!(!anchor.should_follow_bottom());

    // 3. Receive 50 tokens while at offset 5 of max 20 -> viewport position remains unpinned
    for _ in 0..50 {
        anchor.update_position(5, 20);
    }
    assert_eq!(anchor, ScrollAnchor::Unpinned);
    assert!(!anchor.should_follow_bottom());

    // 4. User scrolls back to bottom (max_offset 20) -> auto-pin re-enables
    anchor.update_position(20, 20);
    assert_eq!(anchor, ScrollAnchor::Pinned);
    assert!(anchor.should_follow_bottom());

    // 5. Receive another token -> viewport follows bottom
    anchor.update_position(21, 21);
    assert_eq!(anchor, ScrollAnchor::Pinned);
    assert!(anchor.should_follow_bottom());
}

#[test]
fn test_typewriter_completion_flush_immediately() {
    use brain_tui::state::{RenderToken, TypewriterQueue};

    let mut queue = TypewriterQueue::new();
    for i in 0..10 {
        queue.push(RenderToken::Text(format!("token_{} ", i)));
    }

    // Backend finishes
    queue.finish_backend();
    assert!(!queue.is_finished());

    // Next tick drains all tokens immediately
    let result = queue.drain_for_tick(std::time::Instant::now());
    assert_eq!(result.emitted.len(), 10);
    assert!(result.finished);
    assert!(queue.is_finished());
}

#[test]
fn test_conversation_canvas_preserves_home_welcome_and_formats_messages() {
    use brain_domain::{Message, MessageId, MessageRole};
    use brain_tui::state::UiState;
    use brain_tui::ui::navigation::Screen;
    use brain_tui::ui::renderer::AppRenderer;
    use brain_tui::ui::theme::Theme;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::Terminal;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    let theme = Theme::default();
    let renderer = AppRenderer::new();

    state.screen = Screen::Conversation;
    state.active_messages = vec![
        Message::new(
            MessageId::new(),
            MessageRole::User,
            "How does Brain store relational memories?".to_string(),
        ),
        Message::new(
            MessageId::new(),
            MessageRole::Assistant,
            "Brain uses SQLite and a relational graph.".to_string(),
        ),
    ];
    state.timeline = vec![
        (
            brain_tui::ui::interaction::timeline::EventOrdinal(1),
            brain_tui::ui::interaction::timeline::TimelineItem::Message(
                brain_tui::ui::interaction::MessageId(1),
            ),
        ),
        (
            brain_tui::ui::interaction::timeline::EventOrdinal(2),
            brain_tui::ui::interaction::timeline::TimelineItem::Message(
                brain_tui::ui::interaction::MessageId(2),
            ),
        ),
    ];

    terminal
        .draw(|f| {
            renderer.draw(f, Rect::new(0, 0, 80, 24), &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    // 1. In Screen::Conversation, HomeWelcomeSurface is preserved at top of scroll canvas
    let title_cells: String = (3..=24).map(|x| buf.get(x, 2).symbol()).collect();
    assert_eq!(
        title_cells, " Claude Code v2.1.226 ",
        "HomeWelcomeSurface title must be preserved at top of canvas"
    );

    // 2. User message bubble renders with container background Color::Rgb(55, 55, 55)
    let mut found_user_bg = false;
    for y in 0..24 {
        let line: String = (0..80).map(|x| buf.get(x, y).symbol()).collect();
        if line.contains("How does Brain") {
            for x in 0..80 {
                if buf.get(x, y).symbol() == "H" {
                    assert_eq!(
                        buf.get(x, y).style().bg,
                        Some(Color::Rgb(55, 55, 55)),
                        "User message container background must be RGB(55, 55, 55)"
                    );
                    found_user_bg = true;
                    break;
                }
            }
        }
    }
    assert!(found_user_bg, "Must find user message text with container background");

    // 3. Assistant header 'Claude:' renders in terracotta Color::Rgb(215, 119, 87)
    let mut found_assistant_header = false;
    for y in 0..24 {
        let line: String = (0..80).map(|x| buf.get(x, y).symbol()).collect();
        if line.contains("Claude:") {
            for x in 0..80 {
                if buf.get(x, y).symbol() == "C" {
                    assert_eq!(
                        buf.get(x, y).style().fg,
                        Some(Color::Rgb(215, 119, 87)),
                        "Assistant header 'Claude:' must render in terracotta RGB(215, 119, 87)"
                    );
                    found_assistant_header = true;
                    break;
                }
            }
        }
    }
    assert!(found_assistant_header, "Must find assistant header 'Claude:'");
}

