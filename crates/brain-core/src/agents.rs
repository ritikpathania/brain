use crate::errors::BrainError;
use brain_domain::{Conversation, EdgeDTO, NodeDTO, SessionId, ToolCall};

/// Trait defining standard LLM chat conversation agent capabilities.
pub trait ChatAgent: Send + Sync {
    /// Returns the descriptive name of the agent.
    fn name(&self) -> &str;
    /// Executes a chat interaction step inside a session context.
    fn chat(&self, session_id: SessionId, prompt: &str) -> Result<String, BrainError>;
}

/// Trait defining capability to extract structured knowledge graph entities from raw text.
pub trait ExtractionAgent: Send + Sync {
    /// Returns the descriptive name of the agent.
    fn name(&self) -> &str;
    /// Extracts a set of graph Node and Edge DTOs from raw text input.
    fn extract_graph(&self, text: &str) -> Result<(Vec<NodeDTO>, Vec<EdgeDTO>), BrainError>;
}

/// Trait defining semantic text vector embedding generation capabilities.
pub trait EmbeddingAgent: Send + Sync {
    /// Returns the descriptive name of the agent.
    fn name(&self) -> &str;
    /// Generates a floating-point vector embedding representation for the text input.
    fn embed_text(&self, text: &str) -> Result<Vec<f32>, BrainError>;
    /// Returns the expected dimension size of the generated embeddings.
    fn dimension(&self) -> usize;
}

/// Trait defining LLM planner capabilities for formulating execution tool steps.
pub trait PlannerAgent: Send + Sync {
    /// Returns the descriptive name of the agent.
    fn name(&self) -> &str;
    /// Plans a sequence of tool calls needed to achieve a task, using conversation history context.
    fn plan_steps(
        &self,
        task_description: &str,
        history: &Conversation,
    ) -> Result<Vec<ToolCall>, BrainError>;
}
