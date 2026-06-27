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

use crate::state::{FocusRegion, LoadRequestId};
use brain_domain::SessionId;

async fn trigger_history_load(
    client: &std::sync::Arc<dyn ExecutionClient>,
    sender: tokio::sync::mpsc::UnboundedSender<Event>,
    session_id: SessionId,
    request_id: LoadRequestId,
) {
    let client_clone = client.clone();
    tokio::spawn(async move {
        match client_clone.load_session(session_id).await {
            Ok(messages) => {
                let _ = sender.send(Event::App(AppEvent::HistoryLoaded {
                    session_id,
                    request_id,
                    messages,
                }));
            }
            Err(err) => {
                let _ = sender.send(Event::App(AppEvent::HistoryLoadFailed {
                    session_id,
                    request_id,
                    error: err.to_string(),
                }));
            }
        }
    });
}

/// Main entry point launching the Ratatui interactive user interface.
pub async fn run(client: Box<dyn ExecutionClient>) -> Result<(), BrainError> {
    // 1. Initialize raw mode and alternate screen via the RAII guard
    let _guard = TerminalGuard::new()?;

    // 2. Initialize Ratatui terminal backend
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend).map_err(|e| BrainError::Validation {
        message: format!("Failed to create terminal backend: {}", e),
    })?;

    // Wrap the client in Arc so it can be shared with background tasks
    let client: std::sync::Arc<dyn ExecutionClient> = client.into();

    // 3. Initialize Event multiplexer, Layout renderer, and UI State
    let mut events = EventHandler::new(Duration::from_millis(10)); // 10ms for smooth ticks
    let renderer = AppRenderer::new();
    let mut state = UiState::new();
    let theme = crate::ui::theme::Theme::default();
    
    let mut active_cancel: Option<tokio_util::sync::CancellationToken> = None;
    let mut tokenizer = crate::state::IncrementalTokenizer::new();
    let mut request_id_counter = 0u64;

    // 3a. Query initial session list and history
    {
        let client_clone = client.clone();
        let tx = events.sender();
        let initial_session_id = state.session_id;
        tokio::spawn(async move {
            if let Ok(summaries) = client_clone.list_sessions().await {
                let _ = tx.send(Event::App(AppEvent::SessionsLoaded(summaries)));
            }
            if let Ok(messages) = client_clone.load_session(initial_session_id).await {
                let _ = tx.send(Event::App(AppEvent::HistoryLoaded {
                    session_id: initial_session_id,
                    request_id: LoadRequestId(0),
                    messages,
                }));
            }
        });
    }

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
                        crossterm::event::KeyCode::Tab => Some(Action::ToggleFocus),
                        crossterm::event::KeyCode::Char(c) => Some(Action::InsertChar(c)),
                        crossterm::event::KeyCode::Backspace => {
                            if state.focus == FocusRegion::Sidebar {
                                if state.selected_session_idx < state.sessions.len() {
                                    let session_id = state.sessions[state.selected_session_idx].id;
                                    let client_clone = client.clone();
                                    tokio::spawn(async move {
                                        let _ = client_clone.delete_session(session_id).await;
                                    });
                                    Some(Action::DeleteSession(session_id))
                                } else {
                                    None
                                }
                            } else {
                                Some(Action::Backspace)
                            }
                        }
                        crossterm::event::KeyCode::Delete => {
                            if state.focus == FocusRegion::Sidebar {
                                if state.selected_session_idx < state.sessions.len() {
                                    let session_id = state.sessions[state.selected_session_idx].id;
                                    let client_clone = client.clone();
                                    tokio::spawn(async move {
                                        let _ = client_clone.delete_session(session_id).await;
                                    });
                                    Some(Action::DeleteSession(session_id))
                                } else {
                                    None
                                }
                            } else {
                                Some(Action::Delete)
                            }
                        }
                        crossterm::event::KeyCode::Left => Some(Action::MoveCursorLeft),
                        crossterm::event::KeyCode::Right => Some(Action::MoveCursorRight),
                        crossterm::event::KeyCode::Up => {
                            if state.focus == FocusRegion::Sidebar {
                                Some(Action::MoveSidebarCursorUp)
                            } else {
                                Some(Action::RecallPrevious)
                            }
                        }
                        crossterm::event::KeyCode::Down => {
                            if state.focus == FocusRegion::Sidebar {
                                Some(Action::MoveSidebarCursorDown)
                            } else {
                                Some(Action::RecallNext)
                            }
                        }
                        crossterm::event::KeyCode::Enter => {
                            if state.focus == FocusRegion::Sidebar {
                                if state.selected_session_idx < state.sessions.len() {
                                    let session_id = state.sessions[state.selected_session_idx].id;
                                    request_id_counter += 1;
                                    let req_id = LoadRequestId(request_id_counter);
                                    state.update(Action::ActivateSession {
                                        session_id,
                                        request_id: req_id,
                                    });
                                    trigger_history_load(&client, events.sender(), session_id, req_id).await;
                                    None
                                } else {
                                    None
                                }
                            } else {
                                Some(Action::SubmitPrompt)
                            }
                        }
                        _ => None,
                    };
                    if let Some(act) = action {
                        let res = state.update(act);
                        match res {
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
                            UpdateResult::LoadSession(session_id) => {
                                request_id_counter += 1;
                                let req_id = LoadRequestId(request_id_counter);
                                state.update(Action::ActivateSession {
                                    session_id,
                                    request_id: req_id,
                                });
                                trigger_history_load(&client, events.sender(), session_id, req_id).await;
                            }
                        }
                    }
                }
                Event::Terminal(crate::event::TerminalEvent::Resize(w, h)) => {
                    if let UpdateResult::Exit = state.update(Action::Resize(w, h)) {
                        break;
                    }
                }
                Event::App(AppEvent::SessionsLoaded(summaries)) => {
                    state.update(Action::LoadSessions(summaries));
                }
                Event::App(AppEvent::HistoryLoaded { session_id, request_id, messages }) => {
                    state.update(Action::SessionLoaded { session_id, request_id, messages });
                }
                Event::App(AppEvent::HistoryLoadFailed { session_id, request_id, error }) => {
                    state.update(Action::SessionLoadFailed { session_id, request_id, error });
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
    use crate::state::{GenerationState, SessionLoadState, PendingLoad};
    use crate::ui::theme::Theme;
    use crate::ui::renderer::AppRenderer;
    use brain_domain::Message;
    use async_trait::async_trait;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio_util::sync::CancellationToken;
    use std::time::SystemTime;

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

        // 1. Test size 80x24 (with sidebar)
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| {
            let area = f.size();
            // Verify layout partitions are calculated cleanly
            let (h, sb, c, p, s) = renderer.compute_layout(area);
            assert_eq!(h.height, 3);
            assert_eq!(p.height, 3);
            assert_eq!(s.height, 1);
            assert!(c.height >= 10);
            assert!(sb.height >= 10);
            assert_eq!(sb.width, 25);
            
            renderer.draw(f, area, &state, &theme);
        }).unwrap();

        // 2. Test size 70x24 (compact - no sidebar)
        let backend = TestBackend::new(70, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| {
            let area = f.size();
            let (h, sb, c, p, s) = renderer.compute_layout(area);
            assert_eq!(h.height, 3);
            assert_eq!(p.height, 3);
            assert_eq!(s.height, 1);
            assert!(c.height >= 10);
            assert_eq!(sb.width, 0); // No sidebar area

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

    #[tokio::test]
    async fn test_session_switching_and_flicker_free_transitions() {
        let mut state = UiState::new();
        let session_a = SessionId::new();
        let session_b = SessionId::new();

        // 1. Populate initial active messages (Conversation A)
        let msg_a = Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::User,
            "Hello A".to_string(),
        );
        state.session_id = session_a;
        state.session_title = "Session A".to_string();
        state.active_messages = vec![msg_a];

        // 2. Trigger switch to Session B (enter loading phase)
        let req_1 = LoadRequestId(1);
        state.update(Action::ActivateSession {
            session_id: session_b,
            request_id: req_1,
        });

        // 3. ASSERT: Flicker-free guarantee holds
        // During the loading phase, the displayed active messages must still represent Conversation A
        assert_eq!(state.active_messages.len(), 1);
        assert_eq!(state.active_messages[0].content, "Hello A");
        assert_eq!(state.session_load_state, SessionLoadState::Loading);

        // 4. Session B load completes
        let msg_b = Message::new(
            brain_domain::MessageId::new(),
            brain_domain::MessageRole::User,
            "Hello B".to_string(),
        );
        state.update(Action::SessionLoaded {
            session_id: session_b,
            request_id: req_1,
            messages: vec![msg_b],
        });

        // 5. ASSERT: Conversation B has now fully replaced A
        assert_eq!(state.session_id, session_b);
        assert_eq!(state.active_messages.len(), 1);
        assert_eq!(state.active_messages[0].content, "Hello B");
        assert_eq!(state.session_load_state, SessionLoadState::Loaded(vec![]));
    }

    #[tokio::test]
    async fn test_double_load_and_deletion_race() {
        let mut state = UiState::new();
        let session_a = SessionId::new();
        let session_b = SessionId::new();

        // Setup sessions list
        state.sessions = vec![
            crate::state::SessionViewModel {
                id: session_a,
                title: "Session A".to_string(),
                updated_at: SystemTime::now(),
                active: true,
                preview: None,
            },
            crate::state::SessionViewModel {
                id: session_b,
                title: "Session B".to_string(),
                updated_at: SystemTime::now(),
                active: false,
                preview: None,
            },
        ];

        // 1. Initiate Load A (request 1)
        let req_1 = LoadRequestId(1);
        state.update(Action::ActivateSession { session_id: session_a, request_id: req_1 });
        assert_eq!(state.pending_load, Some(PendingLoad { session_id: session_a, request_id: req_1 }));

        // 2. Initiate Load B (request 2) - overrides target
        let req_2 = LoadRequestId(2);
        state.update(Action::ActivateSession { session_id: session_b, request_id: req_2 });
        assert_eq!(state.pending_load, Some(PendingLoad { session_id: session_b, request_id: req_2 }));

        // 3. Delete Session B mid-load - must invalidate the pending load
        state.update(Action::DeleteSession(session_b));
        assert_eq!(state.pending_load, None);

        // 4. Request 2 completes - must be ignored because pending_load was cleared
        let res_b = state.update(Action::SessionLoaded {
            session_id: session_b,
            request_id: req_2,
            messages: vec![Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                "Hello B".to_string(),
            )],
        });
        assert_eq!(res_b, UpdateResult::NoChange);

        // 5. Request 1 completes - must be ignored because its request ID (1) is older/stale compared to request 2
        let res_a = state.update(Action::SessionLoaded {
            session_id: session_a,
            request_id: req_1,
            messages: vec![Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                "Hello A".to_string(),
            )],
        });
        assert_eq!(res_a, UpdateResult::NoChange);
    }
}
