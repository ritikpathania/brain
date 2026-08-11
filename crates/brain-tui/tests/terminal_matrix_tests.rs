use brain_domain::{NodeId, NodeKind};
use brain_tui::state::{Action, FocusRegion, PinnedNode, UiState};
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use uuid::Uuid;

#[test]
fn test_matrix_viewport_width_invariants_no_panic() {
    let widths = [40, 60, 80, 120];
    let renderer = AppRenderer::new();
    let theme = Theme::default();

    for width in widths {
        let backend = TestBackend::new(width, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut state = UiState::new();
        state.terminal_width = width;
        state.recalculate_viewport();

        // Must draw cleanly without buffer overflow or panic
        let res = terminal.draw(|f| {
            renderer.draw(f, f.size(), &state, &theme);
        });
        assert!(res.is_ok(), "Failed render at width {}", width);
    }
}

#[test]
fn test_matrix_narrow_viewport_sidebar_collapse() {
    let mut state = UiState::new();
    state.terminal_width = 50; // Ultra-narrow (<80 cols)
    state.recalculate_viewport();

    state.update(Action::ToggleFocus);
    assert_eq!(
        state.focus,
        FocusRegion::Editor,
        "Sidebar should not be focusable in ultra-narrow mode"
    );
}

#[test]
fn test_matrix_keyboard_modifier_routing() {
    let mut state = UiState::new();
    state.pinned_nodes.push(PinnedNode {
        node_id: NodeId(Uuid::new_v4()),
        label: "Alpha".to_string(),
        node_type: NodeKind::Concept,
        pinned_at: 0,
    });
    assert!(!state.submit_with_workspace);

    // Toggle Workspace Submit mode (Alt+W action)
    state.update(Action::ToggleSubmitWithWorkspace);
    assert!(state.submit_with_workspace);

    // Reset flag after submit
    state.update(Action::ResetSubmitWithWorkspace);
    assert!(!state.submit_with_workspace);
}

#[test]
fn test_matrix_visual_qa_coverage() {
    let dimensions = [(80, 24), (100, 26), (120, 30), (182, 53)];
    let themes = [
        ("dark", brain_tui::ui::theme::dark_theme()),
        ("light", brain_tui::ui::theme::light_theme()),
    ];
    let renderer = AppRenderer::new();

    for (w, h) in dimensions {
        for (theme_name, theme) in themes {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();

            // 1. Home / Welcome Screen
            let mut home_state = UiState::new();
            home_state.terminal_width = w;
            home_state.terminal_height = h;
            home_state.recalculate_viewport();

            let res_home = terminal.draw(|f| {
                renderer.draw(f, f.size(), &home_state, theme);
            });
            assert!(
                res_home.is_ok(),
                "Failed Home render at {}x{} in {} theme",
                w,
                h,
                theme_name
            );

            // 2. Workspace Screen (with active session and messages)
            let mut ws_state = UiState::new();
            ws_state.screen = brain_tui::ui::navigation::Screen::Workspace;
            ws_state.terminal_width = w;
            ws_state.terminal_height = h;
            ws_state.active_messages.push(brain_domain::Message::new(
                brain_domain::MessageId::new(),
                brain_domain::MessageRole::User,
                "Test query".to_string(),
            ));
            let res_ws = terminal.draw(|f| {
                renderer.draw(f, f.size(), &ws_state, theme);
            });
            assert!(
                res_ws.is_ok(),
                "Failed Workspace render at {}x{} in {} theme",
                w,
                h,
                theme_name
            );
        }
    }
}

#[test]
fn test_home_landing_page_composition_invariants() {
    let dimensions = [(80, 24), (100, 26), (120, 30), (182, 53)];
    let theme = Theme::default();
    let renderer = AppRenderer::new();

    for (w, h) in dimensions {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut home_state = UiState::new();
        home_state.connection_mode = brain_tui::state::ConnectionMode::Daemon;
        home_state.terminal_width = w;
        home_state.terminal_height = h;
        home_state.recalculate_viewport();

        terminal
            .draw(|f| {
                renderer.draw(f, f.size(), &home_state, &theme);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let rendered_text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        // 1. Must contain Core product identity elements
        assert!(
            rendered_text.contains("Welcome back!"),
            "Missing Welcome back! header at {}x{}",
            w,
            h
        );
        assert!(
            !rendered_text.contains("Ready"),
            "Home hero must NOT contain Ready indicator at {}x{}",
            w,
            h
        );
        assert!(
            rendered_text.contains("Think once. Remember."),
            "Missing tagline at {}x{}",
            w,
            h
        );

        // 2. Negative assertions: System Status must NOT exist
        assert!(
            !rendered_text.contains("System Status"),
            "System Status must NOT exist on Home at {}x{}",
            w,
            h
        );
    }
}
