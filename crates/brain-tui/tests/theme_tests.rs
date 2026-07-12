use brain_tui::ui::theme::{ThemeToken, ActiveTheme, dark_theme};

#[test]
fn test_theme_resolver() {
    let theme = dark_theme();
    
    let success_style = theme.style(ThemeToken::Success);
    assert_eq!(success_style.fg, Some(ratatui::style::Color::Rgb(78, 186, 101)));
    
    let danger_style = theme.style(ThemeToken::Danger);
    assert_eq!(danger_style.fg, Some(ratatui::style::Color::Rgb(255, 107, 128)));
}
