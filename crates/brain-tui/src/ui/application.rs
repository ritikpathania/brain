//! Application orchestration loop and workflow coordinators.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::ui::state::AppState;
use crate::ui::protocol::{BackendCommand, BackendEvent, RequestAllocator};
use crate::ui::scheduler::{RenderScheduler, RenderRequest, RenderReason, RenderInvalidation};
use crate::ui::interaction::UiEvent;

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
    pub async fn run<E: UiEventSource>(&mut self, mut ui_source: E) -> Result<(), ApplicationError> {
        self.lifecycle = ApplicationLifecycle::Running;

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
    pub async fn handle_ui_event(&mut self, event: UiEvent) -> Result<Option<RenderRequest>, ApplicationError> {
        match event {
            UiEvent::SubmitPrompt(text) => {
                let req_id = self.allocator.next_id();
                let (_, assistant_id) = self.state.submit_user_message(text.clone());
                let cmd = BackendCommand::SubmitPrompt {
                    request: req_id,
                    message: assistant_id,
                    text,
                };
                self.client.send(cmd).await?;
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
                        Some(BackendCommand::RenameSession { session_id: id, title })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::TogglePin(id) => {
                        self.state.toggle_pin_session(id);
                        Some(BackendCommand::TogglePinSession { session_id: id })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::Archive(id) => {
                        self.state.archive_session(id);
                        let visible_ids: Vec<brain_domain::SessionId> = self.state.sessions()
                            .iter()
                            .filter(|x| !x.archived)
                            .map(|x| x.id)
                            .collect();
                        self.state.sidebar_mut().restore_selection_fallback(&visible_ids);
                        Some(BackendCommand::ArchiveSession { session_id: id })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::Delete(id) => {
                        self.state.delete_session(id);
                        let visible_ids: Vec<brain_domain::SessionId> = self.state.sessions()
                            .iter()
                            .filter(|x| !x.archived)
                            .map(|x| x.id)
                            .collect();
                        self.state.sidebar_mut().restore_selection_fallback(&visible_ids);
                        Some(BackendCommand::DeleteSession { session_id: id })
                    }
                    crate::ui::interaction::sidebar::SidebarEvent::Restore(id) => {
                        self.state.restore_session(id);
                        let visible_ids: Vec<brain_domain::SessionId> = self.state.sessions()
                            .iter()
                            .filter(|x| x.archived)
                            .map(|x| x.id)
                            .collect();
                        self.state.sidebar_mut().restore_selection_fallback(&visible_ids);
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
        }
    }

    /// Handles events returned asynchronously from Daemon Client.
    pub async fn handle_backend_event(&mut self, event: BackendEvent) -> Result<Option<RenderRequest>, ApplicationError> {
        match event {
            BackendEvent::Token { message, sequence, text } => {
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
        }
    }
}
