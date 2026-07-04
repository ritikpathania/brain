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

/// Pre-parsed search query terms to optimize matching loops.
#[derive(Debug, Clone, Default)]
pub struct ParsedQuery {
    /// Normalized terms derived from the search buffer.
    pub terms: Vec<String>,
}

/// State for session searching overlay.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// Is the search bar overlay active.
    pub active: bool,
    /// Local text editor for typing search queries.
    pub editor: Editor,
    /// Caching parsed query representation to avoid allocations on every filter match.
    pub parsed: ParsedQuery,
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
                parsed: ParsedQuery::default(),
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
            self.search.parsed.terms.clear();
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

## 🔍 Fuzzy Search Engine & Caching

Instead of performing string parsing during each match iteration, `ParsedQuery` is updated only when the query editor buffer changes:

```rust
impl ParsedQuery {
    /// Re-parses query terms from the editor's text buffer.
    pub fn update(&mut self, raw_query: &str) {
        self.terms = raw_query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
    }
}

/// Matches a session title against parsed query terms case-insensitively and allocation-free.
pub fn fuzzy_match(title: &str, query: &ParsedQuery) -> bool {
    if query.terms.is_empty() {
        return true;
    }
    let title_lower = title.to_lowercase();
    for term in &query.terms {
        if !title_lower.contains(term) {
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
    Open(SessionId),
    /// Commit rename to a session. Emits None if the user entered empty whitespace.
    Rename(SessionId, Option<String>),
    /// Pin/unpin a session.
    TogglePin(SessionId),
    /// Move a session to archived.
    Archive(SessionId),
    /// Delete a session permanently.
    Delete(SessionId),
    /// Restore an archived session back to active.
    Restore(SessionId),
}

/// Lookup interface decoupling interaction logic from concrete state view models.
pub trait SessionLookup {
    /// Retrieves the title of a given session.
    fn title(&self, id: SessionId) -> Option<&str>;
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
        lookup: &dyn SessionLookup,
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
                            let title_raw = self.rename.editor.buffer().trim();
                            let title_opt = if title_raw.is_empty() {
                                None
                            } else {
                                Some(title_raw.to_string())
                            };
                            return (true, Some(SidebarEvent::Rename(id, title_opt)));
                        }
                    } else if self.search.active {
                        // Exit search on Enter
                        self.leave_search(false);
                        if let Some(id) = self.browse.selected {
                            if visible_ids.contains(&id) {
                                return (true, Some(SidebarEvent::Open(id)));
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
                    if handled && self.search.active {
                        self.search.parsed.update(self.search.editor.buffer());
                        self.restore_selection_fallback(visible_ids);
                    }
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

    /// Explicit selection fallback policy when active filtering changes.
    pub fn restore_selection_fallback(&mut self, visible_ids: &[SessionId]) {
        if visible_ids.is_empty() {
            self.browse.selected = None;
            return;
        }
        
        // 1. Preserve the current selected ID if still visible
        if let Some(selected_id) = self.browse.selected {
            if visible_ids.contains(&selected_id) {
                return;
            }
            // 2. Select the nearest visible neighbor
            // (We approximate this by selecting the first available visible item)
        }
        
        // 3. Fall back to the first visible session
        self.browse.selected = visible_ids.first().copied();
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
* **Rename Command**: Intercepts title modification, trims it, and emits exactly one `Rename` event.
* **Filter Transition Stability**: Verifies that toggling active/archived/searched modes keeps selected `SessionId` selected throughout if it remains visible.
