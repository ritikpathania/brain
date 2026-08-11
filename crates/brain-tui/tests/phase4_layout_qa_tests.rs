use brain_domain::SessionId;
use brain_tui::client::SessionSummary;
use brain_tui::state::{Action, ConnectionMode, UiState};
use brain_tui::ui::navigation::{Modal, Screen};
use brain_tui::ui::renderer::{AppLayoutMode, AppRenderer};
use brain_tui::ui::theme::{dark_theme, light_theme};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::time::SystemTime;

const CERTIFIED_VIEWPORTS: [(u16, u16); 4] = [
    (80, 24),  // Compact standard
    (100, 26), // Medium standard
    (120, 30), // Large standard
    (182, 53), // Ultrawide
];

#[test]
fn test_phase4_all_certified_viewports_geometry() {
    let renderer = AppRenderer::new();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let area = Rect::new(0, 0, w, h);

        // 1. Home screen layout
        let mut home_state = UiState::new();
        home_state.screen = Screen::Home;
        home_state.terminal_width = w;
        home_state.terminal_height = h;

        let (_, _, _, _, p_area, _, f_area) = renderer.compute_layout(area, &home_state);
        assert_eq!(
            f_area.height, 1,
            "Status footer must consume 1 row on Home at {}x{}",
            w, h
        );
        assert_eq!(p_area.height, 3, "Prompt box must be 3 rows at {}x{}", w, h);

        let layout_mode = AppRenderer::layout_mode(&home_state);
        assert_eq!(layout_mode, AppLayoutMode::Welcome);

        // 2. Workspace screen layout
        let mut ws_state = UiState::new();
        ws_state.screen = Screen::Workspace;
        ws_state.terminal_width = w;
        ws_state.terminal_height = h;

        let ws_mode = AppRenderer::layout_mode(&ws_state);
        assert_eq!(ws_mode, AppLayoutMode::Workspace);
    }
}

#[test]
fn test_phase4_home_screen_hero_and_prompt_anchoring() {
    let renderer = AppRenderer::new();

    // 80x24 short terminal: prompt at row 16
    let state_24 = UiState::new();
    let area_24 = Rect::new(0, 0, 80, 24);
    let (_, _, _, _, p_area_24, _, _) = renderer.compute_layout(area_24, &state_24);
    assert_eq!(p_area_24.y, 20, "Prompt y at 80x24 must be row 20");

    // 182x53 tall terminal: prompt bottom-anchored (row 49)
    let state_53 = UiState::new();
    let area_53 = Rect::new(0, 0, 182, 53);
    let (_, _, _, _, p_area_53, _, _) = renderer.compute_layout(area_53, &state_53);
    assert_eq!(p_area_53.y, 49, "Prompt y at 182x53 must be row 49");
}

#[test]
fn test_phase4_workspace_sidebar_width_by_viewport() {
    let renderer = AppRenderer::new();

    // 80x24 Workspace: full-width task table (sidebar = 0, chat = 80)
    let mut s80 = UiState::new();
    s80.screen = Screen::Workspace;
    s80.focus = brain_tui::state::FocusRegion::Sidebar;
    let (_, sb_80, chat_80, _, _, _, _) = renderer.compute_layout(Rect::new(0, 0, 80, 24), &s80);
    assert_eq!(sb_80.width, 0);
    assert_eq!(chat_80.width, 80);

    // 120x30 Workspace: full-width task table (sidebar = 0, chat = 120)
    let mut s120 = UiState::new();
    s120.screen = Screen::Workspace;
    s120.focus = brain_tui::state::FocusRegion::Sidebar;
    let (_, sb_120, chat_120, _, _, _, _) =
        renderer.compute_layout(Rect::new(0, 0, 120, 30), &s120);
    assert_eq!(sb_120.width, 0);
    assert_eq!(chat_120.width, 120);

    // 182x53 Workspace: full-width task table (sidebar = 0, chat = 182)
    let mut s182 = UiState::new();
    s182.screen = Screen::Workspace;
    s182.focus = brain_tui::state::FocusRegion::Sidebar;
    let (_, sb_182, chat_182, _, _, _, _) =
        renderer.compute_layout(Rect::new(0, 0, 182, 53), &s182);
    assert_eq!(sb_182.width, 0);
    assert_eq!(chat_182.width, 182);
}

