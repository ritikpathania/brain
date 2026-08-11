//! Active theme definitions.

use crate::ui::theme::palette::Palette;
use crate::ui::theme::style::ActiveTheme;
use crate::ui::theme::token::ThemeToken;
use ratatui::style::Style;

/// Theme structure mapping semantic tokens to style values.
pub struct Theme {
    /// Style for primary elements.
    pub primary: Style,
    /// Style for secondary elements.
    pub secondary: Style,
    /// Style for accent items.
    pub accent: Style,
    /// Style for muted/passive areas.
    pub muted: Style,
    /// Style for success notifications.
    pub success: Style,
    /// Style for warnings.
    pub warning: Style,
    /// Style for errors/danger.
    pub error: Style,

    // Additional semantic token styles
    info: Style,
    text_primary: Style,
    /// Public color accessor for secondary background.
    pub bg_secondary: ratatui::style::Color,
    /// Public color accessor for secondary text.
    pub text_secondary: ratatui::style::Color,
    text_secondary_style: Style,
    text_muted: Style,
    header_primary: Style,
    header_secondary: Style,
    selection: Style,
    focus: Style,
    border_subtle: Style,
    suggestion: Style,
    code_inline: Style,
    code_block: Style,
    link: Style,
    tag: Style,

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
        let secondary = Style::default().fg(palette.secondary);
        let accent = Style::default().fg(palette.accent);
        let muted = Style::default().fg(palette.muted);
        let success = Style::default().fg(palette.success);
        let warning = Style::default().fg(palette.warning);
        let danger = Style::default().fg(palette.danger);
        let info = Style::default().fg(palette.info);

        let text_primary = Style::default().fg(palette.text_primary);
        let text_secondary = Style::default().fg(palette.text_secondary);
        let text_muted = Style::default().fg(palette.muted);

        let header_primary = Style::default()
            .fg(palette.header_primary)
            .add_modifier(ratatui::style::Modifier::BOLD);
        let header_secondary = Style::default()
            .fg(palette.primary)
            .add_modifier(ratatui::style::Modifier::BOLD);

        let selection = Style::default()
            .bg(palette.selection_bg)
            .fg(palette.selection_fg);
        let focus = primary.add_modifier(ratatui::style::Modifier::BOLD);

        let border = Style::default().fg(palette.muted);
        let border_active = primary;
        let border_subtle = Style::default().fg(palette.border_subtle);
        let suggestion = Style::default().fg(palette.suggestion);

        let code_inline = Style::default().fg(palette.code_inline);
        let code_block = Style::default().fg(palette.code_block);
        let link = Style::default()
            .fg(palette.link)
            .add_modifier(ratatui::style::Modifier::UNDERLINED);
        let tag = Style::default().fg(palette.accent);

        let cursor = selection;

        Self {
            primary,
            secondary,
            accent,
            muted,
            success,
            warning,
            error: danger,
            info,
            text_primary,
            bg_secondary: palette.surface,
            text_secondary: palette.text_secondary,
            text_secondary_style: text_secondary,
            text_muted,
            header_primary,
            header_secondary,
            selection,
            focus,
            border_subtle,
            suggestion,
            code_inline,
            code_block,
            link,
            tag,
            thinking: Style::default().fg(palette.thinking),
            streaming: Style::default().fg(palette.streaming),
            user: Style::default().fg(palette.user),
            assistant: Style::default().fg(palette.assistant),
            tool: Style::default().fg(palette.tool),
            system: Style::default().fg(palette.system),
            background: Style::default()
                .bg(palette.background)
                .fg(palette.text_primary),
            surface: Style::default()
                .bg(palette.surface)
                .fg(palette.text_primary),

            border,
            border_active,
            inactive: muted,
            text: text_primary,
            header: header_primary,
            status: muted,
            cursor,
        }
    }

    /// Constructs a standardized TUI panel block with rounded borders, padded title, and theme styling.
    pub fn panel<'a>(&self, title: &str, has_focus: bool) -> ratatui::widgets::Block<'a> {
        let border_style = if has_focus {
            self.border_active
        } else {
            self.border
        };
        let mut block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(border_style)
            .style(self.background);

        let trimmed = title.trim();
        if !trimmed.is_empty() {
            block = block.title(format!(" {} ", trimmed));
        }
        block
    }

    /// Constructs a standardized TUI input container block with rounded borders and theme styling.
    pub fn input<'a>(&self, has_focus: bool) -> ratatui::widgets::Block<'a> {
        let border_style = if has_focus {
            self.border_active
        } else {
            self.border
        };
        ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(border_style)
            .style(self.background)
    }

    /// Checks if theme uses uncolored/reset profile.
    pub fn is_no_color(&self) -> bool {
        self.text_secondary == ratatui::style::Color::Reset
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
            ThemeToken::Danger => self.error,
            ThemeToken::Info => self.info,
            ThemeToken::TextPrimary => self.text_primary,
            ThemeToken::TextSecondary => self.text_secondary_style,
            ThemeToken::TextMuted => self.text_muted,
            ThemeToken::HeaderPrimary => self.header_primary,
            ThemeToken::HeaderSecondary => self.header_secondary,
            ThemeToken::Selection => self.selection,
            ThemeToken::Focus => self.focus,
            ThemeToken::Border => self.border,
            ThemeToken::BorderActive => self.border_active,
            ThemeToken::BorderSubtle => self.border_subtle,
            ThemeToken::Suggestion => self.suggestion,
            ThemeToken::Cursor => self.cursor,
            ThemeToken::CodeInline => self.code_inline,
            ThemeToken::CodeBlock => self.code_block,
            ThemeToken::Link => self.link,
            ThemeToken::Tag => self.tag,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{dark_theme, high_contrast_theme, light_theme, terminal_theme};

    const ALL_TOKENS: &[ThemeToken] = &[
        ThemeToken::Primary,
        ThemeToken::Secondary,
        ThemeToken::Accent,
        ThemeToken::Muted,
        ThemeToken::Success,
        ThemeToken::Warning,
        ThemeToken::Danger,
        ThemeToken::Info,
        ThemeToken::TextPrimary,
        ThemeToken::TextSecondary,
        ThemeToken::TextMuted,
        ThemeToken::HeaderPrimary,
        ThemeToken::HeaderSecondary,
        ThemeToken::Selection,
        ThemeToken::Focus,
        ThemeToken::Border,
        ThemeToken::BorderActive,
        ThemeToken::BorderSubtle,
        ThemeToken::Suggestion,
        ThemeToken::Cursor,
        ThemeToken::CodeInline,
        ThemeToken::CodeBlock,
        ThemeToken::Link,
        ThemeToken::Tag,
        ThemeToken::Thinking,
        ThemeToken::Streaming,
        ThemeToken::User,
        ThemeToken::Assistant,
        ThemeToken::Tool,
        ThemeToken::System,
        ThemeToken::Background,
        ThemeToken::Surface,
    ];

    #[test]
    fn test_all_tokens_resolve_without_panic() {
        let themes: &[&Theme] = &[
            dark_theme(),
            light_theme(),
            terminal_theme(),
            high_contrast_theme(),
        ];

        for theme in themes {
            for token in ALL_TOKENS {
                let style = theme.style(*token);
                // Invariant check: resolution returns a valid Style object
                let _ = style;
            }
        }
    }
}
