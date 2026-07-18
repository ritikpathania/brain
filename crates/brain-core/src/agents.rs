use crate::errors::BrainError;
use brain_domain::{EdgeDTO, NodeDTO, Session, SessionId, ToolCall};

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
        history: &Session,
    ) -> Result<Vec<ToolCall>, BrainError>;
}

/// Bridge trait exposing core host engine capabilities to Python agents and plugins.
pub trait AgentRuntime: Send + Sync {
    /// Retrieves matching memory nodes for a query within a session context.
    fn retrieve(
        &self,
        session_id: &brain_domain::SessionId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<brain_domain::Node>, BrainError>;

    /// Executes a registered system tool in the tool runtime coordinator.
    fn execute_tool(
        &self,
        session_id: &brain_domain::SessionId,
        tool_name: &str,
        arguments: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<crate::extensibility::ExecutionResult, BrainError>;
}
