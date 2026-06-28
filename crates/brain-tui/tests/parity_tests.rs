use brain_tui::state::{UiState, Action, SessionLoadState, PendingLoad, GenerationState, RenderToken};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::renderer::AppRenderer;
use brain_domain::{SessionId, Message, MessageRole, MessageId};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn test_layout_partitions_verification() {
    let state = UiState::new();
    let theme = Theme::default();
    let renderer = AppRenderer::new();

    // 1. Standard Desktop Size (120x30)
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| {
        let area = f.size();
        let (h, sb, c, p, s) = renderer.compute_layout(area);
        assert_eq!(h.height, 3);
        assert_eq!(p.height, 3);
        assert_eq!(s.height, 1);
        assert_eq!(sb.width, 25);
        assert_eq!(c.height, 23);
        assert_eq!(sb.height, 23);
        
        renderer.draw(f, area, &state, &theme);
    }).unwrap();

    // 2. Compact View Width (70x30) - Sidebar should hide
    let backend = TestBackend::new(70, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| {
        let area = f.size();
        let (h, sb, c, p, s) = renderer.compute_layout(area);
        assert_eq!(h.height, 3);
        assert_eq!(p.height, 3);
        assert_eq!(s.height, 1);
        assert_eq!(sb.width, 0); // hidden
        assert_eq!(c.width, 70); // chat uses full width
        assert_eq!(c.height, 23);

        renderer.draw(f, area, &state, &theme);
    }).unwrap();
}

#[test]
fn test_scroll_limits_and_anchoring() {
    let mut state = UiState::new();
    
    // Add messages to fill screen height
    for i in 0..50 {
        let msg = Message::new(
            MessageId::new(),
            MessageRole::User,
            format!("Message line number {}", i),
        );
        state.active_messages.push(msg);
    }

    // Scroll offset should start at 0
    assert_eq!(state.viewport.scroll_offset, 0);
    assert!(state.viewport.follow_tail);
}

#[test]
fn test_session_switching_stress() {
    let mut state = UiState::new();
    
    // Switch between 100 sessions in rapid succession
    for i in 1..=100 {
        let session_id = SessionId::new();
        let req_id = brain_tui::state::LoadRequestId(i);
        
        state.update(Action::ActivateSession { session_id, request_id: req_id });
        
        assert_eq!(state.pending_load, Some(PendingLoad { session_id, request_id: req_id }));
        assert_eq!(state.session_load_state, SessionLoadState::Loading);
    }
}

#[test]
fn test_rapid_resize_stress() {
    let state = UiState::new();
    let theme = Theme::default();
    let renderer = AppRenderer::new();

    // Loop through 100 different sizes rapidly
    for i in 10..110 {
        let backend = TestBackend::new(i as u16, (i / 3 + 10) as u16);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| {
            let area = f.size();
            let _ = renderer.compute_layout(area);
            renderer.draw(f, area, &state, &theme);
        }).unwrap();
    }
}

#[test]
fn test_cancellation_spam_stress() {
    let mut state = UiState::new();

    for _ in 0..50 {
        // Start stream
        state.update(Action::StartStream);
        assert_eq!(state.generation_state, GenerationState::Starting);

        // Receive token
        state.update(Action::ReceiveToken(RenderToken::Text("partial chunk".to_string())));

        // Cancel immediately
        state.update(Action::CancelStream);
        assert_eq!(state.generation_state, GenerationState::Cancelled(None));
        assert!(state.typewriter.is_empty());
        assert_eq!(state.pending_load, None);
    }
}
