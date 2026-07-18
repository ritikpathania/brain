//! Sidebar Interaction state core and ParsedQuery caching.

use crate::ui::interaction::editor::Editor;
use brain_domain::SessionId;

/// Filter options for filtering sessions in the sidebar list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionFilter {
    /// Show active, non-archived sessions.
    Active,
    /// Show archived sessions.
    Archived,
}

/// The interaction modes the sidebar can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarMode {
    /// Normal browsing / selection mode.
    Browse,
    /// Renaming a specific session.
    Rename,
}

/// The state of the browsing interface in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BrowseState {
    /// Currently selected session ID, if any.
    pub selected: Option<SessionId>,
    /// The active session filter.
    pub filter: SessionFilter,
}

/// A parsed fuzzy search query compiled into lowercase terms.
#[derive(Debug, Clone, Default)]
pub struct ParsedQuery {
    /// Lowercase query terms parsed from the raw input query.
    pub terms: Vec<String>,
}

impl ParsedQuery {
    /// Updates the parsed terms by splitting the raw query string by whitespace and lowercasing.
    pub fn update(&mut self, raw_query: &str) {
        self.terms = raw_query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
    }

    /// Clears the parsed terms.
    pub fn clear(&mut self) {
        self.terms.clear();
    }

    /// Returns true if the query has no search terms.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Evaluates if the given title matches all parsed search terms.
    pub fn matches(&self, title: &str) -> bool {
        if self.terms.is_empty() {
            return true;
        }
        let title_lower = title.to_lowercase();
        for term in &self.terms {
            if !title_lower.contains(term) {
                return false;
            }
        }
        true
    }
}

/// The state of the search/filtering buffer in the sidebar.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// Whether the search mode/input is active.
    pub active: bool,
    /// The text editor instance containing the raw query text.
    pub editor: Editor,
    /// The parsed query terms.
    pub parsed: ParsedQuery,
}

/// The state of the rename interaction in the sidebar.
#[derive(Debug, Clone)]
pub struct RenameState {
    /// The text editor instance containing the session title being edited.
    pub editor: Editor,
}

/// Core coordination state machine for sidebar interactions.
#[derive(Debug, Clone)]
pub struct SidebarInteraction {
    /// Active mode of interaction in the sidebar.
    pub mode: SidebarMode,
    /// Browsing and selection state.
    pub browse: BrowseState,
    /// Searching and filtering state.
    pub search: SearchState,
    /// Renaming state.
    pub rename: RenameState,
}

impl SidebarInteraction {
    /// Creates a new `SidebarInteraction` with default values.
    pub fn new() -> Self {
        Self {
            mode: SidebarMode::Browse,
            browse: BrowseState {
                selected: None,
                filter: SessionFilter::Active,
            },
            search: SearchState {
                active: false,
                editor: Editor::new(),
                parsed: ParsedQuery::default(),
            },
            rename: RenameState {
                editor: Editor::new(),
            },
        }
    }

    /// Enters search mode.
    pub fn enter_search(&mut self) {
        self.search.active = true;
    }

    /// Leaves search mode, optionally clearing the editor and parsed terms.
    pub fn leave_search(&mut self, clear: bool) {
        self.search.active = false;
        if clear {
            self.search.editor.clear();
            self.search.parsed.clear();
        }
    }

    /// Enters rename mode, seeding the rename editor with the current session title.
    pub fn enter_rename(&mut self, current_title: &str) {
        self.mode = SidebarMode::Rename;
        self.rename.editor.clear();
        for c in current_title.chars() {
            self.rename.editor.insert_char(c);
        }
        self.rename.editor.move_to_end();
    }

    /// Leaves rename mode and clears the rename editor.
    pub fn leave_rename(&mut self) {
        self.mode = SidebarMode::Browse;
        self.rename.editor.clear();
    }

