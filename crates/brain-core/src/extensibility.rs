use crate::errors::BrainError;
use crate::agents::{ChatAgent, PlannerAgent, EmbeddingAgent, ExtractionAgent};
use brain_domain::{PluginId, PluginState, SessionId, Node};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Represents system capability permissions requested by tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// Version enumeration for the Plugin API contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApiVersion {
    /// Version 1 of the plugin API.
    V1,
}

impl<'de> serde::Deserialize<'de> for ApiVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "v1" => Ok(ApiVersion::V1),
            other => Err(serde::de::Error::custom(format!(
                "Unsupported plugin API version '{}'. Only 'v1' is supported.",
                other
            ))),
        }
    }
}

impl serde::Serialize for ApiVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ApiVersion::V1 => serializer.serialize_str("v1"),
        }
    }
}

/// Metadata configuration schema loaded from the plugin manifest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    id: PluginId,
    version: semver::Version,
    api_version: ApiVersion,
    entrypoint: PathBuf,
    required_permissions: BTreeSet<Permission>,
}

impl PluginManifest {
    /// Creates a new `PluginManifest`.
    pub fn new(
        id: PluginId,
        version: semver::Version,
        api_version: ApiVersion,
        entrypoint: PathBuf,
        required_permissions: BTreeSet<Permission>,
    ) -> Self {
        Self {
            id,
            version,
            api_version,
            entrypoint,
            required_permissions,
        }
    }

    /// Loads and parses a plugin manifest from the given filesystem path.
    pub fn from_path(path: &Path) -> Result<Self, BrainError> {
        let content = std::fs::read_to_string(path).map_err(|e| BrainError::Storage {
            message: format!("Failed to read plugin manifest: {}", e),
            source: Some(Box::new(e)),
        })?;
        toml::from_str(&content).map_err(|e| BrainError::Validation {
            message: format!("Failed to parse plugin manifest: {}", e),
        })
    }

    /// Returns the unique plugin identifier.
    pub fn id(&self) -> PluginId {
        self.id
    }

    /// Returns the plugin semantic version.
    pub fn version(&self) -> &semver::Version {
        &self.version
    }

    /// Returns the target API version.
    pub fn api_version(&self) -> ApiVersion {
        self.api_version
    }

    /// Returns the entrypoint module path on disk.
    pub fn entrypoint(&self) -> &Path {
        &self.entrypoint
    }

    /// Returns the set of requested capability permissions.
    pub fn required_permissions(&self) -> &BTreeSet<Permission> {
        &self.required_permissions
    }
}

/// Bridge trait exposing host capabilities back to the executing plugins.
pub trait HostContext: Send + Sync {
    /// Performs a semantic search retrieval against the active memory engine.
    fn retrieve(&self, session_id: &SessionId, query: &str, limit: usize) -> Result<Vec<Node>, BrainError>;
    /// Invokes a registered system tool on behalf of a plugin.
    fn execute_tool(
        &self,
        session_id: &SessionId,
        tool_name: &str,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError>;
}

/// Ambient context passed to plugins during event dispatch or tool execution.
pub struct PluginContext<'a> {
    /// Reference to host capabilities bridge.
    pub host: &'a dyn HostContext,
    /// Optional session context.
    pub session_id: Option<SessionId>,
}

/// System events dispatched to active plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginEventKind {
    /// Dispatched when the plugin is loaded.
    Load,
    /// Dispatched when the plugin is about to be unloaded.
    Unload,
    /// Dispatched when a user agent session starts.
    SessionStart,
    /// Dispatched when a user agent session ends.
    SessionEnd,
}

/// Lifecycle or runtime session event payload sent to active plugins.
pub struct PluginEvent<'a> {
    /// The event category classification.
    pub kind: PluginEventKind,
    /// Ambient execution details.
    pub context: &'a PluginContext<'a>,
}

/// Categorized agent capability wrappers exported by plugins.
#[derive(Clone)]
pub enum PluginCapability {
    /// Conversational chat agent capability.
    Chat(Arc<dyn ChatAgent>),
    /// LLM planner/agent tool selection capability.
    Planner(Arc<dyn PlannerAgent>),
    /// Vector text embedding generation capability.
    Embedding(Arc<dyn EmbeddingAgent>),
    /// Knowledge graph extraction capability.
    Extraction(Arc<dyn ExtractionAgent>),
}

/// Metadata descriptor matching a name to an exported capability.
pub struct CapabilityDescriptor {
    /// Descriptive name or identifier of the capability.
    pub name: &'static str,
    /// Extensibility capability payload wrapper.
    pub capability: PluginCapability,
}

/// Trait exposing plugin static manifest metadata.
pub trait PluginMetadata: Send + Sync {
    /// Returns the validated configuration manifest.
    fn manifest(&self) -> &PluginManifest;
}

/// Trait defining lifecycle hooks that plugins must implement to load into the host.
pub trait PluginLifecycle: Send + Sync {
    /// Returns the current runtime lifecycle state of the plugin.
    fn state(&self) -> PluginState;
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

/// Trait exposing all registered capabilities exported by this plugin.
///
/// # Capability Evolution & Compatibility Policy
/// - **Immutability**: Existing capability semantics (such as Chat, Planner, Embedding, and Extraction) are immutable.
/// - **Forward Compatibility**: Unknown capability types defined in new plugin manifests must be gracefully ignored by older runtimes during parsing and loading.
/// - **ApiVersion Boundary**: Removing, modifying, or breaking the contract of any existing capability requires introducing a new `ApiVersion`.
pub trait PluginCapabilities: Send + Sync {
    /// Returns a list of capability descriptors.
    fn capabilities(&self) -> &[CapabilityDescriptor];
}

/// Trait handling host event notifications dispatched to this plugin.
pub trait PluginEventHandler: Send + Sync {
    /// Dispatches an event payload containing ambient host context.
    fn dispatch(&self, event: &PluginEvent<'_>) -> Result<(), BrainError>;
}

/// Composed plugin contract containing metadata, lifecycle, capabilities, and events.
pub trait Plugin:
    PluginMetadata
    + PluginLifecycle
    + PluginCapabilities
    + PluginEventHandler
    + Send
    + Sync
{}

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
    pub working_dir: PathBuf,
    /// Token used to signal cancellation request.
    pub cancellation: Arc<dyn CancellationToken>,
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
    fn register_tool(&self, tool: Arc<dyn Tool>) -> Result<(), BrainError>;
    /// Retrieves a registered tool by its name.
    fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>>;
    /// Lists all registered tools.
    fn list_tools(&self) -> Vec<Arc<dyn Tool>>;
}

/// Trait defining execution of a tool within a specific runner strategy (blocking, async, etc.).
pub trait ToolRunner: Send + Sync {
    /// Runs the tool with the given context and arguments.
    fn run(
        &self,
        tool: Arc<dyn Tool>,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError>;
}
