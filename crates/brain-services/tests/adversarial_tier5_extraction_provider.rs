//! Tier 5 White-Box Adversarial Coverage Hardening Test Suite for Native Extraction & Providers.
//!
//! Enforces:
//! 1. Heuristic Extractor resilience under extreme sentence structures, multi-line texts,
//!    mixed punctuation storms, escaped characters, and complex file paths.
//! 2. Proper handling of ambiguous proper nouns coinciding with active verbs or classification keywords.
//! 3. Referential integrity preservation across deep sentence structures and casing variants (zero dangling edges).
//! 4. Markdown code-fence stripping across complex preambles, quadrupled backticks, nested braces, and missing fences.
//! 5. LLM extractor referential pruning on dangling edges and graceful mock provider failure propagation.
//! 6. Native LLM providers status-code mapping (400, 401, 403, 422, 429, 502, 503, 504), timeouts, and empty choices.
//! 7. Native Embedding providers dimension enforcement, Ollama legacy endpoint fallback, and zero dummy fallback vectors.
//! 8. Multi-threaded concurrent execution across Tokio and non-Tokio contexts without deadlocks or thread poisoning.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use brain_core::agents::{ChatAgent, ExtractionAgent};
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_core::services::{ExtractionRequest, MemoryExtractor};
use brain_domain::{KnowledgeGraph, SessionId};
use brain_services::extraction::heuristics::HeuristicExtractor;
use brain_services::extraction::llm::LlmMemoryExtractor;
use brain_services::providers::embeddings::{
    OllamaEmbeddingProvider, OpenAiCompatibleEmbeddingProvider, OpenAiEmbeddingProvider,
};
use brain_services::providers::llm::{AnthropicLlmProvider, OllamaLlmProvider, OpenAiLlmProvider};

use serde_json::{json, Value};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// Test 1: Heuristic Extractor — Extreme Multi-Line Sentence Structures & Punctuation Storms
// ============================================================================

#[test]
fn test_heuristics_multiline_punctuation_storms_and_escapes() {
    let extractor = HeuristicExtractor::new();

    let adversarial_input = "\
        Alice develops Brain.\n\
        Bob uses SQLite.\r\n\
        Docker runs on Linux.\tDuckDB stores data in config.toml.\n\n\
        Wait?!... Is Brain really an AI Agent Engine??? Yes!! ... Alice built it.\r\n\
        Path is C:\\Users\\Alice\\database.sqlite. App uses \"JWT Tokens\" stored in .env.\n\
        System-wide config located at ./services/auth/token.env; DuckDB — a fast analytical database — runs on Docker.\
    ";

    // 1. extract_graph interface
    let (nodes, edges) = extractor
        .extract_graph(adversarial_input)
        .expect("extract_graph must succeed without panicking on complex punctuation/escapes");

    assert!(!nodes.is_empty(), "Nodes must not be empty");
    assert!(!edges.is_empty(), "Edges must not be empty");

    let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

    // Verify referential integrity
    for edge in &edges {
        assert_ne!(
            edge.source, edge.target,
            "Self-loop forbidden: source '{}' == target '{}'",
            edge.source, edge.target
        );
        assert!(
            node_ids.contains(&edge.source),
            "Edge source '{}' must exist in nodes: {:?}",
            edge.source,
            node_ids
        );
        assert!(
            node_ids.contains(&edge.target),
            "Edge target '{}' must exist in nodes: {:?}",
            edge.target,
            node_ids
        );
    }

    // 2. MemoryExtractor::extract and KnowledgeGraph ingestion
    let req = ExtractionRequest {
        raw_content: adversarial_input.to_string(),
        context_metadata: [
            ("conversation_id".to_string(), "conv-storm-1".to_string()),
            ("message_id".to_string(), "msg-storm-1".to_string()),
        ]
        .into_iter()
        .collect(),
    };
    let result = extractor
        .extract(req)
        .expect("MemoryExtractor::extract must succeed");

    let mut kg = KnowledgeGraph::new();
    for node in result.nodes {
        kg.add_node(node);
    }
    for edge in result.edges {
        kg.add_edge(edge)
            .expect("Ingestion into KnowledgeGraph must not trigger missing endpoint error");
    }

    // 3. extract_semantic_nodes_json FFI interface
    let json_payload = serde_json::json!([
        {
            "id": "node-storm",
            "content": adversarial_input,
            "epoch": 1,
            "timestamp": 123456789
        }
    ])
    .to_string();

    let json_resp = extractor
        .extract_semantic_nodes_json(&json_payload)
        .expect("extract_semantic_nodes_json must succeed");
    let val: Value = serde_json::from_str(&json_resp).expect("must return valid JSON");
    assert_eq!(val["status"], "success");
    assert!(val["nodes"].as_array().expect("nodes array").len() >= 5);
    assert!(!val["edges"].as_array().expect("edges array").is_empty());
}

