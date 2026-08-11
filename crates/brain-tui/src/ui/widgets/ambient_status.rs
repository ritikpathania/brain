use crate::state::UiState;
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Renders the AmbientStatusWidget right-aligned status line.
pub fn draw(f: &mut Frame<'_>, area: Rect, _state: &UiState, theme: &Theme) {
    let text = Line::from(vec![
        Span::styled("● ", theme.style(ThemeToken::Success)),
        Span::styled("xhigh", theme.style(ThemeToken::TextSecondary)),
        Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
        Span::styled("/effort", theme.style(ThemeToken::TextMuted)),
    ]);
    let p = Paragraph::new(text).alignment(ratatui::layout::Alignment::Right);
    f.render_widget(p, area);
}
