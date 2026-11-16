use futures::StreamExt;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use brain_core::model::{
    ChatRole, GenerationChunk, GenerationRequest, ModelChatMessage, ModelGateway, ModelProvider,
};
use brain_services::model::{DefaultModelGateway, DeterministicMockProvider, ScriptedResponse};

#[tokio::test]
async fn test_model_gateway_registration_and_alias_resolution() {
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    let gateway = DefaultModelGateway::with_mock_provider(mock_provider);

    // List models
    let models = gateway.list_models();
    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id == "brain-default"));

    // Exact match
    let resolved_exact = gateway.resolve_model(Some("brain-default")).unwrap();
    assert_eq!(resolved_exact.id, "brain-default");

    // Alias resolution: "sonnet" -> "claude-3-7-sonnet-latest"
    let resolved_alias = gateway.resolve_model(Some("sonnet")).unwrap();
    assert_eq!(resolved_alias.id, "claude-3-7-sonnet-latest");

    // Alias resolution: "r1" -> "deepseek-r1:latest"
    let resolved_r1 = gateway.resolve_model(Some("r1")).unwrap();
    assert_eq!(resolved_r1.id, "deepseek-r1:latest");

    // None query resolves to default model
    let resolved_default = gateway.resolve_model(None).unwrap();
    assert_eq!(resolved_default.id, "brain-default");
    assert!(resolved_default.is_default);

    // Unknown query returns Err
    let unknown_res = gateway.resolve_model(Some("unknown_nonexistent_model_xyz"));
    assert!(unknown_res.is_err());
}

#[tokio::test]
async fn test_mock_provider_deterministic_streaming_lifecycle() {
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    mock_provider.enqueue_response(ScriptedResponse {
        thinking: Some("Reasoning about 42".to_string()),
        tokens: vec!["The answer is ".to_string(), "42.".to_string()],
        tool_calls: Vec::new(),
        error: None,
        finish_reason: Some("end_turn".to_string()),
    });

    let cancel_token = CancellationToken::new();
    let req = GenerationRequest {
        model: "brain-default".to_string(),
        messages: vec![ModelChatMessage::text(
            ChatRole::User,
            "What is the meaning of life?",
        )],
        system_prompt: None,
        tools: Vec::new(),
        thinking_budget: None,
    };

    let mut stream = mock_provider
        .stream_generation(req, cancel_token)
        .await
        .unwrap();

    let mut chunks = Vec::new();
    while let Some(chunk_res) = stream.next().await {
        chunks.push(chunk_res.unwrap());
    }

    assert_eq!(chunks.len(), 6);
    assert_eq!(chunks[0], GenerationChunk::ThinkingStart);
    assert_eq!(
        chunks[1],
        GenerationChunk::ThinkingDelta {
            text: "Reasoning about 42".to_string()
        }
    );
    assert_eq!(chunks[2], GenerationChunk::ThinkingEnd);
    assert_eq!(
        chunks[3],
        GenerationChunk::TextDelta {
            text: "The answer is ".to_string()
        }
    );
    assert_eq!(
        chunks[4],
        GenerationChunk::TextDelta {
            text: "42.".to_string()
        }
    );
    match &chunks[5] {
        GenerationChunk::Completed {
            finish_reason,
            usage,
        } => {
            assert_eq!(finish_reason, "end_turn");
            assert!(usage.output_tokens > 0);
        }
        _ => panic!("Expected Completed chunk"),
    }
}

#[tokio::test]
async fn test_cancellation_token_stops_mock_stream_immediately() {
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    let cancel_token = CancellationToken::new();

    // Cancel before invocation
    cancel_token.cancel();

    let req = GenerationRequest {
        model: "brain-default".to_string(),
        messages: vec![ModelChatMessage::text(ChatRole::User, "Cancelled query")],
        system_prompt: None,
        tools: Vec::new(),
        thinking_budget: None,
    };

    let res = mock_provider.stream_generation(req, cancel_token).await;
    assert!(res.is_err());
    match res {
        Err(brain_core::errors::BrainError::Cancelled { .. }) => {}
        _ => panic!("Expected BrainError::Cancelled"),
    }
}

#[tokio::test]
async fn test_dynamic_provider_registration_mutation() {
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    let gateway = DefaultModelGateway::with_mock_provider(mock_provider);

    let initial_count = gateway.list_models().len();

    // Register a new custom provider in Rust
    let custom_model = brain_core::model::ModelDescriptor {
        id: "custom-dynamic-model".to_string(),
        name: "Custom Dynamic Model (Test)".to_string(),
        provider: "mock-custom".to_string(),
        context_window: 65536,
        max_output_tokens: 4096,
        supports_thinking: true,
        supports_tools: false,
        is_default: false,
    };
    let custom_provider = Arc::new(DeterministicMockProvider::with_models(vec![custom_model]));
    gateway.register_provider(custom_provider);

    let updated_models = gateway.list_models();
    assert_eq!(updated_models.len(), initial_count + 1);
    assert!(updated_models
        .iter()
        .any(|m| m.id == "custom-dynamic-model"));

    let resolved = gateway.resolve_model(Some("custom-dynamic-model")).unwrap();
    assert_eq!(resolved.id, "custom-dynamic-model");
    assert_eq!(resolved.context_window, 65536);
}

#[tokio::test]
async fn test_thinking_budget_passes_to_provider_execution() {
    use futures::StreamExt;
    let mock_provider = Arc::new(DeterministicMockProvider::new());
    let gateway = DefaultModelGateway::with_mock_provider(
        Arc::clone(&mock_provider) as Arc<dyn ModelProvider>
    );
    let cancel_token = CancellationToken::new();

    let req = GenerationRequest {
        model: "brain-default".to_string(),
        messages: vec![ModelChatMessage::text(
            ChatRole::User,
            "Hello with thinking",
        )],
        system_prompt: None,
        tools: Vec::new(),
        thinking_budget: Some(4096),
    };

    let mut stream = gateway
        .stream_generation(req, cancel_token)
        .await
        .expect("Stream generation should succeed");

    while let Some(chunk) = stream.next().await {
        let _ = chunk.expect("Chunk should succeed");
    }

    assert_eq!(mock_provider.last_request_thinking_budget(), Some(4096));
}
