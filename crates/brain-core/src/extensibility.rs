use crate::errors::BrainError;
use brain_domain::{PluginId, PluginState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Trait defining lifecycle hooks that plugins must implement to load into the host.
pub trait PluginLifecycle: Send + Sync {
    /// Returns the unique plugin identifier.
    fn id(&self) -> &PluginId;
    /// Returns the current runtime lifecycle state of the plugin.
    fn current_state(&self) -> PluginState;
    /// Returns the list of dependencies (other plugins) this plugin requires.
    fn dependencies(&self) -> Vec<PluginId>;

    /// Discovers and validates the directory layout.
    fn discover(&mut self, path: &Path) -> Result<(), BrainError>;
    /// Resolves external dependencies using the active registry lookup.
    fn resolve_dependencies(
        &mut self,
        registry: &dyn PluginRegistryLookup,
    ) -> Result<(), BrainError>;
    /// Validates signatures and capabilities.
    fn validate(&mut self) -> Result<(), BrainError>;
    /// Loads plugin assets/scripts into memory.
    fn load(&mut self) -> Result<(), BrainError>;
    /// Runs initialization handlers.
    fn initialize(&mut self) -> Result<(), BrainError>;
    /// Transitions the plugin to the active state.
    fn activate(&mut self) -> Result<(), BrainError>;
    /// Suspends plugin operations.
    fn suspend(&mut self) -> Result<(), BrainError>;
    /// Resumes suspended plugin operations.
    fn resume(&mut self) -> Result<(), BrainError>;
    /// Unloads assets and stops all tasks.
    fn unload(&mut self) -> Result<(), BrainError>;
}

/// Interface for looking up registered plugins during dependency resolution.
pub trait PluginRegistryLookup: Send + Sync {
    /// Returns the active state of a plugin, if registered.
    fn get_plugin_state(&self, id: &PluginId) -> Option<PluginState>;
}

/// Represents system capability permissions requested by tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Read access to the filesystem.
    FilesystemRead,
    /// Write access to the filesystem.
    FilesystemWrite,
    /// Permission to execute shell commands.
    Shell,
    /// Permission to run Git operations.
    Git,
    /// Permission to access the network.
    Network,
    /// Access to the system clipboard.
    Clipboard,
    /// Permission to read/write storage.
    Storage,
    /// Permission to call LLM services.
    Llm,
}

/// Policy settings defining the execution runtime behavior of a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    /// Task execution timeout limit in milliseconds.
    pub timeout_ms: u64,
}

/// Abstraction for checking execution cancellation request.
pub trait CancellationToken: Send + Sync {
    /// Returns true if cancellation has been requested.
    fn is_cancelled(&self) -> bool;
}

/// Execution context for tool invocation, providing access to session state,
/// working directory, cancellation tokens, and other runtime environment details.
#[derive(Clone)]
pub struct ExecutionContext {
    /// The session identifier.
    pub session_id: brain_domain::SessionId,
    /// The working directory for the tool.
    pub working_dir: std::path::PathBuf,
    /// Token used to signal cancellation request.
    pub cancellation: std::sync::Arc<dyn CancellationToken>,
    /// Maximum execution deadline.
    pub deadline: Option<std::time::Instant>,
}

/// Immutable wrapper for tool execution results.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    value: serde_json::Value,
}

impl ExecutionResult {
    /// Creates a new ExecutionResult.
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }

    /// Returns the execution payload value.
    pub fn value(&self) -> &serde_json::Value {
        &self.value
    }
}

/// Structured capability metadata for a system tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// Name of the tool.
    pub name: String,
    /// Detailed description of tool actions.
    pub description: String,
    /// Usage instruction syntax.
    pub usage: String,
    /// Version identifier.
    pub version: String,
    /// Author description.
    pub author: String,
    /// List of capability permissions requested.
    pub required_permissions: Vec<Permission>,
    /// Execution behavior policy.
    pub execution_policy: ExecutionPolicy,
    /// Indicates whether the tool supports chunked stream stdout.
    pub supports_streaming: bool,
    /// Indicates whether calling this tool multiple times yields identical state results.
    pub is_idempotent: bool,
    /// Indicates whether calling this tool alters external state.
    pub causes_side_effects: bool,
}

/// Trait defining a single executable system tool.
pub trait Tool: Send + Sync {
    /// Returns the structural capability metadata of the tool.
    fn metadata(&self) -> &ToolMetadata;
    /// Executes the tool using key-value parameters.
    fn execute(
        &self,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError>;
}

/// Thread-safe registry containing active system tools available to planning agents.
pub trait ToolRegistry: Send + Sync {
    /// Registers a new tool capability in the coordinator.
    fn register_tool(&self, tool: std::sync::Arc<dyn Tool>) -> Result<(), BrainError>;
    /// Retrieves a registered tool by its name.
    fn get_tool(&self, name: &str) -> Option<std::sync::Arc<dyn Tool>>;
    /// Lists all registered tools.
    fn list_tools(&self) -> Vec<std::sync::Arc<dyn Tool>>;
}

/// Trait defining execution of a tool within a specific runner strategy (blocking, async, etc.).
pub trait ToolRunner: Send + Sync {
    /// Runs the tool with the given context and arguments.
    fn run(
        &self,
        tool: std::sync::Arc<dyn Tool>,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError>;
}
