//! Floating dropdown renderer for Command Palette.

use crate::ui::command::palette::{CommandPaletteState, PaletteStage};
use crate::ui::theme::{ActiveTheme, Theme, ThemeToken};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph};
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

/// Renders the floating dropdown Command Palette overlay.
pub fn draw(f: &mut Frame<'_>, area: Rect, state: &CommandPaletteState, theme: &Theme) {
    if !state.open || area.height == 0 || area.width == 0 {
        return;
    }

    // Clear the modal dropdown area
    f.render_widget(Clear, area);

    // Stage-based rendering
    match &state.stage {
        PaletteStage::Search => {
            let matches = state.matches();
            let selected_idx = state.selected_index;

            let suggestion_style = theme.style(ThemeToken::Suggestion);
            let desc_style_base = theme.style(ThemeToken::TextSecondary);
            let text_primary_style = theme.style(ThemeToken::TextPrimary);
            let selection_style = theme.style(ThemeToken::Selection);

            let items: Vec<ListItem> = matches
                .iter()
                .enumerate()
                .map(|(idx, cmd)| {
                    let is_selected = idx == selected_idx;

                    // Column 1: Name (e.g. "/session new")
                    let col1 = format!("  {:<22}", cmd.name);

                    // Column 2: "command ·" or "skill ·"
                    let category_label = if cmd.category.label().eq_ignore_ascii_case("skill")
                        || cmd.id.starts_with("skill")
                        || cmd.name.contains("skill")
                    {
                        "skill ·"
                    } else {
                        "command ·"
                    };
                    let col2 = format!("{:<14}", category_label);

                    // Column 3: Description
                    let col3 = cmd.description.to_string();

                    let (col1_style, col2_style, col3_style, line_style) = if is_selected {
                        (
                            text_primary_style
                                .patch(selection_style)
                                .add_modifier(Modifier::BOLD),
                            suggestion_style.patch(selection_style),
                            desc_style_base.patch(selection_style),
                            selection_style,
                        )
                    } else {
                        (
                            text_primary_style,
                            suggestion_style,
                            desc_style_base,
                            Style::default(),
                        )
                    };

                    let line = Line::from(vec![
                        Span::styled(col1, col1_style),
                        Span::styled(col2, col2_style),
                        Span::styled(col3, col3_style),
                    ]);

                    ListItem::new(line).style(line_style)
                })
                .collect();

            let list = List::new(items);
            f.render_widget(list, area);
        }
        PaletteStage::CollectParameter(_state) => {
            let p = Paragraph::new("Collecting parameters...").style(theme.text);
            f.render_widget(p, area);
        }
        PaletteStage::Confirm { .. } => {
            let p = Paragraph::new("Confirm command execution...").style(theme.text);
            f.render_widget(p, area);
        }
    }
}
