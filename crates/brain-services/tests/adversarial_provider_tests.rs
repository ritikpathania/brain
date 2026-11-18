//! Adversarial stress test suite for native LLM providers, Embedding providers,
//! and `LlmMemoryExtractor`.
//!
//! Validates:
//! - HTTP 500, 502, 503, 504 status codes map to `BrainError::Model`.
//! - HTTP 401, 403 status codes map to `BrainError::Authorization`.
//! - HTTP 400, 422 status codes map to `BrainError::Validation`.
//! - Malformed JSON, empty bodies, and non-JSON payloads map to `BrainError::Validation`.
//! - Empty embedding arrays `[]`, empty vectors, and dimension mismatches map to `BrainError::Validation`.
//! - `LlmMemoryExtractor` gracefully handles empty output, non-JSON markdown, nested code fences,
//!   unicode, and maintains referential integrity.

use std::collections::HashMap;
use std::sync::Arc;

use brain_core::agents::ChatAgent;
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_core::services::{ExtractionRequest, MemoryExtractor};
use brain_domain::{KnowledgeGraph, NodeType, RelationKind, SessionId};
use brain_services::extraction::llm::LlmMemoryExtractor;
use brain_services::providers::embeddings::{
    OllamaEmbeddingProvider, OpenAiCompatibleEmbeddingProvider, OpenAiEmbeddingProvider,
};
use brain_services::providers::llm::{AnthropicLlmProvider, OllamaLlmProvider, OpenAiLlmProvider};

use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// 1. HTTP 5xx Server Error Probing (500, 502, 503, 504 -> BrainError::Model)
// ============================================================================

#[tokio::test]
async fn test_providers_http_5xx_mapping_to_model_error() {
    let server = MockServer::start().await;
    let status_codes = [500, 502, 503, 504];

    for code in status_codes {
        server.reset().await;

        // Mock endpoints with error code
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Ollama 5xx error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(code).set_body_string("OpenAI 5xx error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Anthropic 5xx error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Ollama embed 5xx error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(code).set_body_string("OpenAI embed 5xx error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Custom embed 5xx error"))
            .mount(&server)
            .await;

        // 1. Ollama LLM
        let ollama_llm = OllamaLlmProvider::new("llama3", server.uri());
        match ollama_llm.chat(SessionId::new(), "hi") {
            Err(BrainError::Model { message, .. }) => {
                assert!(
                    message.contains(&code.to_string()),
                    "Code {} not in message",
                    code
                );
            }
            other => panic!(
                "Ollama LLM on HTTP {}: expected Model, got {:?}",
                code, other
            ),
        }

        // 2. OpenAI LLM
        let openai_llm = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("sk-test")
            .with_base_url(server.uri());
        match openai_llm.chat(SessionId::new(), "hi") {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "OpenAI LLM on HTTP {}: expected Model, got {:?}",
                code, other
            ),
        }

        // 3. Anthropic LLM
        let anthropic_llm = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-test")
            .with_base_url(server.uri());
        match anthropic_llm.chat(SessionId::new(), "hi") {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Anthropic LLM on HTTP {}: expected Model, got {:?}",
                code, other
            ),
        }

        // 4. Ollama Embedding
        let ollama_embed = OllamaEmbeddingProvider::new("nomic-embed-text", server.uri());
        match ollama_embed.embed("text") {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Ollama Embed on HTTP {}: expected Model, got {:?}",
                code, other
            ),
        }

        // 5. OpenAI Embedding
        let openai_embed = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("sk-test")
            .with_base_url(server.uri());
        match openai_embed.embed("text") {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "OpenAI Embed on HTTP {}: expected Model, got {:?}",
                code, other
            ),
        }

        // 6. OpenAI-Compatible Embedding
        let custom_embed = OpenAiCompatibleEmbeddingProvider::new("custom", server.uri());
        match custom_embed.embed("text") {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Custom Embed on HTTP {}: expected Model, got {:?}",
                code, other
            ),
        }
    }
}

// ============================================================================
// 2. HTTP 401 & 403 Authorization Probing -> BrainError::Authorization
// ============================================================================

