//! Inc 19: a Thinking-role message persists and reloads inside a session blob.
use brain_core::repositories::SessionRepository;
use brain_domain::{
    Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp, SessionTitle,
};
use brain_storage::{SqliteStorage, TestStorage};

#[test]
fn thinking_message_survives_session_round_trip() {
    let test_store = TestStorage::new();
    let store: &SqliteStorage = test_store.storage();

    let sid = SessionId::new();
    let mut session = Session::new(
        sid.clone(),
        SessionTitle("Inc 19".to_string()),
        SessionTimestamp(1_756_200_000),
    );
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::User,
            "hi".to_string(),
        ))
        .unwrap();
    let envelope =
        r#"{"type":"thinking_block","v":1,"text":"checking the stream loop","duration_ms":800}"#
            .to_string();
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::Thinking,
            envelope.clone(),
        ))
        .unwrap();
    session
        .add_message(Message::new(
            MessageId::new(),
            MessageRole::Assistant,
            "done".to_string(),
        ))
        .unwrap();

    SessionRepository::save_session(store, &sid, &session).unwrap();

    let reloaded = SessionRepository::load_session(store, &sid)
        .unwrap()
        .expect("session must reload");
    assert_eq!(reloaded.messages.len(), 3);
    assert_eq!(reloaded.messages[1].role, MessageRole::Thinking);
    assert_eq!(reloaded.messages[1].content, envelope);
}
