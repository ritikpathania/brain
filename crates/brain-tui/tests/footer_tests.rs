mod common;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::widgets::view_models::{FooterView, ShortcutHint};
use brain_tui::ui::widgets::{Footer, brain_widget::BrainWidget};

#[test]
fn test_footer_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    
    let shortcuts = [
        ShortcutHint { key: "Ctrl+C", description: "Quit" },
        ShortcutHint { key: "Enter", description: "Submit" },
    ];
    let view = FooterView { shortcuts: &shortcuts };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    let widget = Footer { view: &view };
    widget.render(Rect::new(0, 0, 40, 1), &mut buf, &ctx);
    
    common::assert_snapshot(&buf, &ctx, "widgets/footer");
}
