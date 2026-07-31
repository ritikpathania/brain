//! Floating list suggestions renderer for slash command autocompletion.

use crate::ui::command::completion::{SlashCompletionEngine, SlashCompletionState};
use crate::ui::theme::Theme;
use ratatui::layout::Rect;
use ratatui::widgets::{Clear, List, ListItem};
use ratatui::Frame;

/// Renders the floating slash command autocompletion popup list.
pub fn draw(f: &mut Frame<'_>, area: Rect, state: &SlashCompletionState, theme: &Theme) {
    let matches: Vec<_> = SlashCompletionEngine::matches(&state.query).collect();
    if matches.is_empty() {
        return;
    }

    let items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .map(|(idx, cmd)| {
            let style = if idx == state.selected_index {
                theme
                    .primary
                    .add_modifier(ratatui::style::Modifier::REVERSED)
            } else {
                theme.text
            };
            let text = format!("  /{} - {}", cmd.aliases.first().unwrap_or(&""), cmd.title);
            ListItem::new(text).style(style)
        })
        .collect();

    let block = theme.panel(" Commands ", true);

    let list = List::new(items).block(block).style(theme.text);

    // Clear background to prevent overlay bleeding
    f.render_widget(Clear, area);
    f.render_widget(list, area);
}
