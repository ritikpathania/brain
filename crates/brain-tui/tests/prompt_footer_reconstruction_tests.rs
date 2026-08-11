use brain_tui::state::UiState;
use brain_tui::ui::navigation::Screen;
use brain_tui::ui::status_footer::StatusFooterWidget;
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::prompt::{self, PromptView};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_prompt_glyph_and_quiet_footer_content() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = UiState::default();
    let theme = Theme::default();

    // 1. Home mode rendering
    state.screen = Screen::Home;
    let prompt_view = PromptView {
        prompt_text: "".to_string(),
        cursor_position: 0,
        has_focus: true,
        submit_with_workspace: false,
        is_welcome: true,
    };

    terminal
        .draw(|f| {
            let prompt_area = Rect::new(0, 20, 80, 3);
            let footer_area = Rect::new(0, 23, 80, 1);
            prompt::draw(f, prompt_area, &prompt_view, &theme);
            StatusFooterWidget::draw(f, footer_area, &state, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();

    // Assert Prompt Prefix Glyph is "❯ " (starts with "❯ ")
    let row21 = (0..80).map(|x| buf.get(x, 21).symbol()).collect::<String>();
    assert!(
        row21.starts_with("❯ "),
        "Prompt prefix must start with '❯ ', got: '{}'",
        row21
    );

    // Assert Home quiet footer content
    let row23_home = (0..80).map(|x| buf.get(x, 23).symbol()).collect::<String>();
    assert!(
        row23_home.contains("manual mode on"),
        "Home footer must contain 'manual mode on', got: '{}'",
        row23_home
    );
    assert!(
        row23_home.contains("? for shortcuts"),
        "Home footer must contain '? for shortcuts', got: '{}'",
        row23_home
    );
    assert!(
        !row23_home.contains("Daemon: Connected | Latency:"),
        "Raw telemetry must NOT exist in status footer, got: '{}'",
        row23_home
    );

    // 2. Workspace mode rendering
    state.screen = Screen::Workspace;
    terminal
        .draw(|f| {
            let footer_area = Rect::new(0, 23, 80, 1);
            StatusFooterWidget::draw(f, footer_area, &state, &theme);
        })
        .unwrap();

    let buf_ws = terminal.backend().buffer();
    let row23_ws = (0..80).map(|x| buf_ws.get(x, 23).symbol()).collect::<String>();
    assert!(
        row23_ws.contains("enter to return"),
        "Workspace footer must contain 'enter to return', got: '{}'",
        row23_ws
    );
    assert!(
        row23_ws.contains("ctrl+x to delete"),
        "Workspace footer must contain 'ctrl+x to delete', got: '{}'",
        row23_ws
    );
    assert!(
        !row23_ws.contains("Daemon: Connected | Latency:"),
        "Raw telemetry must NOT exist in status footer, got: '{}'",
        row23_ws
    );
}

#[test]
fn test_prompt_glyph_submit_with_workspace_is_always_chevron() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let theme = Theme::default();

    let prompt_view = PromptView {
        prompt_text: "".to_string(),
        cursor_position: 0,
        has_focus: true,
        submit_with_workspace: true, // even when submit_with_workspace is true!
        is_welcome: false,
    };

    terminal
        .draw(|f| {
            let prompt_area = Rect::new(0, 20, 80, 3);
            prompt::draw(f, prompt_area, &prompt_view, &theme);
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let row21 = (0..80).map(|x| buf.get(x, 21).symbol()).collect::<String>();
    assert!(
        row21.starts_with("❯ "),
        "Prompt prefix must start with '❯ ' even with submit_with_workspace=true, got: '{}'",
        row21
    );
}
