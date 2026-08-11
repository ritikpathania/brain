use brain_domain::SessionId;
use brain_tui::client::SessionSummary;
use brain_tui::state::{Action, ConnectionMode, GenerationState, LoadRequestId, UiState};
use brain_tui::ui::navigation::{Modal, Screen};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::dark_theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::SystemTime;

const CERTIFIED_VIEWPORTS: [(u16, u16); 4] = [
    (80, 24),  // Compact standard
    (100, 26), // Medium standard
    (120, 30), // Large standard
    (182, 53), // Ultrawide
];

#[test]
fn audit_canonical_user_flow_end_to_end() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        // 1. Launch -> HOME
        let mut state = UiState::new();
        state.screen = Screen::Home;
        state.connection_mode = ConnectionMode::Daemon;
        state.terminal_width = w;
        state.terminal_height = h;

        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let home_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            home_text.contains("BRAIN") || home_text.contains("/help") || home_text.contains("shortcuts") || home_text.contains("Claude Code"),
            "Step 1 (Launch -> Home) failed at {}x{}",
            w,
            h
        );

        // 2. Enter query on Home
        for c in "Explain memory architecture".chars() {
            state.editor.insert(c);
        }
        state.update(Action::SubmitPrompt);
        state.active_response = "Brain uses SQLite and relational memory graph.".to_string();
        state.generation_state = GenerationState::Finished;
        state.recalculate_viewport();

        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let query_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            query_text.contains("Explain memory architecture"),
            "Step 2 (Home query submission) failed at {}x{}",
            w,
            h
        );

        // 3. Navigate Left -> WORKSPACE
        state.screen = Screen::Workspace;
        let session_1 = SessionId::new();
        let session_2 = SessionId::new();
        state.update(Action::LoadSessions(vec![
            SessionSummary {
                id: session_1,
                title: "Previous Query Session".to_string(),
                updated_at: SystemTime::now(),
                pinned: false,
                archived: false,
            },
            SessionSummary {
                id: session_2,
                title: "Another Session".to_string(),
                updated_at: SystemTime::now(),
                pinned: false,
                archived: false,
            },
        ]));
        state.focus = brain_tui::state::FocusRegion::Sidebar;
        state.selected_session_idx = 0;

        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let ws_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            ws_text.contains("Previous") || ws_text.contains("Query"),
            "Step 3 (Workspace session browser) failed at {}x{}",
            w,
            h
        );

        // 4. Up/Down Selection & Open Session
        state.update(Action::SelectNextSession);
        assert_eq!(state.selected_session_idx, 1);

        state.update(Action::SelectPreviousSession);
        assert_eq!(state.selected_session_idx, 0);

        // Enter -> Open Session
        state.screen = Screen::Home;
        state.update(Action::ActivateSession {
            session_id: session_1,
            request_id: LoadRequestId(1),
        });
        state.update(Action::SessionLoaded {
            session_id: session_1,
            request_id: LoadRequestId(1),
            messages: vec![
                brain_domain::Message::new(
                    brain_domain::MessageId::new(),
                    brain_domain::MessageRole::User,
                    "Previous query?".to_string(),
                ),
                brain_domain::Message::new(
                    brain_domain::MessageId::new(),
                    brain_domain::MessageRole::Assistant,
                    "Previous answer.".to_string(),
                ),
            ],
        });

        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let session_view_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            session_view_text.contains("Previous answer"),
            "Step 4 (Open Session History) failed at {}x{}",
            w,
            h
        );

        // 5. Space -> Reply Composer Modal
        state.screen = Screen::Workspace;
        state.modal = Some(Modal::ReplyComposer);
        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let reply_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            reply_text.contains("Reply") || reply_text.contains("Previous") || reply_text.contains("Needs input"),
            "Step 5 (Reply Composer Modal) failed at {}x{}",
            w,
            h
        );

        // Close modal and send follow-up query
        state.modal = None;
        state.screen = Screen::Home;
        for c in "Tell me more about edges".chars() {
            state.editor.insert(c);
        }
        state.update(Action::SubmitPrompt);
        state.active_response =
            "Edges represent relationships between knowledge nodes.".to_string();

        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let follow_up_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            follow_up_text.contains("edges"),
            "Step 6 (Follow-up Query) failed at {}x{}",
            w,
            h
        );

        // 6. Navigate Right -> HOME
        state.screen = Screen::Home;
        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let return_home_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            return_home_text.contains("BRAIN") || return_home_text.contains("edges"),
            "Step 7 (Return to Home) failed at {}x{}",
            w,
            h
        );
    }
}
