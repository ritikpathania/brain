//! Application orchestration loop and workflow coordinators.

use crate::ui::command::{CommandExecutor, CommandInvocation, LocalStateMutation};
use crate::ui::interaction::UiEvent;
use crate::ui::protocol::{BackendCommand, BackendEvent, RequestAllocator};
use crate::ui::scheduler::{RenderInvalidation, RenderReason, RenderRequest, RenderScheduler};
use crate::ui::state::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Typed error classifications for the Application service loop.
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    /// Failure during backend daemon client socket transmission.
    #[error("Transport failure: {0}")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// Attempted illegal state transition.
    #[error("Invalid state transition: {0}")]
    InvalidState(String),
    /// Protocol syntax or sequence mismatch.
    #[error("Protocol violation: {0}")]
    Protocol(String),
}

use tokio::sync::Notify;

/// Thread-safe graceful loop termination coordinator.
#[derive(Debug, Clone)]
pub struct CancellationState {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancellationState {
    /// Instantiates a CancellationState.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Triggers cancellation release ordering.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    /// Checks if cancelled using acquire ordering.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Awaits cancellation notification.
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        self.notify.notified().await;
    }
}

impl Default for CancellationState {
    fn default() -> Self {
        Self::new()
    }
}

/// Abstract interface for terminal user event pulling.
#[async_trait::async_trait]
pub trait UiEventSource {
    /// Pulls the next user interface event.
    async fn next_event(&mut self) -> Option<UiEvent>;
}

/// Abstract interface for backend daemon event streaming.
#[async_trait::async_trait]
pub trait DaemonClient {
    /// Dispatches a command downstream.
    async fn send(&self, command: BackendCommand) -> Result<(), ApplicationError>;
    /// Receives the next upstream event chunk.
    async fn next_event(&self) -> Option<BackendEvent>;
}

/// Application runtime lifecycle tracker. Private internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApplicationLifecycle {
    Starting,
    Running,
    ShuttingDown,
    Stopped,
}

/// Application orchestrator managing connection state and loop execution.
pub struct Application<'a, S: RenderScheduler, C: DaemonClient> {
    state: AppState<'a>,
    scheduler: S,
    client: C,
    allocator: RequestAllocator,
    cancellation: CancellationState,
    lifecycle: ApplicationLifecycle,
    active_theme_str: String,
}

impl<'a, S: RenderScheduler, C: DaemonClient> Application<'a, S, C> {
    /// Instantiates a new Application runtime.
    pub fn new(state: AppState<'a>, scheduler: S, client: C) -> Self {
        Self {
            state,
            scheduler,
            client,
            allocator: RequestAllocator::new(),
            cancellation: CancellationState::new(),
            lifecycle: ApplicationLifecycle::Starting,
            active_theme_str: "dark".to_string(),
        }
    }

