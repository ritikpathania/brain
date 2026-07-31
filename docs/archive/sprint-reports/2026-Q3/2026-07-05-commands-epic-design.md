# Design Specification: Commands Epic (Command Palette & Slash Commands)

This document specifies the architecture, data models, interaction states, and execution pipeline for the Command Palette and slash command completion systems in the CLI TUI.

---

## 1. Core Data Models and Registry

All command metadata and definitions are treated as immutable, compile-time static data to avoid runtime allocations and dynamic dispatch.

### 1.1 Type-safe Identifiers and Arguments

```rust
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
```

### 1.2 Declarative Parameter Descriptors

```rust
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
```

### 1.3 Command Descriptors and Immutable Registry

```rust
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

// Opaque static slice registry
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
    /// Expose an iterator over all registered static command descriptors.
    pub fn iter() -> impl Iterator<Item = &'static CommandDescriptor> {
        COMMANDS.iter()
    }

    /// Look up a command by its unique ID.
    pub fn find_by_id(id: CommandId) -> Option<&'static CommandDescriptor> {
        Self::iter().find(|cmd| cmd.id == id)
    }

    /// Look up a command by its name or alias.
    pub fn find_by_name_or_alias(name: &str) -> Option<&'static CommandDescriptor> {
        let name_lower = name.to_lowercase();
        Self::iter().find(|cmd| {
            cmd.title.to_lowercase() == name_lower
                || cmd.aliases.iter().any(|&alias| alias.to_lowercase() == name_lower)
        })
    }
}
```

---

## 2. Availability Policies

Command availability rules are decoupled from the static descriptors.

```rust
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
            RENAME_SESSION | ARCHIVE_SESSION | DELETE_SESSION => {
                if !ctx.has_selected_session {
                    Availability::Disabled(AvailabilityReason::NoSessionSelected)
                } else {
                    Availability::Enabled
                }
            }
            SWITCH_MODEL => {
                if ctx.is_generating {
                    Availability::Disabled(AvailabilityReason::StreamingInProgress)
                } else {
                    Availability::Enabled
                }
            }
            _ => Availability::Enabled,
        }
    }
}
```

---

## 3. Focus Management and Geometry

### 3.1 Focus States
We extend the `FocusTarget` enum with a command palette target. Focus remains on the Prompt Editor during slash autocompletion.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Conversation,
    Prompt,
    Sidebar,
    CommandPalette,
}
```

The `FocusManager` stores:
- `saved_focus: Option<FocusTarget>` (cached when opening the palette).
- Closing the palette (via `Esc` or execution completion) restores the target in `saved_focus`.

### 3.2 Floating Centered Geometry
The TUI layout engine calculates the geometry constraints for rendering overlays:

```rust
pub struct CommandPaletteGeometry {
    pub outer_area: ratatui::layout::Rect,
    pub input_area: ratatui::layout::Rect,
    pub list_area: ratatui::layout::Rect,
}
```
The overlay is centered over the main application screen, ensuring consistent layouts without disturbing the sidebar or conversation panel.

---

## 4. UI Interaction State Machines

### 4.1 The Command Palette State Machine

The Command Palette operates as a multi-step parameter collection flow.

```rust
pub struct CollectedParameter {
    pub id: ParameterId,
    pub value: ParameterValue,
}

pub struct ParameterCollectionState {
    pub command_id: CommandId,
    pub collected: Vec<CollectedParameter>,
}

impl ParameterCollectionState {
    /// Resolves the descriptor of the parameter currently being collected.
    pub fn current_parameter(&self, descriptor: &CommandDescriptor) -> Option<&ParameterDescriptor> {
        descriptor.parameters.get(self.collected.len())
    }
}

pub enum PaletteStage {
    /// Filtering the static list of commands.
    Search,
    /// Collecting parameter inputs.
    CollectParameter(ParameterCollectionState),
    /// Confirming execution (e.g. for destructive actions) before building the invocation.
    Confirm {
        command_id: CommandId,
        arguments: ParameterCollectionState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterValue {
    String(String),
    Boolean(bool),
    Theme(ThemeId),
    Session(SessionId),
    Model(ModelId),
}

pub struct CommandPaletteState {
    pub open: bool,
    pub editor: Editor,
    pub selected_index: usize,
    pub stage: PaletteStage,
}
```

### 4.2 The Slash Completion State Machine

Slash completion functions inline without losing editor focus.

```rust
pub struct SlashCompletionState {
    pub visible: bool,
    pub selected_index: usize,
    pub query: String,
}

pub struct SlashCompletionEngine;

impl SlashCompletionEngine {
    /// Searches and filters command descriptors matching a slash prefix as an iterator.
    pub fn matches(query: &str) -> impl Iterator<Item = &'static CommandDescriptor> {
        if !query.starts_with('/') {
            return [].iter().copied().take(0); // empty iterator
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

---

## 5. Execution Pipeline

Commands are parsed into type-safe parameters, resolved to an execution plan, and processed by the application coordinator.

### 5.1 Type-safe Invocations

```rust
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
```

### 5.2 Execution Plans and Effects

We define a pure structure for execution results, decoupling state side-effects from the UI. We reuse `RenderReason` and `RenderInvalidation` defined in `crate::ui::scheduler`:

```rust
use crate::ui::scheduler::{RenderReason, RenderInvalidation, RenderRequest};

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
    /// Maps a completed type-safe invocation to its state mutations and side effects.
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
            // ... plans for remaining commands
        }
    }
}
```

---

## 6. Architectural Invariants

1. **Strict Value Semantics**: Commands and parameter collections are passed as plain data values. We avoid runtime dynamic dispatch (`dyn Command`) and mutably shared execution contexts.
2. **Decoupled Orchestration**: The UI is purely responsible for collecting parameter values and producing a `CommandInvocation`. Real state mutation and backend operations are planned by the `CommandExecutor` and executed in the `Application` layer.
3. **Reconciliation Invariant**:
   > **Every optimistic TUI state mutation MUST correspond to an authoritative reconciliation path inside the backend event listener.**
   
   If the backend fails to apply a change or sends a new sessions list, the TUI must update itself from the authoritative backend data.
