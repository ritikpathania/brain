//! Command Palette and Slash Commands infrastructure data models and registry.

/// Type-safe, opaque identifier for commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub &'static str);

/// Constant identifier for theme change command.
pub const CHANGE_THEME: CommandId = CommandId("theme.change");
/// Constant identifier for renaming sessions.
pub const RENAME_SESSION: CommandId = CommandId("session.rename");
/// Constant identifier for archiving sessions.
pub const ARCHIVE_SESSION: CommandId = CommandId("session.archive");
/// Constant identifier for deleting sessions.
pub const DELETE_SESSION: CommandId = CommandId("session.delete");
/// Constant identifier for restoring sessions.
pub const RESTORE_SESSION: CommandId = CommandId("session.restore");
/// Constant identifier for switching models.
pub const SWITCH_MODEL: CommandId = CommandId("model.switch");
/// Constant identifier for clearing chat.
pub const CLEAR_CHAT: CommandId = CommandId("chat.clear");
/// Constant identifier for showing help.
pub const SHOW_HELP: CommandId = CommandId("help.show");
/// Constant identifier for toggling reflection logs.
pub const TOGGLE_REFLECTION: CommandId = CommandId("reflection.toggle");

/// Type-safe, opaque identifier for visual themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeId(pub &'static str);

/// Dark theme identifier.
pub const THEME_DARK: ThemeId = ThemeId("dark");
/// High contrast theme identifier.
pub const THEME_HIGH_CONTRAST: ThemeId = ThemeId("high_contrast");

/// Type-safe, opaque identifier for model configurations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(pub String);

/// Type-safe wrapper for session renaming titles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTitle(pub String);

/// Type-safe, opaque identifier for command parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParameterId(pub &'static str);

/// Parameter visual and data kind classifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    /// Captures a free text string input.
    String,
    /// Captures a boolean toggle (true/false).
    Boolean,
    /// Static collection of available themes.
    Theme,
    /// Dynamic collection of active sessions.
    Session,
    /// Dynamic collection of available models.
    Model,
    /// Path or file identifier.
    File,
}

/// Declarative descriptor for a single command parameter.
pub struct ParameterDescriptor {
    /// Unique identifier for the parameter.
    pub id: ParameterId,
    /// Display name of the parameter.
    pub name: &'static str,
    /// Detailed description of the parameter.
    pub description: &'static str,
    /// Underneath data type classification.
    pub kind: ParameterKind,
    /// Whether this parameter must be collected before execution.
    pub required: bool,
}

/// Category grouping for command classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    /// Application settings.
    Settings,
    /// Workspace session management.
    Sessions,
    /// Active generation model selection.
    Models,
    /// Navigation commands.
    Navigation,
    /// Developer-focused utilities.
    Developer,
}

/// Visibility constraint for command discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandVisibility {
    /// Only visible inside slash-command autocompletion in prompt editor.
    SlashOnly,
    /// Only visible inside the Command Palette overlay.
    PaletteOnly,
    /// Visible in both contexts.
    Both,
}

/// Declarative descriptor for a command.
pub struct CommandDescriptor {
    /// Unique command identifier.
    pub id: CommandId,
    /// User-facing display title.
    pub title: &'static str,
    /// Detailed description of command function.
    pub description: &'static str,
    /// Category for navigation / documentation classification.
    pub category: CommandCategory,
    /// Where this command should be visible.
    pub visibility: CommandVisibility,
    /// Priority weight to sort matches.
    pub priority: u16,
    /// Direct command triggers (used in prompt or console).
    pub aliases: &'static [&'static str],
    /// Search queries keywords.
    pub keywords: &'static [&'static str],
    /// List of parameter descriptors.
    pub parameters: &'static [ParameterDescriptor],
}

