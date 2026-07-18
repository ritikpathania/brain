use brain_domain::{Message, MessageId, MessageRole};
use brain_tui::state::UiState;
use brain_tui::ui::command::tool::{
    ToolCallId, ToolExecution, ToolExecutionStatus, ToolId, ToolLogEntry,
};
use brain_tui::ui::interaction::navigation::ToolSection;
use brain_tui::ui::interaction::MessageId as LocalMessageId;
use brain_tui::ui::renderer::AppRenderer;
use brain_tui::ui::theme::Theme;
use ratatui::backend::TestBackend;
use ratatui::prelude::Rect;
use ratatui::Terminal;
use std::collections::{HashMap, HashSet};

#[test]
fn test_lazy_timeline_rendering() {
    let mut state = UiState::new();
    let theme = Theme::default();

    // Add a dummy active message so that the chat rendering else branch is entered
    state.active_messages.push(Message::new(
        MessageId::new(),
        MessageRole::User,
        "Hello".to_string(),
    ));

    state.active_response = "Thinking...".to_string();

    let tool_call = ToolExecution {
        message_id: LocalMessageId(1),
        call_id: ToolCallId("call_123".to_string()),
        tool_id: ToolId("web_search".to_string()),
        status: ToolExecutionStatus::Completed {
            result: "Success".to_string(),
        },
        logs: vec![
            ToolLogEntry {
                message: "Searching for rust...".to_string(),
                timestamp: std::time::SystemTime::now(),
            },
            ToolLogEntry {
                message: "Found 10 results".to_string(),
                timestamp: std::time::SystemTime::now(),
            },
        ],
        protocol_state: brain_tui::ui::command::tool::ProtocolState { last_sequence: 2 },
    };

    state.active_tool_calls.push(tool_call.clone());

    // Populate timeline items to match the new persistent timeline presentation model
    state.timeline.push((
        brain_tui::ui::interaction::timeline::EventOrdinal(1),
        brain_tui::ui::interaction::timeline::TimelineItem::Message(LocalMessageId(1)),
    ));
    state.timeline.push((
        brain_tui::ui::interaction::timeline::EventOrdinal(2),
        brain_tui::ui::interaction::timeline::TimelineItem::Message(LocalMessageId(0)),
    ));
    state.timeline.push((
        brain_tui::ui::interaction::timeline::EventOrdinal(3),
        brain_tui::ui::interaction::timeline::TimelineItem::ToolExecution(ToolCallId(
            "call_123".to_string(),
        )),
    ));

    // Scenario 1: Logs collapsed by default
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let renderer = AppRenderer::new();
    terminal
        .draw(|f| {
            renderer.draw(f, Rect::new(0, 0, 80, 24), &state, &theme);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);

    // Should render the collapsed logs message
    assert!(buffer_str.contains("Logs collapsed"));
    // Should NOT render the log message contents
    assert!(!buffer_str.contains("Searching for rust"));

    // Scenario 2: Logs expanded
    let mut expanded = HashMap::new();
    let mut sections = HashSet::new();
    sections.insert(ToolSection::Logs);
    expanded.insert(ToolCallId("call_123".to_string()), sections);
    state.conversation_view.expanded_tool_sections = expanded;

    terminal
        .draw(|f| {
            renderer.draw(f, Rect::new(0, 0, 80, 24), &state, &theme);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);
    // Should NOT contain the collapsed logs summary
    assert!(!buffer_str.contains("Logs collapsed"));
    // Should render the actual logs
    assert!(buffer_str.contains("Searching for rust"));
    assert!(buffer_str.contains("Found 10 results"));
}
