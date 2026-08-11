use crate::state::UiState;
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Draws the workspace dashboard full-width task table.
pub fn draw(f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
    if area.width < 40 || area.height < 10 {
        return;
    }

    let mut lines = Vec::new();

    // 1. Agent Header Block
    lines.push(Line::from(vec![
        Span::styled("▄▀▀  ", theme.style(ThemeToken::Accent)),
        Span::styled(
            "Claude Code ",
            theme
                .style(ThemeToken::HeaderPrimary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("v2.1.226", theme.style(ThemeToken::TextMuted)),
    ]));
    lines.push(Line::from(vec![Span::styled(
        "Opus 5 (1M context) · ~/Developer/PyCharm/brain",
        theme.style(ThemeToken::TextMuted),
    )]));
    lines.push(Line::from(vec![
        Span::styled("4 awaiting input", theme.style(ThemeToken::Accent)),
        Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
        Span::styled("0 working", theme.style(ThemeToken::TextMuted)),
        Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
        Span::styled("17 completed", theme.style(ThemeToken::TextSecondary)),
    ]));
    lines.push(Line::from(""));

    // 2. Background Navigation Banner
    lines.push(Line::from(Span::styled(
        "Your conversation moved to the background — enter opens it · esc returns to it",
        theme
            .style(ThemeToken::TextMuted)
            .add_modifier(Modifier::ITALIC),
    )));
    lines.push(Line::from(""));

    // 3. Needs Input Table Section
    lines.push(Line::from(Span::styled(
        "Needs input",
        theme
            .style(ThemeToken::TextPrimary)
            .add_modifier(Modifier::BOLD),
    )));

    if state.sessions.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("* ", theme.style(ThemeToken::Accent)),
            Span::styled(
                format!("{:<30}", "current session"),
                theme
                    .style(ThemeToken::TextPrimary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<35}", "brain"),
                theme.style(ThemeToken::TextSecondary),
            ),
            Span::styled("2s", theme.style(ThemeToken::TextMuted)),
        ]));
    } else {
        for (idx, session) in state.sessions.iter().enumerate() {
            let is_sel = idx == state.selected_session_idx;
            let style = if is_sel {
                theme.style(ThemeToken::Selection)
            } else {
                theme.style(ThemeToken::TextPrimary)
            };
            let prefix = if is_sel { "* " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(prefix, theme.style(ThemeToken::Accent)),
                Span::styled(format!("{:<30}", session.title), style),
                Span::styled(
                    format!("{:<35}", "active"),
                    theme.style(ThemeToken::TextMuted),
                ),
                Span::styled("1m", theme.style(ThemeToken::TextMuted)),
            ]));
        }
    }

    lines.push(Line::from(""));

    // 4. Completed Section
    lines.push(Line::from(Span::styled(
        "Completed",
        theme
            .style(ThemeToken::TextPrimary)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(vec![
        Span::styled("· ", theme.style(ThemeToken::TextMuted)),
        Span::styled(
            format!("{:<30}", "bg"),
            theme.style(ThemeToken::TextSecondary),
        ),
        Span::styled(
            format!("{:<35}", "(idle - send a prompt to start)"),
            theme.style(ThemeToken::TextMuted),
        ),
        Span::styled("11h", theme.style(ThemeToken::TextMuted)),
    ]));

    let p = Paragraph::new(lines);
    f.render_widget(p, area);
}