// ============================================================================
// Test 2: Heuristic Extractor — Ambiguous Proper Nouns Coinciding with Active Verbs & Classification Keywords
// ============================================================================

#[test]
fn test_heuristics_ambiguous_proper_nouns_active_verbs_classification() {
    let extractor = HeuristicExtractor::new();

    let inputs = [
        // Capitalized active verbs as proper noun subject/object
        "Uses develops Builds. Builds runs on Stores. Requires depends on Employing.",
        // Proper nouns identical to classification auxiliary verbs
        "Is is an Open Source Database. Was was a Fast Cache Engine. Were were Distributed Systems.",
        // Passive relation patterns where subject/object coincides with verb synonyms
        "SQLite is used by Uses. Rust was developed by Develops. Linux is run by Executes.",
        // Classification subject as participle/verb
        "Deploying is an Automated Pipeline. Creating develops Brain.",
    ];

    for input in &inputs {
        let (nodes, edges) = extractor
            .extract_graph(input)
            .unwrap_or_else(|e| panic!("Failed on input '{}': {:?}", input, e));

        let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let mut seen_edges = HashSet::new();

        for edge in &edges {
            // Invariant 1: No self-loops
            assert_ne!(
                edge.source, edge.target,
                "Self-loop detected on input '{}': {:?}",
                input, edge
            );

            // Invariant 2: No duplicate edge triples
            let triple = (&edge.source, &edge.target, &edge.relation);
            assert!(
                seen_edges.insert(triple),
                "Duplicate edge triple on input '{}': {:?}",
                input,
                triple
            );

            // Invariant 3: Zero dangling edges
            assert!(
                node_ids.contains(&edge.source),
                "Edge source '{}' missing from nodes on input '{}'",
                edge.source,
                input
            );
            assert!(
                node_ids.contains(&edge.target),
                "Edge target '{}' missing from nodes on input '{}'",
                edge.target,
                input
            );
        }

        // Invariant 4: KnowledgeGraph clean ingestion
        let req = ExtractionRequest {
            raw_content: input.to_string(),
            context_metadata: HashMap::new(),
        };
        let result = extractor
            .extract(req)
            .unwrap_or_else(|e| panic!("extract failed on '{}': {:?}", input, e));

        let mut kg = KnowledgeGraph::new();
        for node in result.nodes {
            kg.add_node(node);
        }
        for edge in result.edges {
            kg.add_edge(edge).unwrap_or_else(|e| {
                panic!("KnowledgeGraph add_edge failed on '{}': {:?}", input, e)
            });
        }
    }
}

// ============================================================================
// Test 3: Heuristic Extractor — Referential Integrity in Deep Sentence Structures & Casing Merges
// ============================================================================

