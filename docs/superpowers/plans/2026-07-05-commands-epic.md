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

### Task 1: Infrastructure (Public API Freeze Point)

**Files:**
- Create: `crates/brain-tui/src/ui/command/mod.rs`
- Test: `crates/brain-tui/tests/command_palette_tests.rs` (new test suite file)

**Interfaces (Frozen for Phase 2):**
- Consumes: None
- Produces: `CommandId`, `ThemeId`, `ModelId`, `SessionTitle`, `ParameterId`, `ParameterKind`, `ParameterDescriptor`, `CommandDescriptor`, `COMMANDS`, `CommandRegistry`, `AvailabilityReason`, `Availability`, `CommandAvailabilityContext`, `CommandPolicy`

- [ ] **Step 1: Write the failing test for lookup**
  Write a test that verifies `CommandRegistry::find_by_id` and `CommandRegistry::find_by_name_or_alias` resolve static descriptors correctly.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  use brain_tui::ui::command::{CommandRegistry, CHANGE_THEME, RENAME_SESSION};

  #[test]
  fn test_command_registry_lookups() {
      let change_theme = CommandRegistry::find_by_id(CHANGE_THEME).unwrap();
      assert_eq!(change_theme.title, "Change Theme");

      let rename_session = CommandRegistry::find_by_name_or_alias("rename").unwrap();
      assert_eq!(rename_session.id, RENAME_SESSION);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL with compilation error (module command doesn't exist).

- [ ] **Step 3: Write minimal implementation**
  Create `crates/brain-tui/src/ui/command/mod.rs` and add it to `crates/brain-tui/src/lib.rs` (as `pub mod command` under `pub mod ui`).
  
  ```rust
  // crates/brain-tui/src/ui/command/mod.rs
  use brain_domain::SessionId;

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub struct CommandId(pub &'static str);

  pub const CHANGE_THEME: CommandId = CommandId("theme.change");
  pub const RENAME_SESSION: CommandId = CommandId("session.rename");
  pub const ARCHIVE_SESSION: CommandId = CommandId("session.archive");
  pub const DELETE_SESSION: CommandId = CommandId("session.delete");
  pub const RESTORE_SESSION: CommandId = CommandId("session.restore");
  pub const SWITCH_MODEL: CommandId = CommandId("model.switch");
  pub const CLEAR_CHAT: CommandId = CommandId("chat.clear");
  pub const SHOW_HELP: CommandId = CommandId("help.show");

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub struct ThemeId(pub &'static str);

  pub const THEME_DARK: ThemeId = ThemeId("dark");
  pub const THEME_HIGH_CONTRAST: ThemeId = ThemeId("high_contrast");

  #[derive(Debug, Clone, PartialEq, Eq, Hash)]
  pub struct ModelId(pub String);

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct SessionTitle(pub String);

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub struct ParameterId(pub &'static str);

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum ParameterKind {
      String,
      Boolean,
      Theme,
      Session,
      Model,
      File,
  }

  pub struct ParameterDescriptor {
      pub id: ParameterId,
      pub name: &'static str,
      pub description: &'static str,
      pub kind: ParameterKind,
      pub required: bool,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum CommandCategory {
      Settings,
      Sessions,
      Models,
      Navigation,
      Developer,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum CommandVisibility {
      SlashOnly,
      PaletteOnly,
      Both,
  }

  pub struct CommandDescriptor {
      pub id: CommandId,
      pub title: &'static str,
      pub description: &'static str,
      pub category: CommandCategory,
      pub visibility: CommandVisibility,
      pub priority: u16,
      pub aliases: &'static [&'static str],
      pub keywords: &'static [&'static str],
      pub parameters: &'static [ParameterDescriptor],
  }

  pub static COMMANDS: &[CommandDescriptor] = &[
      CommandDescriptor {
          id: CHANGE_THEME,
          title: "Change Theme",
          description: "Switch the TUI appearance mode",
          category: CommandCategory::Settings,
          visibility: CommandVisibility::Both,
          priority: 50,
          aliases: &["theme"],
          keywords: &["appearance", "dark", "light", "color", "style"],
          parameters: &[
              ParameterDescriptor {
                  id: ParameterId("theme"),
                  name: "theme",
                  description: "Theme name to apply",
                  kind: ParameterKind::Theme,
                  required: true,
              }
          ],
      },
      CommandDescriptor {
          id: RENAME_SESSION,
          title: "Rename Session",
          description: "Change the title of the current session",
          category: CommandCategory::Sessions,
          visibility: CommandVisibility::PaletteOnly,
          priority: 100,
          aliases: &["rename"],
          keywords: &["session", "title", "name", "edit"],
          parameters: &[
              ParameterDescriptor {
                  id: ParameterId("title"),
                  name: "title",
                  description: "New session title",
                  kind: ParameterKind::String,
                  required: true,
              }
          ],
      },
  ];

  pub struct CommandRegistry;

  impl CommandRegistry {
      pub fn iter() -> impl Iterator<Item = &'static CommandDescriptor> {
          COMMANDS.iter()
      }

      pub fn find_by_id(id: CommandId) -> Option<&'static CommandDescriptor> {
          Self::iter().find(|cmd| cmd.id == id)
      }

      pub fn find_by_name_or_alias(name: &str) -> Option<&'static CommandDescriptor> {
          let name_lower = name.to_lowercase();
          // NOTE: A future performance optimization would be to store pre-lowercased keys
          // to avoid repeated string allocations during search queries.
          Self::iter().find(|cmd| {
              cmd.title.to_lowercase() == name_lower
                  || cmd.aliases.iter().any(|&alias| alias.to_lowercase() == name_lower)
          })
      }
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum AvailabilityReason {
      NoSessionSelected,
      BackendDisconnected,
      StreamingInProgress,
  }

  pub enum Availability {
      Enabled,
      Disabled(AvailabilityReason),
  }

  pub struct CommandAvailabilityContext {
      pub has_selected_session: bool,
      pub is_connected: bool,
      pub is_generating: bool,
  }

  pub struct CommandPolicy;

  impl CommandPolicy {
      pub fn availability(descriptor: &CommandDescriptor, ctx: &CommandAvailabilityContext) -> Availability {
          match descriptor.id {
              RENAME_SESSION => {
                  if !ctx.has_selected_session {
                      Availability::Disabled(AvailabilityReason::NoSessionSelected)
                  } else {
                      Availability::Enabled
                  }
              }
              _ => Availability::Enabled,
          }
      }
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/brain-tui/src/ui/command/mod.rs crates/brain-tui/tests/command_palette_tests.rs
  git commit -m "feat(tui): add static Command Registry and availability policy"
  ```

---

### Task 2: Slash Commands

**Files:**
- Create: `crates/brain-tui/src/ui/command/completion.rs` (SlashCompletionState, SlashCompletionEngine)
- Modify: `crates/brain-tui/src/ui/state.rs` (reference SlashCompletionState inside AppState)
- Modify: `crates/brain-tui/src/ui/widgets/chat_screen.rs` (render the popup box)

**Interfaces:**
- Consumes: `COMMANDS`, `CommandRegistry`
- Produces: `SlashCompletionState`, `SlashCompletionEngine`

- [ ] **Step 1: Write the failing test for completion matching**
  Write a test in `command_palette_tests.rs` showing that `SlashCompletionEngine::matches("/th")` matches `/theme`.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  use brain_tui::ui::command::completion::SlashCompletionEngine;

  #[test]
  fn test_slash_completion_matching() {
      let matches: Vec<_> = SlashCompletionEngine::matches("/th").collect();
      assert!(!matches.is_empty());
      assert_eq!(matches[0].title, "Change Theme");
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL (compilation errors, completion engine not defined).

- [ ] **Step 3: Write minimal implementation**
  Create `crates/brain-tui/src/ui/command/completion.rs` defining `SlashCompletionState` and `SlashCompletionEngine`.
  
  ```rust
  // crates/brain-tui/src/ui/command/completion.rs
  use crate::ui::command::{COMMANDS, CommandDescriptor, CommandVisibility};

  /// UI state tracker for active inline slash completion popup.
  pub struct SlashCompletionState {
      pub visible: bool,
      pub selected_index: usize,
      pub query: String,
  }

  pub struct SlashCompletionEngine;

  impl SlashCompletionEngine {
      pub fn matches(query: &str) -> impl Iterator<Item = &'static CommandDescriptor> {
          if !query.starts_with('/') {
              return [].iter().copied().take(0);
          }
          let term = query[1..].to_lowercase();
          COMMANDS.iter()
              .filter(move |cmd| {
                  cmd.visibility != CommandVisibility::PaletteOnly
                      && (cmd.title.to_lowercase().contains(&term)
                          || cmd.aliases.iter().any(|alias| alias.to_lowercase().contains(&term)))
              })
      }
  }
  ```
  
  Integrate `SlashCompletionState` into `AppState` in `crates/brain-tui/src/ui/state.rs`. Update the renderer to draw this completion window above the input box.

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/brain-tui/src/ui/command/completion.rs
  git commit -am "feat(tui): implement slash completion engine and state structure"
  ```

---

### Task 3: Command Palette UI

**Files:**
- Create: `crates/brain-tui/src/ui/command/palette.rs` (CommandPaletteState definition)
- Modify: `crates/brain-tui/src/ui/state.rs` (reference CommandPaletteState inside AppState)
- Modify: `crates/brain-tui/src/ui/focus.rs` (FocusTarget, FocusManager updates)
- Modify: `crates/brain-tui/src/ui/layout/mod.rs` (defining overlay bounds)
- Modify: `crates/brain-tui/src/ui/renderer.rs` (overlay rendering)

**Interfaces:**
- Consumes: `FocusManager`
- Produces: `FocusTarget::CommandPalette`, `saved_focus` state, `CommandPaletteGeometry` layout calculation

- [ ] **Step 1: Write the failing test for focus switching**
  Write a test showing that saving and popping focus operates correctly on `FocusManager`.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  use brain_tui::ui::focus::{FocusManager, FocusTarget};

  #[test]
  fn test_focus_restoration() {
      let mut fm = FocusManager::new();
      fm.set_current(FocusTarget::Sidebar);
      
      let saved = fm.current();
      fm.save_focus(saved);
      fm.set_current(FocusTarget::CommandPalette);

      assert_eq!(fm.current(), FocusTarget::CommandPalette);

      if let Some(target) = fm.pop_saved_focus() {
          fm.set_current(target);
      }
      assert_eq!(fm.current(), FocusTarget::Sidebar);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Create `crates/brain-tui/src/ui/command/palette.rs` containing `CommandPaletteState` and the stage/arguments enums.
  Add `FocusTarget::CommandPalette` and `save_focus`/`pop_saved_focus` methods on `FocusManager` inside `crates/brain-tui/src/ui/focus.rs`.
  Define `CommandPaletteGeometry` in the layout files and update `crates/brain-tui/src/ui/renderer.rs` to render a centered bordered box when the palette is open.

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
