//! Inline slash commands autocompletion engine and visual state representation.

use crate::ui::command::{CommandDescriptor, CommandVisibility, COMMANDS};

/// Visual state tracker for active inline slash completion popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCompletionState {
    /// True if the autocompletion suggestions popup should be displayed.
    pub visible: bool,
    /// Currently highlighted match item index in the suggestions popup.
    pub selected_index: usize,
    /// Cached query string representing the slice of prompt text being autocompleted.
    pub query: String,
    /// Optional query snapshot that was explicitly dismissed by Esc key.
    pub dismissed_query: Option<String>,
}

impl SlashCompletionState {
    /// Instantiates a new default (inactive) SlashCompletionState.
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_index: 0,
            query: String::new(),
            dismissed_query: None,
        }
    }

    /// Select next item in slash completion list.
    pub fn select_next(&mut self) {
        let count = SlashCompletionEngine::matches(&self.query).count();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
        }
    }

    /// Select previous item in slash completion list.
    pub fn select_prev(&mut self) {
        let count = SlashCompletionEngine::matches(&self.query).count();
        if count > 0 {
            self.selected_index = if self.selected_index > 0 {
                self.selected_index - 1
            } else {
                count - 1
            };
        }
    }

    /// Returns reference to the currently selected CommandDescriptor if matches exist.
    pub fn selected_command(&self) -> Option<&'static CommandDescriptor> {
        SlashCompletionEngine::matches(&self.query).nth(self.selected_index)
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
        COMMANDS.iter().filter(move |cmd| {
            is_slash
                && cmd.visibility != CommandVisibility::PaletteOnly
                && (cmd.title.to_lowercase().contains(&term)
                    || cmd
                        .aliases
                        .iter()
                        .any(|alias| alias.to_lowercase().contains(&term)))
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
        crate::ui::layout::SlashCompletionGeometry::compute(
            screen_area,
            geometry.prompt_area,
            count,
        )
    }
}
