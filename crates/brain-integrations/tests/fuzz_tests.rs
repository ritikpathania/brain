use brain_domain::{AdapterId, ClientId, ConversationId, EventId, SessionId, WorkspaceId};
use brain_integrations::{Capability, EventIdentity, IngestionEnvelope, IngestionEvent, Value};
use rand::Rng;
use std::collections::BTreeMap;
use std::path::Path;

fn get_schema_validator() -> jsonschema::JSONSchema {
    let schema_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../protocol/brain_events.schema.json");
    let schema_content = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|e| panic!("Failed to read schema file at {:?}: {}", schema_path, e));
    let schema_json: serde_json::Value = serde_json::from_str(&schema_content)
        .expect("Failed to parse schema JSON");
    jsonschema::JSONSchema::compile(&schema_json).expect("Failed to compile schema")
}

fn random_string<R: Rng>(rng: &mut R, len: usize) -> String {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ".chars().collect();
    let mut s = String::new();
    for _ in 0..len {
        let idx = rng.gen_range(0..chars.len());
        s.push(chars[idx]);
    }
    s
}

fn random_value<R: Rng>(rng: &mut R, depth: usize) -> Value {
    if depth > 2 {
        return match rng.gen_range(0..4) {
            0 => Value::Null,
            1 => Value::Bool(rng.gen()),
            2 => Value::Number((rng.gen_range(-1000.0..1000.0) * 100.0f64).round() / 100.0f64),
            _ => Value::String(random_string(rng, 10)),
        };
    }

    match rng.gen_range(0..6) {
        0 => Value::Null,
        1 => Value::Bool(rng.gen()),
        2 => Value::Number((rng.gen_range(-1000.0..1000.0) * 100.0f64).round() / 100.0f64),
        3 => Value::String(random_string(rng, 10)),
        4 => {
            let len = rng.gen_range(0..5);
            let mut arr = Vec::new();
            for _ in 0..len {
                arr.push(random_value(rng, depth + 1));
            }
            Value::Array(arr)
        }
        _ => {
            let len = rng.gen_range(0..5);
            let mut map = BTreeMap::new();
            for _ in 0..len {
                map.insert(random_string(rng, 5), random_value(rng, depth + 1));
            }
            Value::Object(map)
        }
    }
}

