mod common;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::widgets::view_models::{PanelView, FocusState};
use brain_tui::ui::widgets::{Panel, brain_widget::BrainWidget};

#[test]
fn test_panel_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let view = PanelView { title: "Main Console", focus: FocusState::Focused };
    let mut buf = Buffer::empty(Rect::new(0, 0, 20, 5));
    let widget = Panel { view: &view };
    widget.render(Rect::new(0, 0, 20, 5), &mut buf, &ctx);
    
    common::assert_snapshot(&buf, &ctx, "widgets/panel");
}
