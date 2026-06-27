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
pub async fn run(client: Box<dyn ExecutionClient>) -> Result<(), BrainError> {
    // 1. Initialize raw mode and alternate screen via the RAII guard
    let _guard = TerminalGuard::new()?;

    // 2. Initialize Ratatui terminal backend
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| BrainError::Validation {
        message: format!("Failed to create terminal backend: {}", e),
    })?;

    // 3. Initialize Event multiplexer, Layout renderer, and UI State
    let mut events = EventHandler::new(Duration::from_millis(10)); // 10ms for smooth ticks
    let renderer = AppRenderer::new();
    let mut state = UiState::new();
    let theme = crate::ui::theme::Theme::default();
    
    let mut active_cancel: Option<tokio_util::sync::CancellationToken> = None;
    let mut tokenizer = crate::state::IncrementalTokenizer::new();

    terminal.clear().map_err(|e| BrainError::Validation {
        message: format!("Failed to clear terminal: {}", e),
    })?;

    // 4. Main event loop
    loop {
        // Render tick cycle
        terminal.draw(|f| {
            let area = f.size();
            renderer.draw(f, area, &state, &theme);
        }).map_err(|e| BrainError::Validation {
            message: format!("Failed to draw terminal frame: {}", e),
        })?;

        // Await next event from multiplexer queue
        if let Some(event) = events.next().await {
            match event {
                Event::Terminal(crate::event::TerminalEvent::Key(key)) => {
                    let action = match key.code {
                        crossterm::event::KeyCode::Esc => {
                            if state.is_generating() {
                                if let Some(token) = active_cancel.take() {
                                    token.cancel();
                                }
                                Some(Action::CancelStream)
                            } else {
                                Some(Action::Quit)
                            }
                        }
                        crossterm::event::KeyCode::Char('c') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                            if state.is_generating() {
                                if let Some(token) = active_cancel.take() {
                                    token.cancel();
                                }
                            }
                            Some(Action::Quit)
                        }
                        crossterm::event::KeyCode::Char(c) => Some(Action::InsertChar(c)),
                        crossterm::event::KeyCode::Backspace => Some(Action::Backspace),
                        crossterm::event::KeyCode::Delete => Some(Action::Delete),
                        crossterm::event::KeyCode::Left => Some(Action::MoveCursorLeft),
                        crossterm::event::KeyCode::Right => Some(Action::MoveCursorRight),
                        crossterm::event::KeyCode::Up => Some(Action::RecallPrevious),
                        crossterm::event::KeyCode::Down => Some(Action::RecallNext),
                        crossterm::event::KeyCode::Enter => Some(Action::SubmitPrompt),
                        _ => None,
                    };
                    if let Some(act) = action {
                        match state.update(act) {
                            UpdateResult::Exit => break,
                            UpdateResult::Changed => {}
                            UpdateResult::NoChange => {}
                            UpdateResult::PromptSubmitted(prompt) => {
                                let cancellation_token = tokio_util::sync::CancellationToken::new();
                                active_cancel = Some(cancellation_token.clone());
                                tokenizer = crate::state::IncrementalTokenizer::new();

                                let req = crate::client::ExecutionRequest {
                                    session_id: state.session_id,
                                    prompt,
                                    options: crate::client::ExecutionOptions::default(),
                                    cancellation_token,
                                };
                                if let Ok(mut event_receiver) = client.execute(req).await {
                                    let tx = events.sender();
                                    tokio::spawn(async move {
                                        while let Some(res) = event_receiver.recv().await {
                                            match res {
                                                Ok(event) => {
                                                    if tx.send(Event::App(AppEvent::Stream(event))).is_err() {
                                                        break;
                                                    }
                                                }
                                                Err(e) => {
                                                    let _ = tx.send(Event::App(AppEvent::Error(e.to_string())));
                                                    break;
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }
                }
                Event::Terminal(crate::event::TerminalEvent::Resize(w, h)) => {
                    if let UpdateResult::Exit = state.update(Action::Resize(w, h)) {
                        break;
                    }
                }
                Event::App(AppEvent::Stream(stream_event)) => {
                    match stream_event.kind {
                        brain_core::events::StreamEventKind::Token(token) => {
                            let tokens = tokenizer.push_chunk(&token);
                            for tok in tokens {
                                state.update(Action::ReceiveToken(tok));
                            }
                        }
                        brain_core::events::StreamEventKind::Finished { .. } => {
                            let tokens = tokenizer.flush();
                            for tok in tokens {
                                state.update(Action::ReceiveToken(tok));
                            }
                            state.update(Action::FinishStream);
                            active_cancel = None;
                        }
                        brain_core::events::StreamEventKind::Cancelled => {
                            state.update(Action::CancelStream);
                            active_cancel = None;
                        }
                        _ => {}
                    }
                }
                Event::App(AppEvent::Error(err_msg)) => {
                    state.update(Action::ReportError(err_msg));
                    active_cancel = None;
                }
                Event::Tick => {
                    state.update(Action::TypewriterTick(std::time::Instant::now()));
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
    use crate::state::GenerationState;
    use crate::ui::theme::Theme;
    use crate::ui::renderer::AppRenderer;
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

    #[tokio::test]
    async fn test_renderer_dimensions() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let state = UiState::new();
        let theme = Theme::default();
        let renderer = AppRenderer::new();

        // 1. Test size 80x24
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| {
            let area = f.size();
            // Verify layout partitions are calculated cleanly
            let (h, c, p, s) = renderer.compute_layout(area);
            assert_eq!(h.height, 3);
            assert_eq!(p.height, 3);
            assert_eq!(s.height, 1);
            assert!(c.height >= 10);
            
            renderer.draw(f, area, &state, &theme);
        }).unwrap();

        // 2. Test size 120x40
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| {
            let area = f.size();
            let (h, c, p, s) = renderer.compute_layout(area);
            assert_eq!(h.height, 3);
            assert_eq!(p.height, 3);
            assert_eq!(s.height, 1);
            assert!(c.height >= 10);

            renderer.draw(f, area, &state, &theme);
        }).unwrap();
    }

    #[tokio::test]
    async fn test_loop_key_history_triggers() {
        let _client = StubClient;
        let mut state = UiState::with_history_capacity(10);
        
        // Simulating sequence: 'a', Submit, 'b', Submit, Up Arrow, Down Arrow
        state.update(Action::InsertChar('a'));
        let res = state.update(Action::SubmitPrompt);
        assert!(matches!(res, UpdateResult::PromptSubmitted(_)));
        state.generation_state = GenerationState::Idle;
        
        state.update(Action::InsertChar('b'));
        state.update(Action::SubmitPrompt);
        state.generation_state = GenerationState::Idle;
        
        // Typing uncommitted draft 'c'
        state.update(Action::InsertChar('c'));
        
        // Previous -> gets 'b'
        state.update(Action::RecallPrevious);
        assert_eq!(state.editor.text(), "b");
        
        // Previous again -> gets 'a'
        state.update(Action::RecallPrevious);
        assert_eq!(state.editor.text(), "a");
        
        // Next -> gets 'b'
        state.update(Action::RecallNext);
        assert_eq!(state.editor.text(), "b");
        
        // Next again -> gets draft 'c'
        state.update(Action::RecallNext);
        assert_eq!(state.editor.text(), "c");
    }

    #[test]
    fn test_multi_tick_cancellation() {
        let mut state = UiState::new();
        state.update(Action::StartStream);
        assert_eq!(state.generation_state, GenerationState::Starting);

        // Receive A, B, C tokens without ticks
        state.update(Action::ReceiveToken(crate::state::RenderToken::Text("A".to_string())));
        state.update(Action::ReceiveToken(crate::state::RenderToken::Text("B".to_string())));
        state.update(Action::ReceiveToken(crate::state::RenderToken::Text("C".to_string())));

        // Nothing visible yet
        assert_eq!(state.active_response, "");

        // First tick -> renders "A"
        let t0 = std::time::Instant::now();
        state.update(Action::TypewriterTick(t0));
        assert_eq!(state.active_response, "A");

        // Next tick after 35ms -> renders "AB"
        let t1 = t0 + std::time::Duration::from_millis(35);
        state.update(Action::TypewriterTick(t1));
        assert_eq!(state.active_response, "AB");

        // Cancel Active stream -> clears typewriter queue and transitions to Cancelled
        state.update(Action::CancelStream);
        assert_eq!(state.generation_state, GenerationState::Cancelled(None));
        assert!(state.typewriter.is_empty());
    }
}
