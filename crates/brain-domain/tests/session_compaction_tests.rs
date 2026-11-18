use brain_domain::{
    DomainEvent, Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp,
    SessionTitle,
};

#[test]
fn test_session_compaction_preserves_recent_messages_and_emits_event() {
    let session_id = SessionId::new();
    let title = SessionTitle("Test Compaction".to_string());
    let mut session = Session::new(session_id, title, SessionTimestamp(1000));
    // Clear SessionCreated event
    session.staged_events.clear();

    // Add 6 messages
    for i in 1..=6 {
        session
            .add_message(Message::new(
                MessageId::new(),
                if i % 2 == 1 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                format!("Message {}", i),
            ))
            .unwrap();
    }
    assert_eq!(session.messages.len(), 6);
    session.staged_events.clear();

    // Compact keeping last 2 messages
    let summarized = session
        .compact(
            "Summary of earlier conversation".to_string(),
            SessionTimestamp(2000),
            2,
        )
        .unwrap();

    assert_eq!(summarized, 4);
    // Should have 1 summary message + 2 retained messages = 3 messages
    assert_eq!(session.messages.len(), 3);
    assert_eq!(session.messages[0].role, MessageRole::System);
    assert!(session.messages[0]
        .content
        .contains("Summary of earlier conversation"));
    assert_eq!(session.messages[1].content, "Message 5");
    assert_eq!(session.messages[2].content, "Message 6");

    // Verify staged DomainEvent::SessionCompacted
    assert_eq!(session.staged_events.len(), 1);
    match &session.staged_events[0] {
        DomainEvent::SessionCompacted {
            session_id: sid,
            messages_summarized,
            retained_messages,
            compacted_at,
        } => {
            assert_eq!(*sid, session_id);
            assert_eq!(*messages_summarized, 4);
            assert_eq!(*retained_messages, 3);
            assert_eq!(*compacted_at, SessionTimestamp(2000));
        }
        other => panic!("Unexpected event: {:?}", other),
    }
}

#[test]
fn test_session_compaction_noop_when_messages_less_than_keep_last_n() {
    let mut session = Session::new(
        SessionId::new(),
        SessionTitle("Small".to_string()),
        SessionTimestamp(100),
    );
    session.staged_events.clear();
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::User,
            "Hello".to_string(),
        ))
        .unwrap();

    let summarized = session
        .compact("Summary".to_string(), SessionTimestamp(200), 5)
        .unwrap();

    assert_eq!(summarized, 0);
    assert_eq!(session.messages.len(), 1);
}
