mod common;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use brain_tui::ui::theme::{ThemeToken, dark_theme};
use brain_tui::ui::render::{RenderContext, IconSet};
use brain_tui::ui::primitives::{Badge, Divider, Progress, Spinner, SpinnerStyle};

#[test]
fn test_primitives_draw_and_style_no_alloc() {
    let theme = dark_theme();
    let icons = IconSet::new(true);
    let capabilities = common::mock_capabilities();
    let ctx = RenderContext { theme, icons: &icons, capabilities, tick: 0 };
    let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
    
    // Draw Badge & Assert style
    let badge = Badge { label: "OK", token: ThemeToken::Success };
    badge.draw(Rect::new(0, 0, 8, 1), &mut buf, &ctx);
    assert_eq!(buf.get(2, 0).symbol(), "O");
    assert_eq!(buf.get(2, 0).style().fg, Some(ratatui::style::Color::Rgb(78, 186, 101)));
    
    // Draw Divider & Assert symbol
    let div = Divider;
    div.draw(Rect::new(0, 0, 5, 1), &mut buf, &ctx);
    assert_eq!(buf.get(0, 0).symbol(), "─");
    
    // Draw Progress
    let progress = Progress { ratio: 0.8, token: ThemeToken::Success };
    progress.draw(Rect::new(0, 0, 5, 1), &mut buf, &ctx);
    assert_eq!(buf.get(0, 0).symbol(), "█");
    
    // Draw Spinner
    let spinner = Spinner { style: SpinnerStyle::Thinking };
    spinner.draw(Rect::new(0, 0, 1, 1), &mut buf, &ctx);
    assert_eq!(buf.get(0, 0).symbol(), "⠋");
}
