use brain_tui::state::{UiState, Action};
use brain_tui::ui::interaction::timeline::{EventOrdinal, TimelineItem};
use brain_tui::ui::interaction::chat::MessageId as LocalMessageId;
use brain_tui::ui::theme::Theme;
use brain_tui::ui::renderer::AppRenderer;
use brain_domain::bkf::retrieval::{
    RetrievalId, RetrievalInfo, UserRetrievalExplanation, SemanticSimilarity,
    RetrievalWeight, ProvenanceReference, SourceKind
};
use brain_domain::{Message, MessageId, MessageRole};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::Rect;

#[test]
fn test_timeline_state_transitions_and_ordering() {
    let mut state = UiState::new();

    // 1. Send user prompt (resets timeline)
    state.update(Action::SubmitPrompt);
    assert_eq!(state.timeline.len(), 0);
    assert_eq!(state.next_ordinal, 1);

    // 2. Start stream
    state.update(Action::StartStream);
    assert_eq!(state.timeline.len(), 0);

    // 3. Retrieval Started
    state.update(Action::RetrievalStarted {
        message: LocalMessageId(0),
        query: "rust memory".to_string(),
    });
    assert_eq!(state.timeline.len(), 0); // Started itself doesn't append a result item

    // 4. Retrieval Received
    let prov = ProvenanceReference {
        kind: SourceKind::File,
        location: "src/lib.rs".to_string(),
        line_range: Some((1, 10)),
    };
    let explanation = UserRetrievalExplanation {
        matched_keywords: vec!["rust".to_string(), "memory".to_string()],
        semantic_similarity: SemanticSimilarity::High,
        recency_boost: true,
        weight: RetrievalWeight::Critical,
        provenance: prov,
    };
    let info = RetrievalInfo {
        id: RetrievalId(101),
        message_id: MessageId::new(),
        title: "Rust Ownership".to_string(),
        excerpt: "Rust enforces memory safety via compile-time borrow checking.".to_string(),
        explanation,
    };
    state.update(Action::RetrievalReceived {
        message: LocalMessageId(0),
        info: info.clone(),
    });

    assert_eq!(state.timeline.len(), 1);
    assert_eq!(state.timeline[0], (EventOrdinal(1), TimelineItem::Retrieval(RetrievalId(101))));

    // 5. Tool Call Requested
    state.update(Action::ToolCallRequested {
        message: LocalMessageId(0),
        call_id: brain_core::events::ToolCallId("tool_1".to_string()),
        tool_id: brain_core::events::ToolId("read_file".to_string()),
        arguments: "{}".to_string(),
        requires_approval: false,
    });

    assert_eq!(state.timeline.len(), 2);
    assert_eq!(state.timeline[1], (EventOrdinal(2), TimelineItem::ToolExecution(brain_core::events::ToolCallId("tool_1".to_string()))));

    // 6. Receive Token (first token adds the assistant message to the timeline)
    state.update(Action::ReceiveToken(brain_tui::state::RenderToken::Text("Hello".to_string())));
    assert_eq!(state.timeline.len(), 3);
    assert_eq!(state.timeline[2], (EventOrdinal(3), TimelineItem::Message(LocalMessageId(0))));

    // 7. Verify consecutive tokens do not duplicate message in timeline
    state.update(Action::ReceiveToken(brain_tui::state::RenderToken::Text(" world".to_string())));
    assert_eq!(state.timeline.len(), 3);

    // 8. Session Loaded (clears active timeline, populates historical message sequence)
    let messages = vec![
        Message::new(MessageId::new(), MessageRole::User, "Hello".to_string()),
        Message::new(MessageId::new(), MessageRole::Assistant, "Hi".to_string()),
    ];
    let session_id = brain_domain::SessionId::new();
    let request_id = brain_tui::state::LoadRequestId(42);
    state.pending_load = Some(brain_tui::state::PendingLoad { session_id, request_id });
    state.update(Action::SessionLoaded {
        session_id,
        request_id,
        messages,
    });

    assert_eq!(state.timeline.len(), 2);
    assert_eq!(state.timeline[0], (EventOrdinal(1), TimelineItem::Message(LocalMessageId(1))));
    assert_eq!(state.timeline[1], (EventOrdinal(2), TimelineItem::Message(LocalMessageId(2))));
}

#[test]
fn test_timeline_rendering_output() {
    let mut state = UiState::new();
    let theme = Theme::default();
    let renderer = AppRenderer::new();

    // Setup active state with a retrieval and a tool call in the timeline
    state.update(Action::StartStream);
    
    let prov = ProvenanceReference {
        kind: SourceKind::File,
        location: "src/lib.rs".to_string(),
        line_range: Some((1, 10)),
    };
    let explanation = UserRetrievalExplanation {
        matched_keywords: vec!["rust".to_string()],
        semantic_similarity: SemanticSimilarity::High,
        recency_boost: false,
        weight: RetrievalWeight::Critical,
        provenance: prov,
    };
    let info = RetrievalInfo {
        id: RetrievalId(101),
        message_id: MessageId::new(),
        title: "Rust Structs".to_string(),
        excerpt: "Structs define custom types in Rust.".to_string(),
        explanation,
    };
    state.update(Action::RetrievalReceived {
        message: LocalMessageId(0),
        info,
    });

    state.update(Action::ToolCallRequested {
        message: LocalMessageId(0),
        call_id: brain_core::events::ToolCallId("tool_1".to_string()),
        tool_id: brain_core::events::ToolId("read_file".to_string()),
        arguments: "{}".to_string(),
        requires_approval: false,
    });

    state.update(Action::ReceiveToken(brain_tui::state::RenderToken::Text("Generating...".to_string())));

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| {
        renderer.draw(f, Rect::new(0, 0, 80, 24), &state, &theme);
    }).unwrap();

    let buffer = terminal.backend().buffer();
    let buffer_str = format!("{:?}", buffer);

    // Verify all parts are rendered
    assert!(buffer_str.contains("🧠 Memory: Rust Structs"));
    assert!(buffer_str.contains("Structs define custom types"));
    assert!(buffer_str.contains("Source: File at src/lib.rs"));
    assert!(buffer_str.contains("🔧 Tool: read_file"));
}
