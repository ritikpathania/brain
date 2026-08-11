use brain_tui::state::UiState;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_workspace_dashboard_full_width_layout() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let state = UiState::default();
    let theme = Theme::default();

    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            brain_tui::ui::widgets::workspace_dashboard::draw(f, area, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let text = (0..20)
        .map(|y| (0..80).map(|x| buf.get(x, y).symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    // Positive Assertions
    assert!(text.contains("Claude Code v2.1.226"), "Must contain integrated version header");
    assert!(text.contains("awaiting input"), "Must contain awaiting input status");
    assert!(
        text.contains("Your conversation moved to the background"),
        "Must contain background banner text"
    );
    assert!(text.contains("Needs input"), "Must contain Needs input section");
    assert!(text.contains("Completed"), "Must contain Completed section");

    // Negative Assertions: 2-column sidebar split line must NOT exist
    for y in 0..20 {
        assert_ne!(
            buf.get(22, y).symbol(),
            "│",
            "Vertical sidebar divider must not exist at col 22 for y={}",
            y
        );
    }
    assert!(
        !text.contains("Sessions (Active)"),
        "Legacy Sessions (Active) text must not exist"
    );
}

#[test]
fn test_workspace_dashboard_interactivity_reducer() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    let theme = Theme::default();

    // Set screen to Workspace and push onto navigation stack
    state.screen = brain_tui::ui::navigation::Screen::Workspace;
    state.navigation.reset(brain_tui::ui::navigation::Screen::Home);
    state.navigation.push(brain_tui::ui::navigation::Screen::Workspace);

    // Initial render assertion: selected_session_idx is 0
    assert_eq!(state.selected_session_idx, 0);
    assert_eq!(state.screen, brain_tui::ui::navigation::Screen::Workspace);

    // 1. Dispatch Action::NavigateDown -> selected_session_idx changes to 1, row 1 background is Rgb(38, 79, 120)
    state.update(brain_tui::state::Action::NavigateDown);
    assert_eq!(state.selected_session_idx, 1);

    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            brain_tui::ui::widgets::workspace_dashboard::draw(f, area, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let mut found_selection_bg = false;
    for y in 0..20 {
        let line_text: String = (0..80).map(|x| buf.get(x, y).symbol()).collect();
        if line_text.contains("login required") || line_text.contains("/??/") {
            for x in 0..80 {
                if buf.get(x, y).style().bg == Some(ratatui::style::Color::Rgb(38, 79, 120)) {
                    found_selection_bg = true;
                    break;
                }
            }
        }
    }
    assert!(found_selection_bg, "Selected row 1 must have background Rgb(38, 79, 120)");

    // 2. Dispatch Action::NavigateUp -> selected_session_idx returns to 0
    state.update(brain_tui::state::Action::NavigateUp);
    assert_eq!(state.selected_session_idx, 0);

    // 3. Dispatch Action::SelectSession -> screen transitions to Screen::Conversation
    state.update(brain_tui::state::Action::SelectSession);
    assert_eq!(state.screen, brain_tui::ui::navigation::Screen::Conversation);

    // 4. Dispatch Action::Escape -> screen transitions back to previous foreground screen (Screen::Home)
    state.update(brain_tui::state::Action::Escape);
    assert_eq!(state.screen, brain_tui::ui::navigation::Screen::Home);

    // Test Escape when opened from Conversation
    state.screen = brain_tui::ui::navigation::Screen::Conversation;
    state.navigation.reset(brain_tui::ui::navigation::Screen::Home);
    state.navigation.push(brain_tui::ui::navigation::Screen::Conversation);
    state.update(brain_tui::state::Action::NavigateToWorkspace);
    assert_eq!(state.screen, brain_tui::ui::navigation::Screen::Workspace);
    assert_eq!(state.navigation.current(), brain_tui::ui::navigation::Screen::Workspace);

    state.update(brain_tui::state::Action::Escape);
    assert_eq!(state.screen, brain_tui::ui::navigation::Screen::Conversation);
}

