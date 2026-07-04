# Sessions Epic Design Specification

This document details the architectural design and TUI interaction models for the Sessions Epic in the Brain workspace.

---

## 🏗️ Architectural Overview & Component Design

The Sessions Epic shifts from structural TUI infrastructure to high-signal user-facing session management. Ephemeral user interface interaction logic is encapsulated cleanly inside a dedicated `SidebarInteraction` model, preserving the domain model's invariants and emitting semantic events to the main application loop.

```text
                  TUI Dispatcher / InputRouter
                               │
                               ▼ (Raw Key Event)
                      SidebarInteraction
                               │
            (Fuzzy Search / Inline Editor / Mode State)
                               │
                               ▼ (Emits SidebarEvent)
                         Application
                               │
                               ▼ (Mutates Domain)
                            AppState
```

---

## 💾 Component Design & State Structures

All types and logic are located under `crates/brain-tui/src/ui/interaction/sidebar.rs` (new module) and integrated with the main dispatcher.

### 1. Sidebar Modes & Filters
```rust
use crate::ui::interaction::editor::Editor;
use brain_domain::SessionId;

/// Filter determining which sessions are visible in the sidebar list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionFilter {
    /// Show only active sessions.
    Active,
    /// Show only archived sessions.
    Archived,
}

/// Active mode of the sidebar interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarMode {
    /// Regular navigation and browsing.
    Browse,
    /// Inline editing of the selected session's title.
    Rename,
}

/// State for active browsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BrowseState {
    /// Currently selected session ID.
    pub selected: Option<SessionId>,
    /// Current filter applied to the session list.
    pub filter: SessionFilter,
}

/// State for session searching overlay.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// Is the search bar overlay active.
    pub active: bool,
    /// Local text editor for typing search queries.
    pub editor: Editor,
}

/// State for inline session title renaming.
#[derive(Debug, Clone)]
pub struct RenameState {
    /// Local text editor for typing the new session title.
    pub editor: Editor,
}
```

### 2. Main Sidebar Interaction Model
```rust
/// Local ephemeral interaction state for the sidebar.
#[derive(Debug, Clone)]
pub struct SidebarInteraction {
    /// Active mode (Browse or Rename).
    pub mode: SidebarMode,
    /// Browse selection and filtering.
    pub browse: BrowseState,
    /// Search state overlay.
    pub search: SearchState,
    /// Inline rename state.
    pub rename: RenameState,
}

impl SidebarInteraction {
    /// Returns a new default `SidebarInteraction` instance.
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
            },
            rename: RenameState {
                editor: Editor::new(),
            },
        }
    }

    /// Triggers entry into Search mode.
    pub fn enter_search(&mut self) {
        self.search.active = true;
    }

    /// Leaves Search mode.
    pub fn leave_search(&mut self, clear: bool) {
        self.search.active = false;
        if clear {
            self.search.editor.clear();
        }
    }

    /// Triggers entry into Inline Rename mode, pre-filling the title and moving cursor to the end.
    pub fn enter_rename(&mut self, current_title: &str) {
        self.mode = SidebarMode::Rename;
        self.rename.editor.clear();
        for c in current_title.chars() {
            self.rename.editor.insert_char(c);
        }
        self.rename.editor.move_to_end();
    }

    /// Leaves Rename mode.
    pub fn leave_rename(&mut self) {
        self.mode = SidebarMode::Browse;
        self.rename.editor.clear();
    }

    /// Returns a mutable reference to the currently active editor if one exists.
    pub fn active_editor(&mut self) -> Option<&mut Editor> {
        if self.mode == SidebarMode::Rename {
            Some(&mut self.rename.editor)
        } else if self.search.active {
            Some(&mut self.search.editor)
        } else {
            None
        }
    }
}
```

---

## 🔍 Fuzzy Search Engine

We introduce a pre-parsed, memory-efficient search query representation to avoid re-allocating/re-splitting during keystroke filtering:

```rust
/// Pre-parsed search query terms.
pub struct SearchQuery<'a> {
    pub terms: Vec<&'a str>,
}

impl<'a> SearchQuery<'a> {
    /// Formulates a new search query by split-parsing whitespace terms.
    pub fn parse(query: &'a str) -> Self {
        Self {
            terms: query.split_whitespace().collect(),
        }
    }
}

/// Matches a session title against pre-parsed query terms case-insensitively.
pub fn fuzzy_match(title: &str, query: &SearchQuery<'_>) -> bool {
    if query.terms.is_empty() {
        return true;
    }
    let title_lower = title.to_lowercase();
    for term in &query.terms {
        if !title_lower.contains(&term.to_lowercase()) {
            return false;
        }
    }
    true
}
```

---

## 📥 Interaction Event Mappings

```rust
/// Semantic intents emitted from the sidebar interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEvent {
    /// Select and load a session.
    OpenSession(SessionId),
    /// Commit rename to a session.
    RenameSession(SessionId, String),
    /// Pin/unpin a session.
    TogglePin(SessionId),
    /// Move a session to archived.
    ArchiveSession(SessionId),
    /// Delete a session permanently.
    DeleteSession(SessionId),
    /// Restore an archived session back to active.
    RestoreSession(SessionId),
}
```

