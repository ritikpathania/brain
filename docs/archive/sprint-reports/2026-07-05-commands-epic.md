# Commands Epic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a modular, type-safe command execution pipeline including a multi-step modal Command Palette (`Ctrl+K`) and an inline slash-command completion helper (`/`).

**Architecture:** Use a static immutable command registry to define descriptors and parameters, a dedicated `CommandExecutor` that maps collected parameters to a declarative `ExecutionPlan` (effects), and route these plans through the central `Application` controller for state mutations and backend communication.

**Tech Stack:** Rust, crossterm, ratatui.

## Global Constraints
- Naming convention: Follow existing snake_case and camelCase models in the TUI client.
- Zero raw colors: Resolve layout styles strictly via the active TUI `Theme` styles.
- Non-blocking input: Command overlay inputs must not leak into background prompt or history scrolling.
- Authoritative reconciliation: Every optimistic state mutation must be reconcilable by subsequent backend events.

## Milestone Completion Criteria (Exit Criteria)
Every task in this plan must satisfy the following criteria before being considered complete:
1. `cargo test` compiles and passes successfully without any failures.
2. `cargo check` and `cargo clippy` report zero warnings.
3. Unit tests cover all pure business logic changes.
4. Property-based tests are added for complex state transitions.
5. Golden visual snapshots are updated only for intentional visual changes.
6. Public APIs are verified to be stable and frozen where indicated.

## Verification Strategy
- **Unit Tests**: Pure registry logic, completions matching, and `CommandExecutor` plan mappings.
- **Property-based Tests**: Testing state machine transitions under randomized key events.
- **Golden Snapshots**: Overlay modal alignment and popup rendering.
- **Integration Tests**: Focus transitions, input isolation, and `Application` event orchestration.
- **Replay/Determinism Tests**: Verification of event execution order.

---

### Task 1: Infrastructure (Public API Freeze Point) [COMPLETED]

---

### Task 2: Slash Commands [COMPLETED]

---

### Task 3: Command Palette UI

**Files:**
- Create: `crates/brain-tui/src/ui/command/palette.rs` (CommandPaletteState definition)
- Modify: `crates/brain-tui/src/ui/state.rs` (reference CommandPaletteState inside AppState)
- Modify: `crates/brain-tui/src/ui/focus.rs` (FocusTarget, FocusManager updates - use simple `Option<FocusTarget>` for `saved_focus`)
- Modify: `crates/brain-tui/src/ui/layout/mod.rs` (defining overlay bounds and `CommandPaletteGeometry::compute` clamping logic)
- Modify: `crates/brain-tui/src/ui/renderer.rs` (overlay rendering using the shared layout bounds)
- Test: `crates/brain-tui/tests/command_palette_tests.rs` (add overlay tests and double focus restoration test)

**Interfaces:**
- Consumes: `FocusManager`
- Produces: `FocusTarget::CommandPalette`, `saved_focus` state, `CommandPaletteGeometry` layout calculation

