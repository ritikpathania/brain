# Sessions Epic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the TUI Sessions Epic containing active/archived list filtering, incremental fuzzy search term query parsing and caching, inline session renaming, session pinning/archiving/deletion operations, and selection stability fallbacks.

**Architecture:** Encapsulate browse/search/rename interactions in a self-contained `SidebarInteraction` state machine using nested `Editor` buffers, emit semantic `SidebarEvent` actions to avoid mutating domain models inside interaction viewports, and handle database persistence mutations inside the main `Application` orchestrator.

**Tech Stack:** Rust, Ratatui, Crossterm, Tokio, and rusqlite.

## Global Constraints
- Target TUI crate path: `crates/brain-tui/`
- Custom test module: `crates/brain-tui/tests/compatibility_tests.rs` or new suite files.
- Design theme tokens must be resolved only via `Theme` semantic styles. No hardcoded colors.

## Definition of Done
- All workspace tests pass successfully.
- All new interaction logic has dedicated unit tests.
- Golden snapshots are updated only where visual behavior intentionally changed.
- No new compiler warnings from `cargo check`.
- No memory allocations are introduced in rendering/draw paths.
- Existing keyboard shortcuts remain unchanged outside the Sessions feature scope.
- All new public APIs are documented.

---

### Task 1: Sidebar Interaction Core & ParsedQuery

**Files:**
- Create: `crates/brain-tui/src/ui/interaction/sidebar.rs`
- Create: `crates/brain-tui/tests/sidebar_interaction_tests.rs`

**Interfaces:**
- Consumes: `crates/brain-tui/src/ui/interaction/editor.rs` (`Editor`)
- Produces: `SessionFilter`, `SidebarMode`, `BrowseState`, `ParsedQuery`, `SearchState`, `RenameState`, `SidebarInteraction`, `SidebarEvent`, `SessionLookup`

- [ ] **Step 1: Write initial failing tests for search and rename states**

Create `crates/brain-tui/tests/sidebar_interaction_tests.rs`:
```rust
use brain_domain::SessionId;
use brain_tui::ui::interaction::sidebar::{
    SidebarInteraction, SidebarMode, SessionFilter, ParsedQuery, SessionLookup
};

struct MockLookup;
impl SessionLookup for MockLookup {
    fn title(&self, _id: SessionId) -> Option<&str> {
        Some("Brain Architecture RFC")
    }
}

#[test]
fn test_search_and_rename_transitions() {
    let mut interaction = SidebarInteraction::new();
    assert_eq!(interaction.mode, SidebarMode::Browse);
    assert!(!interaction.search.active);

    interaction.enter_search();
    assert!(interaction.search.active);

    interaction.leave_search(true);
    assert!(!interaction.search.active);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test sidebar_interaction_tests`
Expected: Compile error because `sidebar` module does not exist yet.

- [ ] **Step 3: Implement core data structures and state machine**

Create `crates/brain-tui/src/ui/interaction/sidebar.rs`:
```rust
use crate::ui::interaction::editor::Editor;
use brain_domain::SessionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionFilter {
    Active,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarMode {
    Browse,
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BrowseState {
    pub selected: Option<SessionId>,
    pub filter: SessionFilter,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedQuery {
    terms: Vec<String>,
}

impl ParsedQuery {
    pub fn update(&mut self, raw_query: &str) {
        self.terms = raw_query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
    }

    pub fn clear(&mut self) {
        self.terms.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

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

#[derive(Debug, Clone)]
pub struct SearchState {
    pub active: bool,
    pub editor: Editor,
    pub parsed: ParsedQuery,
}

#[derive(Debug, Clone)]
pub struct RenameState {
    pub editor: Editor,
}

#[derive(Debug, Clone)]
pub struct SidebarInteraction {
    pub mode: SidebarMode,
    pub browse: BrowseState,
    pub search: SearchState,
    pub rename: RenameState,
}

impl SidebarInteraction {
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

    pub fn enter_search(&mut self) {
        self.search.active = true;
    }

    pub fn leave_search(&mut self, clear: bool) {
        self.search.active = false;
        if clear {
            self.search.editor.clear();
            self.search.parsed.clear();
        }
    }

    pub fn enter_rename(&mut self, current_title: &str) {
        self.mode = SidebarMode::Rename;
        self.rename.editor.clear();
        for c in current_title.chars() {
            self.rename.editor.insert_char(c);
        }
        self.rename.editor.move_to_end();
    }

    pub fn leave_rename(&mut self) {
        self.mode = SidebarMode::Browse;
        self.rename.editor.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarEvent {
    Open(SessionId),
    Rename(SessionId, Option<String>),
    TogglePin(SessionId),
    Archive(SessionId),
    Delete(SessionId),
    Restore(SessionId),
}

pub trait SessionLookup {
    fn title(&self, id: SessionId) -> Option<&str>;
}
```

