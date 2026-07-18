//! Pure execution engine mapping command invocations to execution plans.

use crate::ui::command::{ModelId, SessionTitle, ThemeId};
use crate::ui::protocol::BackendCommand;
use brain_domain::SessionId;

/// Local state mutations affecting the active TUI client directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalStateMutation {
    /// Request change of current visual theme.
    ApplyTheme(ThemeId),
    /// Request clearing of active chat messages.
    ClearChat,
    /// Request local rename of session.
    RenameSession(SessionId, String),
    /// Request local archive of session.
    ArchiveSession(SessionId),
    /// Request local delete of session.
    DeleteSession(SessionId),
    /// Request local restore of session.
    RestoreSession(SessionId),
    /// Request toggle of KPP reflection logs visibility.
    ToggleReflectionLogs,
}

/// A parsed, type-safe command invocation representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandInvocation {
    /// Change application theme.
    ChangeTheme {
        /// Opaque theme identifier.
        theme: ThemeId,
    },
    /// Rename active session.
    RenameSession {
        /// Opaque session identifier.
        session_id: SessionId,
        /// New session title.
        title: SessionTitle,
    },
    /// Archive active session.
    ArchiveSession {
        /// Opaque session identifier.
        session_id: SessionId,
    },
    /// Delete active session permanently.
    DeleteSession {
        /// Opaque session identifier.
        session_id: SessionId,
    },
    /// Restore archived session to active.
    RestoreSession {
        /// Opaque session identifier.
        session_id: SessionId,
    },
    /// Switch active generation model.
    SwitchModel {
        /// Opaque model config identifier.
        model: ModelId,
    },
    /// Clear chat history.
    ClearChat,
    /// Render help popup.
    ShowHelp,
    /// Toggle reflection logs visibility.
    ToggleReflection,
}

impl CommandInvocation {
    /// Build a CommandInvocation from collected parameters.
    pub fn build(
        command_id: crate::ui::command::CommandId,
        collected: &[crate::ui::command::palette::CollectedParameter],
        active_session: Option<SessionId>,
    ) -> Option<Self> {
        match command_id {
            crate::ui::command::CHANGE_THEME => {
                let theme_param = collected.iter().find(|p| p.id.0 == "theme")?;
                if let crate::ui::command::palette::ParameterValue::Theme(theme_id) =
                    theme_param.value
                {
                    Some(CommandInvocation::ChangeTheme { theme: theme_id })
                } else if let crate::ui::command::palette::ParameterValue::String(ref s) =
                    theme_param.value
                {
                    Some(CommandInvocation::ChangeTheme {
                        theme: crate::ui::command::ThemeId(if s.contains("contrast") {
                            "high_contrast"
                        } else {
                            "dark"
                        }),
                    })
                } else {
                    None
                }
            }
            crate::ui::command::RENAME_SESSION => {
                let session_id = active_session?;
                let title_param = collected.iter().find(|p| p.id.0 == "title")?;
                if let crate::ui::command::palette::ParameterValue::String(ref s) =
                    title_param.value
                {
                    Some(CommandInvocation::RenameSession {
                        session_id,
                        title: crate::ui::command::SessionTitle(s.clone()),
                    })
                } else {
                    None
                }
            }
            crate::ui::command::ARCHIVE_SESSION => {
                let session_id = active_session?;
                Some(CommandInvocation::ArchiveSession { session_id })
            }
            crate::ui::command::DELETE_SESSION => {
                let session_id = active_session?;
                Some(CommandInvocation::DeleteSession { session_id })
            }
            crate::ui::command::RESTORE_SESSION => {
                let session_id = active_session?;
                Some(CommandInvocation::RestoreSession { session_id })
            }
            crate::ui::command::SWITCH_MODEL => {
                let model_param = collected.iter().find(|p| p.id.0 == "model")?;
                if let crate::ui::command::palette::ParameterValue::Model(ref model_id) =
                    model_param.value
                {
                    Some(CommandInvocation::SwitchModel {
                        model: model_id.clone(),
                    })
                } else if let crate::ui::command::palette::ParameterValue::String(ref s) =
                    model_param.value
                {
                    Some(CommandInvocation::SwitchModel {
                        model: crate::ui::command::ModelId(s.clone()),
                    })
                } else {
                    None
                }
            }
            crate::ui::command::CLEAR_CHAT => Some(CommandInvocation::ClearChat),
            crate::ui::command::SHOW_HELP => Some(CommandInvocation::ShowHelp),
            crate::ui::command::TOGGLE_REFLECTION => Some(CommandInvocation::ToggleReflection),
            _ => None,
        }
    }
}

/// An execution plan containing local state mutations and backend commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    /// Local mutations applied immediately to local AppState.
    pub mutations: Vec<LocalStateMutation>,
    /// Outgoing commands transmitted to the Daemon backend.
    pub backend_commands: Vec<BackendCommand>,
}

/// Executor translating high-level invocations to concrete execution plans.
pub struct CommandExecutor;

impl CommandExecutor {
    /// Pure function computing the execution plan for the given invocation.
    pub fn plan(invocation: CommandInvocation) -> ExecutionPlan {
        match invocation {
            CommandInvocation::ChangeTheme { theme } => ExecutionPlan {
                mutations: vec![LocalStateMutation::ApplyTheme(theme)],
                backend_commands: vec![],
            },
            CommandInvocation::RenameSession { session_id, title } => ExecutionPlan {
                mutations: vec![LocalStateMutation::RenameSession(
                    session_id,
                    title.0.clone(),
                )],
                backend_commands: vec![BackendCommand::RenameSession {
                    session_id,
                    title: Some(title.0),
                }],
            },
            CommandInvocation::ArchiveSession { session_id } => ExecutionPlan {
                mutations: vec![LocalStateMutation::ArchiveSession(session_id)],
                backend_commands: vec![BackendCommand::ArchiveSession { session_id }],
            },
            CommandInvocation::DeleteSession { session_id } => ExecutionPlan {
                mutations: vec![LocalStateMutation::DeleteSession(session_id)],
                backend_commands: vec![BackendCommand::DeleteSession { session_id }],
            },
            CommandInvocation::RestoreSession { session_id } => ExecutionPlan {
                mutations: vec![LocalStateMutation::RestoreSession(session_id)],
                backend_commands: vec![BackendCommand::RestoreSession { session_id }],
            },
            CommandInvocation::ClearChat => ExecutionPlan {
                mutations: vec![LocalStateMutation::ClearChat],
                backend_commands: vec![],
            },
            CommandInvocation::SwitchModel { model: _ } => ExecutionPlan {
                mutations: vec![],
                backend_commands: vec![],
            },
            CommandInvocation::ShowHelp => ExecutionPlan {
                mutations: vec![],
                backend_commands: vec![],
            },
            CommandInvocation::ToggleReflection => ExecutionPlan {
                mutations: vec![LocalStateMutation::ToggleReflectionLogs],
                backend_commands: vec![],
            },
        }
    }
}
