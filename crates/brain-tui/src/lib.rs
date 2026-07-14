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

/// Clipboard abstractions and platform implementations.
pub mod clipboard;

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
    
    // Initialize unified omnibox search
    let search_sink = std::sync::Arc::new(ChannelEventSink {
        sender: events.sender(),
    });
    state.command_palette_mut().initialize(client.clone(), search_sink);

    let theme = crate::ui::theme::Theme::default();
    
    let mut active_cancel: Option<tokio_util::sync::CancellationToken> = None;
    let mut tokenizer = crate::state::IncrementalTokenizer::new();
    let mut request_id_counter = 0u64;

    // Connection starts as Connecting — transitions to Daemon on first successful
    // stream_start handshake, or Disconnected on socket error.
    state.update(Action::SetConnectionMode(crate::state::ConnectionMode::Connecting));

    // 3a. Query initial session list and history — also probes connectivity.
    {
        let client_clone = client.clone();
        let tx = events.sender();
        let initial_session_id = state.session_id;
        tokio::spawn(async move {
            match client_clone.list_sessions().await {
                Ok(summaries) => {
                    // Socket reachable — signal Connected before anything else.
                    let _ = tx.send(Event::App(AppEvent::Connected));
                    let _ = tx.send(Event::App(AppEvent::SessionsLoaded(summaries)));
                }
                Err(_) => {
                    let _ = tx.send(Event::App(AppEvent::Disconnected));
                }
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
        state.recalculate_viewport();

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
                    let action = if !state.pending_approvals.is_empty() {
                        let first = state.pending_approvals.first().unwrap().clone();
                        match key.code {
                            crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') | crossterm::event::KeyCode::Enter => {
                                let client_clone = client.clone();
                                let call_id = first.call_id.clone();
                                tokio::spawn(async move {
                                    let _ = client_clone.approve_tool_call(call_id, true).await;
                                });
                                Some(Action::ApproveToolCall { call_id: first.call_id, approved: true })
                            }
                            crossterm::event::KeyCode::Char('n') | crossterm::event::KeyCode::Char('N') | crossterm::event::KeyCode::Esc => {
                                let client_clone = client.clone();
                                let call_id = first.call_id.clone();
                                tokio::spawn(async move {
                                    let _ = client_clone.approve_tool_call(call_id, false).await;
                                });
                                Some(Action::ApproveToolCall { call_id: first.call_id, approved: false })
                            }
                            _ => None,
                        }
                    } else if state.overlay == crate::state::TuiOverlay::PinnedContext {
                        match key.code {
                            crossterm::event::KeyCode::Esc => {
                                Some(Action::ClosePinnedOverlay)
                            }
                            crossterm::event::KeyCode::Up => {
                                Some(Action::PinnedOverlayUp)
                            }
                            crossterm::event::KeyCode::Down => {
                                Some(Action::PinnedOverlayDown)
                            }
                            crossterm::event::KeyCode::Enter => {
                                Some(Action::InspectPinnedNode(state.pinned_overlay_cursor))
                            }
                            crossterm::event::KeyCode::Char('x') | crossterm::event::KeyCode::Char('X') => {
                                if !state.pinned_nodes.is_empty() && state.pinned_overlay_cursor < state.pinned_nodes.len() {
                                    Some(Action::UnpinNode(state.pinned_nodes[state.pinned_overlay_cursor].node_id))
                                } else {
                                    None
                                }
                            }
                            crossterm::event::KeyCode::Char('c') | crossterm::event::KeyCode::Char('C') => {
                                Some(Action::ClearAllPins)
                            }
                            _ => None, // Focus is locked inside overlay
                        }
                    } else if state.mode == crate::state::TuiMode::Exploration {
                        match key.code {
                            crossterm::event::KeyCode::Esc => {
                                Some(Action::CloseInspector)
                            }
                            crossterm::event::KeyCode::Backspace => {
                                Some(Action::PopBreadcrumb)
                            }
                            crossterm::event::KeyCode::Up => {
                                if state.focus == FocusRegion::Inspector {
                                    Some(Action::PrevInspectorRelation)
                                } else {
                                    Some(Action::ScrollUp(1))
                                }
                            }
                            crossterm::event::KeyCode::Down => {
                                if state.focus == FocusRegion::Inspector {
                                    Some(Action::NextInspectorRelation)
                                } else {
                                    Some(Action::ScrollDown(1))
                                }
                            }
                            crossterm::event::KeyCode::Enter => {
                                if state.focus == FocusRegion::Inspector {
                                    Some(Action::TraverseToRelation)
                                } else {
                                    None
                                }
                            }
                            crossterm::event::KeyCode::Tab => {
                                Some(Action::ToggleFocus)
                            }
                            crossterm::event::KeyCode::PageUp => {
                                if state.focus == FocusRegion::Inspector {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollInspectorUp(page))
                                } else {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollUp(page))
                                }
                            }
                            crossterm::event::KeyCode::PageDown => {
                                if state.focus == FocusRegion::Inspector {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollInspectorDown(page))
                                } else {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollDown(page))
                                }
                            }
                            crossterm::event::KeyCode::Char('u') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                if state.focus == FocusRegion::Inspector {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollInspectorUp((page / 2).max(1)))
                                } else {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollUp((page / 2).max(1)))
                                }
                            }
                            crossterm::event::KeyCode::Char('d') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                if state.focus == FocusRegion::Inspector {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollInspectorDown((page / 2).max(1)))
                                } else {
                                    let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                    Some(Action::ScrollDown((page / 2).max(1)))
                                }
                            }
                            crossterm::event::KeyCode::Char('p') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                Some(Action::OpenPinnedOverlay)
                            }
                            crossterm::event::KeyCode::Char('p') | crossterm::event::KeyCode::Char('P') => {
                                Some(Action::PinCurrentNode)
                            }
                            crossterm::event::KeyCode::Char('c') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                Some(Action::Quit)
                            }
                            _ => None,
                        }
                    } else {
                        match key.code {
                            crossterm::event::KeyCode::Char('p') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                Some(Action::OpenPinnedOverlay)
                            }
                            crossterm::event::KeyCode::Esc => {
                                if state.is_generating() {
                                    if let Some(token) = active_cancel.take() {
                                        token.cancel();
                                    }
                                    Some(Action::CancelStream)
                                } else if state.focus == FocusRegion::Sidebar {
                                    Some(Action::ToggleFocus)
                                } else {
                                    None
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
                            crossterm::event::KeyCode::Char('q') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                if state.is_generating() {
                                    if let Some(token) = active_cancel.take() {
                                        token.cancel();
                                    }
                                }
                                Some(Action::Quit)
                            }
                            crossterm::event::KeyCode::Char('n') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                Some(Action::NewSession)
                            }
                            // Alt+W toggles "Submit with Workspace" mode.
                            // Selected after terminal compatibility testing (RFC-007 validation):
                            //   - Ctrl+W is intercepted by VS Code (closes editor tab) — CONFLICT
                            //   - Alt+W passes through cleanly in Terminal.app, iTerm2, tmux, Ghostty
                            //   - crossterm reports: KeyCode::Char('w') + ALT modifier
                            crossterm::event::KeyCode::Char('w') if key.modifiers == crossterm::event::KeyModifiers::ALT => {
                                Some(Action::ToggleSubmitWithWorkspace)
                            }
                            crossterm::event::KeyCode::PageUp => {
                                let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                Some(Action::ScrollUp(page))
                            }
                            crossterm::event::KeyCode::PageDown => {
                                let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                Some(Action::ScrollDown(page))
                            }
                            crossterm::event::KeyCode::Char('u') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                Some(Action::ScrollUp((page / 2).max(1)))
                            }
                            crossterm::event::KeyCode::Char('d') if key.modifiers == crossterm::event::KeyModifiers::CONTROL => {
                                let page = (state.terminal_height.saturating_sub(9) as usize).max(1);
                                Some(Action::ScrollDown((page / 2).max(1)))
                            }
                            crossterm::event::KeyCode::Up if key.modifiers == crossterm::event::KeyModifiers::CONTROL => Some(Action::ScrollUp(1)),
                            crossterm::event::KeyCode::Down if key.modifiers == crossterm::event::KeyModifiers::CONTROL => Some(Action::ScrollDown(1)),
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
                                if state.connection_mode == crate::state::ConnectionMode::Disconnected && state.editor.text().trim().is_empty() {
                                    let client_clone = client.clone();
                                    let tx = events.sender();
                                    let initial_session_id = state.session_id;
                                    state.update(Action::SetConnectionMode(crate::state::ConnectionMode::Connecting));
                                    tokio::spawn(async move {
                                        match client_clone.list_sessions().await {
                                            Ok(summaries) => {
                                                let _ = tx.send(Event::App(AppEvent::Connected));
                                                let _ = tx.send(Event::App(AppEvent::SessionsLoaded(summaries)));
                                            }
                                            Err(_) => {
                                                let _ = tx.send(Event::App(AppEvent::Disconnected));
                                            }
                                        }
                                        if let Ok(messages) = client_clone.load_session(initial_session_id).await {
                                            let _ = tx.send(Event::App(AppEvent::HistoryLoaded {
                                                session_id: initial_session_id,
                                                request_id: LoadRequestId(0),
                                                messages,
                                            }));
                                        }
                                    });
                                    None
                                } else {
                                    Some(Action::SubmitPrompt)
                                }
                            }
                        }
                        _ => None,
                    }
                };

                    if let Some(act) = action {
                        let res = state.update(act);
                        match res {
                            UpdateResult::Exit => break,
                            UpdateResult::Changed => {}
                            UpdateResult::NoChange => {}
                            UpdateResult::PromptSubmitted(prompt) => {
                                // Cancel any in-flight stream before starting a new one.
                                if let Some(old_token) = active_cancel.take() {
                                    old_token.cancel();
                                }
                                let cancellation_token = tokio_util::sync::CancellationToken::new();
                                active_cancel = Some(cancellation_token.clone());
                                tokenizer = crate::state::IncrementalTokenizer::new();

                                // Capture workspace context before the request is dispatched.
                                // The flag is reset ONLY after client.execute() returns Ok so
                                // that a failed dispatch can be retried with the same context.
                                let workspace_context = if state.submit_with_workspace
                                    && !state.pinned_nodes.is_empty()
                                {
                                    Some(
                                        state
                                            .pinned_nodes
                                            .iter()
                                            .map(|pn| pn.node_id)
                                            .collect::<Vec<_>>(),
                                    )
                                } else {
                                    None
                                };

                                let req = crate::client::ExecutionRequest {
                                    session_id: state.session_id,
                                    prompt,
                                    options: crate::client::ExecutionOptions::default(),
                                    cancellation_token,
                                    workspace_context,
                                };
                                if let Ok(mut event_receiver) = client.execute(req).await {
                                    // Dispatch succeeded — safe to reset the flag.
                                    // The user must re-toggle to attach context again.
                                    state.update(Action::ResetSubmitWithWorkspace);

                                    // Only show Connecting if we were previously Disconnected —
                                    // avoids flickering the header on every query when already Daemon.
                                    if state.connection_mode == crate::state::ConnectionMode::Disconnected {
                                        state.update(Action::SetConnectionMode(
                                            crate::state::ConnectionMode::Connecting,
                                        ));
                                    }
                                    let tx = events.sender();
                                    tokio::spawn(async move {
                                        let mut stream_completed = false;
                                        while let Some(res) = event_receiver.recv().await {
                                            match res {
                                                Ok(event) => {
                                                    // Mark the stream as completed so the
                                                    // EOF path below can tell the difference
                                                    // between a clean finish and a crash.
                                                    if matches!(
                                                        event.kind,
                                                        brain_core::events::StreamEventKind::Finished { .. }
                                                            | brain_core::events::StreamEventKind::Cancelled
                                                    ) {
                                                        stream_completed = true;
                                                    }
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
                                        // Channel closed. If we never received a Finished or
                                        // Cancelled event the daemon disconnected unexpectedly.
                                        if !stream_completed {
                                            let _ = tx.send(Event::App(AppEvent::StreamEof));
                                        }
                                    });
                                } else {
                                    // execute() itself failed — socket unreachable.
                                    state.update(Action::SetConnectionMode(
                                        crate::state::ConnectionMode::Disconnected,
                                    ));
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
                            UpdateResult::InspectNode(node_id) => {
                                let client_clone = std::sync::Arc::clone(&client);
                                let tx = events.sender();
                                tokio::spawn(async move {
                                    match client_clone.inspect_node(node_id).await {
                                        Ok(model) => {
                                            let _ = tx.send(Event::App(AppEvent::InspectNodeLoaded(model)));
                                        }
                                        Err(e) => {
                                            let _ = tx.send(Event::App(AppEvent::InspectNodeFailed(e.to_string())));
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                Event::Terminal(crate::event::TerminalEvent::Resize(w, h)) => {
                    if let UpdateResult::Exit = state.update(Action::Resize(w, h)) {
                        break;
                    }
                }
                Event::Terminal(crate::event::TerminalEvent::Mouse(mouse)) => {
                    match mouse.kind {
                        crossterm::event::MouseEventKind::ScrollUp => {
                            if state.mode == crate::state::TuiMode::Exploration && state.focus == FocusRegion::Inspector {
                                state.update(Action::ScrollInspectorUp(3));
                            } else {
                                state.update(Action::ScrollUp(3));
                            }
                        }
                        crossterm::event::MouseEventKind::ScrollDown => {
                            if state.mode == crate::state::TuiMode::Exploration && state.focus == FocusRegion::Inspector {
                                state.update(Action::ScrollInspectorDown(3));
                            } else {
                                state.update(Action::ScrollDown(3));
                            }
                        }
                        _ => {}
                    }
                }
                Event::App(AppEvent::Search(search_event)) => {
                    if let Some(ref mut agg) = state.command_palette_mut().search_aggregator {
                        agg.handle_event(search_event);
                        state.command_palette_mut().view_state = agg.view_state();
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
                Event::App(AppEvent::InspectNodeLoaded(model)) => {
                    state.update(Action::NodeDetailsLoaded(model));
                }
                Event::App(AppEvent::InspectNodeFailed(error)) => {
                    state.update(Action::NodeDetailsFailed(error));
                }
                Event::App(AppEvent::Stream(stream_event)) => {
                    match stream_event.kind {
                        brain_core::events::StreamEventKind::Token(token) => {
                            let tokens = tokenizer.push_chunk(&token);
                            for tok in tokens {
                                state.update(Action::ReceiveToken(tok));
                            }
                        }
                        brain_core::events::StreamEventKind::Stage { ref name, active } if name == "Start" && active => {
                            // Protocol handshake confirmed — connection is live.
                            state.update(Action::SetConnectionMode(
                                crate::state::ConnectionMode::Daemon,
                            ));
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
                        brain_core::events::StreamEventKind::ToolCallRequest { call_id, tool_id, arguments, requires_approval } => {
                            state.update(Action::ToolCallRequested {
                                message: crate::ui::interaction::MessageId(0),
                                call_id,
                                tool_id,
                                arguments,
                                requires_approval,
                            });
                        }
                        brain_core::events::StreamEventKind::ToolProgress { call_id, sequence, detail, message } => {
                            state.update(Action::ToolProgressReceived {
                                message: crate::ui::interaction::MessageId(0),
                                call_id,
                                sequence,
                                detail,
                                log_message: message,
                            });
                        }
                        brain_core::events::StreamEventKind::ToolCallResult { call_id, result, is_error } => {
                            state.update(Action::ToolResultReceived {
                                message: crate::ui::interaction::MessageId(0),
                                call_id,
                                result,
                                is_error,
                            });
                        }
                        brain_core::events::StreamEventKind::RetrievalStarted { query } => {
                            state.update(Action::RetrievalStarted {
                                message: crate::ui::interaction::MessageId(0),
                                query,
                            });
                        }
                        brain_core::events::StreamEventKind::RetrievalRetrieved { info } => {
                            state.update(Action::RetrievalReceived {
                                message: crate::ui::interaction::MessageId(0),
                                info,
                            });
                        }
                        brain_core::events::StreamEventKind::RetrievalCompleted => {
                            state.update(Action::RetrievalCompleted {
                                message: crate::ui::interaction::MessageId(0),
                            });
                        }
                        brain_core::events::StreamEventKind::WorkspaceContextUsed(context_used) => {
                            if !context_used.is_empty() {
                                let labels = context_used.join(", ");
                                let msg = format!("\u{1f4cc} Context used: {}", labels);
                                state.update(Action::SetTransientMessage(msg));
                            }
                        }
                        _ => {}
                    }
                }
                Event::App(AppEvent::Error(err_msg)) => {
                    state.update(Action::ReportError(err_msg));
                    // A stream error means the daemon-side connection is broken.
                    state.update(Action::SetConnectionMode(
                        crate::state::ConnectionMode::Disconnected,
                    ));
                    active_cancel = None;
                }
                Event::App(AppEvent::StreamEof) => {
                    // Daemon closed the socket without sending Finished/Cancelled —
                    // treat as unexpected disconnection.
                    state.update(Action::ReportError(
                        "Daemon disconnected unexpectedly.".to_string(),
                    ));
                    state.update(Action::SetConnectionMode(
                        crate::state::ConnectionMode::Disconnected,
                    ));
                    active_cancel = None;
                }
                Event::Tick => {
                    state.update(Action::TypewriterTick(std::time::Instant::now()));
                }
                Event::App(AppEvent::Shutdown) => {
                    break;
                }
                Event::App(AppEvent::Connected) => {
                    state.update(Action::SetConnectionMode(crate::state::ConnectionMode::Daemon));
                }
                Event::App(AppEvent::Disconnected) => {
                    state.update(Action::SetConnectionMode(crate::state::ConnectionMode::Disconnected));
                }
                _ => {}
            }
        }
    }

    Ok(())
}

struct ChannelEventSink {
    sender: tokio::sync::mpsc::UnboundedSender<Event>,
}

impl crate::ui::search::types::SearchEventSink for ChannelEventSink {
    fn submit(&self, event: crate::ui::search::types::SearchEvent) {
        let _ = self.sender.send(Event::App(AppEvent::Search(event)));
    }
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
        async fn approve_tool_call(&self, _call_id: brain_core::events::ToolCallId, _approved: bool) -> Result<(), BrainError> { Ok(()) }
        async fn search_messages(&self, _query: &str) -> Result<Vec<Message>, BrainError> { Ok(vec![]) }
        async fn inspect_node(&self, id: brain_domain::NodeId) -> Result<brain_domain::query::inspector::InspectorModel, BrainError> {
            let entity = brain_domain::dtos::NodeDTO::new(
                id.to_string(),
                "Mock Node".to_string(),
                "Technology".to_string(),
                serde_json::Value::Null,
            );
            Ok(brain_domain::query::inspector::InspectorModel {
                entity,
                metadata: std::collections::HashMap::new(),
                relationships: vec![],
                provenance: brain_domain::query::inspector::ProvenanceDTO {
                    source: "Mock".to_string(),
                    location: "Mock Location".to_string(),
                    timestamp: 123456,
                    extra_info: std::collections::HashMap::new(),
                },
                retrieval_explanation: None,
                recent_activity: vec![],
            })
        }
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
            let (h, sb, c, _insp, p, s) = renderer.compute_layout(area, &state);
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
            let (h, sb, c, _insp, p, s) = renderer.compute_layout(area, &state);
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
                pinned: false,
                archived: false,
            },
            crate::state::SessionViewModel {
                id: session_b,
                title: "Session B".to_string(),
                updated_at: SystemTime::now(),
                active: false,
                preview: None,
                pinned: false,
                archived: false,
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
