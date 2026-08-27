//! Core model abstractions, provider interfaces, and generation streaming traits.

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::errors::BrainError;

/// Metadata descriptor for an LLM model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelDescriptor {
    /// Unique identifier for the model (e.g. "brain-default", "claude-3-7-sonnet-latest").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Provider type/backend (e.g. "mock", "local-ollama", "anthropic").
    pub provider: String,
    /// Maximum context window size in tokens.
    pub context_window: usize,
    /// Maximum output generation tokens.
    pub max_output_tokens: usize,
    /// Whether the model supports native reasoning / thinking blocks.
    pub supports_thinking: bool,
    /// Whether the model supports tool/function calling.
    pub supports_tools: bool,
    /// Whether this is the default fallback model.
    pub is_default: bool,
}

/// A tool definition provided to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolDefinition {
    /// Unique name of the tool.
    pub name: String,
    /// Description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool inputs.
    pub parameters: serde_json::Value,
}

/// Role of a message in a conversation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    /// System message.
    System,
    /// User prompt.
    User,
    /// Assistant response.
    Assistant,
}

/// Content block in a chat message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContentBlock {
    /// Plain text content.
    Text {
        /// Text string.
        text: String,
    },
    /// Thinking / reasoning content.
    Thinking {
        /// Thinking text.
        thinking: String,
        /// Optional signature.
        #[serde(default)]
        signature: Option<String>,
    },
    /// Tool use invocation by the assistant.
    ToolUse {
        /// Unique tool use ID.
        id: String,
        /// Name of the tool.
        name: String,
        /// Parsed input arguments.
        input: serde_json::Value,
    },
    /// Result of a tool execution.
    ToolResult {
        /// Tool use ID that this result answers.
        tool_use_id: String,
        /// Result text content.
        content: String,
        /// Whether the tool execution resulted in an error.
        #[serde(default)]
        is_error: bool,
    },
}

/// Normalized chat message passed to a model provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelChatMessage {
    /// Message role.
    pub role: ChatRole,
    /// Message content blocks.
    pub content: Vec<MessageContentBlock>,
}

impl ModelChatMessage {
    /// Creates a simple text message.
    pub fn text(role: ChatRole, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![MessageContentBlock::Text { text: text.into() }],
        }
    }
}

/// Token usage metadata for a generation turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    /// Number of input / prompt tokens.
    pub input_tokens: usize,
    /// Number of output / completion tokens.
    pub output_tokens: usize,
}

/// Atomic generation chunk streamed from a model provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerationChunk {
    /// Model started reasoning / thinking.
    ThinkingStart,
    /// Incremental thinking token delta.
    ThinkingDelta {
        /// Incremental thinking text.
        text: String,
    },
    /// Model finished reasoning block.
    ThinkingEnd,
    /// Incremental response text token.
    TextDelta {
        /// Incremental token text.
        text: String,
    },
    /// Model invoked a tool call.
    ToolUse {
        /// Tool use ID.
        id: String,
        /// Tool name.
        name: String,
        /// Tool input arguments.
        input: serde_json::Value,
    },
    /// Generation completed successfully.
    Completed {
        /// Reason for completion (e.g. "end_turn", "tool_use", "stop").
        finish_reason: String,
        /// Token usage statistics.
        usage: TokenUsage,
    },
}

/// Request parameters for streaming text generation from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// Model identifier to use.
    pub model: String,
    /// List of chat messages in the conversation.
    pub messages: Vec<ModelChatMessage>,
    /// Optional system prompt instructions.
    pub system_prompt: Option<String>,
    /// Optional tools available to the model.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    /// Optional reasoning / thinking token budget.
    pub thinking_budget: Option<usize>,
}

/// Interface for concrete model provider backends (e.g. Mock, Ollama, Anthropic).
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Name of this provider.
    fn provider_name(&self) -> &str;

    /// List of models supported by this provider.
    fn supported_models(&self) -> Vec<ModelDescriptor>;

    /// Initiates a streaming text generation.
    async fn stream_generation(
        &self,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
    ) -> Result<BoxStream<'static, Result<GenerationChunk, BrainError>>, BrainError>;
}

/// High-level routing gateway that maps model identifiers to registered providers.
#[async_trait]
pub trait ModelGateway: Send + Sync {
    /// Lists all models available across all registered providers.
    fn list_models(&self) -> Vec<ModelDescriptor>;

    /// Resolves a requested model name or alias to a descriptor.
    fn resolve_model(&self, query: Option<&str>) -> Result<ModelDescriptor, BrainError>;

    /// Routes a generation request to the appropriate provider and streams chunks.
    async fn stream_generation(
        &self,
        request: GenerationRequest,
        cancellation_token: CancellationToken,
    ) -> Result<BoxStream<'static, Result<GenerationChunk, BrainError>>, BrainError>;
}
