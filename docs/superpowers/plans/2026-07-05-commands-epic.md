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

---

### Task 1: Core Registry & Parameter Models

**Files:**
- Create: `crates/brain-tui/src/ui/command/mod.rs`
- Test: `crates/brain-tui/tests/command_palette_tests.rs` (new test suite file)

**Interfaces:**
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
  git commit -m "feat(tui): add core Command Registry and policy checks"
  ```

---

### Task 2: Focus Target & State Structures

**Files:**
- Modify: `crates/brain-tui/src/ui/focus.rs`
- Modify: `crates/brain-tui/src/ui/state.rs`
- Test: `crates/brain-tui/tests/command_palette_tests.rs`

**Interfaces:**
- Consumes: `CommandId`, `ThemeId`, `ModelId`
- Produces: `FocusTarget::CommandPalette`, `saved_focus` storage, `CommandPaletteState`, `SlashCompletionState`

- [ ] **Step 1: Write the failing test for focus restoration**
  Write a test in `command_palette_tests.rs` showing that moving focus to the palette saves the previous target, and restoring focus retrieves it correctly.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  use brain_tui::ui::focus::{FocusManager, FocusTarget};

  #[test]
  fn test_focus_restoration() {
      let mut fm = FocusManager::new();
      fm.set_current(FocusTarget::Sidebar);
      
      // Save current focus and switch to CommandPalette
      let saved = fm.current();
      fm.save_focus(saved);
      fm.set_current(FocusTarget::CommandPalette);

      assert_eq!(fm.current(), FocusTarget::CommandPalette);

      // Restore saved focus
      if let Some(target) = fm.pop_saved_focus() {
          fm.set_current(target);
      }
      assert_eq!(fm.current(), FocusTarget::Sidebar);
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL with compilation error (no `save_focus`, `pop_saved_focus`, or `FocusTarget::CommandPalette`).

- [ ] **Step 3: Write minimal implementation**
  Modify `FocusTarget` in `crates/brain-tui/src/ui/focus.rs` or `view_models.rs` and add `saved_focus: Option<FocusTarget>` with helpers on `FocusManager`.
  
  ```rust
  // in crates/brain-tui/src/ui/focus.rs (or relevant file)
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum FocusTarget {
      Conversation,
      Prompt,
      Sidebar,
      CommandPalette,
  }

  // inside FocusManager struct:
  pub struct FocusManager {
      current: FocusTarget,
      saved_focus: Option<FocusTarget>,
  }

  impl FocusManager {
      pub fn save_focus(&mut self, target: FocusTarget) {
          self.saved_focus = Some(target);
      }
      pub fn pop_saved_focus(&mut self) -> Option<FocusTarget> {
          self.saved_focus.take()
      }
  }
  ```
  
  Also define `CommandPaletteState` and `SlashCompletionState` in `crates/brain-tui/src/ui/state.rs` and add them to `AppState`.

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(tui): add CommandPalette focus target and state structures"
  ```

---

### Task 3: Command Executor & Execution Plan

**Files:**
- Create: `crates/brain-tui/src/ui/command/executor.rs`
- Test: `crates/brain-tui/tests/command_palette_tests.rs`

**Interfaces:**
- Consumes: `CommandId`, `ThemeId`, `ModelId`
- Produces: `CommandInvocation`, `ParameterValue`, `LocalStateMutation`, `ExecutionPlan`, `CommandExecutor`