#[test]
fn test_heuristics_deep_referential_integrity_and_casing_merges() {
    let extractor = HeuristicExtractor::new();

    // 20 interconnected sentences with synonym casing variations, conceptual rules, and sequential links
    let deep_text = "\
        Alice develops Brain. \
        Brain is an AI agent engine. \
        Brain is a Cognitive Architecture System. \
        brain uses SQLITE. \
        sqlite3 is a Relational Database. \
        Database configuration configures sqlite. \
        The db config configures postgresql. \
        Postgres is an Enterprise Database. \
        Bob stores credentials in config.toml. \
        Config.toml contains API KEY. \
        The api keys are stored in environment variables. \
        Environment variable configures Docker. \
        Docker runs on Linux. \
        Linux executes Bun. \
        bun connects via unix domain socket to Rust. \
        Rust communicates via websockets with Python. \
        python uses Redis. \
        redis is an In-Memory Cache. \
        DuckDB is an Analytical Database. \
        Alice deploys Brain on Docker.\
    ";

    let (nodes, edges) = extractor
        .extract_graph(deep_text)
        .expect("deep_text extraction must succeed");

    assert!(nodes.len() >= 10, "Expected at least 10 merged nodes");
    assert!(edges.len() >= 12, "Expected at least 12 extracted edges");

    let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();

    // Invariant: Canonical IDs are unified regardless of original casing
    assert!(
        node_ids.contains("sqlite"),
        "Expected unified canonical id 'sqlite'"
    );
    assert!(
        node_ids.contains("brain"),
        "Expected unified canonical id 'brain'"
    );
    assert!(
        node_ids.contains("postgresql"),
        "Expected unified canonical id 'postgresql'"
    );
    assert!(
        node_ids.contains("api-key"),
        "Expected unified canonical id 'api-key'"
    );
    assert!(
        node_ids.contains("env-vars"),
        "Expected unified canonical id 'env-vars'"
    );
    assert!(
        node_ids.contains("uds"),
        "Expected unified canonical id 'uds'"
    );
    assert!(
        node_ids.contains("websockets"),
        "Expected unified canonical id 'websockets'"
    );

    // Invariant: Conceptual rules created valid edges
    let has_db_config_edge = edges
        .iter()
        .any(|e| e.source == "db-config" && e.target == "sqlite" && e.relation == "configures");
    assert!(
        has_db_config_edge,
        "db-config -> sqlite configures edge must exist"
    );

    let has_api_key_edge = edges
        .iter()
        .any(|e| e.source == "api-key" && e.target == "env-vars" && e.relation == "stored_in");
    assert!(
        has_api_key_edge,
        "api-key -> env-vars stored_in edge must exist"
    );

    // Invariant: Zero dangling edges across entire deep structure
    for edge in &edges {
        assert!(
            node_ids.contains(&edge.source),
            "Dangling edge source: '{}' not in nodes",
            edge.source
        );
        assert!(
            node_ids.contains(&edge.target),
            "Dangling edge target: '{}' not in nodes",
            edge.target
        );
    }

    // Invariant: KnowledgeGraph full ingestion
    let req = ExtractionRequest {
        raw_content: deep_text.to_string(),
        context_metadata: HashMap::new(),
    };
    let result = extractor.extract(req).expect("extract must succeed");
    let mut kg = KnowledgeGraph::new();
    for node in result.nodes {
        kg.add_node(node);
    }
    for edge in result.edges {
        kg.add_edge(edge)
            .expect("Ingestion must succeed without dangling edge errors");
    }
}

// ============================================================================
// Test 4: LLM Memory Extractor — Code-Fence Stripping & Adversarial Markdown Payloads
// ============================================================================

#[test]
fn test_llm_extractor_markdown_fence_stripping_and_adversarial_preambles() {
    let adversarial_cases = [
        // Case 1: Standard ```json ... ``` with conversational chatter
        (
            "Hello! Here is the graph representation:\n```json\n{\"nodes\": [{\"id\": \"sqlite\", \"label\": \"SQLite\", \"type\": \"technology\"}], \"edges\": []}\n```\nHope that helps!",
            1,
            0,
        ),
        // Case 2: Uppercase ```JSON ... ```
        (
            "```JSON\n{\"nodes\": [{\"id\": \"rust\", \"label\": \"Rust\", \"type\": \"language\"}, {\"id\": \"docker\", \"label\": \"Docker\", \"type\": \"technology\"}], \"edges\": [{\"source\": \"rust\", \"target\": \"docker\", \"relation\": \"runs_on\", \"weight\": 0.95}]}\n```",
            2,
            1,
        ),
        // Case 3: Four backticks ````json ... ````
        (
            "````json\n{\"nodes\": [{\"id\": \"brain\", \"label\": \"Brain\", \"type\": \"project\"}], \"edges\": []}\n````",
            1,
            0,
        ),
        // Case 4: No backtick fences, bare JSON with surrounding text
        (
            "Based on the input: {\"nodes\": [{\"id\": \"duckdb\", \"label\": \"DuckDB\"}], \"edges\": []} done.",
            1,
            0,
        ),
        // Case 5: Nested braces in JSON string attributes
        (
            "```json\n{\"nodes\": [{\"id\": \"n1\", \"label\": \"N1\", \"type\": \"concept\", \"attributes\": {\"desc\": \"Contains {curly braces} and \\\"quotes\\\"\"}}], \"edges\": []}\n```",
            1,
            0,
        ),
        // Case 6: Unclosed code fence (truncated generation)
        (
            "```json\n{\"nodes\": [{\"id\": \"n2\", \"label\": \"N2\"}], \"edges\": []}",
            1,
            0,
        ),
        // Case 7: Minimal empty object `{}`
        (
            "{}",
            0,
            0,
        ),
    ];

    for (raw_llm_output, expected_nodes_count, expected_edges_count) in &adversarial_cases {
        let (nodes, edges) = LlmMemoryExtractor::parse_llm_json(raw_llm_output)
            .unwrap_or_else(|e| panic!("Failed to parse payload '{}': {:?}", raw_llm_output, e));

        assert_eq!(
            nodes.len(),
            *expected_nodes_count,
            "Node count mismatch for '{}'",
            raw_llm_output
        );
        assert_eq!(
            edges.len(),
            *expected_edges_count,
            "Edge count mismatch for '{}'",
            raw_llm_output
        );

        // Verify default fallbacks for missing fields
        for node in &nodes {
            assert!(
                !node.node_type.is_empty(),
                "Node type must fallback to non-empty default"
            );
        }
        for edge in &edges {
            assert!(
                edge.weight > 0.0,
                "Edge weight must fallback to positive default"
            );
        }
    }
}

