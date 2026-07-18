//! Semantic TUI theme and palette engine.

use std::sync::OnceLock;

pub mod palette;
pub mod style;
#[allow(clippy::module_inception)]
pub mod theme;
pub mod token;

pub use palette::Palette;
pub use style::ActiveTheme;
pub use theme::Theme;
pub use token::ThemeToken;

/// Static thread-safe reference for the default dark theme.
pub static DARK_THEME: OnceLock<Theme> = OnceLock::new();

/// Static thread-safe reference for the high contrast theme.
pub static HIGH_CONTRAST_THEME: OnceLock<Theme> = OnceLock::new();

/// Returns the lazily-initialized reference to the static default dark theme.
pub fn dark_theme() -> &'static Theme {
    DARK_THEME.get_or_init(|| Theme::new(Palette::dark()))
}

/// Returns the lazily-initialized reference to the static accessibility high contrast theme.
pub fn high_contrast_theme() -> &'static Theme {
    HIGH_CONTRAST_THEME.get_or_init(|| Theme::new(Palette::high_contrast()))
}
