//! Offline integration test suite for native LLM providers, Embedding providers,
//! and `LlmMemoryExtractor` using `wiremock`.
//!
//! Enforces:
//! - 100% offline execution with zero external network access or real credentials.
//! - Typed `BrainError` propagation (Authorization, Model, Timeout, Validation).
//! - Zero dummy fallback vectors (`[0.0; 384]`) on HTTP or authentication failures.
//! - Markdown code fence stripping and referential integrity ingestion into `KnowledgeGraph`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use brain_core::agents::{ChatAgent, EmbeddingAgent, ExtractionAgent};
use brain_core::errors::BrainError;
use brain_core::retrieval::EmbeddingProvider;
use brain_core::services::{ExtractionRequest, MemoryExtractor};
use brain_domain::{KnowledgeGraph, SessionId};
use brain_services::extraction::llm::LlmMemoryExtractor;
use brain_services::providers::embeddings::{
    OllamaEmbeddingProvider, OpenAiCompatibleEmbeddingProvider, OpenAiEmbeddingProvider,
};
use brain_services::providers::llm::{AnthropicLlmProvider, OllamaLlmProvider, OpenAiLlmProvider};

use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ============================================================================
// 1. LLM Providers Test Suite (Ollama, OpenAI, Anthropic)
// ============================================================================

mod llm_provider_tests {
    use super::*;

