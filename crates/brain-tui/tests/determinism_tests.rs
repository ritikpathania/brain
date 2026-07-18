mod common;

use brain_tui::ui::primitives::{Badge, Divider, Progress};
use brain_tui::ui::render::{IconSet, RenderContext};
use brain_tui::ui::theme::{dark_theme, ThemeToken};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[test]
fn test_render_determinism() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext {
        theme,
        icons: &icons,
        capabilities,
        tick: 42,
    };

    let mut buf1 = Buffer::empty(Rect::new(0, 0, 10, 1));
    let mut buf2 = Buffer::empty(Rect::new(0, 0, 10, 1));

    // Draw primitives into first buffer
    let badge = Badge {
        label: "TEST",
        token: ThemeToken::Primary,
    };
    let div = Divider;
    let progress = Progress {
        ratio: 0.5,
        token: ThemeToken::Success,
    };

    badge.draw(Rect::new(0, 0, 10, 1), &mut buf1, &ctx);
    div.draw(Rect::new(0, 0, 10, 1), &mut buf1, &ctx);
    progress.draw(Rect::new(0, 0, 10, 1), &mut buf1, &ctx);

    // Draw primitives into second buffer under identical context
    badge.draw(Rect::new(0, 0, 10, 1), &mut buf2, &ctx);
    div.draw(Rect::new(0, 0, 10, 1), &mut buf2, &ctx);
    progress.draw(Rect::new(0, 0, 10, 1), &mut buf2, &ctx);

    // Assert both render results are byte-for-byte identical
    for y in 0..1 {
        for x in 0..10 {
            let cell1 = buf1.get(x, y);
            let cell2 = buf2.get(x, y);
            assert_eq!(cell1.symbol(), cell2.symbol());
            assert_eq!(cell1.style(), cell2.style());
        }
    }
}