#[tokio::test]
async fn test_providers_http_401_403_mapping_to_authorization_error() {
    let server = MockServer::start().await;
    let auth_codes = [401, 403];

    for code in auth_codes {
        server.reset().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Auth error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Auth error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Auth error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Auth error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Auth error"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Auth error"))
            .mount(&server)
            .await;

        // Ollama LLM
        let ollama = OllamaLlmProvider::new("llama3", server.uri());
        match ollama.chat(SessionId::new(), "test") {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Ollama on {}: expected Authorization, got {:?}",
                code, other
            ),
        }

        // OpenAI LLM
        let openai = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("sk-key")
            .with_base_url(server.uri());
        match openai.chat(SessionId::new(), "test") {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "OpenAI on {}: expected Authorization, got {:?}",
                code, other
            ),
        }

        // Anthropic LLM
        let anthropic = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-key")
            .with_base_url(server.uri());
        match anthropic.chat(SessionId::new(), "test") {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Anthropic on {}: expected Authorization, got {:?}",
                code, other
            ),
        }

        // Ollama Embed
        let ollama_emb = OllamaEmbeddingProvider::new("nomic-embed-text", server.uri());
        match ollama_emb.embed("text") {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Ollama embed on {}: expected Authorization, got {:?}",
                code, other
            ),
        }

        // OpenAI Embed
        let openai_emb = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("sk-key")
            .with_base_url(server.uri());
        match openai_emb.embed("text") {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "OpenAI embed on {}: expected Authorization, got {:?}",
                code, other
            ),
        }

        // Custom Embed
        let custom_emb = OpenAiCompatibleEmbeddingProvider::new("custom", server.uri());
        match custom_emb.embed("text") {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Custom embed on {}: expected Authorization, got {:?}",
                code, other
            ),
        }
    }
}

// ============================================================================
// 3. HTTP 400 & 422 Probing -> BrainError::Validation
// ============================================================================