fn random_event<R: Rng>(rng: &mut R) -> IngestionEvent {
    let mut metadata = BTreeMap::new();
    if rng.gen_bool(0.5) {
        metadata.insert(random_string(rng, 5), random_value(rng, 0));
    }

    match rng.gen_range(0..13) {
        0 => IngestionEvent::Message {
            role: if rng.gen() { "user".to_string() } else { "assistant".to_string() },
            content: random_string(rng, 50),
            metadata,
        },
        1 => {
            let mut args = BTreeMap::new();
            args.insert("path".to_string(), Value::String(random_string(rng, 10)));
            args.insert("param".to_string(), random_value(rng, 0));
            IngestionEvent::ToolCall {
                call_id: random_string(rng, 15),
                tool_name: random_string(rng, 10),
                arguments: Value::Object(args),
                metadata,
            }
        }
        2 => IngestionEvent::ToolResult {
            call_id: random_string(rng, 15),
            output: random_string(rng, 100),
            is_error: rng.gen(),
            metadata,
        },
        3 => IngestionEvent::FileEdit {
            path: random_string(rng, 20),
            diff: if rng.gen() { Some(random_string(rng, 200)) } else { None },
            metadata,
        },
        4 => IngestionEvent::Diagnostic {
            severity: if rng.gen() { "error".to_string() } else { "warning".to_string() },
            message: random_string(rng, 30),
            source: random_string(rng, 10),
            file: if rng.gen() { Some(random_string(rng, 15)) } else { None },
            line: if rng.gen() { Some(rng.gen()) } else { None },
            metadata,
        },
        5 => IngestionEvent::TerminalCommand {
            command: random_string(rng, 20),
            exit_code: if rng.gen() { Some(rng.gen_range(-1..128)) } else { None },
            stdout_summary: if rng.gen() { Some(random_string(rng, 50)) } else { None },
            metadata,
        },
        6 => {
            let num_files = rng.gen_range(0..5);
            let mut files = Vec::new();
            for _ in 0..num_files {
                files.push(random_string(rng, 15));
            }
            IngestionEvent::GitCommit {
                branch: if rng.gen() { Some(random_string(rng, 10)) } else { None },
                files_changed: files,
                hash: random_string(rng, 40),
                message: random_string(rng, 60),
                metadata,
            }
        },
        7 => IngestionEvent::GitBranch {
            action: random_string(rng, 10),
            branch_name: random_string(rng, 20),
            metadata,
        },
        8 => IngestionEvent::SessionStarted { metadata },
        9 => IngestionEvent::SessionEnded { metadata },
        10 => {
            let mut caps = Vec::new();
            if rng.gen() { caps.push(Capability::ConversationMessages); }
            if rng.gen() { caps.push(Capability::ConversationTools); }
            if rng.gen() { caps.push(Capability::WorkspaceGit); }
            if rng.gen() { caps.push(Capability::WorkspaceFiles); }
            if rng.gen() { caps.push(Capability::WorkspaceTerminal); }
            if rng.gen() { caps.push(Capability::WorkspaceDiagnostics); }
            if rng.gen() { caps.push(Capability::Replay); }
            if rng.gen() { caps.push(Capability::Batching); }
            IngestionEvent::AdapterConnected {
                adapter_version: random_string(rng, 8),
                capabilities: caps,
                metadata,
                supported_event_model_versions: vec!["1.0".to_string()],
                supported_serializations: vec!["json".to_string()],
            }
        },
        11 => IngestionEvent::AdapterDisconnected {
            metadata,
            reason: if rng.gen() { Some(random_string(rng, 20)) } else { None },
        },
        _ => IngestionEvent::Text {
            content: random_string(rng, 100),
            metadata,
        },
    }
}

fn create_random_identity<R: Rng>(rng: &mut R) -> EventIdentity {
    EventIdentity {
        event_id: EventId::new(),
        parent_event_id: if rng.gen() { Some(EventId::new()) } else { None },
        workspace_id: WorkspaceId::new(&random_string(rng, 8)),
        client_id: ClientId::new(&random_string(rng, 10)),
        adapter_id: AdapterId::new(&random_string(rng, 10)),
        session_id: SessionId::new(),
        conversation_id: if rng.gen() { Some(ConversationId::new()) } else { None },
        timestamp: chrono::Utc::now(),
    }
}

#[test]
fn test_protocol_fuzzing() {
    let mut rng = rand::thread_rng();
    let validator = get_schema_validator();

    for _ in 0..100 {
        let envelope = IngestionEnvelope {
            event_model_version: "1.0".to_string(),
            identity: create_random_identity(&mut rng),
            event: random_event(&mut rng),
        };

        // 1. Serialize to canonical JSON
        let json_str = brain_integrations::to_canonical_json(&envelope).expect("Fuzzer failed to serialize");

        // 2. Validate against JSON schema
        let json_val: serde_json::Value = serde_json::from_str(&json_str).expect("Fuzzer failed to parse JSON string");
        let validation_res = validator.validate(&json_val);
        if let Err(errors) = validation_res {
            let error_msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
            panic!(
                "Fuzzed variant {:?} failed schema validation.\nErrors:\n{}",
                envelope.event.kind(),
                error_msgs.join("\n")
            );
        }

        // 3. Deserialize back and assert equivalence
        let deserialized: IngestionEnvelope = serde_json::from_str(&json_str).expect("Fuzzer failed to deserialize");
        assert_eq!(envelope, deserialized);
    }
}
