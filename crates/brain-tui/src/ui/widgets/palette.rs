//! Modal centered overlay renderer for Command Palette.

use crate::ui::command::palette::{CommandPaletteState, PaletteStage};
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

/// Command Palette overlay widget.
pub struct CommandPaletteWidget<'a> {
    /// Command palette interaction state.
    pub state: &'a CommandPaletteState,
    /// Active theme tokens.
    pub theme: &'a Theme,
}

impl<'a> CommandPaletteWidget<'a> {
    /// Creates a new `CommandPaletteWidget`.
    pub fn new(state: &'a CommandPaletteState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    /// Renders the command palette widget into the frame area.
    pub fn draw(&self, f: &mut Frame<'_>, area: Rect) {
        draw(f, area, self.state, self.theme);
    }
}

/// Renders the modal centered Command Palette overlay.
pub fn draw(f: &mut Frame<'_>, area: Rect, state: &CommandPaletteState, theme: &Theme) {
    // Clear the modal outer area
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Command Palette ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.border_active);

    // Split the modal area into input query bar (height 3) and search results list area (remaining)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input query bar
            Constraint::Min(3),    // List of matches
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
            let cursor_x = chunks[0].x + state.editor.cursor().visual_col;
            let cursor_y = chunks[0].y;
            if cursor_x < chunks[0].right() {
                f.set_cursor(cursor_x, cursor_y);
            }

            // 2. Draw list of matching search results
            let items: Vec<ListItem> = if state.search_aggregator.is_some() {
                state
                    .results()
                    .into_iter()
                    .enumerate()
                    .map(|(idx, res)| {
                        let is_selected = idx == state.selected_index;

                        let (prefix, prefix_style) = match res.kind {
                            crate::ui::search::types::SearchResultKind::Command => {
                                (" [CMD] ", theme.accent)
                            }
                            crate::ui::search::types::SearchResultKind::Session => {
                                (" [SES] ", theme.primary)
                            }
                            crate::ui::search::types::SearchResultKind::Message => {
                                (" [MSG] ", theme.inactive)
                            }
                            crate::ui::search::types::SearchResultKind::Knowledge => {
                                (" [MEM] ", theme.cursor)
                            }
                        };

                        let line = ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(prefix, prefix_style),
                            ratatui::text::Span::styled(
                                res.title.as_deref().unwrap_or("(untitled)").to_string(),
                                if is_selected {
                                    theme
                                        .primary
                                        .add_modifier(ratatui::style::Modifier::REVERSED)
                                } else {
                                    theme.text
                                },
                            ),
                            ratatui::text::Span::styled(" - ", theme.inactive),
                            ratatui::text::Span::styled(
                                res.subtitle.as_deref().unwrap_or("").to_string(),
                                theme.inactive,
                            ),
                        ]);

                        ListItem::new(line)
                    })
                    .collect()
            } else {
                let matches = state.matches();
                matches
                    .iter()
                    .enumerate()
                    .map(|(idx, cmd)| {
                        let is_selected = idx == state.selected_index;
                        let (name_style, cat_style, desc_style) = if is_selected {
                            (
                                theme
                                    .style(ThemeToken::Accent)
                                    .add_modifier(Modifier::BOLD)
                                    .add_modifier(Modifier::REVERSED),
                                theme
                                    .style(ThemeToken::TextMuted)
                                    .add_modifier(Modifier::REVERSED),
                                theme
                                    .style(ThemeToken::TextSecondary)
                                    .add_modifier(Modifier::REVERSED),
                            )
                        } else {
                            (
                                theme
                                    .style(ThemeToken::Accent)
                                    .add_modifier(Modifier::BOLD),
                                theme.style(ThemeToken::TextMuted),
                                theme.style(ThemeToken::TextSecondary),
                            )
                        };

                        let col1 = format!("  {:<20}", cmd.name);
                        let col2 = format!("{:<15}", cmd.category.label());
                        let col3 = cmd.description.to_string();

                        let line = ratatui::text::Line::from(vec![
                            ratatui::text::Span::styled(col1, name_style),
                            ratatui::text::Span::styled(col2, cat_style),
                            ratatui::text::Span::styled(col3, desc_style),
                        ]);

                        ListItem::new(line)
                    })
                    .collect()
            };

            let list = List::new(items).style(theme.text);
            f.render_widget(list, chunks[1]);
        }
        PaletteStage::CollectParameter(_state) => {
            // Placeholder parameters view
            let p = Paragraph::new("Collecting parameters...").style(theme.text);
            f.render_widget(p, chunks[1]);
        }
        PaletteStage::Confirm { .. } => {
            // Placeholder confirmation view
            let p = Paragraph::new("Confirm command execution...").style(theme.text);
            f.render_widget(p, chunks[1]);
        }
    }
}
