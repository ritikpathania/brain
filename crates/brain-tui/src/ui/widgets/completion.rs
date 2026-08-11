//! Floating borderless suggestions renderer for slash command autocompletion.
//! Visual style deliberately mirrors PaletteWidget — no box border, Clear background.

use crate::ui::command::completion::{SlashCompletionEngine, SlashCompletionState};
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

/// Renders the floating slash command autocompletion popup list.
pub fn draw(f: &mut Frame<'_>, area: Rect, state: &SlashCompletionState, theme: &Theme) {
    let matches: Vec<_> = SlashCompletionEngine::matches(&state.query).collect();
    if matches.is_empty() {
        return;
    }

    let lines: Vec<ratatui::text::Line> = matches
        .iter()
        .enumerate()
        .map(|(idx, cmd)| {
            let is_selected = idx == state.selected_index;
            let (cmd_style, desc_style) = if is_selected {
                (
                    theme.style(ThemeToken::Accent).add_modifier(Modifier::BOLD),
                    theme
                        .style(ThemeToken::TextPrimary)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    theme.style(ThemeToken::TextPrimary),
                    theme.style(ThemeToken::TextMuted),
                )
            };
            let name = cmd.aliases.first().copied().unwrap_or("");
            ratatui::text::Line::from(vec![
                ratatui::text::Span::styled("  ", theme.style(ThemeToken::TextMuted)),
                ratatui::text::Span::styled(format!("/{:<18}", name), cmd_style),
                ratatui::text::Span::styled(cmd.title, desc_style),
            ])
        })
        .collect();

    let render_height = (lines.len() as u16).min(area.height);
    let render_area = Rect::new(area.x, area.y, area.width, render_height);

    // Clear background to prevent overlay bleeding — no border box
    f.render_widget(Clear, render_area);
    f.render_widget(Paragraph::new(lines), render_area);
}