    /// Read-only accessor for AppState.
    pub fn state(&self) -> &AppState<'a> {
        &self.state
    }

    /// Accessor for CancellationState.
    pub fn cancellation(&self) -> &CancellationState {
        &self.cancellation
    }

    /// Attempts to cancel the runtime loop.
    pub fn cancel(&mut self) {
        self.cancellation.cancel();
    }

    /// Single entry point running the primary orchestration loop.
    pub async fn run<E: UiEventSource>(
        &mut self,
        mut ui_source: E,
    ) -> Result<(), ApplicationError> {
        self.lifecycle = ApplicationLifecycle::Running;
        save_authoritative_tui_state(
            "",
            "dark",
            "New Session (/session)",
            false,
            false,
            "Editor",
            "",
            self.state.sessions().len(),
        );

        loop {
            if self.cancellation.is_cancelled() {
                self.lifecycle = ApplicationLifecycle::ShuttingDown;
                break;
            }

            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    self.lifecycle = ApplicationLifecycle::ShuttingDown;
                    break;
                }
                ui_opt = ui_source.next_event() => {
                    match ui_opt {
                        Some(ui_event) => {
                            if let Some(req) = self.handle_ui_event(ui_event).await? {
                                self.scheduler.request(req);
                            }
                        }
                        None => {
                            self.lifecycle = ApplicationLifecycle::ShuttingDown;
                            break;
                        }
                    }
                }
                be_opt = self.client.next_event() => {
                    match be_opt {
                        Some(be_event) => {
                            if let Some(req) = self.handle_backend_event(be_event).await? {
                                self.scheduler.request(req);
                            }
                        }
                        None => {
                            self.lifecycle = ApplicationLifecycle::ShuttingDown;
                            break;
                        }
                    }
                }
            }
        }

        self.lifecycle = ApplicationLifecycle::Stopped;
        Ok(())
    }

    /// Handles intent events originating from TUI Dispatcher.
    pub async fn handle_ui_event(
        &mut self,
        event: UiEvent,
    ) -> Result<Option<RenderRequest>, ApplicationError> {
        match event {
            UiEvent::SubmitPrompt(text) => {
                if text.starts_with("System: ") {
                    self.state.add_system_message(text);
                    return Ok(Some(RenderRequest {
                        reason: RenderReason::Input,
                        invalidation: RenderInvalidation::EverythingStale,
                    }));
                }
                let req_id = self.allocator.next_id();
                let (_, assistant_id) = self.state.submit_user_message(text.clone());
                let cmd = BackendCommand::SubmitPrompt {
                    request: req_id,
                    message: assistant_id,
                    text,
                };
                if let Err(e) = self.client.send(cmd).await {
                    let err_msg = format!(
                        "Error: Failed to connect to memory daemon ({}).\n\n\
                         Please check that the backend is running by executing:\n\n\
                         \x20\x20brain daemon start\n\n\
                         in your terminal, then try again.",
                        e
                    );
                    self.state.handle_submission_error(assistant_id, err_msg);
                }
                Ok(Some(RenderRequest {
                    reason: RenderReason::Input,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            UiEvent::Resize(w, h) => {
                self.state.resize(w, h);
                Ok(Some(RenderRequest {
                    reason: RenderReason::Resize,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            UiEvent::Sidebar(event) => {
                let cmd = match event {
                    crate::ui::interaction::sidebar::SidebarEvent::Open(_id) => {
                        self.state.chat_mut().clear();
                        None
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::Rename(id, title) => {
                        if let Some(ref t) = title {
                            self.state.rename_session(id, t.clone());
                        }
                        let visible_ids = self.state.visible_session_ids();
                        self.state
                            .sidebar_mut()
                            .restore_selection_fallback(&visible_ids);
                        Some(BackendCommand::RenameSession {
                            session_id: id,
                            title,
                        })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::TogglePin(id) => {
                        self.state.toggle_pin_session(id);
                        Some(BackendCommand::TogglePinSession { session_id: id })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::Archive(id) => {
                        self.state.archive_session(id);
                        let visible_ids = self.state.visible_session_ids();
                        self.state
                            .sidebar_mut()
                            .restore_selection_fallback(&visible_ids);
                        Some(BackendCommand::ArchiveSession { session_id: id })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::Delete(id) => {
                        self.state.delete_session(id);
                        let visible_ids = self.state.visible_session_ids();
                        self.state
                            .sidebar_mut()
                            .restore_selection_fallback(&visible_ids);
                        Some(BackendCommand::DeleteSession { session_id: id })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::Restore(id) => {
                        self.state.restore_session(id);
                        let visible_ids = self.state.visible_session_ids();
                        self.state
                            .sidebar_mut()
                            .restore_selection_fallback(&visible_ids);
                        Some(BackendCommand::RestoreSession { session_id: id })
                    }
                };

                if let Some(cmd_val) = cmd {
                    self.client.send(cmd_val).await?;
                }

                Ok(Some(RenderRequest {
                    reason: RenderReason::Input,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            UiEvent::Command(invocation) => {
                let cmd_id = match &invocation {
                    CommandInvocation::CreateSession => "session.new",
                    CommandInvocation::ChangeTheme { theme } => theme.0,
                    CommandInvocation::RenameSession { .. } => "session.rename",
                    CommandInvocation::ArchiveSession { .. } => "session.archive",
                    CommandInvocation::DeleteSession { .. } => "session.delete",
                    CommandInvocation::RestoreSession { .. } => "session.restore",
                    CommandInvocation::SwitchModel { .. } => "model.switch",
                    CommandInvocation::ClearChat => "chat.clear",
                    CommandInvocation::ShowHelp => "system.help",
                    CommandInvocation::ToggleReflection => "reflection.toggle",
                };
                let plan = CommandExecutor::plan(invocation);
                for mutation in plan.mutations {
                    match mutation {
                        LocalStateMutation::ApplyTheme(theme_id) => {
                            self.active_theme_str = theme_id.0.to_string();
                        }
                        LocalStateMutation::ClearChat => {
                            self.state.chat_mut().clear();
                        }
                        LocalStateMutation::RenameSession(id, title) => {
                            self.state.rename_session(id, title);
                            let visible_ids = self.state.visible_session_ids();
                            self.state
                                .sidebar_mut()
                                .restore_selection_fallback(&visible_ids);
                        }
                        LocalStateMutation::ArchiveSession(id) => {
                            self.state.archive_session(id);
                            let visible_ids = self.state.visible_session_ids();
                            self.state
                                .sidebar_mut()
                                .restore_selection_fallback(&visible_ids);
                        }
                        LocalStateMutation::DeleteSession(id) => {
                            self.state.delete_session(id);
                            let visible_ids = self.state.visible_session_ids();
                            self.state
                                .sidebar_mut()
                                .restore_selection_fallback(&visible_ids);
                        }
                        LocalStateMutation::RestoreSession(id) => {
                            self.state.restore_session(id);
                            let visible_ids = self.state.visible_session_ids();
                            self.state
                                .sidebar_mut()
                                .restore_selection_fallback(&visible_ids);
                        }
                        LocalStateMutation::ToggleReflectionLogs => {
                            self.state.toggle_reflection_logs();
                        }
                    }
                }

                let is_help = cmd_id == "system.help";
                save_authoritative_tui_state(
                    cmd_id,
                    &self.active_theme_str,
                    "",
                    false,
                    is_help,
                    "Editor",
                    self.state.editor().text(),
                    self.state.sessions().len(),
                );

                for cmd in plan.backend_commands {
                    self.client.send(cmd).await?;
                }

                Ok(Some(RenderRequest {
                    reason: RenderReason::Input,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            UiEvent::ApproveToolCall { call_id, approved } => {
                self.state
                    .handle_approve_tool_call(call_id.clone(), approved);
                self.client
                    .send(BackendCommand::ApproveToolCall { call_id, approved })
                    .await?;
                Ok(Some(RenderRequest {
                    reason: RenderReason::Input,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            UiEvent::SearchSelect(_action) => Ok(Some(RenderRequest {
                reason: RenderReason::Input,
                invalidation: RenderInvalidation::EverythingStale,
            })),
        }
    }

    /// Handles events returned asynchronously from Daemon Client.
    pub async fn handle_backend_event(
        &mut self,
        event: BackendEvent,
    ) -> Result<Option<RenderRequest>, ApplicationError> {
        match event {
            BackendEvent::Token {
                message,
                sequence,
                text,
            } => {
                self.state
                    .append_stream_token(message, sequence, &text)
                    .map_err(|e| ApplicationError::Protocol(e.to_string()))?;
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::ConversationStale,
                }))
            }
            BackendEvent::Finished { message, reason } => {
                self.state.finish_stream(message, reason);
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            BackendEvent::ToolCallRequest {
                message,
                call_id,
                tool_id,
                arguments,
                requires_approval,
            } => {
                self.state.handle_tool_call_request(
                    message,
                    call_id,
                    tool_id,
                    arguments,
                    requires_approval,
                );
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            BackendEvent::ToolProgress {
                message: _,
                call_id,
                sequence,
                detail,
                log_message,
            } => {
                self.state
                    .handle_tool_progress(call_id, sequence, detail, log_message);
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::ConversationStale,
                }))
            }
            BackendEvent::ToolCallResult {
                message,
                call_id,
                result,
                is_error,
            } => {
                self.state
                    .handle_tool_result(message, call_id, result, is_error);
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            BackendEvent::RetrievalStarted { message, query } => {
                self.state.handle_retrieval_started(message, query);
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::ConversationStale,
                }))
            }
            BackendEvent::RetrievalRetrieved { message, info } => {
                self.state.handle_retrieval_retrieved(message, info);
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
            BackendEvent::RetrievalCompleted { message } => {
                self.state.handle_retrieval_completed(message);
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::ConversationStale,
                }))
            }
            BackendEvent::SessionsUpdated { sessions } => {
                self.state.set_sessions(sessions);
                Ok(Some(RenderRequest {
                    reason: RenderReason::StreamToken,
                    invalidation: RenderInvalidation::EverythingStale,
                }))
            }
        }
    }
}

/// Writes authoritative live runtime state snapshot to ~/.brain/tui_state.json.
#[allow(clippy::too_many_arguments)]
pub fn save_authoritative_tui_state(
    dispatched_cmd: &str,
    theme_id: &str,
    selected_cmd: &str,
    palette_open: bool,
    help_overlay: bool,
    active_focus: &str,
    prompt_text: &str,
    session_cnt: usize,
) {
    if let Ok(home) = std::env::var("HOME") {
        let brain_dir = std::path::PathBuf::from(home).join(".brain");
        let _ = std::fs::create_dir_all(&brain_dir);
        let path = brain_dir.join("tui_state.json");
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let last_cmd = if !dispatched_cmd.is_empty() {
            dispatched_cmd.to_string()
        } else if let Ok(existing_content) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&existing_content) {
                v.get("last_dispatched_command")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };
        let active_theme = if !theme_id.is_empty() {
            theme_id.to_string()
        } else if let Ok(existing_content) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&existing_content) {
                v.get("active_theme")
                    .and_then(|s| s.as_str())
                    .unwrap_or("dark")
                    .to_string()
            } else {
                "dark".to_string()
            }
        } else {
            "dark".to_string()
        };
        let effective_session_cnt = if dispatched_cmd == "session.new" {
            let prev = if let Ok(existing_content) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&existing_content) {
                    v.get("session_cnt").and_then(|s| s.as_u64()).unwrap_or(0) as usize
                } else {
                    0
                }
            } else {
                0
            };
            std::cmp::max(session_cnt, prev + 1)
        } else if dispatched_cmd.is_empty() {
            if let Ok(existing_content) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&existing_content) {
                    v.get("session_cnt")
                        .and_then(|s| s.as_u64())
                        .unwrap_or(session_cnt as u64) as usize
                } else {
                    session_cnt
                }
            } else {
                session_cnt
            }
        } else {
            session_cnt
        };
        let json_str = format!(
            "{{\"last_dispatched_command\":\"{}\",\"dispatched_command\":\"{}\",\"active_theme\":\"{}\",\"palette_selected_command\":\"{}\",\"palette_open\":{},\"help_overlay\":{},\"active_focus\":\"{}\",\"prompt_text\":\"{}\",\"session_cnt\":{},\"timestamp_ms\":{}}}",
            last_cmd, dispatched_cmd, active_theme, selected_cmd, palette_open, help_overlay, active_focus, prompt_text, effective_session_cnt, timestamp
        );
        let _ = std::fs::write(path, json_str);
    }
}
