use crate::ui::theme::Theme;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Stylize;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// ViewModel carrying immutable presentation data for the Header widget.
pub struct HeaderView {
    /// Fully formatted window header title.
    pub title: String,
    /// Connection mode label (e.g. "[Embedded]", "[Daemon]").
    pub connection_status: String,
    /// Boolean indicator showing whether the connection is active.
    pub connection_color_ok: bool,
    /// Boolean toggle state for showing/hiding reflection logs.
    pub enable_reflection_logs: bool,
    /// Count of pinned nodes in the working context.
    pub pins_count: usize,
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

    let mut spans = vec![
        ratatui::text::Span::styled(format!(" {} ", view.title), theme.header),
        ratatui::text::Span::styled(format!("  {}", view.connection_status), status_style),
    ];
    if view.enable_reflection_logs {
        spans.push(ratatui::text::Span::styled(
            "  [Reflection Logs]",
            theme.accent,
        ));
    }
    if view.pins_count > 0 {
        spans.push(ratatui::text::Span::styled(
            format!("  📌 Context ({})", view.pins_count),
            theme.accent.bold(),
        ));
    }

    let text = ratatui::text::Line::from(spans);
    let p = Paragraph::new(text)
        .block(title_block)
        .alignment(Alignment::Left);

    f.render_widget(p, area);
}
