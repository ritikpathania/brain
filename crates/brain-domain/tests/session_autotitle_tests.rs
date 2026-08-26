//! B5: one-time default-title backfill from the first user message.

use brain_domain::{
    Message, MessageId, MessageRole, Session, SessionId, SessionTimestamp, SessionTitle,
};

fn fresh_session() -> Session {
    Session::new(SessionId::new(), SessionTitle::default(), SessionTimestamp(0))
}

fn push(session: &mut Session, role: MessageRole, content: &str) {
    session
        .messages
        .push(Message::new(MessageId::new(), role, content.to_string()));
}

#[test]
fn derives_from_first_user_message_when_default() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "Help me debug the login flow");
    s.autotitle();
    assert_eq!(
        s.title,
        SessionTitle("Help me debug the login flow".to_string())
    );
}

#[test]
fn leaves_non_default_titles_untouched() {
    let mut s = Session::new(
        SessionId::new(),
        SessionTitle("Custom".to_string()),
        SessionTimestamp(0),
    );
    push(&mut s, MessageRole::User, "Help me debug the login flow");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("Custom".to_string()));
}

#[test]
fn keeps_default_without_user_messages() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::Assistant, "An answer");
    s.autotitle();
    assert_eq!(s.title, SessionTitle::default());
}

#[test]
fn derives_from_user_even_after_assistant_messages() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::Assistant, "welcome");
    push(&mut s, MessageRole::User, "the real prompt");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("the real prompt".to_string()));
}

#[test]
fn multiline_takes_first_nonempty_line_collapsed() {
    let mut s = fresh_session();
    push(
        &mut s,
        MessageRole::User,
        "\n   \nFix the   login\tbug\nsecond line",
    );
    s.autotitle();
    assert_eq!(s.title, SessionTitle("Fix the login bug".to_string()));
}

#[test]
fn long_line_capped_at_43_with_ellipsis() {
    let mut s = fresh_session();
    let fifty = "abcdefghij".repeat(5); // 50 chars, single word
    push(&mut s, MessageRole::User, &fifty);
    s.autotitle();
    let t = s.title.0;
    let expected_head: String = fifty.chars().take(43).collect();
    assert_eq!(t.chars().count(), 44); // 43 + ellipsis
    assert_eq!(t, format!("{expected_head}…"));
}

#[test]
fn exactly_43_chars_stays_untruncated() {
    let mut s = fresh_session();
    let forty_three = "a".repeat(43);
    push(&mut s, MessageRole::User, &forty_three);
    s.autotitle();
    assert_eq!(s.title, SessionTitle(forty_three));
}

#[test]
fn bang_command_is_a_valid_source() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "! cargo test --workspace");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("! cargo test --workspace".to_string()));
}

#[test]
fn whitespace_only_prompt_keeps_default() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "   \n\t  ");
    s.autotitle();
    assert_eq!(s.title, SessionTitle::default());
}

#[test]
fn second_call_after_derivation_is_noop() {
    let mut s = fresh_session();
    push(&mut s, MessageRole::User, "first prompt");
    s.autotitle();
    push(&mut s, MessageRole::User, "second prompt");
    s.autotitle();
    assert_eq!(s.title, SessionTitle("first prompt".to_string()));
}