    // ------------------------------------------------------------------------
    // OllamaLlmProvider Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_ollama_llm_200_ok() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "model": "llama3",
                "response": "Hello from Ollama!",
                "done": true
            })))
            .mount(&server)
            .await;

        let provider = OllamaLlmProvider::new("llama3", server.uri());
        let response = provider
            .chat(SessionId::new(), "Say hello")
            .expect("Chat should succeed");

        assert_eq!(response, "Hello from Ollama!");
        assert_eq!(provider.name(), "ollama-llama3");
    }

    #[tokio::test]
    async fn test_ollama_llm_401_unauthorized() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized access"))
            .mount(&server)
            .await;

        let provider = OllamaLlmProvider::new("llama3", server.uri());
        let result = provider.chat(SessionId::new(), "Say hello");

        match result {
            Err(BrainError::Authorization { message }) => {
                assert!(
                    message.contains("401") || message.to_lowercase().contains("authorization")
                );
            }
            other => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_ollama_llm_500_internal_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal server failure"))
            .mount(&server)
            .await;

        let provider = OllamaLlmProvider::new("llama3", server.uri());
        let result = provider.chat(SessionId::new(), "Say hello");

        match result {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains("500") || message.contains("server error"));
            }
            Err(BrainError::Internal { .. }) => {}
            other => panic!("Expected BrainError::Model or Internal, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_ollama_llm_timeout() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(200))
                    .set_body_json(json!({"response": "Delayed response"})),
            )
            .mount(&server)
            .await;

        let provider =
            OllamaLlmProvider::new("llama3", server.uri()).with_timeout(Duration::from_millis(50));
        let result = provider.chat(SessionId::new(), "Say hello");

        match result {
            Err(BrainError::Timeout { elapsed_ms, .. }) => {
                assert!(elapsed_ms >= 50);
            }
            Err(BrainError::Network { message, .. }) => {
                assert!(
                    message.to_lowercase().contains("timeout") || message.contains("timed out")
                );
            }
            other => panic!("Expected BrainError::Timeout, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_ollama_llm_malformed_json() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{ broken: json ["))
            .mount(&server)
            .await;

        let provider = OllamaLlmProvider::new("llama3", server.uri());
        let result = provider.chat(SessionId::new(), "Say hello");

        match result {
            Err(BrainError::Validation { .. }) | Err(BrainError::Model { .. }) => {}
            other => panic!(
                "Expected Validation or Model error on malformed JSON, got {:?}",
                other
            ),
        }
    }

    // ------------------------------------------------------------------------
    // OpenAiLlmProvider Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_openai_llm_200_ok() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "created": 1677652288,
                "model": "gpt-4o-mini",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello from OpenAI!"
                    },
                    "finish_reason": "stop"
                }]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("test-key")
            .with_base_url(server.uri());

        let response = provider
            .chat(SessionId::new(), "Hello")
            .expect("Chat should succeed");

        assert_eq!(response, "Hello from OpenAI!");
        assert_eq!(provider.name(), "openai-gpt-4o-mini");
    }

    #[tokio::test]
    async fn test_openai_llm_401_unauthorized() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {
                    "message": "Incorrect API key provided",
                    "type": "invalid_request_error"
                }
            })))
            .mount(&server)
            .await;

        let provider = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("bad-key")
            .with_base_url(server.uri());

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Authorization { message }) => {
                assert!(
                    message.contains("401") || message.to_lowercase().contains("authorization")
                );
            }
            other => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_openai_llm_500_server_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal server error"))
            .mount(&server)
            .await;

        let provider = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("test-key")
            .with_base_url(server.uri());

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains("500") || message.contains("server error"));
            }
            Err(BrainError::Internal { .. }) => {}
            other => panic!("Expected BrainError::Model or Internal, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_openai_llm_timeout() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(200))
                    .set_body_json(json!({
                        "choices": [{"message": {"content": "Slow reply"}}]
                    })),
            )
            .mount(&server)
            .await;

        let provider = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("test-key")
            .with_base_url(server.uri())
            .with_timeout(Duration::from_millis(50));

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Timeout { .. }) | Err(BrainError::Network { .. }) => {}
            other => panic!("Expected Timeout/Network error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_openai_llm_malformed_json() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [] // Missing required message structure
            })))
            .mount(&server)
            .await;

        let provider = OpenAiLlmProvider::new("gpt-4o-mini")
            .with_api_key("test-key")
            .with_base_url(server.uri());

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Validation { .. }) | Err(BrainError::Model { .. }) => {}
            other => panic!("Expected Validation/Model error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_openai_llm_missing_api_key() {
        let provider = OpenAiLlmProvider::new("gpt-4o-mini").with_api_key("");
        let result = provider.chat(SessionId::new(), "Hello");
        match result {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains("OPENAI_API_KEY"));
            }
            other => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // AnthropicLlmProvider Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_anthropic_llm_200_ok() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "ant-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "msg_01X9",
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "text",
                    "text": "Hello from Claude!"
                }],
                "model": "claude-3-5-sonnet-20240620"
            })))
            .mount(&server)
            .await;

        let provider = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-key")
            .with_base_url(server.uri());

        let response = provider
            .chat(SessionId::new(), "Hello")
            .expect("Chat should succeed");

        assert_eq!(response, "Hello from Claude!");
        assert_eq!(provider.name(), "anthropic-claude-3-5-sonnet-20240620");
    }

    #[tokio::test]
    async fn test_anthropic_llm_401_unauthorized() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "type": "error",
                "error": {
                    "type": "authentication_error",
                    "message": "invalid x-api-key"
                }
            })))
            .mount(&server)
            .await;

        let provider = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("bad-key")
            .with_base_url(server.uri());

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Authorization { message }) => {
                assert!(
                    message.contains("401") || message.to_lowercase().contains("authorization")
                );
            }
            other => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_anthropic_llm_500_server_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "type": "error",
                "error": {
                    "type": "api_error",
                    "message": "Internal server error"
                }
            })))
            .mount(&server)
            .await;

        let provider = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-key")
            .with_base_url(server.uri());

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Model { message, .. }) => {
                assert!(message.contains("500") || message.contains("server error"));
            }
            Err(BrainError::Internal { .. }) => {}
            other => panic!("Expected Model or Internal error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_anthropic_llm_timeout() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(200))
                    .set_body_json(json!({
                        "content": [{"type": "text", "text": "Delayed response"}]
                    })),
            )
            .mount(&server)
            .await;

        let provider = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-key")
            .with_base_url(server.uri())
            .with_timeout(Duration::from_millis(50));

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Timeout { .. }) | Err(BrainError::Network { .. }) => {}
            other => panic!("Expected Timeout/Network error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_anthropic_llm_malformed_json() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Not JSON at all"))
            .mount(&server)
            .await;

        let provider = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620")
            .with_api_key("ant-key")
            .with_base_url(server.uri());

        let result = provider.chat(SessionId::new(), "Hello");

        match result {
            Err(BrainError::Validation { .. }) | Err(BrainError::Model { .. }) => {}
            other => panic!(
                "Expected Validation/Model error on malformed JSON, got {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_anthropic_llm_missing_api_key() {
        let provider = AnthropicLlmProvider::new("claude-3-5-sonnet-20240620").with_api_key("");
        let result = provider.chat(SessionId::new(), "Hello");
        match result {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains("ANTHROPIC_API_KEY"));
            }
            other => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }
}