/// Immutable static slice registry of all system commands.
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
        parameters: &[ParameterDescriptor {
            id: ParameterId("theme"),
            name: "theme",
            description: "Theme name to apply",
            kind: ParameterKind::Theme,
            required: true,
        }],
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
        parameters: &[ParameterDescriptor {
            id: ParameterId("title"),
            name: "title",
            description: "New session title",
            kind: ParameterKind::String,
            required: true,
        }],
    },
    CommandDescriptor {
        id: ARCHIVE_SESSION,
        title: "Archive Session",
        description: "Archive the selected session",
        category: CommandCategory::Sessions,
        visibility: CommandVisibility::PaletteOnly,
        priority: 90,
        aliases: &["archive"],
        keywords: &["session", "hide", "archive"],
        parameters: &[],
    },
    CommandDescriptor {
        id: DELETE_SESSION,
        title: "Delete Session",
        description: "Permanently delete the selected session",
        category: CommandCategory::Sessions,
        visibility: CommandVisibility::PaletteOnly,
        priority: 80,
        aliases: &["delete", "remove"],
        keywords: &["session", "delete", "remove", "erase"],
        parameters: &[],
    },
    CommandDescriptor {
        id: RESTORE_SESSION,
        title: "Restore Session",
        description: "Restore an archived session",
        category: CommandCategory::Sessions,
        visibility: CommandVisibility::PaletteOnly,
        priority: 70,
        aliases: &["restore"],
        keywords: &["session", "unarchive", "restore"],
        parameters: &[],
    },
    CommandDescriptor {
        id: SWITCH_MODEL,
        title: "Switch Model",
        description: "Change the active AI model",
        category: CommandCategory::Models,
        visibility: CommandVisibility::Both,
        priority: 60,
        aliases: &["model", "switch-model"],
        keywords: &["llm", "ai", "model", "select", "change"],
        parameters: &[ParameterDescriptor {
            id: ParameterId("model"),
            name: "model",
            description: "AI model name",
            kind: ParameterKind::Model,
            required: true,
        }],
    },
    CommandDescriptor {
        id: CLEAR_CHAT,
        title: "Clear Chat",
        description: "Clear current conversation messages",
        category: CommandCategory::Developer,
        visibility: CommandVisibility::Both,
        priority: 40,
        aliases: &["clear", "reset"],
        keywords: &["chat", "messages", "clear", "reset", "erase"],
        parameters: &[],
    },
    CommandDescriptor {
        id: SHOW_HELP,
        title: "Show Help",
        description: "Display documentation overlay for commands",
        category: CommandCategory::Navigation,
        visibility: CommandVisibility::Both,
        priority: 30,
        aliases: &["help", "info"],
        keywords: &["docs", "help", "commands", "manual"],
        parameters: &[],
    },
    CommandDescriptor {
        id: TOGGLE_REFLECTION,
        title: "Toggle Reflection Logs",
        description: "Toggle KPP offline reflection critiques and log outputs",
        category: CommandCategory::Settings,
        visibility: CommandVisibility::Both,
        priority: 45,
        aliases: &["reflection", "toggle-reflection"],
        keywords: &["reflection", "kpp", "critique", "toggle", "logs"],
        parameters: &[],
    },
];

/// Command registry accessor.
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
                || cmd
                    .aliases
                    .iter()
                    .any(|&alias| alias.to_lowercase() == name_lower)
        })
    }
}

/// Reason for command unavailability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvailabilityReason {
    /// No session is selected in active panel.
    NoSessionSelected,
    /// Daemon is currently offline.
    BackendDisconnected,
    /// Model generation stream is active.
    StreamingInProgress,
}

/// Availability evaluation outcome.
pub enum Availability {
    /// Ready to be invoked.
    Enabled,
    /// Disabled with a specific reason constraint.
    Disabled(AvailabilityReason),
}

/// Context state snapshot passed to policy checking.
pub struct CommandAvailabilityContext {
    /// True if there is a currently selected session.
    pub has_selected_session: bool,
    /// True if TUI is connected to backend.
    pub is_connected: bool,
    /// True if AI generation is active.
    pub is_generating: bool,
}

/// Policy evaluator for command availability.
pub struct CommandPolicy;

impl CommandPolicy {
    /// Evaluate if a command can run under the current state context.
    pub fn availability(
        descriptor: &CommandDescriptor,
        ctx: &CommandAvailabilityContext,
    ) -> Availability {
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

/// Autocomplete module for inline slash commands.
pub mod completion;

/// Command Palette state and multi-step inputs.
pub mod palette;

/// Pure command execution module.
pub mod executor;

/// Tool execution domain models.
pub mod tool;

pub use executor::{CommandExecutor, CommandInvocation, ExecutionPlan, LocalStateMutation};
