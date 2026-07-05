//! AppState aggregate root unifying all UI and routing components.

use crate::ui::interaction::{
    Editor, ScrollState, ChatState, GenerationState, MessageId, MessageRole
};
use crate::ui::focus::FocusManager;
use crate::ui::router::ScreenRouter;
use crate::ui::protocol::FinishReason;

use crate::ui::interaction::sidebar::SidebarInteraction;
use crate::ui::command::completion::SlashCompletionState;


use crate::client::SessionSummary;
use brain_domain::SessionId;

/// Container aggregating all subsystem states. Fields are private to protect invariants.
pub struct AppState<'a> {
    chat: ChatState,
    generation: GenerationState,
    editor: Editor,
    scroll: ScrollState,
    focus: FocusManager,
    sidebar: SidebarInteraction,
    slash_completion: SlashCompletionState,
    sessions: Vec<SessionSummary>,
    router: ScreenRouter<'a>,

    cols: u16,
    rows: u16,
}

impl<'a> AppState<'a> {
    /// Instantiates a new AppState aggregate root.
    pub fn new(
        chat: ChatState,
        editor: Editor,
        scroll: ScrollState,
        focus: FocusManager,
        sidebar: SidebarInteraction,
        router: ScreenRouter<'a>,
    ) -> Self {
        Self {
            chat,
            generation: GenerationState::Idle,
            editor,
            scroll,
            focus,
            sidebar,
            slash_completion: SlashCompletionState::new(),
            sessions: Vec::new(),
            router,

            cols: 80,
            rows: 24,
        }
    }

    /// Read-only accessor for ChatState.
    pub fn chat(&self) -> &ChatState {
        &self.chat
    }

    /// Mutable accessor for ChatState.
    pub fn chat_mut(&mut self) -> &mut ChatState {
        &mut self.chat
    }

    /// Read-only accessor for GenerationState.
    pub fn generation(&self) -> &GenerationState {
        &self.generation
    }

    /// Read-only accessor for Editor.
    pub fn editor(&self) -> &Editor {
        &self.editor
    }

    /// Read-only accessor for ScrollState.
    pub fn scroll(&self) -> &ScrollState {
        &self.scroll
    }

    /// Read-only accessor for FocusManager.
    pub fn focus(&self) -> &FocusManager {
        &self.focus
    }