### Key Handling Routine
```rust
impl SidebarInteraction {
    /// Processes keyboard events, updating local buffers and returning optional semantic actions.
    pub fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        visible_ids: &[SessionId],
        current_title_fn: &dyn Fn(SessionId) -> Option<String>,
    ) -> (bool, Option<SidebarEvent>) {
        use crossterm::event::KeyCode;

        // If there's an active editor (Rename or Search), let it intercept text editing keys first
        if let Some(editor) = self.active_editor() {
            match key.code {
                KeyCode::Esc => {
                    if self.mode == SidebarMode::Rename {
                        self.leave_rename();
                    } else if self.search.active {
                        self.leave_search(true);
                    }
                    return (true, None);
                }
                KeyCode::Enter => {
                    if self.mode == SidebarMode::Rename {
                        let active_id = self.browse.selected;
                        self.leave_rename();
                        if let Some(id) = active_id {
                            let title = self.rename.editor.buffer().to_string();
                            return (true, Some(SidebarEvent::RenameSession(id, title)));
                        }
                    } else if self.search.active {
                        // Exit search on Enter
                        self.leave_search(false);
                        if let Some(id) = self.browse.selected {
                            if visible_ids.contains(&id) {
                                return (true, Some(SidebarEvent::OpenSession(id)));
                            }
                        }
                    }
                    return (true, None);
                }
                // Maintain list arrow navigation even during search overlay
                KeyCode::Up if self.search.active => {
                    self.navigate_selection(visible_ids, -1);
                    return (true, None);
                }
                KeyCode::Down if self.search.active => {
                    self.navigate_selection(visible_ids, 1);
                    return (true, None);
                }
                _ => {
                    let handled = editor.handle_key(key);
                    return (handled, None);
                }
            }
        }

        // Default Browse Mode keys
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
                    (true, Some(SidebarEvent::OpenSession(id)))
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
                        return (true, Some(SidebarEvent::ArchiveSession(id)));
                    }
                }
                (false, None)
            }
            KeyCode::Char('r') => {
                if self.browse.filter == SessionFilter::Archived {
                    if let Some(id) = self.browse.selected {
                        return (true, Some(SidebarEvent::RestoreSession(id)));
                    }
                }
                (false, None)
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                if let Some(id) = self.browse.selected {
                    (true, Some(SidebarEvent::DeleteSession(id)))
                } else {
                    (false, None)
                }
            }
            KeyCode::Char('a') => {
                self.browse.filter = match self.browse.filter {
                    SessionFilter::Active => SessionFilter::Archived,
                    SessionFilter::Archived => SessionFilter::Active,
                };
                self.browse.selected = visible_ids.first().copied();
                (true, None)
            }
            KeyCode::Char('/') => {
                self.enter_search();
                (true, None)
            }
            KeyCode::Char('e') => {
                if let Some(id) = self.browse.selected {
                    if let Some(title) = current_title_fn(id) {
                        self.enter_rename(&title);
                        return (true, None);
                    }
                }
                (false, None)
            }
            _ => (false, None),
        }
    }

    fn navigate_selection(&mut self, visible_ids: &[SessionId], delta: i32) {
        if visible_ids.is_empty() {
            self.browse.selected = None;
            return;
        }
        let current_pos = self.browse.selected
            .and_then(|id| visible_ids.iter().position(|&x| x == id))
            .unwrap_or(0);
        let new_pos = (current_pos as i32 + delta)
            .clamp(0, visible_ids.len() as i32 - 1) as usize;
        self.browse.selected = Some(visible_ids[new_pos]);
    }
}
```

---

## 🎨 Rendering & Theme Mappings

Presentation layouts are isolated completely from the interaction module. The `Sidebar` widget renders:
1. **Header Display**: `Conversations (Active)` or `Conversations (Archived)` based on `SessionFilter`.
2. **Search Input Row**: Displays if `SearchState.active` is true.
3. **Session Item Row**:
   * Pinned indicator rendering: `📌` or `[P]` prepended to pinned active sessions.
   * Rename bracket editing: Selected row rendered as `▶ [TitleEditorBuffer]` when `SidebarMode::Rename` is active.

---

## 🧪 Verification Plan

### Automated Test Cases (`tests/sidebar_interaction_tests.rs`)
* **Capability Fingerprint & Determinism**: Validates state machine is deterministic under repeated transitions.
* **Fuzzy Matching Split Words**: Verifies search query parsed terms fuzzy match correctly.
* **Selection Stability Invariant**: Changing filters/searches preserves selection if the selected `SessionId` is present in the new set, and defaults safely otherwise.
* **Search Persistence**: Leaving and re-entering search preserves buffer query state.
* **Rename Command**: Intercepts title modification and emits exactly one `RenameSession` event.