#[tokio::test]
async fn test_providers_http_400_422_mapping_to_validation_error() {
    let server = MockServer::start().await;
    let bad_request_codes = [400, 422];

    for code in bad_request_codes {
        server.reset().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Invalid param"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Unprocessable entity"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Bad request body"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Invalid model"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Unprocessable input"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(code).set_body_string("Validation failed"))
            .mount(&server)
            .await;

        let ollama = OllamaLlmProvider::new("llama3", server.uri());
        match ollama.chat(SessionId::new(), "test") {
            Err(BrainError::Validation { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!("Ollama on {}: expected Validation, got {:?}", code, other),
        }

        let openai = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("sk-key")
            .with_base_url(server.uri());
        match openai.chat(SessionId::new(), "test") {
            Err(BrainError::Validation { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!("OpenAI on {}: expected Validation, got {:?}", code, other),
        }

        let anthropic = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-key")
            .with_base_url(server.uri());
        match anthropic.chat(SessionId::new(), "test") {
            Err(BrainError::Validation { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Anthropic on {}: expected Validation, got {:?}",
                code, other
            ),
        }

        let ollama_emb = OllamaEmbeddingProvider::new("nomic-embed-text", server.uri());
        match ollama_emb.embed("text") {
            Err(BrainError::Validation { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Ollama embed on {}: expected Validation, got {:?}",
                code, other
            ),
        }

        let openai_emb = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("sk-key")
            .with_base_url(server.uri());
        match openai_emb.embed("text") {
            Err(BrainError::Validation { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "OpenAI embed on {}: expected Validation, got {:?}",
                code, other
            ),
        }

        let custom_emb = OpenAiCompatibleEmbeddingProvider::new("custom", server.uri());
        match custom_emb.embed("text") {
            Err(BrainError::Validation { message }) => {
                assert!(message.contains(&code.to_string()));
            }
            other => panic!(
                "Custom embed on {}: expected Validation, got {:?}",
                code, other
            ),
        }
    }
}

// ============================================================================
// 4. Malformed JSON, Truncated JSON, and HTML/Binary Bodies
// ============================================================================

#[tokio::test]
async fn test_providers_malformed_and_binary_bodies() {
    let server = MockServer::start().await;

    let malformed_bodies = [
        "",                                                               // Completely empty
        "   \n\t  ",                                                      // Whitespace only
        "<!DOCTYPE html><html><body>Error 502 Bad Gateway</body></html>", // Cloudflare / Nginx HTML
        "{ \"incomplete\": [ true, ",                                     // Truncated JSON
        "\u{001f}\u{008b}\u{0008}\u{0000}", // Binary / non-ASCII bytes
        "{\"unexpected\": 12345}",          // Valid JSON but wrong schema
    ];

    for body in malformed_bodies {
        server.reset().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let ollama = OllamaLlmProvider::new("llama3", server.uri());
        // For empty schema on Ollama, it has #[serde(default)] for response, so {"unexpected": 12345} parses response as ""
        let res = ollama.chat(SessionId::new(), "test");
        if body.contains("unexpected") {
            assert!(
                res.is_ok(),
                "Ollama defaults missing response to empty string"
            );
        } else {
            assert!(
                matches!(res, Err(BrainError::Validation { .. })),
                "Expected Validation error on body: {:?}",
                body
            );
        }

        let openai = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("sk-key")
            .with_base_url(server.uri());
        assert!(
            matches!(
                openai.chat(SessionId::new(), "test"),
                Err(BrainError::Validation { .. })
            ),
            "OpenAI expected Validation error on body: {:?}",
            body
        );

        let anthropic = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-key")
            .with_base_url(server.uri());
        assert!(
            matches!(
                anthropic.chat(SessionId::new(), "test"),
                Err(BrainError::Validation { .. })
            ),
            "Anthropic expected Validation error on body: {:?}",
            body
        );

        let ollama_emb = OllamaEmbeddingProvider::new("nomic-embed-text", server.uri());
        assert!(
            matches!(ollama_emb.embed("text"), Err(BrainError::Validation { .. })),
            "Ollama embed expected Validation error on body: {:?}",
            body
        );

        let openai_emb = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("sk-key")
            .with_base_url(server.uri());
        assert!(
            matches!(openai_emb.embed("text"), Err(BrainError::Validation { .. })),
            "OpenAI embed expected Validation error on body: {:?}",
            body
        );

        let custom_emb = OpenAiCompatibleEmbeddingProvider::new("custom", server.uri());
        assert!(
            matches!(custom_emb.embed("text"), Err(BrainError::Validation { .. })),
            "Custom embed expected Validation error on body: {:?}",
            body
        );
    }
}

// ============================================================================
// 5. Embedding Edge Cases: Empty Arrays, Zero Vectors, Dimension Mismatches
// ============================================================================

#[tokio::test]
async fn test_embedding_edge_cases_empty_arrays_and_dimension_mismatches() {
    let server = MockServer::start().await;

    // Case 5.1: Ollama returns empty embeddings array `{"embeddings": []}`
    Mock::given(method("POST"))
        .and(path("/api/embed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "nomic-embed-text",
            "embeddings": []
        })))
        .mount(&server)
        .await;

    let ollama_emb = OllamaEmbeddingProvider::new("nomic-embed-text", server.uri());
    match ollama_emb.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("empty embeddings array"));
        }
        other => panic!("Expected empty array Validation error, got {:?}", other),
    }

    // Case 5.2: Ollama returns array with an empty vector `{"embeddings": [[]]}`
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/api/embed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "nomic-embed-text",
            "embeddings": [[]]
        })))
        .mount(&server)
        .await;

    match ollama_emb.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("empty embedding vector"));
        }
        other => panic!("Expected empty vector Validation error, got {:?}", other),
    }

    // Case 5.3: Ollama legacy fallback returns empty vector `{"embedding": []}`
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/api/embed"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "embedding": []
        })))
        .mount(&server)
        .await;

    match ollama_emb.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("empty embedding vector"));
        }
        other => panic!(
            "Expected legacy empty vector Validation error, got {:?}",
            other
        ),
    }

    // Case 5.4: OpenAI returns empty data array `{"data": []}`
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": []
        })))
        .mount(&server)
        .await;

    let openai_emb = OpenAiEmbeddingProvider::new("text-embedding-3-small")
        .with_api_key("sk-test")
        .with_base_url(server.uri());
    match openai_emb.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("empty data array"));
        }
        other => panic!("Expected OpenAI empty data array error, got {:?}", other),
    }

    // Case 5.5: OpenAI returns array with empty embedding vector `{"data": [{"embedding": []}]}`
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "object": "list",
            "data": [{"embedding": []}]
        })))
        .mount(&server)
        .await;

    match openai_emb.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("empty embedding vector"));
        }
        other => panic!("Expected OpenAI empty vector error, got {:?}", other),
    }

    // Case 5.6: OpenAI-Compatible returns empty data array
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .mount(&server)
        .await;

    let custom_emb = OpenAiCompatibleEmbeddingProvider::new("custom", server.uri());
    match custom_emb.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("empty data array"));
        }
        other => panic!("Expected Custom empty data array error, got {:?}", other),
    }

    // Case 5.7: OpenAI-Compatible returns empty vector
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"embedding": []}]
        })))
        .mount(&server)
        .await;

    match custom_emb.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("empty embedding vector"));
        }
        other => panic!("Expected Custom empty vector error, got {:?}", other),
    }

    // Case 5.8: Huge vector dimension mismatch (e.g. 4096 dims returned when 384 expected)
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/api/embed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "embeddings": [vec![0.1f32; 4096]]
        })))
        .mount(&server)
        .await;

    let ollama_emb_384 =
        OllamaEmbeddingProvider::new("nomic-embed-text", server.uri()).with_dimension(384);
    match ollama_emb_384.embed("text") {
        Err(BrainError::Validation { message }) => {
            assert!(message.contains("4096") && message.contains("384"));
        }
        other => panic!("Expected dimension mismatch error, got {:?}", other),
    }
}

