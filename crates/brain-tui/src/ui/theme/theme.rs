//! Active theme definitions.

use ratatui::style::Style;
use crate::ui::theme::token::ThemeToken;
use crate::ui::theme::palette::Palette;
use crate::ui::theme::style::ActiveTheme;

/// Theme structure mapping semantic tokens and legacy fields to style values.
pub struct Theme {
    /// Style for primary elements.
    pub primary: Style,
    /// Style for accent items.
    pub accent: Style,
    /// Style for success notifications.
    pub success: Style,
    /// Style for warnings.
    pub warning: Style,
    /// Style for errors.
    pub error: Style,

    // New private fields
    secondary: Style,
    muted: Style,
    danger: Style,
    thinking: Style,
    streaming: Style,
    user: Style,
    assistant: Style,
    tool: Style,
    system: Style,
    background: Style,
    surface: Style,

    /// Style for borders.
    pub border: Style,
    /// Style for active borders.
    pub border_active: Style,
    /// Style for inactive states.
    pub inactive: Style,
    /// Style for regular body text.
    pub text: Style,
    /// Style for section headers.
    pub header: Style,
    /// Style for passive status labels.
    pub status: Style,
    /// Style for terminal input cursor.
    pub cursor: Style,
}

impl Theme {
    /// Creates a new Theme instance resolved from the given color palette.
    pub fn new(palette: Palette) -> Self {
        let primary = Style::default().fg(palette.primary);
        let accent = Style::default().fg(palette.accent);
        let success = Style::default().fg(palette.success);
        let warning = Style::default().fg(palette.warning);
        let error = Style::default().fg(palette.danger);
        
        Self {
            primary,
            secondary: Style::default().fg(palette.secondary),
            accent,
            muted: Style::default().fg(palette.muted),
            success,
            warning,
            danger: error,
            thinking: Style::default().fg(palette.thinking),
            streaming: Style::default().fg(palette.streaming),
            user: Style::default().fg(palette.user),
            assistant: Style::default().fg(palette.assistant),
            tool: Style::default().fg(palette.tool),
            system: Style::default().fg(palette.system),
            background: Style::default().bg(palette.background),
            surface: Style::default().bg(palette.surface),

            // Compatibility fields mapped to Design System specs
            border: Style::default().fg(palette.muted),
            border_active: primary,
            inactive: Style::default().fg(palette.muted),
            text: Style::default().fg(ratatui::style::Color::White),
            header: Style::default().fg(ratatui::style::Color::White).add_modifier(ratatui::style::Modifier::BOLD),
            status: Style::default().fg(palette.muted),
            cursor: Style::default().bg(ratatui::style::Color::White).fg(ratatui::style::Color::Black),
            error,
        }
    }
}

impl ActiveTheme for Theme {
    fn style(&self, token: ThemeToken) -> Style {
        match token {
            ThemeToken::Primary => self.primary,
            ThemeToken::Secondary => self.secondary,
            ThemeToken::Accent => self.accent,
            ThemeToken::Muted => self.muted,
            ThemeToken::Success => self.success,
            ThemeToken::Warning => self.warning,
            ThemeToken::Danger => self.danger,
            ThemeToken::Thinking => self.thinking,
            ThemeToken::Streaming => self.streaming,
            ThemeToken::User => self.user,
            ThemeToken::Assistant => self.assistant,
            ThemeToken::Tool => self.tool,
            ThemeToken::System => self.system,
            ThemeToken::Background => self.background,
            ThemeToken::Surface => self.surface,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(Palette::dark())
    }
}
