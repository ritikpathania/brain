//! Modal centered overlay renderer for Command Palette.

use ratatui::layout::{Rect, Layout, Constraint, Direction};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Clear};
use ratatui::Frame;
use crate::ui::theme::Theme;
use crate::ui::command::palette::{CommandPaletteState, PaletteStage};

/// Renders the modal centered Command Palette overlay.
pub fn draw(f: &mut Frame<'_>, area: Rect, state: &CommandPaletteState, theme: &Theme) {
    // Clear the modal outer area
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Command Palette ")
        .borders(Borders::ALL)
        .border_style(theme.border_active);

    // Split the modal area into input query bar (height 3) and search results list area (remaining)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input query bar
            Constraint::Min(3),   // List of matches
        ])
        .split(block.inner(area));

    // Draw the main container block
    f.render_widget(block, area);

    // Stage-based rendering
    match &state.stage {
        PaletteStage::Search => {
            // 1. Draw input query bar
            let query_block = Block::default()
                .borders(Borders::BOTTOM)
                .border_style(theme.border);
            let query_p = Paragraph::new(state.editor.text())
                .block(query_block)
                .style(theme.text);
            f.render_widget(query_p, chunks[0]);

            // Draw typing cursor in input bar
            let cursor_x = chunks[0].x + state.editor.cursor().visual_col as u16;
            let cursor_y = chunks[0].y;
            if cursor_x < chunks[0].right() {
                f.set_cursor(cursor_x, cursor_y);
            }

            // 2. Draw list of matching search results
            let items: Vec<ListItem> = if state.search_aggregator.is_some() {
                let results = state.results();
                results.iter().enumerate().map(|(idx, res)| {
                    let is_selected = idx == state.selected_index;
                    
                    let (prefix, prefix_style) = match res.kind {
                        crate::ui::search::types::SearchResultKind::Command => (" [CMD] ", theme.accent),
                        crate::ui::search::types::SearchResultKind::Session => (" [SES] ", theme.primary),
                        crate::ui::search::types::SearchResultKind::Message => (" [MSG] ", theme.inactive),
                    };

                    let line = ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(prefix, prefix_style),
                        ratatui::text::Span::styled(&res.title, if is_selected {
                            theme.primary.add_modifier(ratatui::style::Modifier::REVERSED)
                        } else {
                            theme.text
                        }),
                        ratatui::text::Span::styled(" - ", theme.inactive),
                        ratatui::text::Span::styled(&res.subtitle, theme.inactive),
                    ]);

                    ListItem::new(line)
                }).collect()
            } else {
                let matches: Vec<_> = state.matches().collect();
                matches.iter().enumerate().map(|(idx, cmd)| {
                    let style = if idx == state.selected_index {
                        theme.primary.add_modifier(ratatui::style::Modifier::REVERSED)
                    } else {
                        theme.text
                    };
                    let text = format!("  {} - {}", cmd.title, cmd.description);
                    ListItem::new(text).style(style)
                }).collect()
            };

            let list = List::new(items).style(theme.text);
            f.render_widget(list, chunks[1]);
        }
        PaletteStage::CollectParameter(_state) => {
            // Placeholder parameters view
            let p = Paragraph::new("Collecting parameters...")
                .style(theme.text);
            f.render_widget(p, chunks[1]);
        }
        PaletteStage::Confirm { .. } => {
            // Placeholder confirmation view
            let p = Paragraph::new("Confirm command execution...")
                .style(theme.text);
            f.render_widget(p, chunks[1]);
        }
    }
}
