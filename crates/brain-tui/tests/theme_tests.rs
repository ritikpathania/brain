use brain_tui::ui::theme::{
    dark_theme, high_contrast_theme, light_theme, terminal_theme, ActiveTheme, ThemeToken,
};
use ratatui::style::Color;

#[test]
fn test_theme_resolver() {
    let theme = dark_theme();

    let success_style = theme.style(ThemeToken::Success);
    assert_eq!(success_style.fg, Some(Color::Rgb(78, 186, 101)));

    let danger_style = theme.style(ThemeToken::Danger);
    assert_eq!(danger_style.fg, Some(Color::Rgb(255, 107, 128)));
}

#[test]
fn test_theme_palette_invariants() {
    // 1. Dark theme invariants
    let dark = dark_theme();
    assert_eq!(
        dark.style(ThemeToken::HeaderPrimary).fg,
        Some(Color::Rgb(215, 119, 87)) // brand orange
    );

    // 2. Light theme invariants
    let light = light_theme();
    assert_eq!(
        light.style(ThemeToken::TextPrimary).fg,
        Some(Color::Rgb(32, 33, 36))
    );
    assert_eq!(
        light.style(ThemeToken::HeaderPrimary).fg,
        Some(Color::Rgb(192, 86, 33)) // brand orange
    );

    // 3. Adaptive terminal theme invariants:
    // Base fg/bg must be Color::Reset, while semantic accents remain explicit ANSI colors.
    let term = terminal_theme();
    assert_eq!(term.style(ThemeToken::TextPrimary).fg, Some(Color::Reset));
    assert_eq!(term.style(ThemeToken::HeaderPrimary).fg, Some(Color::Reset));
    assert_eq!(term.style(ThemeToken::Success).fg, Some(Color::Green));
    assert_eq!(term.style(ThemeToken::Danger).fg, Some(Color::Red));
    assert_eq!(term.style(ThemeToken::Warning).fg, Some(Color::Yellow));
    assert_eq!(term.style(ThemeToken::Info).fg, Some(Color::Cyan));

    // 4. High contrast theme invariants
    let hc = high_contrast_theme();
    assert_eq!(hc.style(ThemeToken::TextPrimary).fg, Some(Color::White));
    assert_eq!(hc.style(ThemeToken::HeaderPrimary).fg, Some(Color::White));
}

#[test]
fn test_claude_rgb_palette_alignment() {
    let theme = dark_theme();

    assert_eq!(
        theme.token_color(ThemeToken::HeaderPrimary),
        Color::Rgb(215, 119, 87)
    );
    assert_eq!(
        theme.token_color(ThemeToken::Accent),
        Color::Rgb(215, 119, 87)
    );
    assert_eq!(
        theme.token_color(ThemeToken::Selection),
        Color::Rgb(38, 79, 120)
    );
    assert_eq!(
        theme.token_color(ThemeToken::BorderSubtle),
        Color::Rgb(80, 80, 80)
    );
    assert_eq!(
        theme.token_color(ThemeToken::Suggestion),
        Color::Rgb(177, 185, 249)
    );
}
