//! Inline slash commands autocompletion engine and visual state representation.

use crate::ui::command::{COMMANDS, CommandDescriptor, CommandVisibility};

/// Visual state tracker for active inline slash completion popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCompletionState {
    /// True if the autocompletion suggestions popup should be displayed.
    pub visible: bool,
    /// Currently highlighted match item index in the suggestions popup.
    pub selected_index: usize,
    /// Cached query string representing the slice of prompt text being autocompleted.
    pub query: String,
}

impl SlashCompletionState {
    /// Instantiates a new default (inactive) SlashCompletionState.
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_index: 0,
            query: String::new(),
        }
    }
}

impl Default for SlashCompletionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Autocomplete query matching engine.
pub struct SlashCompletionEngine;

impl SlashCompletionEngine {
    /// Filters and returns an iterator over command descriptors that match the query prefix.
    pub fn matches(query: &str) -> impl Iterator<Item = &'static CommandDescriptor> {
        let is_slash = query.starts_with('/');
        let term = if is_slash {
            query[1..].to_lowercase()
        } else {
            String::new()
        };
        COMMANDS.iter()
            .filter(move |cmd| {
                is_slash
                    && cmd.visibility != CommandVisibility::PaletteOnly
                    && (cmd.title.to_lowercase().contains(&term)
                        || cmd.aliases.iter().any(|alias| alias.to_lowercase().contains(&term)))
            })
    }
}

impl crate::ui::layout::Overlay for SlashCompletionState {
    fn is_visible(&self) -> bool {
        self.visible
    }

    fn geometry(&self, screen_area: ratatui::layout::Rect) -> ratatui::layout::Rect {
        let geometry = crate::ui::layout::LayoutEngine::chat_screen(screen_area);
        let count = SlashCompletionEngine::matches(&self.query).count();
        crate::ui::layout::SlashCompletionGeometry::compute(screen_area, geometry.prompt_area, count)
    }
}

