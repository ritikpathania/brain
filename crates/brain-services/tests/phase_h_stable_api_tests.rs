//! Integration test suite for Phase H Stable Application Interface & Handshake Protocol Negotiation.

use brain_services::protocol::{
    ExecuteGoalCommandDTO, HandshakeAckDTO, HandshakeHelloDTO, ProtocolError, ProtocolNegotiator,
    ProtocolVersion, QueryKnowledgeGraphDTO, StreamEventsSubscriptionDTO, SupportedRange,
    CURRENT_PROTOCOL_VERSION, MIN_SUPPORTED_PROTOCOL_VERSION,
};

#[test]
fn test_protocol_version_negotiation_success() {
    let client_ver = ProtocolVersion(1);
    let server_range = SupportedRange::default_range();

    let negotiated = ProtocolNegotiator::negotiate(client_ver, server_range).unwrap();
    assert_eq!(negotiated, ProtocolVersion(1));
}

#[test]
fn test_protocol_version_negotiation_unsupported_version_failure() {
    let unsupported_ver = ProtocolVersion(999);
    let server_range = SupportedRange::default_range();

    let err = ProtocolNegotiator::negotiate(unsupported_ver, server_range).unwrap_err();
    assert_eq!(
        err,
        ProtocolError::UnsupportedVersion {
            requested: 999,
            min: MIN_SUPPORTED_PROTOCOL_VERSION,
            max: CURRENT_PROTOCOL_VERSION,
        }
    );
}

use brain_services::protocol::ProtocolCapability;
use std::collections::HashSet;

#[test]
fn test_handshake_hello_ack_dto_serialization_roundtrip() {
    let mut requested_capabilities = HashSet::new();
    requested_capabilities.insert(ProtocolCapability::Replay);
    requested_capabilities.insert(ProtocolCapability::Streaming);

    let hello = HandshakeHelloDTO {
        client_version: ProtocolVersion(1),
        client_id: "client_test_01".to_string(),
        requested_capabilities: requested_capabilities.clone(),
    };

    let hello_json = serde_json::to_string(&hello).unwrap();
    let deserialized_hello: HandshakeHelloDTO = serde_json::from_str(&hello_json).unwrap();
    assert_eq!(deserialized_hello, hello);

    let ack = HandshakeAckDTO {
        negotiated_version: ProtocolVersion(1),
        server_range: SupportedRange::default_range(),
        accepted_capabilities: requested_capabilities,
    };

    let ack_json = serde_json::to_string(&ack).unwrap();
    let deserialized_ack: HandshakeAckDTO = serde_json::from_str(&ack_json).unwrap();
    assert_eq!(deserialized_ack, ack);
}

#[test]
fn test_frozen_command_query_event_dto_serialization() {
    let cmd = ExecuteGoalCommandDTO {
        goal_prompt: "Refactor storage layer".to_string(),
        workspace_id: "/tmp/workspace".to_string(),
        timeout_seconds: 60,
    };

    let cmd_json = serde_json::to_string(&cmd).unwrap();
    let deserialized_cmd: ExecuteGoalCommandDTO = serde_json::from_str(&cmd_json).unwrap();
    assert_eq!(deserialized_cmd, cmd);

    let query = QueryKnowledgeGraphDTO {
        query_text: "KnowledgeGraph".to_string(),
        limit: 10,
    };

    let query_json = serde_json::to_string(&query).unwrap();
    let deserialized_query: QueryKnowledgeGraphDTO = serde_json::from_str(&query_json).unwrap();
    assert_eq!(deserialized_query, query);

    let sub = StreamEventsSubscriptionDTO {
        plan_id: "plan_test_01".to_string(),
        start_sequence: Some(1),
    };

    let sub_json = serde_json::to_string(&sub).unwrap();
    let deserialized_sub: StreamEventsSubscriptionDTO = serde_json::from_str(&sub_json).unwrap();
    assert_eq!(deserialized_sub, sub);
}
