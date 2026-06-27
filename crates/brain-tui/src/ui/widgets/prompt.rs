use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use crate::ui::theme::Theme;

/// ViewModel carrying the prompt text and cursor position.
pub struct PromptView {
    /// Text content inside prompt editor buffer.
    pub prompt_text: String,
    /// Terminal cell index where typing cursor sits.
    pub cursor_position: usize,
}

/// Renders the input prompt bar widget.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &PromptView, theme: &Theme) {
    let block = Block::default()
        .title(" Prompt ")
        .borders(Borders::ALL)
        .border_style(theme.border_active);

    let p = Paragraph::new(view.prompt_text.as_str())
        .block(block)
        .style(theme.text);

    f.render_widget(p, area);

    // Set cursor position.
    let cursor_x = area.x + 1 + view.cursor_position as u16;
    let cursor_y = area.y + 1;
    if cursor_x < area.x + area.width - 1 {
        f.set_cursor(cursor_x, cursor_y);
    }
}
