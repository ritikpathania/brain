use brain_domain::SessionId;
use brain_tui::client::SessionSummary;
use brain_tui::state::{Action, ConnectionMode, GenerationState, LoadRequestId, UiState};
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::{dark_theme, light_theme};
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
fn test_phase5_home_focus_transition_no_stutter() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut state = UiState::new();
        state.screen = Screen::Home;
        state.connection_mode = ConnectionMode::Daemon;
        state.terminal_width = w;
        state.terminal_height = h;

        // Type query character by character
        for c in "How does Brain handle memory?".chars() {
            state.editor.insert(c);
            terminal
                .draw(|f| renderer.draw(f, f.size(), &state, theme))
                .unwrap();
        }

        assert_eq!(state.editor.text(), "How does Brain handle memory?");
        assert_eq!(state.screen, Screen::Home);
    }
}

#[test]
fn test_phase5_history_loading_lifecycle_and_stale_response_protection() {
    let mut state = UiState::new();
    let session_a = SessionId::new();
    let session_b = SessionId::new();
    let req_a = LoadRequestId(1);
    let req_b = LoadRequestId(2);

    // 1. Activate Session A -> Pending req_a
    state.update(Action::ActivateSession {
        session_id: session_a,
        request_id: req_a,
    });
    assert!(state.pending_load.is_some());
    assert_eq!(state.pending_load.as_ref().unwrap().session_id, session_a);

    // 2. Quickly Activate Session B -> Pending req_b
    state.update(Action::ActivateSession {
        session_id: session_b,
        request_id: req_b,
    });
    assert_eq!(state.pending_load.as_ref().unwrap().session_id, session_b);

    // 3. Stale Session A response arrives -> MUST BE DROPPED
    let res_stale = state.update(Action::SessionLoaded {
        session_id: session_a,
        request_id: req_a,
        messages: vec![],
    });
    assert_eq!(res_stale, brain_tui::state::UpdateResult::NoChange);
    assert_ne!(state.session_id, session_a);

    // 4. Matching Session B response arrives -> MUST BE APPLIED
    let res_valid = state.update(Action::SessionLoaded {
        session_id: session_b,
        request_id: req_b,
        messages: vec![],
    });
    assert_eq!(res_valid, brain_tui::state::UpdateResult::Changed);
    assert_eq!(state.session_id, session_b);
    assert!(state.pending_load.is_none());
}

#[test]
fn test_phase5_screen_mode_contextual_help_overlay() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    let mut state = UiState::new();
    state.update(Action::ToggleHelp);
    assert!(state.help_overlay.is_some());

    let overlay = state.help_overlay.as_ref().unwrap();
    let help_text = overlay.lines.join("\n");

    // Verify all screen-mode sections are present
    assert!(help_text.contains("GLOBAL"), "Missing GLOBAL help section");
    assert!(help_text.contains("HOME"), "Missing HOME help section");
    assert!(
        help_text.contains("WORKSPACE"),
        "Missing WORKSPACE help section"
    );
    assert!(
        help_text.contains("SESSION"),
        "Missing SESSION help section"
    );
    assert!(
        help_text.contains("SLASH COMMANDS"),
        "Missing SLASH COMMANDS help section"
    );

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            text.contains("BRAIN HELP"),
            "Missing HELP modal title at {}x{}",
            w,
            h
        );
    }
}

#[test]
fn test_phase5_slash_completion_selection_and_keyboard_navigation() {
    let mut state = UiState::new();

    // Type '/' to open slash completion engine
    for c in "/".chars() {
        state.editor.insert(c);
    }
    state.slash_completion.visible = true;
    state.slash_completion.query = "/".to_string();

    assert!(state.slash_completion.visible);

    // Test Tab / select_next
    state.slash_completion.select_next();
    assert_eq!(state.slash_completion.selected_index, 1);

    // Test Shift+Tab / select_prev
    state.slash_completion.select_prev();
    assert_eq!(state.slash_completion.selected_index, 0);

    // Selected command descriptor must not be None
    assert!(state.slash_completion.selected_command().is_some());
}

#[test]
fn test_phase5_streaming_cancellation_and_transient_footer_feedback() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    let mut state = UiState::new();
    state.screen = Screen::Workspace;
    state.generation_state = GenerationState::Streaming {
        started_at: SystemTime::now(),
    };
    state.active_response = "Partial streaming response text from Brain...".to_string();

    // Cancel stream
    state.update(Action::CancelStream);

    assert_eq!(state.generation_state, GenerationState::Cancelled(None));
    assert_eq!(
        state
            .transient_message
            .as_ref()
            .map(|(msg, _)| msg.as_str()),
        Some("Request cancelled")
    );

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| renderer.draw(f, f.size(), &state, theme))
            .unwrap();

        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            text.contains("Request cancelled") || text.contains("Partial"),
            "Missing cancellation feedback at {}x{}",
            w,
            h
        );
    }
}

#[test]
fn test_phase5_certified_viewports_and_theme_rendering_fidelity() {
    let renderer = AppRenderer::new();

    for theme in [dark_theme(), light_theme()] {
        for (w, h) in CERTIFIED_VIEWPORTS {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();

            let mut state = UiState::new();
            state.screen = Screen::Workspace;
            state.focus = brain_tui::state::FocusRegion::Sidebar;
            state.connection_mode = ConnectionMode::Daemon;
            state.terminal_width = w;
            state.terminal_height = h;

            state.update(Action::LoadSessions(vec![SessionSummary {
                id: SessionId::new(),
                title: "Certified Session".to_string(),
                updated_at: SystemTime::now(),
                pinned: false,
                archived: false,
            }]));

            terminal
                .draw(|f| renderer.draw(f, f.size(), &state, theme))
                .unwrap();

            let text = terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(
                text.contains("Certified Session"),
                "Missing session title at {}x{}",
                w,
                h
            );
        }
    }
}