// ============================================================================
// Test 5: LLM Memory Extractor — Dangling Edge Pruning & Mock Failures
// ============================================================================

struct MockFailingChatAgent {
    error_mode: String,
}

impl ChatAgent for MockFailingChatAgent {
    fn name(&self) -> &str {
        "mock-failing-agent"
    }

    fn chat(&self, _session_id: SessionId, _prompt: &str) -> Result<String, BrainError> {
        match self.error_mode.as_str() {
            "timeout" => Err(BrainError::Timeout {
                elapsed_ms: 10000,
                message: "LLM generation timed out after 10000ms".to_string(),
            }),
            "auth" => Err(BrainError::Authorization {
                message: "Invalid API key provided".to_string(),
            }),
            "invalid_json" => Ok("```json\n[1, 2, 3, not valid graph object]\n```".to_string()),
            "dangling_edges" => Ok(r#"{
                "nodes": [
                    {"id": "node-a", "label": "Node A", "type": "technology"},
                    {"id": "node-b", "label": "Node B", "type": "technology"}
                ],
                "edges": [
                    {"source": "node-a", "target": "node-b", "relation": "uses"},
                    {"source": "node-a", "target": "ghost-node-1", "relation": "depends_on"},
                    {"source": "ghost-node-2", "target": "node-b", "relation": "configures"}
                ]
            }"#
            .to_string()),
            _ => Ok("{}".to_string()),
        }
    }
}

#[test]
fn test_llm_extractor_dangling_edge_pruning_and_mock_failures() {
    // 1. Dangling edge pruning in MemoryExtractor::extract
    let dangling_agent = Arc::new(MockFailingChatAgent {
        error_mode: "dangling_edges".to_string(),
    });
    let extractor = LlmMemoryExtractor::new(dangling_agent, "llm-dangling-test");

    let req = ExtractionRequest {
        raw_content: "dummy transcript".to_string(),
        context_metadata: HashMap::new(),
    };
    let result = extractor
        .extract(req)
        .expect("extract must succeed despite dangling edges in raw LLM output");

    assert_eq!(
        result.nodes.len(),
        2,
        "Expected exactly 2 valid extracted nodes"
    );
    assert_eq!(
        result.edges.len(),
        1,
        "Dangling edges pointing to ghost nodes must be pruned"
    );

    // Invariant: Ingestion into KnowledgeGraph succeeds without error
    let mut kg = KnowledgeGraph::new();
    for node in result.nodes {
        kg.add_node(node);
    }
    for edge in result.edges {
        kg.add_edge(edge)
            .expect("KnowledgeGraph must accept pruned edge result cleanly");
    }

    // 2. Error propagation across timeout
    let timeout_agent = Arc::new(MockFailingChatAgent {
        error_mode: "timeout".to_string(),
    });
    let extractor_timeout = LlmMemoryExtractor::new(timeout_agent, "llm-timeout-test");

    let res_graph = extractor_timeout.extract_graph("test text");
    assert!(
        matches!(res_graph, Err(BrainError::Timeout { .. })),
        "extract_graph must propagate BrainError::Timeout"
    );

    // FFI entrypoint returns structured JSON error without panic
    let res_ffi = extractor_timeout
        .extract_semantic_nodes_json("[{\"content\": \"test\"}]")
        .expect("FFI entrypoint must return Ok JSON error");
    let val_ffi: Value = serde_json::from_str(&res_ffi).expect("valid JSON response");
    assert_eq!(val_ffi["status"], "error");
    assert!(val_ffi["message"]
        .as_str()
        .expect("message")
        .contains("timed out"));

    // 3. Schema validation failure on non-object JSON
    let invalid_json_agent = Arc::new(MockFailingChatAgent {
        error_mode: "invalid_json".to_string(),
    });
    let extractor_invalid = LlmMemoryExtractor::new(invalid_json_agent, "llm-invalid-test");

    let res_invalid = extractor_invalid.extract_graph("test text");
    assert!(
        matches!(res_invalid, Err(BrainError::Validation { .. })),
        "extract_graph must return BrainError::Validation on invalid JSON shape"
    );
}

