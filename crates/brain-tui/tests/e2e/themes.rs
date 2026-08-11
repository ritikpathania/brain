//! E2E Theme & Accessibility behavioral tests.
//!
//! Verifies 4-theme resolution completeness and light mode WCAG AA contrast compliance.

use brain_tui::ui::theme::{
    dark_theme, high_contrast_theme, light_theme, terminal_theme, ActiveTheme, Palette, ThemeToken,
};
use ratatui::style::Color;

fn relative_luminance(c: Color) -> f64 {
    if let Color::Rgb(r, g, b) = c {
        let channel = |v: u8| {
            let s = v as f64 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    } else {
        1.0
    }
}

fn contrast_ratio(c1: Color, c2: Color) -> f64 {
    let l1 = relative_luminance(c1);
    let l2 = relative_luminance(c2);
    let (max, min) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (max + 0.05) / (min + 0.05)
}

#[test]
fn test_four_theme_switching_and_token_resolution() {
    let themes = [
        ("Dark", dark_theme()),
        ("Light", light_theme()),
        ("Terminal", terminal_theme()),
        ("High Contrast", high_contrast_theme()),
    ];

    let tokens = [
        ThemeToken::Primary,
        ThemeToken::Secondary,
        ThemeToken::Accent,
        ThemeToken::Muted,
        ThemeToken::TextPrimary,
        ThemeToken::TextSecondary,
        ThemeToken::TextMuted,
        ThemeToken::HeaderPrimary,
        ThemeToken::Selection,
        ThemeToken::Border,
        ThemeToken::BorderActive,
        ThemeToken::Background,
        ThemeToken::Surface,
    ];

    for (name, theme) in themes {
        for token in &tokens {
            let style = theme.style(*token);
            let _ = style; // Ensures style resolves cleanly for every token across all 4 themes
        }

        // Verify panel blocks construct cleanly with theme styling
        let block = theme.panel(&format!("{} Theme Test", name), true);
        let _ = block;
    }
}

#[test]
fn test_light_mode_wcag_aa_contrast_ratios() {
    let light = Palette::light();
    let bg = if light.background == Color::Reset {
        Color::Rgb(255, 255, 255)
    } else {
        light.background
    };

    assert!(
        contrast_ratio(light.text_primary, bg) >= 4.5,
        "TextPrimary contrast against background must be >= 4.5:1 (WCAG AA)"
    );

    assert!(
        contrast_ratio(light.text_secondary, bg) >= 4.5,
        "TextSecondary contrast against background must be >= 4.5:1 (WCAG AA)"
    );

    assert!(
        contrast_ratio(light.muted, bg) >= 4.5,
        "Muted contrast against background must be >= 4.5:1 (WCAG AA)"
    );

    assert!(
        contrast_ratio(light.success, bg) >= 4.5,
        "Success contrast against background must be >= 4.5:1 (WCAG AA)"
    );

    assert!(
        contrast_ratio(light.warning, bg) >= 4.5,
        "Warning contrast against background must be >= 4.5:1 (WCAG AA)"
    );

    assert!(
        contrast_ratio(light.danger, bg) >= 4.5,
        "Danger contrast against background must be >= 4.5:1 (WCAG AA)"
    );

    assert!(
        contrast_ratio(light.info, bg) >= 4.5,
        "Info contrast against background must be >= 4.5:1 (WCAG AA)"
    );
}
