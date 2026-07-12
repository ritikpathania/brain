//! ActiveTheme trait definitions.

use ratatui::style::Style;
use crate::ui::theme::token::ThemeToken;

/// A trait implemented by styles that can resolve theme tokens.
pub trait ActiveTheme: Send + Sync {
    /// Returns the resolved ratatui style for a given theme token.
    fn style(&self, token: ThemeToken) -> Style;
}
