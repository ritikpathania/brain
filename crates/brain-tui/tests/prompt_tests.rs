use ratatui::backend::TestBackend;
use ratatui::Terminal;
use ratatui::layout::Rect;
use brain_tui::ui::widgets::prompt::{self, PromptView};
use brain_tui::ui::theme::dark_theme;

#[test]
fn test_prompt_minimized_rendering_no_panic() {
    let theme = dark_theme();
    
    // Test zero dimensions (0 width, 0 height)
    let backend = TestBackend::new(0, 0);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| {
        let area = Rect::new(0, 0, 0, 0);
        let view = PromptView {
            prompt_text: "hello".to_string(),
            cursor_position: 2,
            has_focus: true,
            submit_with_workspace: false,
        };
        prompt::draw(f, area, &view, &theme);
    }).unwrap();

    // Test width = 1 (triggers subtraction boundary)
    let backend_small_w = TestBackend::new(1, 2);
    let mut terminal_small_w = Terminal::new(backend_small_w).unwrap();
    terminal_small_w.draw(|f| {
        let area = Rect::new(0, 0, 1, 2);
        let view = PromptView {
            prompt_text: "hello".to_string(),
            cursor_position: 2,
            has_focus: true,
            submit_with_workspace: false,
        };
        prompt::draw(f, area, &view, &theme);
    }).unwrap();

    // Test normal rendering
    let backend_normal = TestBackend::new(30, 3);
    let mut terminal_normal = Terminal::new(backend_normal).unwrap();
    terminal_normal.draw(|f| {
        let area = Rect::new(0, 0, 30, 3);
        let view = PromptView {
            prompt_text: "hello".to_string(),
            cursor_position: 2,
            has_focus: true,
            submit_with_workspace: false,
        };
        prompt::draw(f, area, &view, &theme);
    }).unwrap();
}