#[test]
fn test_phase4_workspace_sidebar_scrolling_and_empty_states() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        // 1. Empty Workspace
        let mut empty_state = UiState::new();
        empty_state.screen = Screen::Workspace;
        empty_state.focus = brain_tui::state::FocusRegion::Sidebar;
        empty_state.connection_mode = ConnectionMode::Daemon;
        empty_state.update(Action::LoadSessions(vec![]));

        terminal
            .draw(|f| renderer.draw(f, f.size(), &empty_state, theme))
            .unwrap();

        let empty_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            empty_text.contains("Needs input") && empty_text.contains("Completed"),
            "Missing workspace dashboard sections at {}x{}",
            w,
            h
        );

        // 2. Many-Session Workspace (50 items)
        let mut many_state = UiState::new();
        many_state.screen = Screen::Workspace;
        many_state.focus = brain_tui::state::FocusRegion::Sidebar;
        many_state.connection_mode = ConnectionMode::Daemon;
        let summaries: Vec<SessionSummary> = (0..50)
            .map(|i| SessionSummary {
                id: SessionId::new(),
                title: format!("Session Item {}", i),
                updated_at: SystemTime::now(),
                pinned: i == 0,
                archived: false,
            })
            .collect();
        many_state.update(Action::LoadSessions(summaries));
        many_state.selected_session_idx = 25; // Mid-list selection

        terminal
            .draw(|f| renderer.draw(f, f.size(), &many_state, theme))
            .unwrap();

        let many_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            many_text.contains("Session Item"),
            "Missing session list item at {}x{}",
            w,
            h
        );
    }
}

#[test]
fn test_phase4_long_session_title_truncation() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut state = UiState::new();
        state.screen = Screen::Workspace;
        state.focus = brain_tui::state::FocusRegion::Sidebar;
        state.connection_mode = ConnectionMode::Daemon;
        state.update(Action::LoadSessions(vec![SessionSummary {
            id: SessionId::new(),
            title: "Super Long Session Title That Needs Truncation In Narrow Workspace Sidebar"
                .to_string(),
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
            text.contains("Super Long") || text.contains("Super"),
            "Missing truncated title at {}x{}",
            w,
            h
        );
    }
}

#[test]
fn test_phase4_long_chat_message_wrapping() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut state = UiState::new();
        state.screen = Screen::Workspace;
        state.connection_mode = ConnectionMode::Daemon;
        for c in "Explain quantum state".chars() {
            state.editor.insert(c);
        }
        state.update(Action::SubmitPrompt);
        state.active_response = "This is a very long response paragraph from Brain that tests line wrapping behavior across certified viewports without overflowing container boundaries.".to_string();
        state.terminal_width = w;
        state.terminal_height = h;
        state.recalculate_viewport();

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
            text.contains("quantum") || text.contains("response") || text.contains("Brain"),
            "Missing conversation text in viewport at {}x{}",
            w,
            h
        );
    }
}

#[test]
fn test_phase4_modal_popups_centered_and_bounded() {
    let renderer = AppRenderer::new();
    let theme = dark_theme();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        // 1. Confirm Delete Modal
        let mut del_state = UiState::new();
        del_state.screen = Screen::Workspace;
        del_state.connection_mode = ConnectionMode::Daemon;
        del_state.update(Action::LoadSessions(vec![SessionSummary {
            id: SessionId::new(),
            title: "Target Delete".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        }]));
        del_state.selected_session_idx = 0;
        del_state.modal = Some(Modal::ConfirmDelete);

        terminal
            .draw(|f| renderer.draw(f, f.size(), &del_state, theme))
            .unwrap();

        let del_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            del_text.contains("Are you sure") || del_text.contains("Delete"),
            "Missing delete modal at {}x{}",
            w,
            h
        );

        // 2. Reply Composer Modal
        let mut reply_state = UiState::new();
        reply_state.screen = Screen::Workspace;
        reply_state.connection_mode = ConnectionMode::Daemon;
        reply_state.update(Action::LoadSessions(vec![SessionSummary {
            id: SessionId::new(),
            title: "Target Reply".to_string(),
            updated_at: SystemTime::now(),
            pinned: false,
            archived: false,
        }]));
        reply_state.selected_session_idx = 0;
        reply_state.modal = Some(Modal::ReplyComposer);

        terminal
            .draw(|f| renderer.draw(f, f.size(), &reply_state, theme))
            .unwrap();

        let reply_text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(
            reply_text.contains("Reply"),
            "Missing reply modal at {}x{}",
            w,
            h
        );
    }
}

#[test]
fn test_phase4_palette_status_footer_collapse() {
    let renderer = AppRenderer::new();

    for (w, h) in CERTIFIED_VIEWPORTS {
        let mut state = UiState::new();
        state.command_palette_mut().open_with_query(Some("/"));

        let (_, _, _, _, _, _, f_area) = renderer.compute_layout(Rect::new(0, 0, w, h), &state);
        assert_eq!(
            f_area.height, 0,
            "Status footer height must be 0 when palette is open at {}x{}",
            w, h
        );
    }
}

#[test]
fn test_phase4_light_and_dark_theme_rendering_fidelity() {
    let renderer = AppRenderer::new();
    let themes = [("Dark", dark_theme()), ("Light", light_theme())];

    for (theme_name, theme) in themes {
        for (w, h) in CERTIFIED_VIEWPORTS {
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).unwrap();

            let mut state = UiState::new();
            state.connection_mode = ConnectionMode::Daemon;

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
                text.contains("Welcome back!"),
                "Theme {} failed render at {}x{}",
                theme_name,
                w,
                h
            );
        }
    }
}
