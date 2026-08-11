use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
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

/// Renders the Header bar panel at the top of the interface using a clean single horizontal rule.
pub fn draw(f: &mut Frame<'_>, area: Rect, view: &HeaderView, theme: &Theme) {
    let status_style = if view.connection_color_ok {
        theme.success
    } else {
        theme.inactive
    };

    let width = area.width as usize;
    let left_title = format!(" {}", view.title);
    let right_status = format!("{} ", view.connection_status);

    let left_len = left_title.chars().count();
    let right_len = right_status.chars().count();
    let padding_len = width.saturating_sub(left_len + right_len);

    let header_line = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(left_title, theme.header),
        ratatui::text::Span::raw(" ".repeat(padding_len)),
        ratatui::text::Span::styled(right_status, status_style),
    ]);

    if area.height >= 2 {
        let divider_char = if area.width > 0 { "─" } else { "-" };
        let divider_line = ratatui::text::Line::from(vec![ratatui::text::Span::styled(
            divider_char.repeat(width),
            theme.border,
        )]);

        let p = Paragraph::new(vec![header_line, divider_line]);
        f.render_widget(p, area);
    } else {
        let p = Paragraph::new(header_line);
        f.render_widget(p, area);
    }
}
