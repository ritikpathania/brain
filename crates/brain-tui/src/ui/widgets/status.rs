use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;

/// ViewModel carrying visual help labels or runtime diagnostics.
pub struct StatusView {
    /// Status line display message.
    pub message: String,
}

/// Renders the footer status/help line.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &StatusView, theme: &Theme) {
    let block = Block::default();

    let p = Paragraph::new(view.message.as_str())
        .block(block)
        .style(theme.status);

    f.render_widget(p, area);
}