// ============================================================================
// 2. Embedding Providers Test Suite (NO [0.0; 384] DUMMY FALLBACK VECTORS)
// ============================================================================

mod embedding_provider_tests {
    use super::*;

    // ------------------------------------------------------------------------
    // OllamaEmbeddingProvider Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_ollama_embedding_200_ok_primary_endpoint() {
        let server = MockServer::start().await;
        let mock_vec = vec![0.25f32; 384];

        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "model": "nomic-embed-text",
                "embeddings": [mock_vec]
            })))
            .mount(&server)
            .await;

        let provider =
            OllamaEmbeddingProvider::new("nomic-embed-text", server.uri()).with_dimension(384);

        let vec = provider
            .embed("sample document")
            .expect("Embedding should succeed");
        assert_eq!(vec.len(), 384);
        assert!((vec[0] - 0.25).abs() < 1e-6);

        // Verify EmbeddingAgent trait
        assert_eq!(provider.dimension(), 384);
        let agent_vec = provider
            .embed_text("sample document")
            .expect("embed_text succeeds");
        assert_eq!(agent_vec.len(), 384);
    }

    #[tokio::test]
    async fn test_ollama_embedding_200_ok_fallback_endpoint() {
        let server = MockServer::start().await;
        let mock_vec = vec![0.5f32; 384];

        // Primary /api/embed returns 404
        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        // Fallback /api/embeddings returns 200
        Mock::given(method("POST"))
            .and(path("/api/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embedding": mock_vec
            })))
            .mount(&server)
            .await;

        let provider =
            OllamaEmbeddingProvider::new("nomic-embed-text", server.uri()).with_dimension(384);

        let vec = provider
            .embed("sample document")
            .expect("Fallback should succeed");
        assert_eq!(vec.len(), 384);
        assert!((vec[0] - 0.5).abs() < 1e-6);
    }

    #[tokio::test]
    async fn test_ollama_embedding_error_returns_typed_err_no_dummy_fallback() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Server failure"))
            .mount(&server)
            .await;

        let provider = OllamaEmbeddingProvider::new("nomic-embed-text", server.uri());
        let result = provider.embed("sample document");

        // CRITICAL INVARIANT: Never return Ok(vec![0.0; 384])
        assert!(
            result.is_err(),
            "Must return typed Err on failure, NOT dummy fallback!"
        );
        if let Ok(vec) = result {
            panic!("Forbidden dummy vector returned: {:?}", vec);
        }
    }

    #[tokio::test]
    async fn test_ollama_embedding_dimension_validation() {
        let server = MockServer::start().await;

        // Returns 5 dimensions when expected is 384
        Mock::given(method("POST"))
            .and(path("/api/embed"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "embeddings": [[0.1, 0.2, 0.3, 0.4, 0.5]]
            })))
            .mount(&server)
            .await;

        let provider =
            OllamaEmbeddingProvider::new("nomic-embed-text", server.uri()).with_dimension(384);

        let result = provider.embed("sample document");
        match result {
            Err(BrainError::Validation { message }) => {
                assert!(
                    message.contains("dimension")
                        || message.contains("384")
                        || message.contains("5")
                );
            }
            Ok(vec) => panic!(
                "Expected dimension validation error, got vector with len {}",
                vec.len()
            ),
            Err(other) => panic!("Expected BrainError::Validation, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // OpenAiEmbeddingProvider Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_openai_embedding_200_ok() {
        let server = MockServer::start().await;
        let mock_vec = vec![0.123f32; 1536];

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(header("Authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "object": "list",
                "data": [{
                    "object": "embedding",
                    "embedding": mock_vec,
                    "index": 0
                }],
                "model": "text-embedding-3-small",
                "usage": {"prompt_tokens": 5, "total_tokens": 5}
            })))
            .mount(&server)
            .await;

        let provider = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("test-key")
            .with_base_url(server.uri())
            .with_dimension(1536);

        let vec = provider
            .embed("sample text")
            .expect("Embedding should succeed");
        assert_eq!(vec.len(), 1536);
        assert_eq!(provider.dimension(), 1536);
    }

    #[tokio::test]
    async fn test_openai_embedding_401_returns_typed_err_no_dummy_fallback() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {"message": "Invalid API key"}
            })))
            .mount(&server)
            .await;

        let provider = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("bad-key")
            .with_base_url(server.uri());

        let result = provider.embed("sample text");

        // CRITICAL INVARIANT: Typed Authorization error, NEVER Ok([0.0; 384])
        match result {
            Err(BrainError::Authorization { message }) => {
                assert!(
                    message.contains("401") || message.to_lowercase().contains("authorization")
                );
            }
            Ok(vec) => panic!(
                "Forbidden dummy vector [0.0; 384] returned on 401: {:?}",
                vec
            ),
            Err(other) => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_openai_embedding_500_server_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal server error"))
            .mount(&server)
            .await;

        let provider = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("test-key")
            .with_base_url(server.uri());

        let result = provider.embed("sample text");
        assert!(result.is_err(), "Must fail on 500 server error");
        match result {
            Err(BrainError::Model { .. }) | Err(BrainError::Internal { .. }) => {}
            other => panic!("Expected Model/Internal error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_openai_embedding_dimension_validation() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "embedding": vec![0.1f32; 512], // Mismatch: 512 vs expected 1536
                    "index": 0
                }]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiEmbeddingProvider::new("text-embedding-3-small")
            .with_api_key("test-key")
            .with_base_url(server.uri())
            .with_dimension(1536);

        let result = provider.embed("sample text");
        match result {
            Err(BrainError::Validation { message }) => {
                assert!(
                    message.contains("dimension")
                        || message.contains("1536")
                        || message.contains("512")
                );
            }
            Ok(vec) => panic!(
                "Expected dimension validation error, got vector len {}",
                vec.len()
            ),
            Err(other) => panic!("Expected Validation error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_openai_embedding_missing_api_key() {
        let provider = OpenAiEmbeddingProvider::new("text-embedding-3-small").with_api_key("");
        let result = provider.embed("sample text");
        match result {
            Err(BrainError::Authorization { message }) => {
                assert!(message.contains("OPENAI_API_KEY"));
            }
            other => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }

    // ------------------------------------------------------------------------
    // OpenAiCompatibleEmbeddingProvider Tests
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn test_custom_embedding_200_ok() {
        let server = MockServer::start().await;
        let mock_vec = vec![0.42f32; 384];

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .and(header("Authorization", "Bearer custom-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "embedding": mock_vec,
                    "index": 0
                }]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatibleEmbeddingProvider::new("custom-model", server.uri())
            .with_api_key("custom-key")
            .with_dimension(384);

        let vec = provider.embed("custom text").expect("Embedding succeeds");
        assert_eq!(vec.len(), 384);
        assert_eq!(provider.dimension(), 384);
    }

    #[tokio::test]
    async fn test_custom_embedding_401_returns_typed_err_no_dummy_fallback() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Invalid token"))
            .mount(&server)
            .await;

        let provider = OpenAiCompatibleEmbeddingProvider::new("custom-model", server.uri())
            .with_dimension(384);

        let result = provider.embed("custom text");

        // CRITICAL INVARIANT: Typed Authorization error, NEVER Ok([0.0; 384])
        match result {
            Err(BrainError::Authorization { .. }) => {}
            Ok(vec) => panic!(
                "Forbidden dummy vector [0.0; 384] returned on 401: {:?}",
                vec
            ),
            Err(other) => panic!("Expected BrainError::Authorization, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_custom_embedding_500_server_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Service unavailable"))
            .mount(&server)
            .await;

        let provider = OpenAiCompatibleEmbeddingProvider::new("custom-model", server.uri());
        let result = provider.embed("custom text");

        assert!(result.is_err());
        match result {
            Err(BrainError::Model { .. }) | Err(BrainError::Internal { .. }) => {}
            other => panic!("Expected Model/Internal error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_custom_embedding_dimension_validation() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "embedding": vec![0.1f32; 100], // Mismatch: 100 vs expected 384
                    "index": 0
                }]
            })))
            .mount(&server)
            .await;

        let provider = OpenAiCompatibleEmbeddingProvider::new("custom-model", server.uri())
            .with_dimension(384);

        let result = provider.embed("custom text");
        match result {
            Err(BrainError::Validation { message }) => {
                assert!(
                    message.contains("dimension")
                        || message.contains("384")
                        || message.contains("100")
                );
            }
            Ok(vec) => panic!("Expected validation error, got vector len {}", vec.len()),
            Err(other) => panic!("Expected Validation error, got {:?}", other),
        }
    }
}