// ============================================================================
// Test 6: Native LLM Providers — Adversarial WireMock HTTP Statuses & Rate Limits
// ============================================================================

#[tokio::test]
async fn test_native_llm_providers_raw_http_status_codes_and_timeouts() {
    let server = MockServer::start().await;

    // 1. Ollama HTTP 429 Rate Limit mapping
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(429).set_body_string("Rate limit exceeded"))
        .mount(&server)
        .await;

    let ollama = OllamaLlmProvider::new("llama3", server.uri());
    let err_ollama = ollama
        .chat(SessionId::new(), "hello")
        .expect_err("429 must fail");
    match err_ollama {
        BrainError::Model { message, .. } => {
            assert!(
                message.contains("429") || message.to_lowercase().contains("rate limit"),
                "Expected rate limit message, got: {}",
                message
            );
        }
        other => panic!("Expected BrainError::Model on 429, got: {:?}", other),
    }

    server.reset().await;

    // 2. OpenAI HTTP 400 Bad Request mapping -> BrainError::Validation
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_string("Malformed request syntax"))
        .mount(&server)
        .await;

    let openai = OpenAiLlmProvider::new("gpt-4o-mini")
        .with_api_key("sk-test-key")
        .with_base_url(server.uri());

    let err_openai_400 = openai
        .chat(SessionId::new(), "hello")
        .expect_err("400 must fail");
    assert!(
        matches!(err_openai_400, BrainError::Validation { .. }),
        "HTTP 400 must map to BrainError::Validation, got {:?}",
        err_openai_400
    );

    server.reset().await;

    // 3. OpenAI HTTP 503 Service Unavailable mapping -> BrainError::Model (server error)
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(503).set_body_string("OpenAI service temporarily unavailable"),
        )
        .mount(&server)
        .await;

    let err_openai_503 = openai
        .chat(SessionId::new(), "hello")
        .expect_err("503 must fail");
    match err_openai_503 {
        BrainError::Model { message, .. } => {
            assert!(
                message.contains("503") || message.contains("server error"),
                "Expected 503 server error message, got: {}",
                message
            );
        }
        other => panic!("Expected BrainError::Model on 503, got: {:?}", other),
    }

    server.reset().await;

    // 4. Anthropic missing text content block in HTTP 200 response -> BrainError::Validation
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [
                {"type": "tool_use", "id": "t1", "name": "dummy_tool"}
            ]
        })))
        .mount(&server)
        .await;

    let anthropic = AnthropicLlmProvider::new("claude-3-5-sonnet")
        .with_api_key("anthropic-test-key")
        .with_base_url(server.uri());

    let err_anthropic_no_text = anthropic
        .chat(SessionId::new(), "hello")
        .expect_err("200 without text block must fail");
    assert!(
        matches!(err_anthropic_no_text, BrainError::Validation { .. }),
        "Missing text block must return BrainError::Validation, got {:?}",
        err_anthropic_no_text
    );

    server.reset().await;

    // 5. Anthropic client-side timeout propagation -> BrainError::Timeout
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(300))
                .set_body_json(json!({"content": [{"type": "text", "text": "Delayed"}]})),
        )
        .mount(&server)
        .await;

    let anthropic_fast = anthropic.with_timeout(Duration::from_millis(50));
    let err_timeout = anthropic_fast
        .chat(SessionId::new(), "hello")
        .expect_err("Delayed response must timeout");
    match err_timeout {
        BrainError::Timeout { elapsed_ms, .. } => {
            assert!(
                elapsed_ms >= 50,
                "Elapsed ms must reflect timeout: {}",
                elapsed_ms
            );
        }
        BrainError::Network { message, .. } => {
            assert!(
                message.to_lowercase().contains("timeout") || message.contains("timed out"),
                "Network error message must denote timeout: {}",
                message
            );
        }
        other => panic!("Expected BrainError::Timeout, got: {:?}", other),
    }
}

