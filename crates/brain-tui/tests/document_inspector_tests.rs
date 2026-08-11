//! Unit tests for Milestone A3 Document Inspector modal and navigation stack deep stack.

use brain_tui::ui::navigation::modal::Modal;
use brain_tui::ui::navigation::screen::Screen;
use brain_tui::ui::navigation::stack::NavigationStack;
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::document_inspector::{DocumentInspectorModal, DocumentInspectorState};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_document_inspector_modal_rendering() {
    let state = DocumentInspectorState {
        document_id: "doc-123".to_string(),
        source_path: "crates/brain-domain/src/retrieval/canonical.rs".to_string(),
        content: vec![
            "pub struct CanonicalRetrievalResult {".to_string(),
            "    pub query_id: QueryId,".to_string(),
            "    pub answer: String,".to_string(),
            "}".to_string(),
        ],
        line_range: Some((1, 3)),
        scroll_offset: 0,
    };

    let theme = dark_theme();
    let backend = TestBackend::new(80, 8);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let modal = DocumentInspectorModal { state: &state };
            modal.render(Rect::new(0, 0, 80, 8), f.buffer_mut(), theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("Document Inspector"));
    assert!(buffer_str.contains("crates/brain-domain/src/retrieval/canonical.rs"));
    assert!(buffer_str.contains("CanonicalRetrievalResult"));
}

#[test]
fn test_deep_navigation_stack_integration() {
    let mut stack = NavigationStack::new(Screen::Home);

    // 1. Workspace
    stack.push(Screen::Workspace);
    assert_eq!(stack.current(), Screen::Workspace);

    // 2. Open Inspector Modal
    let modal = Modal::DocumentInspector;
    assert_eq!(modal.title(), "Document Inspector");

    // 3. Open Graph Explorer Screen from Inspector
    stack.push(Screen::GraphExplorer);
    assert_eq!(stack.current(), Screen::GraphExplorer);

    // 4. Return to Workspace via Esc
    assert_eq!(stack.pop(), Some(Screen::GraphExplorer));
    assert_eq!(stack.current(), Screen::Workspace);
}
