use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use crate::ui::theme::Theme;

/// ViewModel carrying the prompt text, cursor position, and focus status.
pub struct PromptView {
    /// Text content inside prompt editor buffer.
    pub prompt_text: String,
    /// Terminal cell index where typing cursor sits.
    pub cursor_position: usize,
    /// Whether the editor currently has focus.
    pub has_focus: bool,
}

/// Renders the input prompt bar widget.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &PromptView, theme: &Theme) {
    let border_style = if view.has_focus {
        theme.border_active
    } else {
        theme.border
    };

    let block = Block::default()
        .title(" Prompt ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let p = Paragraph::new(view.prompt_text.as_str())
        .block(block)
        .style(theme.text);


    f.render_widget(p, area);

    // Set cursor position.
    if area.width > 2 && area.height > 2 {
        let max_x = area.x + area.width - 2;
        let cursor_x = (area.x + 1).saturating_add(view.cursor_position as u16);
        let cursor_y = area.y + 1;
        if cursor_x <= max_x {
            f.set_cursor(cursor_x, cursor_y);
        }
    }
}