- [ ] **Step 1: Write the failing test for focus switching & double restoration**
  Write a test showing that saving and popping focus operates correctly on `FocusManager` multiple times without retaining stale saved focus.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  use brain_tui::ui::focus::{FocusManager, FocusTarget, FocusProfile};

  #[test]
  fn test_focus_restoration_cycle() {
      let mut fm = FocusManager::new(FocusTarget::Sidebar, FocusProfile::Chat);
      
      // First cycle
      let saved1 = fm.current();
      fm.save_focus(saved1);
      fm.set_current(FocusTarget::CommandPalette);
      assert_eq!(fm.current(), FocusTarget::CommandPalette);

      let restored1 = fm.pop_saved_focus().expect("Should have saved focus");
      fm.set_current(restored1);
      assert_eq!(fm.current(), FocusTarget::Sidebar);
      assert!(fm.pop_saved_focus().is_none(), "Saved focus must be cleared after restoration");

      // Second cycle
      let saved2 = fm.current();
      fm.save_focus(saved2);
      fm.set_current(FocusTarget::CommandPalette);
      assert_eq!(fm.current(), FocusTarget::CommandPalette);

      let restored2 = fm.pop_saved_focus().expect("Should have saved focus");
      fm.set_current(restored2);
      assert_eq!(fm.current(), FocusTarget::Sidebar);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Create `crates/brain-tui/src/ui/command/palette.rs` containing `CommandPaletteState` and the stage/arguments enums.
  Add `FocusTarget::CommandPalette` and `save_focus`/`pop_saved_focus` methods on `FocusManager` inside `crates/brain-tui/src/ui/focus.rs`.
  Define `CommandPaletteGeometry::compute(terminal: Rect) -> Rect` in the layout files (clamping width: min 40, max 80; height: min 8, max 15).
  Update `crates/brain-tui/src/ui/renderer.rs` to render a centered bordered box when the palette is open.

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/brain-tui/src/ui/command/palette.rs
  git commit -am "feat(tui): add Command Palette overlay bounds, rendering, and focus targets"
  ```

---

### Task 4: Parameter Collection

**Files:**
- Modify: `crates/brain-tui/src/ui/command/palette.rs`
- Modify: `crates/brain-tui/src/ui/state.rs`
- Modify: `crates/brain-tui/src/ui/interaction/dispatcher.rs`

**Interfaces:**
- Consumes: `CollectedParameter`, `ParameterCollectionState`, `PaletteStage`
- Produces: State transition from `Search` -> `CollectParameter` -> `Confirm`

- [ ] **Step 1: Write the failing test for collection transitions**
  Write a test showing that committing a command transitions `PaletteStage` to `CollectParameter`.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  // Setup CommandPaletteState, select CHANGE_THEME, press Enter, assert stage is CollectParameter.
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Move `ParameterValue`, `CollectedParameter`, `ParameterCollectionState`, and `PaletteStage` into `crates/brain-tui/src/ui/command/palette.rs`.
  Implement key bindings for navigation and collection inside the dispatcher so pressing `Enter` moves the stage state forward or updates the query.

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(tui): implement multi-step parameter collection state transitions"
  ```

---

### Task 5: Execution Pipeline

**Files:**
- Create: `crates/brain-tui/src/ui/command/executor.rs`
- Modify: `crates/brain-tui/src/ui/application.rs`

**Interfaces:**
- Consumes: `CommandInvocation`
- Produces: `ExecutionPlan` mapping to optimistic state updates and `BackendCommand` transport events

- [ ] **Step 1: Write the failing test for pure execution planning**
  Write a test showing that `CommandExecutor` generates the correct `ExecutionPlan` with a backend command and a local theme mutation.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  use brain_tui::ui::command::executor::{CommandExecutor, CommandInvocation, LocalStateMutation};
  use brain_tui::ui::command::ThemeId;

  #[test]
  fn test_executor_theme_plan() {
      let invocation = CommandInvocation::ChangeTheme { theme: ThemeId("dark") };
      let plan = CommandExecutor::plan(invocation);
      
      assert_eq!(plan.mutations.len(), 1);
      assert!(matches!(plan.mutations[0], LocalStateMutation::ApplyTheme(ThemeId("dark"))));
      assert_eq!(plan.backend_commands.len(), 1);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL

- [ ] **Step 3a: Write minimal implementation for CommandExecutor (pure mapping)**
  Create `crates/brain-tui/src/ui/command/executor.rs` implementing `CommandInvocation`, `LocalStateMutation`, `ExecutionPlan`, and `CommandExecutor` mappings.
  Run unit tests to verify correctness of pure mappings.

- [ ] **Step 3b: Integrate ExecutionPlan with Application loop (async orchestration)**
  Update `crates/brain-tui/src/ui/application.rs` to process mutations on `AppState` and forward backend command effects to the client.
  
  > **Reconciliation Invariant Note**: Ensure all optimistic updates (such as renaming or deleting sessions) correspond directly to downstream event handlers for reconciliation.
  > **Transport Invariant Note**: `ExecutionPlan` is kept transport-agnostic; it simply declares abstract backend effects to keep clean architectural boundaries.

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/brain-tui/src/ui/command/executor.rs
  git commit -am "feat(tui): implement pure CommandExecutor and Application execution loops"
  ```

---

### Task 6: Verification & Integration Tests

**Files:**
- Modify: `crates/brain-tui/tests/command_palette_tests.rs`

- [ ] **Step 1: Write detailed integration tests**
  Write tests covering keyboard navigation (`Up`/`Down`), fuzzy matching, and replay loops.
  
- [ ] **Step 2: Run all tests in the workspace**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
  Expected: PASS
  
- [ ] **Step 3: Update snapshots**
  Run: `UPDATE_EXPECT=1 PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test`
  Expected: PASS with golden snapshot updates.

- [ ] **Step 4: Commit**
  ```bash
  git commit -am "test(tui): verify Command Palette and Slash Commands with integration tests"
  ```