// ============================================================================
// 6. LlmMemoryExtractor Adversarial Probing: Nested Fences, Non-JSON, Unicode, Invariants
// ============================================================================

type ChatResponseFn = Box<dyn Fn(&str) -> Result<String, BrainError> + Send + Sync>;

struct DynamicMockChat {
    response_fn: ChatResponseFn,
}

impl DynamicMockChat {
    fn new<F>(f: F) -> Self
    where
        F: Fn(&str) -> Result<String, BrainError> + Send + Sync + 'static,
    {
        Self {
            response_fn: Box::new(f),
        }
    }
}

impl ChatAgent for DynamicMockChat {
    fn name(&self) -> &str {
        "DynamicMockChat"
    }

    fn chat(&self, _session_id: SessionId, prompt: &str) -> Result<String, BrainError> {
        (self.response_fn)(prompt)
    }
}

#[test]
fn test_extractor_adversarial_strip_code_fences_and_empty() {
    // 1. Completely empty and whitespace strings
    assert_eq!(LlmMemoryExtractor::strip_code_fences(""), "");
    assert_eq!(LlmMemoryExtractor::strip_code_fences("    "), "");
    assert_eq!(LlmMemoryExtractor::strip_code_fences("\n\t  \n"), "");

    // 2. Non-JSON markdown with headers, code blocks, lists
    let md = "# Title\n\nSome explanation here.\n\n- bullet 1\n- bullet 2";
    assert_eq!(LlmMemoryExtractor::strip_code_fences(md), md);

    // 3. Nested code fences: markdown containing embedded code fences inside JSON attributes
    let nested = r#"```json
    {
      "nodes": [
        {
          "id": "code_snippet",
          "label": "Code",
          "type": "concept",
          "attributes": {
            "snippet": "```rust\nfn main() {}\n```"
          }
        }
      ],
      "edges": []
    }
    ```"#;

    let cleaned = LlmMemoryExtractor::strip_code_fences(nested);
    assert!(cleaned.starts_with('{'));
    assert!(cleaned.ends_with('}'));

    let (nodes, edges) = LlmMemoryExtractor::parse_llm_json(nested)
        .expect("Should cleanly parse JSON with internal code fences");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, "code_snippet");
    assert_eq!(edges.len(), 0);

    // 4. Multiple backtick lines and trailing chatter
    let chatter = "Sure, here is your graph:\n```json\n{\"nodes\": [], \"edges\": []}\n```\nLet me know if you need more changes!";
    let cleaned_chatter = LlmMemoryExtractor::strip_code_fences(chatter);
    assert_eq!(cleaned_chatter, "{\"nodes\": [], \"edges\": []}");

    // 5. Code fences with uppercase JSON and weird spacing
    let upper = "```JSON   \n  {\"nodes\": [], \"edges\": []}   \n  ```";
    assert_eq!(
        LlmMemoryExtractor::strip_code_fences(upper),
        "{\"nodes\": [], \"edges\": []}"
    );

    // 6. Generic code fence without language tag
    let generic = "```\n{\"nodes\": [], \"edges\": []}\n```";
    assert_eq!(
        LlmMemoryExtractor::strip_code_fences(generic),
        "{\"nodes\": [], \"edges\": []}"
    );

    // 7. Unclosed code fence
    let unclosed = "```json\n{\"nodes\": [], \"edges\": []}";
    let cleaned_unclosed = LlmMemoryExtractor::strip_code_fences(unclosed);
    assert_eq!(cleaned_unclosed, "{\"nodes\": [], \"edges\": []}");
}

#[test]
fn test_extractor_adversarial_graph_parsing_edge_cases() {
    // 1. Empty string throws Validation error
    let res = LlmMemoryExtractor::parse_llm_json("");
    assert!(matches!(res, Err(BrainError::Validation { .. })));

    // 2. Pure prose without JSON throws Validation error
    let res2 = LlmMemoryExtractor::parse_llm_json("I cannot fulfill this request.");
    assert!(matches!(res2, Err(BrainError::Validation { .. })));

    // 3. Truncated JSON throws Validation error
    let res3 = LlmMemoryExtractor::parse_llm_json("{\"nodes\": [{\"id\": \"test\"");
    assert!(matches!(res3, Err(BrainError::Validation { .. })));

    // 4. Missing fields in JSON (empty object `{}`) should parse with empty defaults
    let (nodes, edges) =
        LlmMemoryExtractor::parse_llm_json("{}").expect("Empty object defaults to empty vecs");
    assert_eq!(nodes.len(), 0);
    assert_eq!(edges.len(), 0);

    // 5. Unknown node types safely map to NodeType::Concept
    let custom_types_payload = r#"{
      "nodes": [
        {"id": "alien1", "label": "Alien Entity", "type": "extraterrestrial"},
        {"id": "unknown2", "label": "Unknown Item", "type": "nonexistent_type"}
      ],
      "edges": []
    }"#;

    let mock_chat = Arc::new(DynamicMockChat::new(move |_| {
        Ok(custom_types_payload.to_string())
    }));
    let extractor = LlmMemoryExtractor::new(mock_chat, "test-extractor");
    let req = ExtractionRequest {
        raw_content: "some input".to_string(),
        context_metadata: HashMap::new(),
    };
    let result = extractor.extract(req).expect("Extraction succeeds");
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.nodes[0].node_type, NodeType::Concept);
    assert_eq!(result.nodes[1].node_type, NodeType::Concept);

    // 6. Unknown relation kinds safely map to RelationKind::AssociatedWith
    let custom_relation_payload = r#"{
      "nodes": [
        {"id": "n1", "label": "Node 1", "type": "concept"},
        {"id": "n2", "label": "Node 2", "type": "concept"}
      ],
      "edges": [
        {"source": "n1", "target": "n2", "relation": "telepathically_linked"}
      ]
    }"#;
    let mock_chat2 = Arc::new(DynamicMockChat::new(move |_| {
        Ok(custom_relation_payload.to_string())
    }));
    let extractor2 = LlmMemoryExtractor::new(mock_chat2, "test-extractor");
    let req2 = ExtractionRequest {
        raw_content: "some input".to_string(),
        context_metadata: HashMap::new(),
    };
    let result2 = extractor2.extract(req2).expect("Extraction succeeds");
    assert_eq!(result2.edges.len(), 1);
    assert_eq!(result2.edges[0].relation, RelationKind::Unknown);
}