// ============================================================================
// 3. LlmMemoryExtractor & KnowledgeGraph Ingestion Test Suite
// ============================================================================

mod llm_memory_extractor_tests {
    use super::*;

    struct MockChatEngine {
        canned_response: Result<String, BrainError>,
    }

    impl MockChatEngine {
        fn new(response: &str) -> Self {
            Self {
                canned_response: Ok(response.to_string()),
            }
        }
        fn failing(err: BrainError) -> Self {
            Self {
                canned_response: Err(err),
            }
        }
    }

    impl ChatAgent for MockChatEngine {
        fn name(&self) -> &str {
            "MockChatEngine"
        }
        fn chat(&self, _session_id: SessionId, _prompt: &str) -> Result<String, BrainError> {
            match &self.canned_response {
                Ok(resp) => Ok(resp.clone()),
                Err(e) => Err(BrainError::Internal {
                    message: format!("Chat failed: {}", e),
                }),
            }
        }
    }

    #[test]
    fn test_markdown_code_fence_stripping_variants() {
        // Variant 1: standard ```json ... ```
        let raw1 = "```json\n{\"nodes\": [], \"edges\": []}\n```";
        let cleaned1 = LlmMemoryExtractor::strip_code_fences(raw1);
        assert_eq!(cleaned1, "{\"nodes\": [], \"edges\": []}");

        // Variant 2: uppercase ```JSON ... ```
        let raw2 = "```JSON\n{\"nodes\": [], \"edges\": []}\n```";
        let cleaned2 = LlmMemoryExtractor::strip_code_fences(raw2);
        assert_eq!(cleaned2, "{\"nodes\": [], \"edges\": []}");

        // Variant 3: generic ``` ... ```
        let raw3 = "```\n{\"nodes\": [], \"edges\": []}\n```";
        let cleaned3 = LlmMemoryExtractor::strip_code_fences(raw3);
        assert_eq!(cleaned3, "{\"nodes\": [], \"edges\": []}");

        // Variant 4: Conversational preamble & postscript
        let raw4 = "Here is the knowledge graph you requested:\n```json\n{\"nodes\": [], \"edges\": []}\n```\nHope that helps!";
        let cleaned4 = LlmMemoryExtractor::strip_code_fences(raw4);
        assert_eq!(cleaned4, "{\"nodes\": [], \"edges\": []}");

        // Variant 5: Raw JSON without fences
        let raw5 = "{\"nodes\": [], \"edges\": []}";
        let cleaned5 = LlmMemoryExtractor::strip_code_fences(raw5);
        assert_eq!(cleaned5, "{\"nodes\": [], \"edges\": []}");
    }

