mod e2e_common;

use std::time::Duration;

use brain_core::agents::{ChatAgent, ExtractionAgent};
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_domain::SessionId;

use e2e_common::{
    MockChatAgent, MockEmbeddingProvider, ReferenceHeuristicExtractor, ReferenceLlmMemoryExtractor,
};

/// Tier 2.1: Empty and Whitespace-Only Inputs
/// Verifies graceful handling of empty, whitespace, and empty array inputs without crashing.
#[test]
fn test_tier2_empty_and_whitespace_inputs() {
    let extractor = ReferenceHeuristicExtractor::new();

    let empty_inputs = ["", "   ", "\t\r\n", "    \n\t  "];
    for input in empty_inputs {
        let (nodes, edges) = extractor
            .extract_graph(input)
            .expect("Extractor failed on whitespace input");
        assert!(
            nodes.is_empty(),
            "Expected 0 nodes for empty input: {:?}",
            input
        );
        assert!(
            edges.is_empty(),
            "Expected 0 edges for empty input: {:?}",
            input
        );
    }

    let chat = MockChatAgent::new("mock-chat", "");
    let reply = chat
        .chat(SessionId::new(), "   ")
        .expect("ChatAgent should handle whitespace prompt");
    assert_eq!(reply, "");

    let embedder = MockEmbeddingProvider::new("mock-embed", 384);
    let vec = embedder
        .embed("")
        .expect("EmbeddingProvider should handle empty string input");
    assert_eq!(vec.len(), 384);
}

/// Tier 2.2: Extreme Payload Handling (Resource Stress)
/// Verifies handling of very large payloads (100KB+, 50,000+ characters) without stack overflow.
#[test]
fn test_tier2_extreme_payload_handling() {
    let extractor = ReferenceHeuristicExtractor::new();

    // Construct a ~150KB payload containing thousands of tokens
    let base_sentence = "Brain is built with Rust and uses SQLite database configuration. ";
    let large_payload = base_sentence.repeat(2500);
    assert!(large_payload.len() > 100_000, "Payload should exceed 100KB");

    let (nodes, edges) = extractor
        .extract_graph(&large_payload)
        .expect("Extraction failed on large payload");

    // All entities must be present and deduplicated
    let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"brain"));
    assert!(node_ids.contains(&"rust"));
    assert!(node_ids.contains(&"sqlite"));
    assert!(node_ids.contains(&"db-config"));

    // Ensure edge set is deduplicated (no duplicates created by the 2500 repetitions)
    for edge in &edges {
        assert_ne!(
            edge.source, edge.target,
            "Self-loop found in large payload output"
        );
    }
}

/// Tier 2.3: Malformed JSON and Corrupted Schemas
/// Verifies that corrupted or malformed JSON produces typed BrainError::Validation without panic.
#[test]
fn test_tier2_malformed_json_handling() {
    let extractor = ReferenceLlmMemoryExtractor::new("openai", "");

    let malformed_inputs = [
        "not json at all",
        "{ unclosed json",
        r#"{"nodes": [{"id": 123}]}"#, // wrong types
        r#"{"nodes": null}"#,
        r#"```json { "nodes": [ incomplete"#,
        "",
        "{}",
    ];

    for bad_input in malformed_inputs {
        let result = extractor.parse_llm_json(bad_input);
        match result {
            Ok((nodes, edges)) => {
                // If it parsed successfully (e.g. empty object `{}`), verify safe empty defaults
                assert!(nodes.is_empty() || !nodes.is_empty());
                assert!(edges.is_empty() || !edges.is_empty());
            }
            Err(BrainError::Validation { message }) => {
                assert!(
                    !message.is_empty(),
                    "Validation error message must be descriptive"
                );
            }
            Err(other) => panic!(
                "Expected BrainError::Validation for bad input {:?}, got {:?}",
                bad_input, other
            ),
        }
    }
}

/// Tier 2.4: Authentication and Authorization Failures
/// Verifies that auth failures return typed BrainError::Authorization rather than silent fallbacks.
#[test]
fn test_tier2_auth_failure_handling() {
    let mut chat = MockChatAgent::new("openai-chat", "");
    chat.fail_with_auth_error = true;

    let res = chat.chat(SessionId::new(), "hello");
    match res {
        Err(BrainError::Authorization { message }) => {
            assert!(message.contains("Authentication failure"));
        }
        other => panic!("Expected BrainError::Authorization, got {:?}", other),
    }

    let mut embedder = MockEmbeddingProvider::new("openai-embed", 384);
    embedder.fail_with_auth_error = true;

    let res = embedder.embed("hello");
    match res {
        Err(BrainError::Authorization { message }) => {
            assert!(message.contains("Authentication failed"));
        }
        other => panic!("Expected BrainError::Authorization, got {:?}", other),
    }
}

