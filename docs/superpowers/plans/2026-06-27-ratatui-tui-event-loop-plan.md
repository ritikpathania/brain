# Ratatui TUI Client Migration (Milestone 2: Event Loop & State Dispatcher) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the TUI's central interactive state manager, action mapping, and dispatch loop within the presentation crate `brain-tui`, ensuring that the main loop remains decoupled from the composition root (`main.rs`).

**Architecture:** Define `UiState` representing active rendering context (connection modes, input editor text, viewports). Create a centralized state dispatcher processing `Event` items (Keys, Ticks, App streams) in `brain-tui::run`.
We utilize a clean Action/Reducer pattern:
```
KeyEvent ──► Action ──► UiState::update() (Reducer) ──► UpdateResult ──► UiState
```

**Tech Stack:** Rust, Ratatui, Crossterm, Tokio.

## Global Constraints
- `brain-tui` remains a pure presentation layer crate.
- No direct database or runtime execution internals.
- Widgets never compute layout; the renderer coordinates cell allocation.
- Main loop orchestration lives inside `brain-tui::run`, not in `apps/brain-v2`.
- **Action/Reducer Invariant**: The run loop does not directly mutate state fields; all updates occur by passing structured `Action` enums to `UiState::update`.
- **Encapsulated Editing**: `EditorState` owns all line buffer invariants and exposes methods (`insert`, `backspace`, `move_left`, `move_right`) rather than raw string mutations.
- **UpdateResult Protocol**: `UiState::update` returns an `UpdateResult` enum (`NoChange`, `Changed`, `Exit`) to optimize redraw cycles and cleanly signal terminal shutdown.
- **UI-Oriented Action Names**: Action enum variants are named after presentation outcomes (e.g. `BeginAssistantResponse`, `AppendToken`, `FinishAssistantResponse`, `SetConnectionMode`) rather than transport event tags (`StreamStart`, `StreamChunk`).

---

### Task 1: UI State Structure & Reducer Actions

**Files:**
- Create: `crates/brain-tui/src/state.rs`

**Interfaces:**
- Consumes: `brain_domain::SessionId`.
- Produces: `UiState`, `Action`, `ConnectionMode`, `ViewportState`, `EditorState`, `UpdateResult` enum, and the `UiState::update` reducer method.

- [ ] **Step 1: Write a unit test for editor input transitions**
  Create `crates/brain-tui/src/state.rs` and verify typing characters mutates the editor buffer and cursor position.
- [ ] **Step 2: Define UiState & ConnectionMode**
  Implement `UiState`, `ConnectionMode`, `ViewportState`, and `EditorState` structures. Use `#![allow(dead_code)]` as needed during development.
- [ ] **Step 3: Implement Editor operations**
  Implement encapsulated `insert`, `backspace`, `delete`, `move_left`, and `move_right` actions on `EditorState`.
- [ ] **Step 4: Implement Action Enum, UpdateResult, and Reducer**
  Define `Action` carrying variants like `InsertChar(char)`, `MoveCursorLeft`, `MoveCursorRight`, `Backspace`, `Delete`, `Resize(u16, u16)`, and `Quit`. Implement `UpdateResult` enum (`NoChange`, `Changed`, `Exit`). Implement `UiState::update(&mut self, action: Action) -> UpdateResult` as the single state reducer.
- [ ] **Step 5: Expose state module in lib.rs**
  Add `pub mod state;` to `crates/brain-tui/src/lib.rs`.
- [ ] **Step 6: Run compiler check and unit tests**
  Verify code compiles and tests pass: `cargo test -p brain-tui`.
- [ ] **Step 7: Commit**
  Commit Task 1: `git add . && git commit -m "feat(tui): implement UiState and editor action transitions"`

---

### Task 2: Central Loop Dispatcher & Action Mapping

**Files:**
- Modify: `crates/brain-tui/src/lib.rs`

**Interfaces:**
- Consumes: `Event` and `UiState`.
- Produces: State transition dispatching inside `run()`.

- [ ] **Step 1: Write an event-loop transition test**
  Add a test in `crates/brain-tui/src/lib.rs` verifying that feeding keystrokes maps to structured `Action`s and modifies TUI state correctly.
- [ ] **Step 2: Update TUI run loop to use UiState**
  Refactor `run()` in `lib.rs` to maintain a mutable `UiState`.
- [ ] **Step 3: Implement Action Mapping**
  Map incoming terminal keystrokes to `Action` variants:
  - Text characters: `Action::InsertChar(c)`.
  - Left / Right arrows: `Action::MoveCursorLeft` / `MoveCursorRight`.
  - Backspace / Delete: `Action::Backspace` / `Action::Delete`.
  - Esc or Ctrl+C: `Action::Quit`.
  - Resize event: `Action::Resize(w, h)`.
  Pass mapped actions into `state.update(action)` and handle `UpdateResult` to conditionally trigger redraws or exit the loop.
- [ ] **Step 4: Run clippy and workspace test suite**
  Verify all 110 tests pass cleanly.
- [ ] **Step 5: Commit**
  Commit Task 2: `git add . && git commit -m "feat(tui): map keystrokes and dispatch state transitions in run loop"`