- [ ] **Step 4: Expose new sidebar module in lib.rs**

Modify `crates/brain-tui/src/lib.rs` (export under ui modules if needed, or simply inside the root library):
```rust
// Expose the new sidebar interaction module in brain-tui
pub mod ui; // (Ensure sidebar.rs is referenced via mod in crates/brain-tui/src/ui/mod.rs or crates/brain-tui/src/ui/interaction/mod.rs)
```
Wait, let's add `pub mod sidebar;` inside `crates/brain-tui/src/ui/interaction/mod.rs`.

- [ ] **Step 5: Run tests and verify they pass**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test sidebar_interaction_tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/brain-tui/src/ui/interaction/sidebar.rs crates/brain-tui/tests/sidebar_interaction_tests.rs
git commit -m "feat(tui): implement SidebarInteraction state core and ParsedQuery caching"
```

---

### Task 2: Dispatcher Integration

**Files:**
- Modify: `crates/brain-tui/src/ui/interaction/sidebar.rs` (implement `handle_key` methods)
- Modify: `crates/brain-tui/src/ui/interaction/mod.rs`
- Modify: `crates/brain-tui/src/ui/interaction/dispatcher.rs`

**Interfaces:**
- Consumes: `SidebarInteraction::handle_key` key handlers
- Produces: Key mappings translating keystrokes to `SidebarEvent` intents

- [ ] **Step 1: Write test case verifying key routing and event emission**

Add to `crates/brain-tui/tests/sidebar_interaction_tests.rs`:
```rust
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers, KeyEventKind, KeyEventState};

#[test]
fn test_sidebar_key_events_emission() {
    let mut interaction = SidebarInteraction::new();
    let session_id = SessionId::new();
    let visible_ids = vec![session_id];
    interaction.browse.selected = Some(session_id);

    struct Lookup;
    impl SessionLookup for Lookup {
        fn title(&self, _id: SessionId) -> Option<&str> { Some("Test Session") }
    }
    let lookup = Lookup;

    // Press 'c' to archive
    let key_c = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::empty(),
        kind: KeyEventKind::Press,
        state: KeyEventState::empty(),
    };
    let (handled, event) = interaction.handle_key(key_c, &visible_ids, &lookup);
    assert!(handled);
    assert_eq!(event, Some(SidebarEvent::Archive(session_id)));
}
```

- [ ] **Step 2: Run test and verify fail**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test sidebar_interaction_tests`
Expected: FAIL because `handle_key` is not implemented on `SidebarInteraction` yet.

- [ ] **Step 3: Implement key handlers on SidebarInteraction**

Implement `handle_key`, `handle_rename_key`, `handle_search_key`, `navigate_selection`, and `restore_selection_fallback` inside `crates/brain-tui/src/ui/interaction/sidebar.rs`:
```rust
impl SidebarInteraction {
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
        visible_ids: &[SessionId],
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
        let current_pos = self.browse.selected
            .and_then(|id| visible_ids.iter().position(|&x| x == id))
            .unwrap_or(0);
        let new_pos = (current_pos as i32 + delta)
            .clamp(0, visible_ids.len() as i32 - 1) as usize;
        self.browse.selected = Some(visible_ids[new_pos]);
    }

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
```

