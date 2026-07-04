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
