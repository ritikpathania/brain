mod common;

use brain_tui::ui::render::{IconSet, RenderContext};
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::view_models::{StatusBarView, StatusKind};
use brain_tui::ui::widgets::{brain_widget::BrainWidget, StatusBar};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_status_bar_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 0,
    };

    let view = StatusBarView {
        title: "BrainTUI",
        kind: StatusKind::Working,
        message: "Streaming logs...",
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 1));
    let widget = StatusBar { view: &view };
    widget.render(Rect::new(0, 0, 30, 1), &mut buf, &ctx);

    common::assert_snapshot(&buf, &ctx, "widgets/status_bar");
}
