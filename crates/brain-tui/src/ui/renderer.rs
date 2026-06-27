use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;
use crate::state::{UiState, ConnectionMode, FocusRegion};
use crate::ui::theme::Theme;
use crate::ui::widgets::{
    header::{self, HeaderView},
    chat::{self, ChatView, ChatMessageViewModel},
    prompt::{self, PromptView},
    status::{self, StatusView},
    sidebar,
};

/// Layout grid organizer dividing the screen cells and assembling widget view models.
pub struct AppRenderer;

impl AppRenderer {
    /// Creates a new `AppRenderer`.
    pub fn new() -> Self {
        Self
    }

    /// Computes constraints and returns partitioned area Rects for widgets.
    pub fn compute_layout(&self, area: Rect) -> (Rect, Rect, Rect, Rect, Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Logo / Header
                Constraint::Min(10),  // Mid section (Sidebar + Chat)
                Constraint::Length(3), // Prompt input editor
                Constraint::Length(1), // Footer status bar
            ])
            .split(area);

        let mid_area = chunks[1];
        let has_sidebar = area.width >= 80;

        let (sidebar_area, chat_area) = if has_sidebar {
            let mid_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(25), Constraint::Min(20)])
                .split(mid_area);
            (mid_chunks[0], mid_chunks[1])
        } else {
            (Rect::default(), mid_area)
        };

        (chunks[0], sidebar_area, chat_area, chunks[2], chunks[3])
    }

    /// Derives lightweight ViewModels from state and draws all TUI components.
    pub fn draw(&self, f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
        let (header_area, sidebar_area, chat_area, prompt_area, status_area) = self.compute_layout(area);

        // 1. Build Header ViewModel
        let connection_status = match state.connection_mode {
            ConnectionMode::Daemon => "[Connected: Daemon]".to_string(),
            ConnectionMode::Embedded => "[Connected: In-Process]".to_string(),
            ConnectionMode::Disconnected => "[Disconnected]".to_string(),
            ConnectionMode::Connecting => "[Connecting...]".to_string(),
        };
        let connection_color_ok = matches!(
            state.connection_mode,
            ConnectionMode::Daemon | ConnectionMode::Embedded
        );
        let header_view = HeaderView {
            title: "BRAIN v2 Engine".to_string(),
            connection_status,
            connection_color_ok,
        };

        // 2. Draw Sidebar if visible
        if sidebar_area.width > 0 {
            let sidebar_view = sidebar::SidebarView {
                sessions: &state.sessions,
                selected_idx: state.selected_session_idx,
                has_focus: state.focus == FocusRegion::Sidebar,
            };
            sidebar::draw(f, sidebar_area, &sidebar_view, theme);
        }

        // 3. Build Chat ViewModel
        let mut chat_messages = Vec::new();
        if state.active_messages.is_empty() && !state.is_generating() {
            chat_messages.push(ChatMessageViewModel {
                sender: "System".to_string(),
                content: "No messages in this conversation.".to_string(),
            });
        } else {
            for msg in &state.active_messages {
                let sender = match msg.role {
                    brain_domain::MessageRole::User => "User".to_string(),
                    brain_domain::MessageRole::Assistant => "Assistant".to_string(),
                    brain_domain::MessageRole::System => "System".to_string(),
                };
                chat_messages.push(ChatMessageViewModel {
                    sender,
                    content: msg.content.clone(),
                });
            }
            if !state.active_response.is_empty() || state.is_generating() {
                chat_messages.push(ChatMessageViewModel {
                    sender: "Assistant".to_string(),
                    content: state.active_response.clone(),
                });
            }
        }

        let title = if state.session_load_state == crate::state::SessionLoadState::Loading {
            format!(" Conversation (Loading...) - {} ", state.session_title)
        } else {
            format!(" Conversation - {} ", state.session_title)
        };

        let chat_view = ChatView {
            title,
            messages: chat_messages,
            scroll_offset: state.viewport.scroll_offset,
        };

        // 4. Build Prompt ViewModel
        let prompt_view = PromptView {
            prompt_text: state.editor.text(),
            cursor_position: state.editor.cursor(),
            has_focus: state.focus == FocusRegion::Editor,
        };

        // 5. Build Status ViewModel
        let status_view = StatusView {
            message: " Tab: Switch Focus | Esc: Exit | Ctrl+C: Cancel | Enter: Submit ".to_string(),
        };

        // 6. Draw stateless widgets
        header::draw(f, header_area, &header_view, theme);
        chat::draw(f, chat_area, &chat_view, theme);
        prompt::draw(f, prompt_area, &prompt_view, theme);
        status::draw(f, status_area, &status_view, theme);
    }
}

impl Default for AppRenderer {
    fn default() -> Self {
        Self::new()
    }
}

