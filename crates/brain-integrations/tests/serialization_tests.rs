use brain_domain::{AdapterId, ClientId, ConversationId, EventId, SessionId, WorkspaceId};
use brain_integrations::{Capability, EventIdentity, IngestionEnvelope, IngestionEvent, Value};
use std::collections::BTreeMap;
use std::path::Path;

fn get_schema_validator() -> jsonschema::JSONSchema {
    let schema_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocol/brain_events.schema.json");
    let schema_content = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("Failed to read schema file at {:?}: {}", schema_path, e));
    let schema_json: serde_json::Value =
        serde_json::from_str(&schema_content).expect("Failed to parse schema JSON");
    jsonschema::JSONSchema::compile(&schema_json).expect("Failed to compile schema")
}

fn create_identity() -> EventIdentity {
    EventIdentity {
        event_id: EventId::new(),
        parent_event_id: Some(EventId::new()),
        workspace_id: WorkspaceId::new("proj-123"),
        client_id: ClientId::new("cursor-1.0.0"),
        adapter_id: AdapterId::new("brain-vscode-ext"),
        session_id: SessionId::new(),
        conversation_id: Some(ConversationId::new()),
        timestamp: chrono::Utc::now(),
    }
}

fn assert_conformance(envelope: &IngestionEnvelope) {
    // 1. Serialization / Deserialization Round-trip using canonical JSON serializer
    let json_str =
        brain_integrations::to_canonical_json(envelope).expect("Failed to serialize envelope");
    let roundtripped: IngestionEnvelope =
        serde_json::from_str(&json_str).expect("Failed to deserialize envelope");
    assert_eq!(envelope, &roundtripped);

    // 2. Validate against schema
    let json_val: serde_json::Value =
        serde_json::from_str(&json_str).expect("Failed to parse as Value");
    let validator = get_schema_validator();

    let validation_result = validator.validate(&json_val);
    if let Err(errors) = validation_result {
        let error_msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
        panic!(
            "JSON Schema validation failed for variant {:?}.\nErrors:\n{}",
            envelope.event.kind(),
            error_msgs.join("\n")
        );
    }
}