    /// Read-only accessor for ScreenRouter.
    pub fn router(&self) -> &ScreenRouter<'a> {
        &self.router
    }

    /// Read-only accessor for SidebarInteraction.
    pub fn sidebar(&self) -> &SidebarInteraction {
        &self.sidebar
    }

    /// Mutable accessor for SidebarInteraction.
    pub fn sidebar_mut(&mut self) -> &mut SidebarInteraction {
        &mut self.sidebar
    }

    /// Read-only accessor for SlashCompletionState.
    pub fn slash_completion(&self) -> &SlashCompletionState {
        &self.slash_completion
    }

    /// Mutable accessor for SlashCompletionState.
    pub fn slash_completion_mut(&mut self) -> &mut SlashCompletionState {
        &mut self.slash_completion
    }


    /// Read-only accessor for session summaries.
    pub fn sessions(&self) -> &[SessionSummary] {
        &self.sessions
    }

    /// Sets/loads the sessions list.
    pub fn set_sessions(&mut self, sessions: Vec<SessionSummary>) {
        self.sessions = sessions;
    }

    /// Renames a session in the local state model.
    pub fn rename_session(&mut self, id: SessionId, title: String) {
        if let Some(s) = self.sessions.iter_mut().find(|x| x.id == id) {
            s.title = title;
        }
    }

    /// Toggles the pinned status of a session in the local state.
    pub fn toggle_pin_session(&mut self, id: SessionId) {
        if let Some(s) = self.sessions.iter_mut().find(|x| x.id == id) {
            s.pinned = !s.pinned;
        }
    }

    /// Explicitly sets the pinned status of a session (fully idempotent).
    pub fn set_session_pinned(&mut self, id: SessionId, pinned: bool) {
        if let Some(s) = self.sessions.iter_mut().find(|x| x.id == id) {
            s.pinned = pinned;
        }
    }

    /// Returns the list of visible session IDs based on active filter and search query.
    pub fn visible_session_ids(&self) -> Vec<SessionId> {
        let filter = self.sidebar.browse.filter;
        let mut ids = Vec::new();
        for s in &self.sessions {
            let matches_filter = match filter {
                crate::ui::interaction::sidebar::SessionFilter::Active => !s.archived,
                crate::ui::interaction::sidebar::SessionFilter::Archived => s.archived,
            };
            if matches_filter && self.sidebar.search.parsed.matches(&s.title) {
                ids.push(s.id);
            }
        }
        ids
    }

    /// Archives a session in the local state.
    pub fn archive_session(&mut self, id: SessionId) {
        if let Some(s) = self.sessions.iter_mut().find(|x| x.id == id) {
            s.archived = true;
        }
    }

    /// Restores an archived session back to active in local state.
    pub fn restore_session(&mut self, id: SessionId) {
        if let Some(s) = self.sessions.iter_mut().find(|x| x.id == id) {
            s.archived = false;
        }
    }

    /// Permanently deletes a session in local state.
    pub fn delete_session(&mut self, id: SessionId) {
        self.sessions.retain(|x| x.id != id);
    }

    /// Mutable accessor for FocusManager.
    pub fn focus_mut(&mut self) -> &mut FocusManager {
        &mut self.focus
    }

    /// Mutable accessor for ScrollState.
    pub fn scroll_mut(&mut self) -> &mut ScrollState {
        &mut self.scroll
    }

    /// Mutable accessor for Editor.
    pub fn editor_mut(&mut self) -> &mut Editor {
        &mut self.editor
    }

    /// Read-only accessor for terminal width.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Read-only accessor for terminal height.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Resize the tracked dimensions.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
    }

    /// Domain operation to submit a user message and set up the assistant placeholder response.
    pub fn submit_user_message(&mut self, text: String) -> (MessageId, MessageId) {
        let user_id = self.chat.push_message(MessageRole::User, text);
        let assistant_id = self.chat.push_message(MessageRole::Assistant, String::new());
        self.generation = GenerationState::Waiting;
        (user_id, assistant_id)
    }

    /// Domain operation to append a streaming token response cell.
    pub fn append_stream_token(&mut self, id: MessageId, sequence: u64, text: &str) -> Result<(), &'static str> {
        match self.generation {
            GenerationState::Waiting => {
                self.chat.append_token(id, text)?;
                self.generation = GenerationState::Streaming {
                    message: id,
                    last_sequence: sequence,
                };
                Ok(())
            }
            GenerationState::Streaming { message, last_sequence } => {
                if message != id {
                    return Err("Message ID mismatch in active stream");
                }
                // Sequence monotonicity check: ignore if sequence <= last_sequence
                if sequence <= last_sequence {
                    return Ok(()); // Ignored safely
                }
                self.chat.append_token(id, text)?;
                self.generation = GenerationState::Streaming {
                    message: id,
                    last_sequence: sequence,
                };
                Ok(())
            }
            _ => {
                // Ignore any tokens arriving when not active (e.g. out-of-order tokens after Finished)
                Ok(())
            }
        }
    }

    /// Domain operation to complete streaming.
    pub fn finish_stream(&mut self, id: MessageId, reason: FinishReason) {
        match self.generation {
            GenerationState::Waiting
            | GenerationState::Streaming { .. }
            | GenerationState::Cancelling { .. }
            | GenerationState::Error { .. }
            | GenerationState::Completed { .. } => {
                // Ensure id matches the generating message if streaming or cancelling
                let message_matches = match self.generation {
                    GenerationState::Streaming { message, .. } => message == id,
                    GenerationState::Cancelling { message } => message == id,
                    GenerationState::Error { message } => message == id,
                    GenerationState::Completed { message } => message == id,
                    _ => true, // Waiting has no id bound, so match
                };

                if message_matches {
                    match reason {
                        FinishReason::Completed => {
                            self.generation = GenerationState::Completed { message: id };
                        }
                        FinishReason::Cancelled => {
                            self.generation = GenerationState::Idle;
                        }
                        FinishReason::Error => {
                            self.generation = GenerationState::Error { message: id };
                        }
                    }
                }
            }
            GenerationState::Idle => {}
        }
    }

    /// Domain operation to cancel active streaming.
    pub fn cancel_stream(&mut self, id: MessageId) {
        match self.generation {
            GenerationState::Waiting | GenerationState::Streaming { .. } => {
                self.generation = GenerationState::Cancelling { message: id };
            }
            _ => {}
        }
    }

    /// Domain operation to transition state out of completed/error.
    pub fn reset_generation(&mut self) {
        self.generation = GenerationState::Idle;
    }
}
