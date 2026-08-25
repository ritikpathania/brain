//! Inc 19: persisted thinking blocks must never enter model-bound context.
use brain_domain::{Message, MessageId, MessageRole};
use brain_services::conversation::{ContextBudget, ContextBuilder, WordSpaceTokenCounter};

#[test]
fn thinking_messages_are_excluded_from_context_window() {
    let counter = WordSpaceTokenCounter;
    let budget = ContextBudget {
        max_tokens: 100,
        reserved_system_tokens: 5,
        reserved_completion_tokens: 5,
    };
    let history = vec![
        Message::new(MessageId::new(), MessageRole::System, "sys".to_string()),
        Message::new(MessageId::new(), MessageRole::User, "question".to_string()),
        Message::new(
            MessageId::new(),
            MessageRole::Thinking,
            r#"{"type":"thinking_block","v":1,"text":"hidden","duration_ms":10}"#.to_string(),
        ),
        Message::new(MessageId::new(), MessageRole::Assistant, "answer".to_string()),
    ];

    let window = ContextBuilder::build(&counter, budget, &history, None, vec![]);
    let roles: Vec<MessageRole> = window.messages().iter().map(|m| m.role).collect();
    assert_eq!(
        roles,
        vec![MessageRole::System, MessageRole::User, MessageRole::Assistant],
        "Thinking envelopes must not reach generation input"
    );
}