#[test]
fn test_message_event() {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "provider.model".to_string(),
        Value::String("claude-3-5".to_string()),
    );

    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: "Hello Brain".to_string(),
            metadata,
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_tool_call_event() {
    let mut args = BTreeMap::new();
    args.insert("path".to_string(), Value::String("src/lib.rs".to_string()));
    args.insert("line".to_string(), Value::Number(42.0));

    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::ToolCall {
            call_id: "call-99".to_string(),
            tool_name: "edit_file".to_string(),
            arguments: Value::Object(args),
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_tool_result_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::ToolResult {
            call_id: "call-99".to_string(),
            output: "File updated".to_string(),
            is_error: false,
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_file_edit_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::FileEdit {
            path: "src/main.rs".to_string(),
            diff: Some("--- a/src/main.rs\n+++ b/src/main.rs".to_string()),
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_diagnostic_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::Diagnostic {
            severity: "error".to_string(),
            message: "Unresolved import".to_string(),
            source: "rustc".to_string(),
            file: Some("src/main.rs".to_string()),
            line: Some(12),
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_terminal_command_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::TerminalCommand {
            command: "cargo check".to_string(),
            exit_code: Some(0),
            stdout_summary: Some("42 passed".to_string()),
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_git_commit_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::GitCommit {
            hash: "abc123def456".to_string(),
            message: "feat: add schema".to_string(),
            branch: Some("main".to_string()),
            files_changed: vec!["protocol/brain_events.schema.json".to_string()],
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_git_branch_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::GitBranch {
            action: "switch".to_string(),
            branch_name: "feature/schema".to_string(),
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_session_started_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::SessionStarted {
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_session_ended_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::SessionEnded {
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_adapter_connected_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::AdapterConnected {
            adapter_version: "1.2.3".to_string(),
            supported_event_model_versions: vec!["1.0".to_string()],
            supported_serializations: vec!["json".to_string()],
            capabilities: vec![Capability::ConversationMessages, Capability::Replay],
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_adapter_disconnected_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::AdapterDisconnected {
            reason: Some("normal exit".to_string()),
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_text_event() {
    let envelope = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity(),
        event: IngestionEvent::Text {
            content: "arbitrary notes".to_string(),
            metadata: BTreeMap::new(),
        },
    };
    assert_conformance(&envelope);
}

#[test]
fn test_golden_files_compatibility() {
    let golden_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");

    // Create test objects corresponding to each file
    let mut message_metadata = BTreeMap::new();
    message_metadata.insert(
        "provider.model".to_string(),
        Value::String("claude-3-5".to_string()),
    );
    let message_env = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity_fixed(),
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: "Hello Brain".to_string(),
            metadata: message_metadata,
        },
    };

    let mut tool_args = BTreeMap::new();
    tool_args.insert("path".to_string(), Value::String("src/lib.rs".to_string()));
    tool_args.insert("line".to_string(), Value::Number(42.0));
    let tool_call_env = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity_fixed(),
        event: IngestionEvent::ToolCall {
            call_id: "call-99".to_string(),
            tool_name: "edit_file".to_string(),
            arguments: Value::Object(tool_args),
            metadata: BTreeMap::new(),
        },
    };

    let file_edit_env = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity_fixed(),
        event: IngestionEvent::FileEdit {
            path: "src/main.rs".to_string(),
            diff: Some("--- a/src/main.rs\n+++ b/src/main.rs".to_string()),
            metadata: BTreeMap::new(),
        },
    };

    let git_commit_env = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity_fixed(),
        event: IngestionEvent::GitCommit {
            hash: "abc123def456".to_string(),
            message: "feat: add schema".to_string(),
            branch: Some("main".to_string()),
            files_changed: vec!["protocol/brain_events.schema.json".to_string()],
            metadata: BTreeMap::new(),
        },
    };

    let adapter_connected_env = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: create_identity_fixed(),
        event: IngestionEvent::AdapterConnected {
            adapter_version: "1.2.3".to_string(),
            supported_event_model_versions: vec!["1.0".to_string()],
            supported_serializations: vec!["json".to_string()],
            capabilities: vec![Capability::ConversationMessages, Capability::Replay],
            metadata: BTreeMap::new(),
        },
    };

    let test_cases = vec![
        ("message.json", message_env),
        ("tool_call.json", tool_call_env),
        ("file_edit.json", file_edit_env),
        ("git_commit.json", git_commit_env),
        ("adapter_connected.json", adapter_connected_env),
    ];

    for (filename, envelope) in test_cases {
        let filepath = golden_dir.join(filename);
        let raw_json = std::fs::read_to_string(&filepath)
            .unwrap_or_else(|e| panic!("Failed to read golden file {:?}: {}", filepath, e));

        // Deserialization check
        let roundtrip_envelope: IngestionEnvelope = serde_json::from_str(&raw_json)
            .unwrap_or_else(|e| panic!("Failed to deserialize golden file {}: {}", filename, e));
        assert_eq!(envelope, roundtrip_envelope);

        // Serialization check (must produce byte-for-byte identical output)
        let serialized = brain_integrations::to_canonical_json(&envelope)
            .unwrap_or_else(|e| panic!("Failed to serialize envelope for {}: {}", filename, e));

        assert_eq!(
            raw_json.trim(),
            serialized.trim(),
            "Golden file mismatch for {}.\nExpected:\n{}\nActual:\n{}",
            filename,
            raw_json,
            serialized
        );
    }
}

fn create_identity_fixed() -> EventIdentity {
    EventIdentity {
        event_id: "a4a7541f-8239-44d4-95e2-b91c0683072c".parse().unwrap(),
        parent_event_id: Some("b3b7541f-8239-44d4-95e2-b91c0683072c".parse().unwrap()),
        workspace_id: WorkspaceId::new("proj-123"),
        client_id: ClientId::new("cursor-1.0"),
        adapter_id: AdapterId::new("vscode-ext"),
        session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
        conversation_id: Some("01H7X1F8Z9Y000000000000001".parse().unwrap()),
        timestamp: chrono::DateTime::parse_from_rfc3339("2026-06-29T15:30:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    }
}
