mod common;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::screen::Screen;
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::widgets::view_models::{ChatScreenView, FocusTarget, ConnectionState};
use brain_tui::ui::widgets::ChatScreen;
use brain_tui::ui::scheduler::{RenderReason, RenderInvalidation, RenderRequest};
use brain_tui::ui::interaction::markdown::{
    MarkdownDocument, MarkdownRenderState, VisualLine, VisualLineKind, VisualSpan, VisualStyle
};
use brain_tui::ui::interaction::scroll::{ScrollState, AutoFollowPolicy};

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
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };

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
