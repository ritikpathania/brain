use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;
use crate::state::{UiState, ConnectionMode};
use crate::ui::theme::Theme;
use crate::ui::widgets::{
    header::{self, HeaderView},
    chat::{self, ChatView, ChatMessageViewModel},
    prompt::{self, PromptView},
    status::{self, StatusView},
};

/// Layout grid organizer dividing the screen cells and assembling widget view models.
pub struct AppRenderer;

impl AppRenderer {
    /// Creates a new `AppRenderer`.
    pub fn new() -> Self {
        Self
    }

    /// Computes constraints and returns partitioned area Rects for widgets.
    pub fn compute_layout(&self, area: Rect) -> (Rect, Rect, Rect, Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Logo / Header
                Constraint::Min(10),  // Chat window viewport
                Constraint::Length(3), // Prompt input editor
                Constraint::Length(1), // Footer status bar
            ])
            .split(area);
        
        (chunks[0], chunks[1], chunks[2], chunks[3])
    }

    /// Derives lightweight ViewModels from state and draws all TUI components.
    pub fn draw(&self, f: &mut Frame<'_>, area: Rect, state: &UiState, theme: &Theme) {
        let (header_area, chat_area, prompt_area, status_area) = self.compute_layout(area);

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

        // 2. Build Chat ViewModel
        let messages = vec![
            ChatMessageViewModel {
                sender: "System".to_string(),
                content: "Welcome to BRAIN v2 interface scaffold.".to_string(),
            }
        ];
        let chat_view = ChatView {
            messages,
            scroll_offset: state.viewport.scroll_offset,
        };

        // 3. Build Prompt ViewModel
        let prompt_view = PromptView {
            prompt_text: state.editor.text(),
            cursor_position: state.editor.cursor(),
        };

        // 4. Build Status ViewModel
        let status_view = StatusView {
            message: " Esc: Exit | Ctrl+C: Cancel | Enter: Submit ".to_string(),
        };

        // 5. Draw stateless widgets
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
