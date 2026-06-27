use ratatui::layout::{Alignment, Rect};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use crate::ui::theme::Theme;

/// ViewModel carrying immutable presentation data for the Header widget.
pub struct HeaderView {
    /// Fully formatted window header title.
    pub title: String,
    /// Connection mode label (e.g. "[Embedded]", "[Daemon]").
    pub connection_status: String,
    /// Boolean indicator showing whether the connection is active.
    pub connection_color_ok: bool,
}

/// Renders the Header bar panel at the top of the interface.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &HeaderView, theme: &Theme) {
    let status_style = if view.connection_color_ok {
        theme.success
    } else {
        theme.inactive
    };

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border);

    let text = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(format!(" {} ", view.title), theme.header),
        ratatui::text::Span::styled(format!("  {}", view.connection_status), status_style),
    ]);
    let p = Paragraph::new(text)
        .block(title_block)
        .alignment(Alignment::Left);

    f.render_widget(p, area);
}
