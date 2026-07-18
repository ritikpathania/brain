mod common;

use brain_tui::ui::render::{IconSet, RenderContext, TextRenderer};
use brain_tui::ui::theme::{dark_theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_render_helpers() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 42,
    };
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));

    TextRenderer::draw(
        &mut buf,
        Rect::new(0, 0, 5, 1),
        "Test",
        ThemeToken::Primary,
        &ctx,
    );
    assert_eq!(buf.get(0, 0).symbol(), "T");
    assert_eq!(
        buf.get(0, 0).style().fg,
        Some(ratatui::style::Color::Rgb(215, 119, 87))
    );
}