#[test]
fn test_extractor_referential_integrity_filtering() {
    // LLM generates edges that reference nonexistent source or target node IDs
    let dangling_edge_payload = r#"{
      "nodes": [
        {"id": "valid_node_1", "label": "Valid Node", "type": "concept"}
      ],
      "edges": [
        {"source": "valid_node_1", "target": "ghost_node_2", "relation": "connects_to"},
        {"source": "ghost_node_3", "target": "valid_node_1", "relation": "depends_on"},
        {"source": "ghost_node_4", "target": "ghost_node_5", "relation": "relates"}
      ]
    }"#;

    let mock_chat = Arc::new(DynamicMockChat::new(move |_| {
        Ok(dangling_edge_payload.to_string())
    }));
    let extractor = LlmMemoryExtractor::new(mock_chat, "test-extractor");
    let req = ExtractionRequest {
        raw_content: "some input".to_string(),
        context_metadata: HashMap::new(),
    };
    let result = extractor.extract(req).expect("Extraction succeeds");
    assert_eq!(result.nodes.len(), 1);
    // All dangling edges must be safely filtered to prevent KnowledgeGraph referential integrity panics/errors!
    assert_eq!(result.edges.len(), 0, "Dangling edges must be filtered out");

    // Adding to KnowledgeGraph must succeed without error
    let mut kg = KnowledgeGraph::new();
    for node in result.nodes {
        kg.add_node(node);
    }
    for edge in result.edges {
        kg.add_edge(edge)
            .expect("All remaining edges must be valid");
    }
    assert_eq!(kg.nodes.len(), 1);
    assert_eq!(kg.edges.len(), 0);
}

