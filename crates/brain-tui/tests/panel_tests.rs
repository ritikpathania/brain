mod common;

use brain_tui::ui::render::{IconSet, RenderContext};
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::view_models::{FocusState, PanelView};
use brain_tui::ui::widgets::{brain_widget::BrainWidget, Panel};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_panel_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 0,
    };

    let view = PanelView {
        title: "Main Console",
        focus: FocusState::Focused,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    let widget = Panel { view: &view };
    widget.render(Rect::new(0, 0, 20, 5), &mut buf, &ctx);

    common::assert_snapshot(&buf, &ctx, "widgets/panel");
}