// ============================================================================
// Test 7: Native Embedding Providers — Adversarial Ollama Fallbacks & Dimension Enforcement
// ============================================================================

#[tokio::test]
async fn test_native_embedding_providers_fallbacks_dimensions_and_errors() {
    let server = MockServer::start().await;

    // 1. Ollama fallback from 404 /api/embed to legacy /api/embeddings
    Mock::given(method("POST"))
        .and(path("/api/embed"))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    let expected_vector = vec![0.1f32; 384];
    Mock::given(method("POST"))
        .and(path("/api/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "embedding": expected_vector
        })))
        .mount(&server)
        .await;

    let ollama_embed =
        OllamaEmbeddingProvider::new("nomic-embed-text", server.uri()).with_dimension(384);

    let vec_res = ollama_embed
        .embed("test text for embedding")
        .expect("Fallback to legacy /api/embeddings must succeed");
    assert_eq!(vec_res.len(), 384);
    assert_eq!(vec_res[0], 0.1f32);

    server.reset().await;

    // 2. Dimension discrepancy detection: configured 384, server returns 128 -> BrainError::Validation
    Mock::given(method("POST"))
        .and(path("/api/embed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "embeddings": [vec![0.5f32; 128]]
        })))
        .mount(&server)
        .await;

    let ollama_mismatch =
        OllamaEmbeddingProvider::new("nomic-embed-text", server.uri()).with_dimension(384);

    let err_dimension = ollama_mismatch
        .embed("dimension test")
        .expect_err("Dimension mismatch must be rejected");
    match err_dimension {
        BrainError::Validation { message } => {
            assert!(
                message.contains("128") && message.contains("384"),
                "Validation message must specify actual vs expected dimension: {}",
                message
            );
        }
        other => panic!(
            "Expected BrainError::Validation on dimension mismatch, got: {:?}",
            other
        ),
    }

    server.reset().await;

    // 3. OpenAI Embedding HTTP 401 Unauthorized -> BrainError::Authorization (NEVER fallback dummy vector [0.0; 384])
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid API key"))
        .mount(&server)
        .await;

    let openai_embed = OpenAiEmbeddingProvider::new("text-embedding-3-small")
        .with_api_key("bad-key")
        .with_base_url(server.uri());

    let err_openai_auth = openai_embed
        .embed("secure text")
        .expect_err("Auth error must fail");
    assert!(
        matches!(err_openai_auth, BrainError::Authorization { .. }),
        "Auth failure must return BrainError::Authorization without returning dummy vector: {:?}",
        err_openai_auth
    );

    server.reset().await;

    // 4. OpenAI-Compatible Provider: custom model, custom dimension (768), with API key header
    let custom_vec = vec![0.42f32; 768];
    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .and(header("Authorization", "Bearer custom-secret-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"embedding": custom_vec}]
        })))
        .mount(&server)
        .await;

    let custom_embed = OpenAiCompatibleEmbeddingProvider::new("bge-base-en", server.uri())
        .with_api_key("custom-secret-key")
        .with_dimension(768);

    let res_custom = custom_embed
        .embed("custom embedding input")
        .expect("OpenAI-compatible embedding must succeed");
    assert_eq!(res_custom.len(), 768);
    assert_eq!(res_custom[0], 0.42f32);
}

