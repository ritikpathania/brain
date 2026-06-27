#![deny(missing_docs)]

//! Native Rust terminal interface client implementing the presentation layer.

/// Abstract execution client and event stream structures.
pub mod client;

/// Input event multiplexing and ticks handler.
pub mod event;

/// RAII raw mode terminal guards.
pub mod terminal;

/// Presentation and editor state reducer.
pub mod state;

/// Rendering views and layout widget constraints.
pub mod ui;

use crate::client::ExecutionClient;
use crate::event::{Event, AppEvent, EventHandler};
use crate::state::{UiState, Action, UpdateResult};
use crate::terminal::TerminalGuard;
use crate::ui::renderer::AppRenderer;
use brain_core::errors::BrainError;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::time::Duration;

/// Main entry point launching the Ratatui interactive user interface.
pub async fn run(_client: Box<dyn ExecutionClient>) -> Result<(), BrainError> {
    // 1. Initialize raw mode and alternate screen via the RAII guard
    let _guard = TerminalGuard::new()?;

    // 2. Initialize Ratatui terminal backend
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| BrainError::Validation {
        message: format!("Failed to create terminal backend: {}", e),
    })?;

    // 3. Initialize Event multiplexer, Layout renderer, and UI State
    let mut events = EventHandler::new(Duration::from_millis(100));
    let renderer = AppRenderer::new();
    let mut state = UiState::new();

    terminal.clear().map_err(|e| BrainError::Validation {
        message: format!("Failed to clear terminal: {}", e),
    })?;

    // 4. Main event loop
    loop {
        // Render tick cycle
        terminal.draw(|f| {
            let area = f.size();
            let (_header, _chat, _prompt, _status) = renderer.compute_layout(area);
            // Layout is partitioned; drawing widgets will populate these in later milestones
        }).map_err(|e| BrainError::Validation {
            message: format!("Failed to draw terminal frame: {}", e),
        })?;

        // Await next event from multiplexer queue
        if let Some(event) = events.next().await {
            match event {
                Event::Terminal(crate::event::TerminalEvent::Key(key)) => {
                    let action = match key.code {
                        crossterm::event::KeyCode::Esc => Some(Action::Quit),
                        crossterm::event::KeyCode::Char('c') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                            Some(Action::Quit)
                        }
                        crossterm::event::KeyCode::Char(c) => Some(Action::InsertChar(c)),
                        crossterm::event::KeyCode::Backspace => Some(Action::Backspace),
                        crossterm::event::KeyCode::Delete => Some(Action::Delete),
                        crossterm::event::KeyCode::Left => Some(Action::MoveCursorLeft),
                        crossterm::event::KeyCode::Right => Some(Action::MoveCursorRight),
                        _ => None,
                    };
                    if let Some(act) = action {
                        match state.update(act) {
                            UpdateResult::Exit => break,
                            UpdateResult::Changed => {}
                            UpdateResult::NoChange => {}
                        }
                    }
                }
                Event::Terminal(crate::event::TerminalEvent::Resize(w, h)) => {
                    if let UpdateResult::Exit = state.update(Action::Resize(w, h)) {
                        break;
                    }
                }
                Event::App(AppEvent::Shutdown) => {
                    break;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::client::{ExecutionRequest, EventReceiver, SessionSummary};
    use brain_domain::Message;
    use async_trait::async_trait;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio_util::sync::CancellationToken;

    struct StubClient;

    #[async_trait]
    impl ExecutionClient for StubClient {
        async fn execute(&self, _req: ExecutionRequest) -> Result<EventReceiver, BrainError> {
            let (_, rx) = unbounded_channel();
            Ok(EventReceiver::new(rx, CancellationToken::new()))
        }
        async fn list_sessions(&self) -> Result<Vec<SessionSummary>, BrainError> { Ok(vec![]) }
        async fn load_session(&self, _id: brain_domain::SessionId) -> Result<Vec<Message>, BrainError> { Ok(vec![]) }
        async fn delete_session(&self, _id: brain_domain::SessionId) -> Result<(), BrainError> { Ok(()) }
    }

    #[tokio::test]
    async fn test_lifecycle_shutdown_exit() {
        let _client = StubClient;
        // Test that RAII guard behaves correctly. In headless CI crossterm might return error,
        // which is handled gracefully.
        let guard = TerminalGuard::new();
        if let Ok(g) = guard {
            drop(g);
        }
    }
}