    /// Handles key events for the sidebar, routing them based on the current mode.
    pub fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        visible_ids: &[SessionId],
        lookup: &dyn SessionLookup,
    ) -> (bool, Option<SidebarEvent>) {
        use crossterm::event::KeyCode;

        if self.mode == SidebarMode::Rename {
            return self.handle_rename_key(key, visible_ids);
        }

        if self.search.active {
            return self.handle_search_key(key, visible_ids);
        }

        match key.code {
            KeyCode::Up => {
                self.navigate_selection(visible_ids, -1);
                (true, None)
            }
            KeyCode::Down => {
                self.navigate_selection(visible_ids, 1);
                (true, None)
            }
            KeyCode::Enter => {
                if let Some(id) = self.browse.selected {
                    (true, Some(SidebarEvent::Open(id)))
                } else {
                    (false, None)
                }
            }
            KeyCode::Char('p') => {
                if let Some(id) = self.browse.selected {
                    (true, Some(SidebarEvent::TogglePin(id)))
                } else {
                    (false, None)
                }
            }
            KeyCode::Char('c') => {
                if self.browse.filter == SessionFilter::Active {
                    if let Some(id) = self.browse.selected {
                        return (true, Some(SidebarEvent::Archive(id)));
                    }
                }
                (false, None)
            }
            KeyCode::Char('r') => {
                if self.browse.filter == SessionFilter::Archived {
                    if let Some(id) = self.browse.selected {
                        return (true, Some(SidebarEvent::Restore(id)));
                    }
                }
                (false, None)
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                if let Some(id) = self.browse.selected {
                    (true, Some(SidebarEvent::Delete(id)))
                } else {
                    (false, None)
                }
            }
            KeyCode::Char('a') => {
                self.browse.filter = match self.browse.filter {
                    SessionFilter::Active => SessionFilter::Archived,
                    SessionFilter::Archived => SessionFilter::Active,
                };
                self.restore_selection_fallback(visible_ids);
                (true, None)
            }
            KeyCode::Char('/') => {
                self.enter_search();
                (true, None)
            }
            KeyCode::Char('e') => {
                if let Some(id) = self.browse.selected {
                    if let Some(title) = lookup.title(id) {
                        self.enter_rename(title);
                        return (true, None);
                    }
                }
                (false, None)
            }
            _ => (false, None),
        }
    }

    fn handle_rename_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        _visible_ids: &[SessionId],
    ) -> (bool, Option<SidebarEvent>) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.leave_rename();
                (true, None)
            }
            KeyCode::Enter => {
                let active_id = self.browse.selected;
                let title_raw = self.rename.editor.buffer().trim().to_string();
                let title_opt = if title_raw.is_empty() {
                    None
                } else {
                    Some(title_raw)
                };
                self.leave_rename();
                if let Some(id) = active_id {
                    (true, Some(SidebarEvent::Rename(id, title_opt)))
                } else {
                    (true, None)
                }
            }
            _ => {
                let handled = self.rename.editor.handle_key(key);
                (handled, None)
            }
        }
    }

    fn handle_search_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        visible_ids: &[SessionId],
    ) -> (bool, Option<SidebarEvent>) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.leave_search(true);
                (true, None)
            }
            KeyCode::Enter => {
                self.leave_search(false);
                if let Some(id) = self.browse.selected {
                    if visible_ids.contains(&id) {
                        return (true, Some(SidebarEvent::Open(id)));
                    }
                }
                (true, None)
            }
            KeyCode::Up => {
                self.navigate_selection(visible_ids, -1);
                (true, None)
            }
            KeyCode::Down => {
                self.navigate_selection(visible_ids, 1);
                (true, None)
            }
            _ => {
                let handled = self.search.editor.handle_key(key);
                if handled {
                    self.search.parsed.update(self.search.editor.buffer());
                    self.restore_selection_fallback(visible_ids);
                }
                (handled, None)
            }
        }
    }

    fn navigate_selection(&mut self, visible_ids: &[SessionId], delta: i32) {
        if visible_ids.is_empty() {
            self.browse.selected = None;
            return;
        }
        let current_pos = self
            .browse
            .selected
            .and_then(|id| visible_ids.iter().position(|&x| x == id))
            .unwrap_or(0);
        let new_pos = (current_pos as i32 + delta).clamp(0, visible_ids.len() as i32 - 1) as usize;
        self.browse.selected = Some(visible_ids[new_pos]);
    }

    /// Restores the selected session fallback if the current selection is no longer valid.
    pub fn restore_selection_fallback(&mut self, visible_ids: &[SessionId]) {
        if visible_ids.is_empty() {
            self.browse.selected = None;
            return;
        }
        if let Some(selected_id) = self.browse.selected {
            if visible_ids.contains(&selected_id) {
                return;
            }
        }
        self.browse.selected = visible_ids.first().copied();
    }
}

impl Default for SidebarInteraction {
    fn default() -> Self {
        Self::new()
    }
}

/// Events dispatched by sidebar actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEvent {
    /// Open the session with the specified ID.
    Open(SessionId),
    /// Rename the session, or cancel if title is None.
    Rename(SessionId, Option<String>),
    /// Toggle pinning status of a session.
    TogglePin(SessionId),
    /// Archive a session.
    Archive(SessionId),
    /// Delete a session.
    Delete(SessionId),
    /// Restore an archived session.
    Restore(SessionId),
}

/// Interface for looking up session titles during interaction validation.
pub trait SessionLookup {
    /// Returns the title of the session if it exists.
    fn title(&self, id: SessionId) -> Option<&str>;
}