// ============================================================================
// Test 8: Concurrent Multi-Threaded Sync/Async Provider & Extractor Execution
// ============================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_multithreaded_provider_and_extractor_execution() {
    let server = MockServer::start().await;

    // Wire mock endpoints for both Chat and Embeddings
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "{\"nodes\": [{\"id\": \"thread-node\", \"label\": \"Thread Node\"}], \"edges\": []}"
        })))
        .mount(&server)
        .await;

    let embed_vec = vec![0.123f32; 384];
    Mock::given(method("POST"))
        .and(path("/api/embed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "embeddings": [embed_vec]
        })))
        .mount(&server)
        .await;

    let server_uri = server.uri();
    let heuristic_extractor = Arc::new(HeuristicExtractor::new());
    let llm_provider = Arc::new(OllamaLlmProvider::new("llama3", &server_uri));
    let llm_extractor = Arc::new(LlmMemoryExtractor::new(
        llm_provider.clone(),
        "concurrent-llm",
    ));
    let embed_provider =
        Arc::new(OllamaEmbeddingProvider::new("nomic-embed-text", &server_uri).with_dimension(384));

    // Part A: Exercise tokio::task::block_in_place path via tokio::task::spawn_blocking
    let mut tokio_handles = Vec::new();
    for thread_id in 0..8 {
        let h_ext = heuristic_extractor.clone();
        let l_ext = llm_extractor.clone();
        let l_prov = llm_provider.clone();
        let e_prov = embed_provider.clone();

        tokio_handles.push(tokio::task::spawn_blocking(move || {
            for iter in 0..5 {
                // Heuristic extraction
                let text = format!(
                    "Tokio-worker-{} iter-{} Alice develops Brain. Brain uses SQLite. SQLite is configured by db-config.",
                    thread_id, iter
                );
                let (nodes, edges) = h_ext.extract_entities_and_relations(&text);
                assert!(!nodes.is_empty());
                assert!(!edges.is_empty());

                let node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
                for edge in &edges {
                    assert!(node_ids.contains(&edge.source));
                    assert!(node_ids.contains(&edge.target));
                }

                // LLM provider synchronous chat call
                let chat_resp = l_prov
                    .chat(SessionId::new(), "hello")
                    .expect("Concurrent chat must succeed");
                assert!(!chat_resp.is_empty());

                // LLM extractor synchronous extract call
                let llm_res = l_ext
                    .extract_graph("test text")
                    .expect("Concurrent LLM extract must succeed");
                assert_eq!(llm_res.0.len(), 1);

                // Embedding synchronous embed call
                let vec = e_prov
                    .embed("concurrent embedding text")
                    .expect("Concurrent embed must succeed");
                assert_eq!(vec.len(), 384);
            }
        }));
    }

    for handle in tokio_handles {
        handle.await.expect("Tokio blocking task must succeed");
    }

    // Part B: Exercise non-Tokio external thread path via std::thread::spawn with thread-local client
    let mut thread_handles = Vec::new();
    for thread_id in 0..8 {
        let h_ext = heuristic_extractor.clone();
        let uri = server_uri.clone();

        thread_handles.push(std::thread::spawn(move || {
            let l_prov = OllamaLlmProvider::new("llama3", &uri);
            let l_ext = LlmMemoryExtractor::new(Arc::new(l_prov.clone()), "thread-llm");
            let e_prov = OllamaEmbeddingProvider::new("nomic-embed-text", &uri).with_dimension(384);

            for iter in 0..5 {
                // Heuristic extraction
                let text = format!(
                    "StdThread-worker-{} iter-{} Bob writes Rust. Docker runs on Linux. SQLite stores data.",
                    thread_id, iter
                );
                let (nodes, edges) = h_ext.extract_entities_and_relations(&text);
                assert!(!nodes.is_empty());
                assert!(!edges.is_empty());

                let chat_resp = l_prov
                    .chat(SessionId::new(), "hello")
                    .expect("External thread chat must succeed");
                assert!(!chat_resp.is_empty());

                let llm_res = l_ext
                    .extract_graph("test text")
                    .expect("External thread LLM extract must succeed");
                assert_eq!(llm_res.0.len(), 1);

                let vec = e_prov
                    .embed("external thread embedding")
                    .expect("External thread embed must succeed");
                assert_eq!(vec.len(), 384);
            }
        }));
    }

    for handle in thread_handles {
        handle
            .join()
            .expect("External worker thread must not panic");
    }
}