- [ ] **Step 1: Write the failing test for execution planning**
  Write a test verifying that `CommandExecutor::plan` maps a `ChangeTheme` invocation to the correct mutations and config save commands.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  use brain_tui::ui::command::executor::{CommandExecutor, CommandInvocation, LocalStateMutation};
  use brain_tui::ui::command::ThemeId;

  #[test]
  fn test_change_theme_execution_plan() {
      let invocation = CommandInvocation::ChangeTheme { theme: ThemeId("dark") };
      let plan = CommandExecutor::plan(invocation);
      
      assert_eq!(plan.mutations.len(), 1);
      assert!(matches!(plan.mutations[0], LocalStateMutation::ApplyTheme(ThemeId("dark"))));
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL (compilation errors, executor module not found).

- [ ] **Step 3: Write minimal implementation**
  Create `crates/brain-tui/src/ui/command/executor.rs` and register it in `crates/brain-tui/src/ui/command/mod.rs`.
  
  ```rust
  // crates/brain-tui/src/ui/command/executor.rs
  use crate::ui::command::{ThemeId, ModelId, SessionTitle};
  use crate::ui::protocol::BackendCommand;
  use crate::ui::scheduler::{RenderReason, RenderInvalidation, RenderRequest};
  use brain_domain::SessionId;

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum CommandInvocation {
      ChangeTheme { theme: ThemeId },
      RenameSession { id: SessionId, title: SessionTitle },
      ArchiveSession { id: SessionId },
      DeleteSession { id: SessionId },
      RestoreSession { id: SessionId },
      SwitchModel { model: ModelId },
      ClearChat,
      ShowHelp,
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub enum LocalStateMutation {
      ApplyTheme(ThemeId),
      RenameSession(SessionId, String),
      ArchiveSession(SessionId),
      DeleteSession(SessionId),
      RestoreSession(SessionId),
      ClearChat,
  }

  pub struct ExecutionPlan {
      pub mutations: Vec<LocalStateMutation>,
      pub backend_commands: Vec<BackendCommand>,
      pub invalidation: RenderRequest,
  }

  pub struct CommandExecutor;

  impl CommandExecutor {
      pub fn plan(invocation: CommandInvocation) -> ExecutionPlan {
          match invocation {
              CommandInvocation::ChangeTheme { theme } => ExecutionPlan {
                  mutations: vec![LocalStateMutation::ApplyTheme(theme)],
                  backend_commands: vec![BackendCommand::SaveConfig {
                      key: "theme".to_string(),
                      val: theme.0.to_string(),
                  }],
                  invalidation: RenderRequest {
                      reason: RenderReason::ThemeChanged,
                      invalidation: RenderInvalidation::EverythingStale,
                  },
              },
              CommandInvocation::RenameSession { id, title } => ExecutionPlan {
                  mutations: vec![LocalStateMutation::RenameSession(id, title.0.clone())],
                  backend_commands: vec![BackendCommand::RenameSession {
                      session_id: id,
                      title: Some(title.0),
                  }],
                  invalidation: RenderRequest {
                      reason: RenderReason::Input,
                      invalidation: RenderInvalidation::EverythingStale,
                  },
              },
              // ... map remaining variants to empty or basic vectors for initial completeness
              _ => ExecutionPlan {
                  mutations: vec![],
                  backend_commands: vec![],
                  invalidation: RenderRequest {
                      reason: RenderReason::Input,
                      invalidation: RenderInvalidation::EverythingStale,
                  },
              }
          }
      }
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add crates/brain-tui/src/ui/command/executor.rs
  git commit -m "feat(tui): implement CommandExecutor and ExecutionPlan plan mapping"
  ```

---

### Task 4: Dispatcher & Application Integration

**Files:**
- Modify: `crates/brain-tui/src/ui/interaction/dispatcher.rs`
- Modify: `crates/brain-tui/src/ui/application.rs`
- Test: `crates/brain-tui/tests/command_palette_tests.rs`

**Interfaces:**
- Consumes: `CommandPaletteState`, `CommandExecutor`
- Produces: Routing of `Ctrl+K` key input, command execution in `Application`

- [ ] **Step 1: Write the failing test for dispatch routing**
  Write an integration test that simulates pressing `Ctrl+K` in Prompt focus, verifying that the Command Palette opens and grabs focus.
  
  ```rust
  // crates/brain-tui/tests/command_palette_tests.rs
  // Setup dispatcher and AppState context, send Ctrl+K, verify fm.current() becomes CommandPalette
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Add keybinding check in `InputRouter` and dispatcher key routing inside `crates/brain-tui/src/ui/interaction/dispatcher.rs`.
  Then implement `execute_plan` execution flow inside `crates/brain-tui/src/ui/application.rs` to process mutations on `AppState` and push backend commands.

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(tui): integrate Command Palette input routing and application execution"
  ```

---

### Task 5: UI Rendering (Palette Overlay & Completion Box)

**Files:**
- Modify: `crates/brain-tui/src/ui/renderer.rs`
- Modify: `crates/brain-tui/src/ui/layout/` (geometry computation files)
- Test: `crates/brain-tui/tests/command_palette_tests.rs`

- [ ] **Step 1: Write the failing test for rendering overlay**
  Verify that when `CommandPaletteState::open` is true, layout calculations yield a valid centered Rect.

- [ ] **Step 2: Run test to verify it fails**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Implement `CommandPaletteGeometry` layout calculation and render functions inside `crates/brain-tui/src/ui/renderer.rs` to draw the bordered overlay modal, query prompt line, and suggestion results list when open.

- [ ] **Step 4: Run test to verify it passes**
  Run: `PYO3_PYTHON=$(pwd)/daemon/.venv/bin/python cargo test --test command_palette_tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git commit -am "feat(tui): implement Command Palette centered modal overlay rendering"
  ```
