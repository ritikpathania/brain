use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// ViewModel carrying the prompt text, cursor position, and focus status.
pub struct PromptView {
    /// Text content inside prompt editor buffer.
    pub prompt_text: String,
    /// Terminal cell index where typing cursor sits.
    pub cursor_position: usize,
    /// Whether the editor currently has focus.
    pub has_focus: bool,
    /// When true, the next submission will include the Active Workspace context.
    /// Renders as `[With WS]` in the prompt border title.
    pub submit_with_workspace: bool,
    /// Whether this prompt is being rendered on the Welcome/Home product landing page.
    pub is_welcome: bool,
}

/// Renders the input prompt bar widget.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &PromptView, theme: &Theme) {
    let prefix = "❯ ";

    let prefix_span = ratatui::text::Span::styled(prefix, theme.style(ThemeToken::Accent));

    let divider_line = ratatui::text::Line::from(vec![ratatui::text::Span::styled(
        "─".repeat(area.width as usize),
        theme.style(ThemeToken::BorderSubtle),
    )]);

    let placeholder_text = if view.is_welcome {
        "Ask anything or type / for commands..."
    } else {
        "Ask a question or type / for commands..."
    };

    let content_line = if view.prompt_text.is_empty() {
        let placeholder_style = theme
            .style(ThemeToken::TextMuted)
            .add_modifier(ratatui::style::Modifier::ITALIC);
        ratatui::text::Line::from(vec![
            prefix_span,
            ratatui::text::Span::styled(placeholder_text, placeholder_style),
        ])
    } else {
        ratatui::text::Line::from(vec![
            prefix_span,
            ratatui::text::Span::styled(&view.prompt_text, theme.style(ThemeToken::TextPrimary)),
        ])
    };

    if area.height >= 3 {
        let p = Paragraph::new(vec![divider_line.clone(), content_line, divider_line]);
        f.render_widget(p, area);
    } else {
        let p = Paragraph::new(content_line);
        f.render_widget(p, area);
    }

    if area.width > 2 && area.height > 1 {
        let prefix_len = prefix.chars().count() as u16;
        let max_x = area.x + area.width.saturating_sub(1);
        let cursor_x = (area.x + prefix_len).saturating_add(view.cursor_position as u16);
        let cursor_y = if area.height >= 3 { area.y + 1 } else { area.y };
        if cursor_x <= max_x {
            f.set_cursor(cursor_x, cursor_y);
        }
    }
}