#[test]
fn test_extractor_unicode_emoji_and_special_characters() {
    let unicode_payload = r#"{
      "nodes": [
        {"id": "node_🦀", "label": "🦀 Rust Lang 🚀", "type": "language", "attributes": {"desc": "日本語とالعربية"}},
        {"id": "node_db", "label": "PostgreSQL 🐘", "type": "database", "attributes": {"unicode_key": "Ünïcödë \\u00A9 😀"}}
      ],
      "edges": [
        {"source": "node_🦀", "target": "node_db", "relation": "connects_to", "weight": 0.95}
      ]
    }"#;

    let mock_chat = Arc::new(DynamicMockChat::new(move |_| {
        Ok(unicode_payload.to_string())
    }));
    let extractor = LlmMemoryExtractor::new(mock_chat, "test-extractor");
    let req = ExtractionRequest {
        raw_content: "Rust and Postgres with unicode".to_string(),
        context_metadata: HashMap::new(),
    };

    let result = extractor.extract(req).expect("Unicode extraction succeeds");
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.edges.len(), 1);

    let mut kg = KnowledgeGraph::new();
    for node in result.nodes {
        kg.add_node(node);
    }
    for edge in result.edges {
        kg.add_edge(edge)
            .expect("Unicode edge referential integrity passes");
    }
    assert_eq!(kg.nodes.len(), 2);
    assert_eq!(kg.edges.len(), 1);
}

#[test]
fn test_extractor_extract_semantic_nodes_json_ffi_failures() {
    // 1. Invalid input JSON to extract_semantic_nodes_json
    let mock_chat = Arc::new(DynamicMockChat::new(|_| Ok("{}".to_string())));
    let extractor = LlmMemoryExtractor::new(mock_chat, "test-extractor");

    let ffi_res = extractor.extract_semantic_nodes_json("{ broken input json [");
    assert!(ffi_res.is_ok());
    let val: serde_json::Value = serde_json::from_str(&ffi_res.unwrap()).unwrap();
    assert_eq!(val["status"], "error");
    assert!(val["message"]
        .as_str()
        .unwrap()
        .contains("Failed to parse input STM nodes"));

    // 2. Upstream LLM failure
    let failing_chat = Arc::new(DynamicMockChat::new(|_| {
        Err(BrainError::Model {
            model_id: "test".to_string(),
            message: "Model quota exhausted".to_string(),
        })
    }));
    let extractor2 = LlmMemoryExtractor::new(failing_chat, "test-extractor");
    let ffi_res2 = extractor2.extract_semantic_nodes_json("[]");
    assert!(ffi_res2.is_ok());
    let val2: serde_json::Value = serde_json::from_str(&ffi_res2.unwrap()).unwrap();
    assert_eq!(val2["status"], "error");
    assert!(val2["message"]
        .as_str()
        .unwrap()
        .contains("LLM generation failed"));
}
