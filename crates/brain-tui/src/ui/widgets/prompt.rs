use crate::ui::theme::Theme;
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
}

/// Renders the input prompt bar widget.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &PromptView, theme: &Theme) {
    let block = theme.input(view.has_focus);

    let prefix = if view.submit_with_workspace {
        "brain> "
    } else {
        "❯ "
    };

    let prefix_span = ratatui::text::Span::styled(
        prefix,
        theme.accent.add_modifier(ratatui::style::Modifier::BOLD),
    );

    let content_line = if view.prompt_text.is_empty() {
        let placeholder_style = theme
            .inactive
            .add_modifier(ratatui::style::Modifier::ITALIC);
        ratatui::text::Line::from(vec![
            prefix_span,
            ratatui::text::Span::styled("Type a message or / for commands...", placeholder_style),
        ])
    } else {
        ratatui::text::Line::from(vec![
            prefix_span,
            ratatui::text::Span::styled(&view.prompt_text, theme.text),
        ])
    };

    let p = Paragraph::new(content_line).block(block);

    f.render_widget(p, area);

    // Set cursor position accounting for prompt prefix length.
    if area.width > 2 && area.height > 2 {
        let prefix_len = prefix.chars().count() as u16;
        let max_x = area.x + area.width - 2;
        let cursor_x = (area.x + 1 + prefix_len).saturating_add(view.cursor_position as u16);
        let cursor_y = area.y + 1;
        if cursor_x <= max_x {
            f.set_cursor(cursor_x, cursor_y);
        }
    }
}
