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
    /// List of capability permissions requested (e.g. "fs:read", "net:connect").
    pub required_permissions: Vec<String>,
    /// Task execution timeout limit in milliseconds.
    pub timeout_ms: u64,
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
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, BrainError>;
}

/// Thread-safe registry containing active system tools available to planning agents.
pub trait ToolRegistry: Send + Sync {
    /// Registers a new tool capability in the coordinator.
    fn register_tool(&self, tool: Box<dyn Tool>) -> Result<(), BrainError>;
    /// Retrieves a registered tool by its name.
    fn get_tool(&self, name: &str) -> Option<Box<dyn Tool>>;
    /// Lists all registered tools.
    fn list_tools(&self) -> Vec<Box<dyn Tool>>;
}
