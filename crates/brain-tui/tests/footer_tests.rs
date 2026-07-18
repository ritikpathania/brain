mod common;

use brain_tui::ui::render::{IconSet, RenderContext};
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::view_models::{FooterView, ShortcutHint};
use brain_tui::ui::widgets::{brain_widget::BrainWidget, Footer};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_footer_snapshot() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 0,
    };

    let shortcuts = [
        ShortcutHint {
            key: "Ctrl+C",
            description: "Quit",
        },
        ShortcutHint {
            key: "Enter",
            description: "Submit",
        },
    ];
    let view = FooterView {
        shortcuts: &shortcuts,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
    let widget = Footer { view: &view };
    widget.render(Rect::new(0, 0, 40, 1), &mut buf, &ctx);

    common::assert_snapshot(&buf, &ctx, "widgets/footer");
}
