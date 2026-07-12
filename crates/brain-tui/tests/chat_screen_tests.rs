mod common;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::screen::Screen;
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::widgets::view_models::{ChatScreenView, FocusTarget, ConnectionState};
use brain_tui::ui::widgets::ChatScreen;
use brain_tui::ui::layout::{LayoutEngine, ResponsiveProfile, SIDEBAR_BREAKPOINT};

#[test]
fn test_chat_screen_partition_invariants() {
    let test_sizes = [
        Rect::new(0, 0, 90, 24),
        Rect::new(0, 0, 60, 20),
        Rect::new(0, 0, 10, 5),
        Rect::new(0, 0, 0, 0),
    ];

    for area in test_sizes {
        let geometry = LayoutEngine::chat_screen(area);

        assert!(geometry.status_bar_area.x >= area.x);
        assert!(geometry.status_bar_area.right() <= area.right());
        assert!(geometry.footer_area.x >= area.x);
        assert!(geometry.footer_area.right() <= area.right());
        assert!(geometry.prompt_area.x >= area.x);
        assert!(geometry.prompt_area.right() <= area.right());
        assert!(geometry.sidebar_area.x >= area.x);
        assert!(geometry.sidebar_area.right() <= area.right());
        assert!(geometry.chat_viewport_area.x >= area.x);
        assert!(geometry.chat_viewport_area.right() <= area.right());

        if area.height >= 5 {
            assert_eq!(geometry.status_bar_area.height, 1);
            assert_eq!(geometry.footer_area.height, 1);
            assert_eq!(geometry.prompt_area.height, 3);

            let body_h = geometry.sidebar_area.height;
            assert_eq!(
                geometry.status_bar_area.height + body_h + geometry.prompt_area.height + geometry.footer_area.height,
                area.height
            );
        }

        if area.width < SIDEBAR_BREAKPOINT.0 {
            assert_eq!(geometry.profile, ResponsiveProfile::Compact);
            assert_eq!(geometry.sidebar_area.width, 0);
            assert_eq!(geometry.chat_viewport_area.width, area.width);
        } else {
            assert_eq!(geometry.profile, ResponsiveProfile::Standard);
            assert_eq!(geometry.sidebar_area.width, 25);
            assert_eq!(
                geometry.sidebar_area.width + geometry.chat_viewport_area.width,
                area.width
            );
        }
    }
}

#[test]
fn test_chat_screen_golden_snapshots() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };

    let view = ChatScreenView {
        session_title: "General Thread",
        connection: ConnectionState::Connected,
        is_working: false,
        message_count: 42,
        input_buffer: "Hello world!",
        focus: FocusTarget::Prompt,
    };

    let mut buf = Buffer::empty(Rect::new(0, 0, 90, 24));
    let widget = ChatScreen { view: &view };
    widget.render(Rect::new(0, 0, 90, 24), &mut buf, &ctx);
    common::assert_snapshot(&buf, &ctx, "screens/chat/standard");

    let mut buf_compact = Buffer::empty(Rect::new(0, 0, 60, 20));
    widget.render(Rect::new(0, 0, 60, 20), &mut buf_compact, &ctx);
    common::assert_snapshot(&buf_compact, &ctx, "screens/chat/compact");
}