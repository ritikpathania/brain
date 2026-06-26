use brain_domain::SessionId;
use brain_events::*;

#[test]
fn test_command_envelope_serialization() {
    let cmd = Command::Session(SessionCommand::Close(SessionId::new()));
    let correlation_id = uuid::Uuid::new_v4();
    let env = CommandEnvelope::new_with_correlation(cmd, correlation_id);
    let serialized = serde_json::to_string(&env).unwrap();
    let deserialized: CommandEnvelope<Command> = serde_json::from_str(&serialized).unwrap();

    assert_eq!(env.id, deserialized.id);
    assert_eq!(env.correlation_id, deserialized.correlation_id);
    assert_eq!(correlation_id, deserialized.correlation_id);
    assert_eq!(env.version, deserialized.version);
    assert_eq!(env.timestamp_ms, deserialized.timestamp_ms);
    match deserialized.command {
        Command::Session(SessionCommand::Close(_)) => {}
        _ => panic!("Expected SessionCommand::Close variant"),
    }
}

#[test]
fn test_event_envelope_serialization() {
    let session_id = SessionId::new();
    let payload = DomainEvent::Session(SessionEvent::SessionCreated(session_id));
    let correlation_id = uuid::Uuid::new_v4();
    let env = EventEnvelope::new_with_correlation("daemon".to_string(), payload, correlation_id);
    let serialized = serde_json::to_string(&env).unwrap();
    let deserialized: EventEnvelope = serde_json::from_str(&serialized).unwrap();

    assert_eq!(env.event_id, deserialized.event_id);
    assert_eq!(env.correlation_id, deserialized.correlation_id);
    assert_eq!(correlation_id, deserialized.correlation_id);
    assert_eq!(env.version, deserialized.version);
    assert_eq!(env.source, deserialized.source);
    match deserialized.payload {
        DomainEvent::Session(SessionEvent::SessionCreated(id)) => {
            assert_eq!(session_id, id);
        }
        _ => panic!("Expected SessionEvent::SessionCreated variant"),
    }
}

#[test]
fn test_event_topic_serialization() {
    let topic = EventTopic::Storage;
    let serialized = serde_json::to_string(&topic).unwrap();
    assert_eq!(serialized, "\"storage\"");

    let deserialized: EventTopic = serde_json::from_str(&serialized).unwrap();
    assert_eq!(topic, deserialized);
}
