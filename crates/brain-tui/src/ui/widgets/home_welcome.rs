use crate::state::UiState;
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Renders the HomeWelcomeWidget surface.
pub fn draw(f: &mut Frame<'_>, area: Rect, _state: &UiState, theme: &Theme) {
    if area.width < 40 || area.height < 6 {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.style(ThemeToken::HeaderPrimary))
        .title(Line::from(vec![
            Span::styled("─", theme.style(ThemeToken::HeaderPrimary)),
            Span::styled(
                " Claude ",
                theme
                    .style(ThemeToken::HeaderPrimary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Code ",
                theme
                    .style(ThemeToken::HeaderPrimary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("v2.1.226 ", theme.style(ThemeToken::TextMuted)),
        ]));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split inner area horizontally into Left Welcome Pane (58% / min 45 cols), Divider (1 col), and Right Information Rail
    let left_width = (inner.width * 58 / 100).max(45);
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_width),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let left_area = chunks[0];
    let divider_area = chunks[1];
    let right_area = chunks[2];

    // Render Vertical Divider at boundary (column x=47) in subtle grey (#505050 / RGB(80, 80, 80))
    let buf = f.buffer_mut();
    let divider_style = theme.style(ThemeToken::BorderSubtle);
    for y in divider_area.y..(divider_area.y + divider_area.height) {
        if divider_area.x < buf.area.width && y < buf.area.height {
            buf.get_mut(divider_area.x, y)
                .set_symbol("│")
                .set_style(divider_style);
        }
    }

    // Left Welcome Pane Content
    let left_lines = vec![
        Line::from(Span::styled(
            "Welcome back!",
            theme
                .style(ThemeToken::TextPrimary)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("    ▄▀▀▀▄", theme.style(ThemeToken::Accent))),
        Line::from(Span::styled("    █ █ █", theme.style(ThemeToken::Accent))),
        Line::from(Span::styled(
            "Think once. Remember.",
            theme.style(ThemeToken::TextMuted),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Opus 5 (1M context) with xhigh",
                theme.style(ThemeToken::TextSecondary),
            ),
            Span::styled(" · ", theme.style(ThemeToken::TextMuted)),
            Span::styled("API Usage Billing", theme.style(ThemeToken::TextMuted)),
        ]),
        Line::from(Span::styled(
            "~/Developer/PyCharm/brain",
            theme.style(ThemeToken::TextMuted),
        )),
    ];
    let left_p = Paragraph::new(left_lines);
    f.render_widget(left_p, left_area);

    // Right Information Rail Content
    let right_lines = vec![
        Line::from(Span::styled(
            "Tips for getting started",
            theme
                .style(ThemeToken::Accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Run /init to create a ...",
            theme.style(ThemeToken::TextPrimary),
        )),
        Line::from(Span::styled(
            "─────────────────────────────",
            theme.style(ThemeToken::BorderSubtle),
        )),
        Line::from(Span::styled(
            "What's new",
            theme
                .style(ThemeToken::Accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "Bug fixes and reliabil...",
            theme.style(ThemeToken::TextSecondary),
        )),
        Line::from(Span::styled(
            "Added gateway spend-li...",
            theme.style(ThemeToken::TextSecondary),
        )),
        Line::from(Span::styled(
            "/release-notes for more",
            theme
                .style(ThemeToken::TextMuted)
                .add_modifier(Modifier::ITALIC),
        )),
    ];
    let right_p = Paragraph::new(right_lines);
    f.render_widget(right_p, right_area);
}