    #[test]
    fn test_extractor_parses_valid_json_into_dtos() {
        let json_payload = r#"```json
        {
          "nodes": [
            {
              "id": "sqlite",
              "label": "SQLite Database",
              "type": "database",
              "attributes": {"port": 0}
            },
            {
              "id": "backend",
              "label": "Rust Backend",
              "type": "service",
              "attributes": {}
            }
          ],
          "edges": [
            {
              "source": "backend",
              "target": "sqlite",
              "relation": "stored_in",
              "weight": 0.8
            }
          ]
        }
        ```"#;

        let (nodes, edges) = LlmMemoryExtractor::parse_llm_json(json_payload)
            .expect("Should parse valid JSON payload");

        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].id, "sqlite");
        assert_eq!(nodes[0].label, "SQLite Database");
        assert_eq!(nodes[0].node_type, "database");

        assert_eq!(nodes[1].id, "backend");
        assert_eq!(nodes[1].label, "Rust Backend");
        assert_eq!(nodes[1].node_type, "service");

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].source, "backend");
        assert_eq!(edges[0].target, "sqlite");
        assert_eq!(edges[0].relation, "stored_in");
        assert_eq!(edges[0].weight, 0.8);
    }

    #[test]
    fn test_extractor_ingestion_into_domain_knowledge_graph() {
        let json_payload = r#"{
          "nodes": [
            {"id": "rust", "label": "Rust", "type": "language"},
            {"id": "tokio", "label": "Tokio Runtime", "type": "library"}
          ],
          "edges": [
            {"source": "tokio", "target": "rust", "relation": "depends_on", "weight": 1.0}
          ]
        }"#;

        let mock_chat = Arc::new(MockChatEngine::new(json_payload));
        let extractor = LlmMemoryExtractor::new(mock_chat, "test-llm-extractor");

        let mut context_meta = HashMap::new();
        context_meta.insert("conversation_id".to_string(), "conv-123".to_string());
        context_meta.insert("message_id".to_string(), "msg-456".to_string());

        let req = ExtractionRequest {
            raw_content: "Tokio is an async runtime in Rust".to_string(),
            context_metadata: context_meta,
        };

        let result = extractor.extract(req).expect("Extraction must succeed");

        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.edges.len(), 1);

        // Verify referential integrity and ingestion into brain_domain::KnowledgeGraph
        let mut kg = KnowledgeGraph::new();

        for node in result.nodes {
            assert!(node.provenance.source_conversation.as_deref() == Some("conv-123"));
            assert!(node.provenance.source_message.as_deref() == Some("msg-456"));
            kg.add_node(node);
        }

        assert_eq!(kg.nodes.len(), 2);

        for edge in result.edges {
            assert!(edge.provenance.source_conversation.as_deref() == Some("conv-123"));
            // add_edge verifies that edge.source and edge.target both exist in kg.nodes
            let add_res = kg.add_edge(edge);
            assert!(
                add_res.is_ok(),
                "Referential integrity must hold for extracted edges"
            );
        }

        assert_eq!(kg.edges.len(), 1);
    }

    #[test]
    fn test_extractor_malformed_json_error_propagation() {
        let broken_payload = "```json\n{ \"nodes\": [ broken json syntax ...\n```";

        let mock_chat = Arc::new(MockChatEngine::new(broken_payload));
        let extractor = LlmMemoryExtractor::new(mock_chat, "test-llm-extractor");

        let req = ExtractionRequest {
            raw_content: "Some raw content".to_string(),
            context_metadata: HashMap::new(),
        };

        let result = extractor.extract(req);

        match result {
            Err(BrainError::Validation { message }) => {
                assert!(message.contains("parse") || message.contains("LLM graph output JSON"));
            }
            other => panic!(
                "Expected BrainError::Validation on malformed JSON, got {:?}",
                other
            ),
        }

        // Test ExtractionAgent trait error propagation
        let graph_res = extractor.extract_graph("Some raw content");
        match graph_res {
            Err(BrainError::Validation { .. }) => {}
            other => panic!(
                "Expected BrainError::Validation on extract_graph, got {:?}",
                other
            ),
        }

        // Test Python FFI parity JSON extraction
        let ffi_res = extractor
            .extract_semantic_nodes_json(r#"[{"id": "n1", "content": "text"}]"#)
            .expect("FFI function itself succeeds");
        let parsed: serde_json::Value =
            serde_json::from_str(&ffi_res).expect("Valid JSON response");
        assert_eq!(parsed["status"], "error");
        assert!(parsed["message"]
            .as_str()
            .unwrap()
            .contains("LLM parsing failed"));
    }

    #[test]
    fn test_extractor_upstream_chat_error_propagation() {
        let mock_chat = Arc::new(MockChatEngine::failing(BrainError::Authorization {
            message: "Missing token".to_string(),
        }));
        let extractor = LlmMemoryExtractor::new(mock_chat, "test-llm-extractor");

        let req = ExtractionRequest {
            raw_content: "Some text".to_string(),
            context_metadata: HashMap::new(),
        };

        let result = extractor.extract(req);
        assert!(result.is_err(), "Must propagate upstream ChatAgent failure");
    }
}
