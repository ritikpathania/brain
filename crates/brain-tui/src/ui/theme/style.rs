//! ActiveTheme trait definitions.

use crate::ui::theme::token::ThemeToken;
use ratatui::style::{Color, Style};

/// A trait implemented by styles that can resolve theme tokens.
pub trait ActiveTheme: Send + Sync {
    /// Returns the resolved ratatui style for a given theme token.
    fn style(&self, token: ThemeToken) -> Style;

    /// Returns the primary color associated with a given theme token.
    fn token_color(&self, token: ThemeToken) -> Color {
        let style = self.style(token);
        if token == ThemeToken::Selection {
            style.bg.unwrap_or(Color::Reset)
        } else {
            style.fg.unwrap_or(Color::Reset)
        }
    }
}

/// Simplified appearance modes (Dark or Light Paper theme).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// macOS Dark Mode / Dark terminal palette
    Dark,
    /// macOS Light Mode / Paper off-white palette
    Light,
}

impl Appearance {
    /// Automatically detects system appearance from macOS settings.
    pub fn detect_system() -> Self {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("defaults")
                .args(["read", "-g", "AppleInterfaceStyle"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.trim().eq_ignore_ascii_case("dark") {
                    return Appearance::Dark;
                }
            }
            Appearance::Light
        }
        #[cfg(not(target_os = "macos"))]
        {
            Appearance::Dark
        }
    }
}