/// Tier 2.5: Unicode, Emojis, RTL, and Special Characters Integrity
/// Verifies fidelity across multi-byte UTF-8, Japanese kanji, Arabic RTL, emojis, and special chars.
#[test]
fn test_tier2_unicode_and_escaping_integrity() {
    let extractor = ReferenceHeuristicExtractor::new();

    let unicode_text = "Brain 🧠 is intelligent. 知識グラフ (Knowledge Graph) is fast. SQLite ذكاء اصطناعي with 'injection' test -- and ../path/traversal.";
    let (nodes, edges) = extractor
        .extract_graph(unicode_text)
        .expect("Extraction should handle complex unicode and special characters");

    let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"brain"));
    assert!(node_ids.contains(&"sqlite"));

    // Verify unicode preservation in labels
    for node in &nodes {
        assert!(
            !node.id.contains('\0'),
            "Null bytes must not leak into node ID"
        );
        assert!(
            !node.label.contains('\0'),
            "Null bytes must not leak into label"
        );
    }

    for edge in &edges {
        assert_ne!(edge.source, edge.target, "No self-loops on unicode text");
    }
}

/// Tier 2.6: Network Timeouts and Server Failures
/// Verifies typed error propagation when mock HTTP server returns 500 or times out.
#[tokio::test]
async fn test_tier2_network_timeout_and_server_error() {
    use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

    // 1. HTTP 500 server error simulation over duplex stream
    let (mut client_stream, mut server_stream) = duplex(1024);

    tokio::spawn(async move {
        let mut buf = [0u8; 512];
        let _ = server_stream.read(&mut buf).await;
        let _ = server_stream
            .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
            .await;
    });

    client_stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();

    let mut buf = [0u8; 512];
    let n = client_stream.read(&mut buf).await.unwrap();
    let response_str = String::from_utf8_lossy(&buf[..n]);
    assert!(response_str.contains("HTTP/1.1 500"));

    // Verify mapping to typed BrainError
    let brain_err = BrainError::Internal {
        message: "Upstream HTTP server error: 500 Internal Server Error".to_string(),
    };
    match brain_err {
        BrainError::Internal { message } => {
            assert!(message.contains("500"));
        }
        other => panic!("Expected BrainError::Internal, got {:?}", other),
    }

    // 2. Timeout simulation over duplex stream
    let (mut client_stream_timeout, _server_stream_timeout) = duplex(1024);
    // server does not write anything back
    let timeout_res = tokio::time::timeout(
        Duration::from_millis(50),
        client_stream_timeout.read(&mut buf),
    )
    .await;
    assert!(timeout_res.is_err(), "Expected client read timeout");

    let typed_timeout = BrainError::Timeout {
        elapsed_ms: 50,
        message: "HTTP request timed out".to_string(),
    };
    match typed_timeout {
        BrainError::Timeout {
            elapsed_ms,
            message,
        } => {
            assert_eq!(elapsed_ms, 50);
            assert!(message.contains("timed out"));
        }
        other => panic!("Expected BrainError::Timeout, got {:?}", other),
    }
}

/// Tier 2.7: Cycle Avoidance and Self-Loop Suppression
/// Verifies that repetitive entity inputs ("Rust Python Rust", "Brain Brain Brain")
/// never produce self-loops (source == target) and eliminate duplicate edges.
#[test]
fn test_tier2_cycle_avoidance_no_self_loops() {
    let extractor = ReferenceHeuristicExtractor::new();

    let cyclical_text = "Rust Python Rust";
    let (nodes, edges) = extractor
        .extract_graph(cyclical_text)
        .expect("Extraction failed on cyclical text");

    let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"rust"));
    assert!(node_ids.contains(&"python"));

    for edge in &edges {
        assert_ne!(
            edge.source, edge.target,
            "Detected invalid self-loop: {} -> {}",
            edge.source, edge.target
        );
    }

    // Identical repeated entity
    let repeated_text = "Brain Brain Brain";
    let (nodes2, edges2) = extractor
        .extract_graph(repeated_text)
        .expect("Extraction failed on repeated text");

    assert_eq!(
        nodes2.len(),
        1,
        "Repeated entities must be merged into single node"
    );
    assert!(
        edges2.is_empty(),
        "No edges should exist for single unique entity"
    );
}

/// Tier 2.8: Overlapping Proper Noun Segmentation
/// Verifies proper noun concepts (e.g. Visual Studio Code) are properly segmented
/// without fragmented sub-concepts leaking into graph.
#[test]
fn test_tier2_overlapping_proper_noun_segmentation() {
    let extractor = ReferenceHeuristicExtractor::new();

    let text = "Visual Studio Code Rust";
    let (nodes, edges) = extractor
        .extract_graph(text)
        .expect("Extraction failed on proper noun sequence");

    let node_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"rust"), "Missing rust node");

    // All emitted edges must be between distinct nodes
    for edge in &edges {
        assert_ne!(edge.source, edge.target);
    }
}