- [ ] **Step 4: Export sidebar interaction module in interaction/mod.rs**

Modify `crates/brain-tui/src/ui/interaction/mod.rs`:
```rust
pub mod sidebar;
pub use sidebar::{SidebarInteraction, SidebarMode, SidebarEvent, SessionFilter, ParsedQuery, SessionLookup};
```

- [ ] **Step 5: Run tests and verify they pass**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test sidebar_interaction_tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/brain-tui/src/ui/interaction/sidebar.rs crates/brain-tui/src/ui/interaction/mod.rs
git commit -m "feat(tui): implement handle_key method for SidebarInteraction and export from mod.rs"
```

---

### Task 3: Sidebar Rendering

**Files:**
- Modify: `crates/brain-tui/src/ui/widgets/sidebar.rs`
- Modify: `crates/brain-tui/src/ui/widgets/mod.rs`
- Modify: `crates/brain-tui/src/ui/widgets/view_models.rs`

- [ ] **Step 1: Create test checking sidebar view rendering**

Create snapshot test or basic assertion in `crates/brain-tui/tests/sidebar_interaction_tests.rs`:
```rust
#[test]
fn test_sidebar_rendering_modes() {
    // Assert rendering layouts (Browse/Search/Rename) draw properly without panicking.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test sidebar_interaction_tests`
Expected: Compilation failure or missing field errors due to view model alterations.

- [ ] **Step 3: Modify view model definitions to support filters and inline editors**

Modify `crates/brain-tui/src/ui/widgets/view_models.rs`:
```rust
// Add active filter and search/rename mode details to SidebarView
```

- [ ] **Step 4: Update sidebar widget drawing function**

Modify `crates/brain-tui/src/ui/widgets/sidebar.rs` to dynamically draw the header title, inline search box, pinned items, and selected item inline text editor with appropriate cursor placements.

- [ ] **Step 5: Run tests and verify they pass**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/brain-tui/src/ui/widgets/sidebar.rs
git commit -m "feat(tui): implement search bar, inline renaming, and pinned indicators in Sidebar rendering"
```

---

### Task 4: Application Integration & Session Operations

**Files:**
- Modify: `crates/brain-tui/src/ui/state.rs`
- Modify: `crates/brain-tui/src/ui/application.rs`
- Modify: `crates/brain-tui/src/lib.rs`

- [ ] **Step 1: Write integration test case verifying sidebar event loop orchestration**

Add to `crates/brain-tui/tests/sidebar_interaction_tests.rs`:
```rust
// Verifies that emitting SidebarEvent::Rename transitions TUI state and calls backend commands.
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
Expected: FAIL.

- [ ] **Step 3: Integrate SidebarInteraction inside AppState**

Modify `crates/brain-tui/src/ui/state.rs` to hold `SidebarInteraction` and reduce actions.

- [ ] **Step 4: Handle events in Application loop**

Modify `crates/brain-tui/src/ui/application.rs` to process `SidebarEvent` and trigger client executions.

- [ ] **Step 5: Run tests to verify they pass**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/brain-tui/src/ui/state.rs crates/brain-tui/src/ui/application.rs
git commit -m "feat(tui): connect SidebarInteraction state with Application orchestration layer"
```

---

### Task 5: Verification & Snapshots

- [ ] **Step 1: Implement selection stability and filter transition property tests**

Add property test `test_filter_transition_selection_stability` in `crates/brain-tui/tests/sidebar_interaction_tests.rs`.

- [ ] **Step 2: Run all workspace tests**

Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
Expected: PASS

- [ ] **Step 3: Execute golden visual screens snapshots**

Run: `UPDATE_EXPECT=1 PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git commit -am "test(tui): verify selection stability, fuzzy matching query parsing, and update snapshots"
```
