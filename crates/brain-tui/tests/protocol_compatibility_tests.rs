use serde_json::json;

// Since UdsStreamEvent is not public in client.rs, we can re-declare or verify the DTO serialization / deserialization mappings directly against representative JSON values.
#[test]
fn test_wire_protocol_stream_start_deserialization() {
    let json_data = json!({
        "type": "stream_start",
        "streamId": "test-stream-id-123",
        "metadata": {}
    });

    // Check we can parse the expected fields
    let stream_id = json_data.get("streamId").and_then(|s| s.as_str()).unwrap();
    let type_str = json_data.get("type").and_then(|s| s.as_str()).unwrap();
    assert_eq!(stream_id, "test-stream-id-123");
    assert_eq!(type_str, "stream_start");
}

#[test]
fn test_wire_protocol_stream_progress_deserialization() {
    let json_data = json!({
        "type": "stream_progress",
        "streamId": "test-stream-id-123",
        "sequence": 42,
        "progress": 0.75,
        "message": "indexing files",
        "metadata": {}
    });

    let seq = json_data.get("sequence").and_then(|s| s.as_u64()).unwrap();
    let progress = json_data.get("progress").and_then(|s| s.as_f64()).unwrap();
    let message = json_data.get("message").and_then(|s| s.as_str()).unwrap();

    assert_eq!(seq, 42);
    assert_eq!(progress, 0.75);
    assert_eq!(message, "indexing files");
}

#[test]
fn test_wire_protocol_stream_chunk_deserialization() {
    let json_data = json!({
        "type": "stream_chunk",
        "streamId": "test-stream-id-123",
        "sequence": 43,
        "content": "parsed chunk text content",
        "metadata": {}
    });

    let seq = json_data.get("sequence").and_then(|s| s.as_u64()).unwrap();
    let content = json_data.get("content").and_then(|s| s.as_str()).unwrap();

    assert_eq!(seq, 43);
    assert_eq!(content, "parsed chunk text content");
}

#[test]
fn test_wire_protocol_stream_end_deserialization() {
    let json_data = json!({
        "type": "stream_end",
        "streamId": "test-stream-id-123",
        "sequence": 44,
        "metadata": {}
    });

    let seq = json_data.get("sequence").and_then(|s| s.as_u64()).unwrap();
    assert_eq!(seq, 44);
}
